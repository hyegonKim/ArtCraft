//! Slicers & Timelines: interactive on-sheet floating filter cards for Tables and PivotTables.
//!
//! - A Table [`Slicer`] writes a [`FilterCriterion::Values`] into the sheet's [`AutoFilter`] and
//!   re-applies row hiding via [`super::data::apply_filter`].
//! - A PivotTable [`Slicer`] updates its linked PivotTables' field filters and refreshes their
//!   rendered reports on the sheet.
//! - A PivotTable [`Timeline`] filters linked PivotTables by an inclusive calendar date range
//!   (`Years`, `Quarters`, `Months`, or `Days`) and refreshes their rendered reports.

use gridcraft_core::CellRef;
use gridcraft_core::date::{DateSystem, days_in_month_in};
use gridcraft_model::{
    Anchor, AnchorMode, AutoFilter, FilterCriterion, PivotDateFilter, PivotDateGroup, PivotFilter,
    PivotTable, Slicer, Timeline, TimelineLevel, Workbook,
};
use serde_json::{Value as Json, json};

use super::{
    CommandSpec, EngineError, Result, Session, bad, bool_param, cell_param_on, cmd, edit,
    f64_param, has_doc, str_param, target_sheet, u32_param,
};

const SHORT_MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Cap on generated timeline tiles so pathological multi-century serial spans stay bounded.
const MAX_TIMELINE_PERIODS: usize = 732;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimelinePeriod {
    pub label: String,
    pub sublabel: String,
    pub start_date: String,
    pub end_date: String,
    pub selected: bool,
    pub has_data: bool,
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "insert.slicer",
            "Slicer",
            ["Insert", "Filters"],
            None,
            "{table?, pivot?, pivots?: [...], column?/field?: \"Region\", fields?: [...], multiSelect?: false, columns?: 1, caption?, style?, at?: \"H2\", width?, height?}",
            has_doc,
            insert_slicer
        ),
        cmd!(
            "table.insertSlicer",
            "Insert Slicer",
            ["Table", "Tools"],
            None,
            "{table?, column?: \"Region\", columns?: 1, at?, width?, height?}",
            has_doc,
            insert_slicer
        ),
        cmd!(
            "slicer.toggle",
            "Toggle Slicer Value",
            [],
            None,
            "{slicer?: id, value: \"East\", multi?: false}",
            has_doc,
            slicer_toggle
        ),
        cmd!(
            "slicer.toggleItem",
            "Toggle Slicer Value",
            [],
            None,
            "{slicer?: id, value: \"East\", multi?: false}",
            has_doc,
            slicer_toggle
        ),
        cmd!(
            "slicer.select",
            "Select Slicer Items",
            [],
            None,
            "{slicer?: id, values?: [\"East\", \"West\"] | null, value?: \"East\", multi?: false}",
            has_doc,
            slicer_select
        ),
        cmd!(
            "slicer.selectAll",
            "Clear Slicer Filter",
            [],
            None,
            "{slicer?: id}",
            has_doc,
            slicer_select_all
        ),
        cmd!(
            "slicer.clear",
            "Clear Slicer Filter",
            [],
            None,
            "{slicer?: id}",
            has_doc,
            slicer_select_all
        ),
        cmd!(
            "slicer.move",
            "Move Slicer",
            [],
            None,
            "{slicer?: id, at?, dx?, dy?, width?, height?, caption?, columns?, multiSelect?, style?, pivots?}",
            has_doc,
            slicer_update
        ),
        cmd!(
            "slicer.update",
            "Update Slicer",
            [],
            None,
            "{slicer?: id, at?, dx?, dy?, width?, height?, caption?, columns?, multiSelect?, style?, pivots?}",
            has_doc,
            slicer_update
        ),
        cmd!(
            "slicer.delete",
            "Delete Slicer",
            [],
            None,
            "{slicer?: id}",
            has_doc,
            slicer_delete
        ),
        cmd!(
            "insert.timeline",
            "Timeline",
            ["Insert", "Filters"],
            None,
            "{pivot?, pivots?: [...], field?: \"Date\", fields?: [...], level?: \"years\"|\"quarters\"|\"months\"|\"days\", start?: \"YYYY-MM-DD\", end?: \"YYYY-MM-DD\", caption?, style?, at?, width?, height?}",
            has_doc,
            insert_timeline
        ),
        cmd!(
            "timeline.setRange",
            "Set Timeline Filter",
            [],
            None,
            "{timeline?: id, start?: \"YYYY-MM-DD\" | null, end?: \"YYYY-MM-DD\" | null}",
            has_doc,
            timeline_set_range
        ),
        cmd!(
            "timeline.setLevel",
            "Set Timeline Level",
            [],
            None,
            "{timeline?: id, level: \"years\"|\"quarters\"|\"months\"|\"days\"}",
            has_doc,
            timeline_set_level
        ),
        cmd!(
            "timeline.clear",
            "Clear Timeline Filter",
            [],
            None,
            "{timeline?: id}",
            has_doc,
            timeline_clear
        ),
        cmd!(
            "timeline.move",
            "Move Timeline",
            [],
            None,
            "{timeline?: id, at?, dx?, dy?, width?, height?, caption?, style?, level?, pivots?}",
            has_doc,
            timeline_update
        ),
        cmd!(
            "timeline.update",
            "Update Timeline",
            [],
            None,
            "{timeline?: id, at?, dx?, dy?, width?, height?, caption?, style?, level?, pivots?}",
            has_doc,
            timeline_update
        ),
        cmd!(
            "timeline.delete",
            "Delete Timeline",
            [],
            None,
            "{timeline?: id}",
            has_doc,
            timeline_delete
        ),
    ]
}

/// Distinct, non-blank display values of a table column, in first-seen order. Used to populate a
/// table slicer's tiles and to detect "everything selected".
pub fn column_values(wb: &Workbook, sheet: usize, table: &str, column: &str) -> Vec<String> {
    let resolved = wb
        .sheet(sheet)
        .and_then(|sh| {
            sh.tables
                .iter()
                .position(|t| t.name.eq_ignore_ascii_case(table))
                .map(|ti| (sheet, ti))
        })
        .or_else(|| wb.table(table));
    let Some((t_sheet, ti)) = resolved else { return vec![] };
    let Some(sh) = wb.sheet(t_sheet) else { return vec![] };
    let Some(t) = sh.tables.get(ti) else { return vec![] };
    let Some(ci) = t.column_index(column) else { return vec![] };
    let Some(dr) = t.data_range() else { return vec![] };
    let col = dr.start.col + ci as u32;
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for row in dr.start.row..=dr.end.row {
        let text = crate::display::cell_text(wb, sh, CellRef::new(row, col));
        if text.is_empty() {
            continue;
        }
        if seen.insert(text.to_lowercase()) {
            out.push(text);
        }
    }
    out
}

/// Distinct, non-blank item labels for any [`Slicer`] (PivotTable-backed or Table-backed).
pub fn slicer_values(wb: &Workbook, sheet: usize, sl: &Slicer) -> Vec<String> {
    if sl.is_pivot() {
        for target in &sl.pivots {
            let Some((pt_sheet, pt_idx)) = wb.pivot(target) else { continue };
            let Some(pt) = wb.sheet(pt_sheet).and_then(|s| s.pivots.get(pt_idx)) else { continue };
            let Ok(src) = crate::pivot::read_source(wb, &pt.source, pt_sheet) else { continue };
            let Some(col) = src.col(&sl.column) else { continue };
            let group = pt
                .rows
                .iter()
                .chain(pt.columns.iter())
                .find(|f| f.source_col.eq_ignore_ascii_case(&sl.column))
                .map_or(PivotDateGroup::None, |f| f.date_group);
            return crate::pivot::distinct_labels(wb, &src, col, group, 10_000);
        }
        vec![]
    } else {
        column_values(wb, sheet, &sl.table, &sl.column)
    }
}

/// A slicer's tiles as `(value, selected)`, for the UI. `selected` is true for every tile when the
/// slicer has no filter (`selected == None`).
pub fn slicer_items(wb: &Workbook, sheet: usize, sl: &Slicer) -> Vec<(String, bool)> {
    slicer_values(wb, sheet, sl)
        .into_iter()
        .map(|v| {
            let on = match &sl.selected {
                None => true,
                Some(set) => set.iter().any(|x| x.eq_ignore_ascii_case(&v)),
            };
            (v, on)
        })
        .collect()
}

/// All source field headers of a PivotTable.
pub fn pivot_fields(wb: &Workbook, pt_sheet: usize, pt: &PivotTable) -> Vec<String> {
    crate::pivot::read_source(wb, &pt.source, pt_sheet)
        .map(|src| src.headers)
        .unwrap_or_default()
}

/// Source fields of a PivotTable that hold dates (either formatted Excel date serials or ISO date text).
pub fn pivot_date_fields(wb: &Workbook, pt_sheet: usize, pt: &PivotTable) -> Vec<String> {
    let Ok(src) = crate::pivot::read_source(wb, &pt.source, pt_sheet) else { return vec![] };
    src.headers
        .iter()
        .enumerate()
        .filter(|(c, _)| column_is_date(&src, *c))
        .map(|(_, h)| h.clone())
        .collect()
}

fn column_is_date(src: &crate::pivot::Source, col: usize) -> bool {
    if src.dates.get(col).copied().unwrap_or(false) {
        return true;
    }
    if src.numeric.get(col).copied().unwrap_or(false) {
        return false;
    }
    let mut any_text_date = false;
    for rec in &src.rows {
        let Some(v) = rec.get(col) else { continue };
        if v.is_empty() {
            continue;
        }
        match v {
            gridcraft_core::Value::Text(s) if crate::pivot::parse_iso_ymd(s).is_some() => {
                any_text_date = true;
            }
            _ => return false,
        }
    }
    any_text_date
}

fn format_iso_ymd(y: i32, m: u32, d: u32) -> String {
    format!("{y:04}-{m:02}-{d:02}")
}

/// Period tiles for a [`Timeline`] at its current [`TimelineLevel`], derived from the linked
/// PivotTable's source date range.
pub fn timeline_periods(wb: &Workbook, tl: &Timeline) -> Vec<TimelinePeriod> {
    let mut observed: Vec<(i32, u32, u32)> = Vec::new();
    for target in &tl.pivots {
        let Some((pt_sheet, pt_idx)) = wb.pivot(target) else { continue };
        let Some(pt) = wb.sheet(pt_sheet).and_then(|s| s.pivots.get(pt_idx)) else { continue };
        let Ok(src) = crate::pivot::read_source(wb, &pt.source, pt_sheet) else { continue };
        let Some(col) = src.col(&tl.field) else { continue };
        for rec in &src.rows {
            if let Some(v) = rec.get(col)
                && let Some(ymd) = crate::pivot::value_ymd(wb.date_system, v)
            {
                observed.push(ymd);
            }
        }
        break;
    }
    observed.sort_unstable();
    observed.dedup();

    let filter_lo = tl.start_date.as_deref().and_then(crate::pivot::parse_iso_ymd);
    let filter_hi = tl.end_date.as_deref().and_then(crate::pivot::parse_iso_ymd);

    let (min_ymd, max_ymd) = match (observed.first().copied(), observed.last().copied()) {
        (Some(lo), Some(hi)) => {
            let lo = filter_lo.map_or(lo, |f| lo.min(f));
            let hi = filter_hi.map_or(hi, |f| hi.max(f));
            (lo, hi)
        }
        _ => {
            let y = filter_lo.or(filter_hi).map_or(2024, |(yr, _, _)| yr);
            ((y, 1, 1), (y, 12, 31))
        }
    };

    let filtered = filter_lo.is_some() || filter_hi.is_some();
    let mut out = Vec::new();
    let has_in_range = |lo: (i32, u32, u32), hi: (i32, u32, u32)| -> bool {
        let idx = observed.partition_point(|&d| d < lo);
        observed.get(idx).is_some_and(|&d| d <= hi)
    };
    let is_selected = |lo: (i32, u32, u32), hi: (i32, u32, u32)| -> bool {
        if !filtered {
            return true;
        }
        filter_lo.is_none_or(|flo| hi >= flo) && filter_hi.is_none_or(|fhi| lo <= fhi)
    };

    match tl.level {
        TimelineLevel::Years => {
            for y in min_ymd.0..=max_ymd.0 {
                if out.len() >= MAX_TIMELINE_PERIODS {
                    break;
                }
                let lo = (y, 1, 1);
                let hi = (y, 12, 31);
                out.push(TimelinePeriod {
                    label: format!("{y}"),
                    sublabel: String::new(),
                    start_date: format_iso_ymd(lo.0, lo.1, lo.2),
                    end_date: format_iso_ymd(hi.0, hi.1, hi.2),
                    selected: is_selected(lo, hi),
                    has_data: has_in_range(lo, hi),
                });
            }
        }
        TimelineLevel::Quarters => {
            let q_start = ((min_ymd.1.saturating_sub(1)) / 3) + 1;
            let q_end = ((max_ymd.1.saturating_sub(1)) / 3) + 1;
            for y in min_ymd.0..=max_ymd.0 {
                let qa = if y == min_ymd.0 { q_start } else { 1 };
                let qb = if y == max_ymd.0 { q_end } else { 4 };
                for q in qa..=qb {
                    if out.len() >= MAX_TIMELINE_PERIODS {
                        break;
                    }
                    let m1 = (q - 1) * 3 + 1;
                    let m2 = m1 + 2;
                    let d2 = days_in_month_in(DateSystem::D1900, y, m2).max(28);
                    let lo = (y, m1, 1);
                    let hi = (y, m2, d2);
                    out.push(TimelinePeriod {
                        label: format!("Q{q}"),
                        sublabel: format!("{y}"),
                        start_date: format_iso_ymd(lo.0, lo.1, lo.2),
                        end_date: format_iso_ymd(hi.0, hi.1, hi.2),
                        selected: is_selected(lo, hi),
                        has_data: has_in_range(lo, hi),
                    });
                }
            }
        }
        TimelineLevel::Months => {
            for y in min_ymd.0..=max_ymd.0 {
                let ma = if y == min_ymd.0 { min_ymd.1 } else { 1 };
                let mb = if y == max_ymd.0 { max_ymd.1 } else { 12 };
                for m in ma..=mb {
                    if out.len() >= MAX_TIMELINE_PERIODS {
                        break;
                    }
                    let d2 = days_in_month_in(DateSystem::D1900, y, m).max(28);
                    let lo = (y, m, 1);
                    let hi = (y, m, d2);
                    let m_name = SHORT_MONTHS.get((m - 1) as usize).copied().unwrap_or("Jan");
                    out.push(TimelinePeriod {
                        label: m_name.to_string(),
                        sublabel: format!("{y}"),
                        start_date: format_iso_ymd(lo.0, lo.1, lo.2),
                        end_date: format_iso_ymd(hi.0, hi.1, hi.2),
                        selected: is_selected(lo, hi),
                        has_data: has_in_range(lo, hi),
                    });
                }
            }
        }
        TimelineLevel::Days => {
            for y in min_ymd.0..=max_ymd.0 {
                let ma = if y == min_ymd.0 { min_ymd.1 } else { 1 };
                let mb = if y == max_ymd.0 { max_ymd.1 } else { 12 };
                for m in ma..=mb {
                    let max_d = days_in_month_in(DateSystem::D1900, y, m).max(28);
                    let da = if y == min_ymd.0 && m == min_ymd.1 { min_ymd.2 } else { 1 };
                    let db = if y == max_ymd.0 && m == max_ymd.1 { max_ymd.2.min(max_d) } else { max_d };
                    let m_name = SHORT_MONTHS.get((m - 1) as usize).copied().unwrap_or("Jan");
                    for d in da..=db {
                        if out.len() >= MAX_TIMELINE_PERIODS {
                            return out;
                        }
                        let pt_d = (y, m, d);
                        let iso = format_iso_ymd(y, m, d);
                        out.push(TimelinePeriod {
                            label: format!("{d}"),
                            sublabel: format!("{m_name} {y}"),
                            start_date: iso.clone(),
                            end_date: iso,
                            selected: is_selected(pt_d, pt_d),
                            has_data: has_in_range(pt_d, pt_d),
                        });
                    }
                }
            }
        }
    }
    out
}

/// Writes the slicer's current selection into the linked Table AutoFilter or PivotTable(s) and refreshes.
pub(crate) fn reapply_slicer(wb: &mut Workbook, sheet: usize, id: u32) {
    let Some(sl) = wb.sheet(sheet).and_then(|sh| sh.slicers.iter().find(|s| s.id == id)).cloned() else {
        return;
    };
    if sl.is_pivot() {
        apply_pivot_slicer_selection(wb, &sl.pivots, &sl.column, sl.selected);
        return;
    }
    let resolved = wb
        .sheet(sheet)
        .and_then(|sh| {
            sh.tables
                .iter()
                .position(|t| t.name.eq_ignore_ascii_case(&sl.table))
                .map(|ti| (sheet, ti))
        })
        .or_else(|| wb.table(&sl.table));
    let Some((t_sheet, ti)) = resolved else { return };
    let info = wb.sheet(t_sheet).and_then(|sh| {
        let t = sh.tables.get(ti)?;
        let ci = t.column_index(&sl.column)?;
        Some((t.range, t.range.start.col + ci as u32))
    });
    let Some((table_range, abs_col)) = info else { return };
    if let Some(sh) = wb.sheet_mut(t_sheet) {
        let af = sh.autofilter.get_or_insert_with(|| AutoFilter { range: table_range, criteria: vec![] });
        let off = abs_col.saturating_sub(af.range.start.col);
        af.criteria.retain(|(o, _)| *o != off);
        if let Some(values) = sl.selected {
            af.criteria.push((off, FilterCriterion::Values { values, blanks: false }));
        }
    }
    super::data::apply_filter(wb, t_sheet);
}

fn apply_pivot_slicer_selection(
    wb: &mut Workbook,
    pivots: &[String],
    column: &str,
    selected: Option<Vec<String>>,
) {
    let mut targets: Vec<(usize, usize)> = Vec::new();
    for target in pivots {
        if let Some(loc) = wb.pivot(target)
            && !targets.contains(&loc)
        {
            targets.push(loc);
        }
    }
    for (pt_sheet, pt_idx) in targets {
        if let Some(sh) = wb.sheet_mut(pt_sheet)
            && let Some(pt) = sh.pivots.get_mut(pt_idx)
        {
            let on_axis = pt
                .rows
                .iter()
                .chain(pt.columns.iter())
                .any(|f| f.source_col.eq_ignore_ascii_case(column));
            if let Some(pos) = pt.filters.iter().position(|f| f.source_col.eq_ignore_ascii_case(column)) {
                if selected.is_none() && on_axis {
                    pt.filters.remove(pos);
                } else if let Some(f) = pt.filters.get_mut(pos) {
                    f.selected = selected.clone();
                }
            } else if on_axis && selected.is_some() {
                pt.filters.push(PivotFilter {
                    source_col: column.to_string(),
                    selected: selected.clone(),
                });
            }
        }
        let _ = crate::pivot::refresh(wb, pt_sheet, pt_idx, true);
    }
}

/// Cleans up the filter owned by `sl` after `sl` has been removed from its sheet.
pub(crate) fn cleanup_deleted_slicer(wb: &mut Workbook, sheet: usize, sl: &Slicer) {
    if sl.is_pivot() {
        apply_pivot_slicer_selection(wb, &sl.pivots, &sl.column, None);
        return;
    }
    let resolved = wb
        .sheet(sheet)
        .and_then(|sh| {
            sh.tables
                .iter()
                .position(|t| t.name.eq_ignore_ascii_case(&sl.table))
                .map(|ti| (sheet, ti))
        })
        .or_else(|| wb.table(&sl.table));
    let Some((t_sheet, ti)) = resolved else { return };
    let abs_col = wb.sheet(t_sheet).and_then(|sh| {
        let t = sh.tables.get(ti)?;
        let ci = t.column_index(&sl.column)?;
        Some(t.range.start.col + ci as u32)
    });
    if let (Some(abs), Some(sh)) = (abs_col, wb.sheet_mut(t_sheet))
        && let Some(af) = sh.autofilter.as_mut()
    {
        let off = abs.saturating_sub(af.range.start.col);
        af.criteria.retain(|(o, _)| *o != off);
    }
    super::data::apply_filter(wb, t_sheet);
}

/// Writes the timeline's current date range into its linked PivotTable(s) and refreshes them.
pub(crate) fn reapply_timeline(wb: &mut Workbook, sheet: usize, id: u32) {
    let Some(tl) = wb.sheet(sheet).and_then(|sh| sh.timelines.iter().find(|t| t.id == id)).cloned() else {
        return;
    };
    apply_timeline_filter(wb, &tl.pivots, &tl.field, tl.start_date, tl.end_date);
}

/// Cleans up the date filter owned by `tl` after `tl` has been removed from its sheet.
pub(crate) fn cleanup_deleted_timeline(wb: &mut Workbook, tl: &Timeline) {
    apply_timeline_filter(wb, &tl.pivots, &tl.field, None, None);
}

fn apply_timeline_filter(
    wb: &mut Workbook,
    pivots: &[String],
    field: &str,
    start_date: Option<String>,
    end_date: Option<String>,
) {
    let mut targets: Vec<(usize, usize)> = Vec::new();
    for target in pivots {
        if let Some(loc) = wb.pivot(target)
            && !targets.contains(&loc)
        {
            targets.push(loc);
        }
    }
    for (pt_sheet, pt_idx) in targets {
        if let Some(sh) = wb.sheet_mut(pt_sheet)
            && let Some(pt) = sh.pivots.get_mut(pt_idx)
        {
            pt.date_filters.retain(|df| !df.source_col.eq_ignore_ascii_case(field));
            if start_date.is_some() || end_date.is_some() {
                pt.date_filters.push(PivotDateFilter {
                    source_col: field.to_string(),
                    start_date: start_date.clone(),
                    end_date: end_date.clone(),
                });
            }
        }
        let _ = crate::pivot::refresh(wb, pt_sheet, pt_idx, true);
    }
}

fn string_list_param(p: &Json, key: &str) -> Option<Vec<String>> {
    let arr = p.get(key)?.as_array()?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        if let Some(s) = item.as_str() {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                out.push(trimmed.to_string());
            }
        }
    }
    Some(out)
}

fn pivot_targets_param(wb: &Workbook, p: &Json, cmd: &str) -> Result<Option<(usize, usize, Vec<String>)>> {
    if let Some(list) = string_list_param(p, "pivots") {
        if list.is_empty() {
            return Err(bad(cmd, "`pivots` must name at least one PivotTable"));
        }
        let mut first_loc = None;
        let mut canonical = Vec::with_capacity(list.len());
        for name in &list {
            let (si, pi) = wb.pivot(name).ok_or_else(|| bad(cmd, format!("no PivotTable named `{name}`")))?;
            if first_loc.is_none() {
                first_loc = Some((si, pi));
            }
            let canon = pivot_ref_name(wb, si, pi, name);
            if !canonical.iter().any(|c: &String| c.eq_ignore_ascii_case(&canon)) {
                canonical.push(canon);
            }
        }
        if let Some((si, pi)) = first_loc {
            return Ok(Some((si, pi, canonical)));
        }
    }
    if let Some(name) = str_param(p, "pivot") {
        let (si, pi) = wb.pivot(name).ok_or_else(|| bad(cmd, format!("no PivotTable named `{name}`")))?;
        let canon = pivot_ref_name(wb, si, pi, name);
        return Ok(Some((si, pi, vec![canon])));
    }
    Ok(None)
}

fn pivot_ref_name(wb: &Workbook, si: usize, pi: usize, fallback: &str) -> String {
    let Some(sh) = wb.sheet(si) else { return fallback.to_string() };
    let Some(pt) = sh.pivots.get(pi) else { return fallback.to_string() };
    let same_name_count = wb
        .sheets
        .iter()
        .flat_map(|s| s.pivots.iter())
        .filter(|p| p.name.eq_ignore_ascii_case(&pt.name))
        .count();
    if same_name_count > 1 || fallback.contains('!') {
        format!("{}!{}", crate::pivot::quote_sheet(&sh.name), pt.name)
    } else {
        pt.name.clone()
    }
}

fn sanitize_object_name(prefix: &str, base: &str, id: u32) -> String {
    let clean: String = base
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    if clean.is_empty() {
        format!("{prefix}_{id}")
    } else {
        format!("{prefix}_{clean}_{id}")
    }
}

fn requested_fields_param(p: &Json) -> Option<Vec<String>> {
    if let Some(list) = string_list_param(p, "fields").or_else(|| string_list_param(p, "columns_list"))
        && !list.is_empty()
    {
        return Some(list);
    }
    str_param(p, "column")
        .or_else(|| str_param(p, "field"))
        .map(|s| vec![s.to_string()])
}

fn insert_slicer(s: &mut Session, p: &Json) -> Result<Json> {
    let wb_ref = &s.doc()?.wb;
    let explicit_pivots = pivot_targets_param(wb_ref, p, "insert.slicer")?;
    let explicit_table = match str_param(p, "table") {
        Some(name) => Some(
            wb_ref
                .table(name)
                .ok_or_else(|| bad("insert.slicer", format!("no table named `{name}`")))?,
        ),
        None => None,
    };

    if let Some((pt_sheet, pt_idx, pivot_names)) = explicit_pivots {
        let sheet = if p.get("sheet").is_some() { target_sheet(s, p)? } else { pt_sheet };
        return insert_pivot_slicers(s, p, sheet, pt_sheet, pt_idx, pivot_names);
    }

    if explicit_table.is_none() {
        let target_sh = target_sheet(s, p)?;
        let d = s.doc()?;
        let sh = d.wb.sheet(target_sh).ok_or(EngineError::NoDocument)?;
        if sh.table_at(d.selection.active).is_none() {
            let found_pt = sh
                .pivot_at(d.selection.active)
                .and_then(|pt| sh.pivots.iter().position(|x| x.id == pt.id))
                .or_else(|| (sh.pivots.len() == 1 && sh.tables.is_empty()).then_some(0));
            if let Some(pt_idx) = found_pt {
                let canon = pivot_ref_name(&d.wb, target_sh, pt_idx, "");
                return insert_pivot_slicers(s, p, target_sh, target_sh, pt_idx, vec![canon]);
            }
        }
    }

    let sheet = match explicit_table {
        Some((si, _)) => si,
        None => target_sheet(s, p)?,
    };
    let d = s.doc()?;
    let sh = d.wb.sheet(sheet).ok_or(EngineError::NoDocument)?;
    let t = match explicit_table {
        Some((_, ti)) => sh.tables.get(ti),
        None => sh.table_at(d.selection.active),
    }
    .ok_or_else(|| {
        EngineError::Other("Select a cell inside a table or PivotTable first, or pass `table` or `pivot`.".into())
    })?;

    let req_fields = match requested_fields_param(p) {
        Some(list) => {
            let mut resolved = Vec::with_capacity(list.len());
            for c in list {
                let tc = t
                    .columns
                    .iter()
                    .find(|tc| tc.name.eq_ignore_ascii_case(&c))
                    .map(|tc| tc.name.clone())
                    .ok_or_else(|| bad("insert.slicer", format!("table has no column `{c}`")))?;
                resolved.push(tc);
            }
            resolved
        }
        None => {
            let idx = d
                .selection
                .active
                .col
                .checked_sub(t.range.start.col)
                .map(|o| o as usize)
                .filter(|o| *o < t.columns.len())
                .unwrap_or(0);
            let col = t
                .columns
                .get(idx)
                .map(|tc| tc.name.clone())
                .ok_or_else(|| bad("insert.slicer", "the table has no columns"))?;
            vec![col]
        }
    };

    let table = t.name.clone();
    let default_at = CellRef::new(
        t.range.start.row,
        t.range.end.col.saturating_add(2).min(gridcraft_core::MAX_COLS - 1),
    );
    let at = cell_param_on(s, p, "at", sheet)?.unwrap_or(default_at);
    let columns = u32_param(p, "columns").unwrap_or(1).clamp(1, 10);
    let multi_select = bool_param(p, "multiSelect").unwrap_or(false);
    let style = str_param(p, "style").unwrap_or("SlicerStyleLight1").to_string();
    let caption_override = str_param(p, "caption").map(str::to_string);
    let name_override = str_param(p, "name").map(str::to_string);
    let width = f64_param(p, "width").unwrap_or(168.0).clamp(60.0, 10000.0) as f32;
    let height = f64_param(p, "height").unwrap_or(200.0).clamp(60.0, 10000.0) as f32;
    let ids = d.wb.free_object_ids(req_fields.len());

    let mut built = Vec::with_capacity(req_fields.len());
    for (idx, column) in req_fields.into_iter().enumerate() {
        let id = ids.get(idx).copied().unwrap_or(u32::MAX);
        let offset = (idx as f32) * 24.0;
        let caption = if idx == 0 {
            caption_override.clone().unwrap_or_else(|| column.clone())
        } else {
            column.clone()
        };
        let name = if idx == 0 {
            name_override
                .clone()
                .unwrap_or_else(|| sanitize_object_name("Slicer", &column, id))
        } else {
            sanitize_object_name("Slicer", &column, id)
        };
        built.push(Slicer {
            id,
            name,
            table: table.clone(),
            pivots: vec![],
            column,
            caption,
            anchor: Anchor {
                cell: at,
                dx: 4.0 + offset,
                dy: 4.0 + offset,
                width,
                height,
                mode: AnchorMode::MoveOnly,
            },
            selected: None,
            columns,
            multi_select,
            style: style.clone(),
        });
    }

    let created_ids: Vec<u32> = built.iter().map(|sl| sl.id).collect();
    let first_id = created_ids.first().copied().unwrap_or(0);
    edit(s, |cx| {
        cx.sheet_mut(sheet)?.slicers.extend(built);
        Ok(json!({"slicer": first_id, "slicers": created_ids}))
    })
}

fn insert_pivot_slicers(
    s: &mut Session,
    p: &Json,
    sheet: usize,
    pt_sheet: usize,
    pt_idx: usize,
    pivot_names: Vec<String>,
) -> Result<Json> {
    let (headers, default_at, ids) = {
        let d = s.doc()?;
        let pt = d
            .wb
            .sheet(pt_sheet)
            .and_then(|sh| sh.pivots.get(pt_idx))
            .ok_or_else(|| bad("insert.slicer", "no such PivotTable"))?;
        let src = crate::pivot::read_source(&d.wb, &pt.source, pt_sheet)
            .map_err(|e| bad("insert.slicer", e))?;
        let right_col = pt
            .last_range
            .map_or(pt.anchor.col.saturating_add(3), |r| r.end.col.saturating_add(2))
            .min(gridcraft_core::MAX_COLS - 1);
        let top_row = pt.last_range.map_or(pt.anchor.row, |r| r.start.row);
        let count = requested_fields_param(p).map_or(1, |v| v.len().max(1));
        (src.headers, CellRef::new(top_row, right_col), d.wb.free_object_ids(count))
    };

    let req_fields = match requested_fields_param(p) {
        Some(list) => {
            let mut resolved = Vec::with_capacity(list.len());
            for c in list {
                let h = headers
                    .iter()
                    .find(|h| h.eq_ignore_ascii_case(&c))
                    .cloned()
                    .ok_or_else(|| bad("insert.slicer", format!("PivotTable has no field `{c}`")))?;
                resolved.push(h);
            }
            resolved
        }
        None => {
            let first = headers
                .first()
                .cloned()
                .ok_or_else(|| bad("insert.slicer", "the PivotTable source has no fields"))?;
            vec![first]
        }
    };

    let at = cell_param_on(s, p, "at", sheet)?.unwrap_or(default_at);
    let columns = u32_param(p, "columns").unwrap_or(1).clamp(1, 10);
    let multi_select = bool_param(p, "multiSelect").unwrap_or(false);
    let style = str_param(p, "style").unwrap_or("SlicerStyleLight1").to_string();
    let caption_override = str_param(p, "caption").map(str::to_string);
    let name_override = str_param(p, "name").map(str::to_string);
    let width = f64_param(p, "width").unwrap_or(180.0).clamp(60.0, 10000.0) as f32;
    let height = f64_param(p, "height").unwrap_or(210.0).clamp(60.0, 10000.0) as f32;

    let mut built = Vec::with_capacity(req_fields.len());
    for (idx, column) in req_fields.into_iter().enumerate() {
        let id = ids.get(idx).copied().unwrap_or(u32::MAX);
        let offset = (idx as f32) * 24.0;
        let caption = if idx == 0 {
            caption_override.clone().unwrap_or_else(|| column.clone())
        } else {
            column.clone()
        };
        let name = if idx == 0 {
            name_override
                .clone()
                .unwrap_or_else(|| sanitize_object_name("Slicer", &column, id))
        } else {
            sanitize_object_name("Slicer", &column, id)
        };
        built.push(Slicer {
            id,
            name,
            table: String::new(),
            pivots: pivot_names.clone(),
            column,
            caption,
            anchor: Anchor {
                cell: at,
                dx: 4.0 + offset,
                dy: 4.0 + offset,
                width,
                height,
                mode: AnchorMode::MoveOnly,
            },
            selected: None,
            columns,
            multi_select,
            style: style.clone(),
        });
    }

    let created_ids: Vec<u32> = built.iter().map(|sl| sl.id).collect();
    let first_id = created_ids.first().copied().unwrap_or(0);
    edit(s, |cx| {
        cx.sheet_mut(sheet)?.slicers.extend(built);
        Ok(json!({"slicer": first_id, "slicers": created_ids}))
    })
}

/// The target slicer: by `slicer` or `id` across sheets (preferring active sheet), else the last one on the active sheet.
fn find_slicer(s: &Session, p: &Json) -> Result<(usize, usize, u32)> {
    let d = s.doc()?;
    let active_si = d.wb.active_sheet();
    if let Some(id) = u32_param(p, "slicer").or_else(|| u32_param(p, "id")) {
        if let Some(sh) = d.wb.sheet(active_si)
            && let Some(idx) = sh.slicers.iter().position(|x| x.id == id)
        {
            return Ok((active_si, idx, id));
        }
        for (si, sh) in d.wb.sheets.iter().enumerate() {
            if let Some(idx) = sh.slicers.iter().position(|x| x.id == id) {
                return Ok((si, idx, id));
            }
        }
        return Err(bad("slicer", "no slicer with that id"));
    }
    let sh = d.wb.sheet(active_si).ok_or(EngineError::NoDocument)?;
    let idx = sh
        .slicers
        .len()
        .checked_sub(1)
        .ok_or_else(|| bad("slicer", "no slicer on this sheet"))?;
    let id = sh.slicers.get(idx).map_or(0, |x| x.id);
    Ok((active_si, idx, id))
}

fn slicer_toggle(s: &mut Session, p: &Json) -> Result<Json> {
    let (sheet, idx, id) = find_slicer(s, p)?;
    let value = str_param(p, "value")
        .or_else(|| str_param(p, "item"))
        .ok_or_else(|| bad("slicer.toggle", "missing `value`"))?
        .to_string();
    let (all, current, sl_multi) = {
        let d = s.doc()?;
        let sl = d
            .wb
            .sheet(sheet)
            .and_then(|sh| sh.slicers.get(idx))
            .ok_or_else(|| bad("slicer.toggle", "no slicer"))?;
        let all = slicer_values(&d.wb, sheet, sl);
        (all, sl.selected.clone(), sl.multi_select)
    };
    let multi = bool_param(p, "multi").unwrap_or(sl_multi);
    let canonical_val = all
        .iter()
        .find(|v| v.eq_ignore_ascii_case(&value))
        .cloned()
        .ok_or_else(|| bad("slicer.toggle", "value is not in the field"))?;

    let new_selected: Option<Vec<String>> = if multi {
        let mut set: Vec<String> = current.unwrap_or_else(|| all.clone());
        match set.iter().position(|x| x.eq_ignore_ascii_case(&canonical_val)) {
            Some(pos) => {
                set.remove(pos);
            }
            None => set.push(canonical_val),
        }
        if set.len() == all.len() && all.iter().all(|a| set.iter().any(|x| x.eq_ignore_ascii_case(a))) {
            None
        } else {
            Some(set)
        }
    } else {
        match &current {
            Some(set) if set.len() == 1 && set.iter().any(|x| x.eq_ignore_ascii_case(&canonical_val)) => None,
            _ => Some(vec![canonical_val]),
        }
    };
    edit(s, |cx| {
        if let Some(sl) = cx.sheet_mut(sheet)?.slicers.get_mut(idx) {
            sl.selected = new_selected;
        }
        reapply_slicer(&mut cx.wb, sheet, id);
        cx.structural = true;
        Ok(Json::Null)
    })
}

fn slicer_select(s: &mut Session, p: &Json) -> Result<Json> {
    if p.get("values").is_none() && (p.get("value").is_some() || p.get("item").is_some()) {
        return slicer_toggle(s, p);
    }
    let (sheet, idx, id) = find_slicer(s, p)?;
    let all = {
        let d = s.doc()?;
        let sl = d
            .wb
            .sheet(sheet)
            .and_then(|sh| sh.slicers.get(idx))
            .ok_or_else(|| bad("slicer.select", "no slicer"))?;
        slicer_values(&d.wb, sheet, sl)
    };
    let new_selected: Option<Vec<String>> = match p.get("values") {
        None | Some(Json::Null) => None,
        Some(Json::Array(arr)) => {
            let mut chosen = Vec::with_capacity(arr.len());
            for entry in arr {
                let raw = entry
                    .as_str()
                    .ok_or_else(|| bad("slicer.select", "`values` must be an array of strings"))?;
                let canon = all
                    .iter()
                    .find(|v| v.eq_ignore_ascii_case(raw))
                    .cloned()
                    .ok_or_else(|| bad("slicer.select", format!("value `{raw}` is not in the field")))?;
                if !chosen.iter().any(|x: &String| x.eq_ignore_ascii_case(&canon)) {
                    chosen.push(canon);
                }
            }
            if chosen.len() == all.len() && all.iter().all(|a| chosen.iter().any(|x| x.eq_ignore_ascii_case(a))) {
                None
            } else {
                Some(chosen)
            }
        }
        Some(_) => return Err(bad("slicer.select", "`values` must be an array of strings or null")),
    };
    edit(s, |cx| {
        if let Some(sl) = cx.sheet_mut(sheet)?.slicers.get_mut(idx) {
            sl.selected = new_selected;
        }
        reapply_slicer(&mut cx.wb, sheet, id);
        cx.structural = true;
        Ok(Json::Null)
    })
}

fn slicer_select_all(s: &mut Session, p: &Json) -> Result<Json> {
    let (sheet, idx, id) = find_slicer(s, p)?;
    edit(s, |cx| {
        if let Some(sl) = cx.sheet_mut(sheet)?.slicers.get_mut(idx) {
            sl.selected = None;
        }
        reapply_slicer(&mut cx.wb, sheet, id);
        cx.structural = true;
        Ok(Json::Null)
    })
}

fn slicer_update(s: &mut Session, p: &Json) -> Result<Json> {
    let (sheet, idx, id) = find_slicer(s, p)?;
    let at = cell_param_on(s, p, "at", sheet)?;
    let dx = f64_param(p, "dx").map(|v| v.clamp(-100_000.0, 100_000.0) as f32);
    let dy = f64_param(p, "dy").map(|v| v.clamp(-100_000.0, 100_000.0) as f32);
    let width = f64_param(p, "width").map(|v| v.clamp(60.0, 10_000.0) as f32);
    let height = f64_param(p, "height").map(|v| v.clamp(60.0, 10_000.0) as f32);
    let caption = str_param(p, "caption").map(str::to_string);
    let columns = u32_param(p, "columns").map(|c| c.clamp(1, 10));
    let multi_select = bool_param(p, "multiSelect");
    let style = str_param(p, "style").map(str::to_string);
    let new_pivots = if p.get("pivots").is_some() || p.get("pivot").is_some() {
        pivot_targets_param(&s.doc()?.wb, p, "slicer.update")?.map(|(_, _, names)| names)
    } else {
        None
    };

    edit(s, |cx| {
        if let Some(sl) = cx.sheet_mut(sheet)?.slicers.get_mut(idx) {
            if let Some(c) = at {
                sl.anchor.cell = c;
            }
            if let Some(x) = dx {
                sl.anchor.dx = x;
            }
            if let Some(y) = dy {
                sl.anchor.dy = y;
            }
            if let Some(w) = width {
                sl.anchor.width = w;
            }
            if let Some(h) = height {
                sl.anchor.height = h;
            }
            if let Some(cap) = caption {
                sl.caption = cap;
            }
            if let Some(cols) = columns {
                sl.columns = cols;
            }
            if let Some(ms) = multi_select {
                sl.multi_select = ms;
            }
            if let Some(st) = style {
                sl.style = st;
            }
            if let Some(pv) = new_pivots.clone() {
                sl.pivots = pv;
                sl.table.clear();
            }
        }
        if new_pivots.is_some() {
            reapply_slicer(&mut cx.wb, sheet, id);
            cx.structural = true;
        }
        Ok(Json::Null)
    })
}

fn slicer_delete(s: &mut Session, p: &Json) -> Result<Json> {
    let (sheet, idx, id) = find_slicer(s, p)?;
    let removed = s
        .doc()?
        .wb
        .sheet(sheet)
        .and_then(|sh| sh.slicers.get(idx))
        .cloned()
        .ok_or_else(|| bad("slicer.delete", "no slicer"))?;
    edit(s, |cx| {
        cx.sheet_mut(sheet)?.slicers.retain(|x| x.id != id);
        cleanup_deleted_slicer(&mut cx.wb, sheet, &removed);
        cx.structural = true;
        Ok(Json::Null)
    })
}

// ---------------------------------------------------------------- timelines

fn parse_optional_date_param(p: &Json, key: &str, cmd: &str) -> Result<Option<String>> {
    match p.get(key) {
        None | Some(Json::Null) => Ok(None),
        Some(Json::String(raw)) if raw.trim().is_empty() => Ok(None),
        Some(Json::String(raw)) => {
            let (y, m, d) = crate::pivot::parse_iso_ymd(raw)
                .ok_or_else(|| bad(cmd, format!("`{key}` must be an ISO date `YYYY-MM-DD`")))?;
            Ok(Some(format_iso_ymd(y, m, d)))
        }
        Some(_) => Err(bad(cmd, format!("`{key}` must be an ISO date string or null"))),
    }
}

fn normalize_date_bounds(
    start: Option<String>,
    end: Option<String>,
) -> (Option<String>, Option<String>) {
    match (&start, &end) {
        (Some(a), Some(b)) if a > b => (end, start),
        _ => (start, end),
    }
}

fn insert_timeline(s: &mut Session, p: &Json) -> Result<Json> {
    let wb_ref = &s.doc()?.wb;
    let resolved_pivots = match pivot_targets_param(wb_ref, p, "insert.timeline")? {
        Some(found) => found,
        None => {
            let target_sh = target_sheet(s, p)?;
            let d = s.doc()?;
            let sh = d.wb.sheet(target_sh).ok_or(EngineError::NoDocument)?;
            let pt_idx = sh
                .pivot_at(d.selection.active)
                .and_then(|pt| sh.pivots.iter().position(|x| x.id == pt.id))
                .or_else(|| (!sh.pivots.is_empty()).then_some(0))
                .ok_or_else(|| {
                    EngineError::Other(
                        "Select a cell inside a PivotTable first, or pass `pivot`.".into(),
                    )
                })?;
            let canon = pivot_ref_name(&d.wb, target_sh, pt_idx, "");
            (target_sh, pt_idx, vec![canon])
        }
    };
    let (pt_sheet, pt_idx, pivot_names) = resolved_pivots;
    let sheet = if p.get("sheet").is_some() { target_sheet(s, p)? } else { pt_sheet };

    let level = match str_param(p, "level") {
        Some(lv) => TimelineLevel::parse(lv)
            .ok_or_else(|| bad("insert.timeline", "level must be years, quarters, months or days"))?,
        None => TimelineLevel::Months,
    };
    let raw_start = parse_optional_date_param(p, "start", "insert.timeline")?
        .or(parse_optional_date_param(p, "startDate", "insert.timeline")?);
    let raw_end = parse_optional_date_param(p, "end", "insert.timeline")?
        .or(parse_optional_date_param(p, "endDate", "insert.timeline")?);
    let (start_date, end_date) = normalize_date_bounds(raw_start, raw_end);

    let (all_headers, date_headers, default_at, ids) = {
        let d = s.doc()?;
        let pt = d
            .wb
            .sheet(pt_sheet)
            .and_then(|sh| sh.pivots.get(pt_idx))
            .ok_or_else(|| bad("insert.timeline", "no such PivotTable"))?;
        let src = crate::pivot::read_source(&d.wb, &pt.source, pt_sheet)
            .map_err(|e| bad("insert.timeline", e))?;
        let date_cols: Vec<String> = src
            .headers
            .iter()
            .enumerate()
            .filter(|(c, _)| column_is_date(&src, *c))
            .map(|(_, h)| h.clone())
            .collect();
        let right_col = pt
            .last_range
            .map_or(pt.anchor.col.saturating_add(3), |r| r.end.col.saturating_add(2))
            .min(gridcraft_core::MAX_COLS - 1);
        let top_row = pt.last_range.map_or(pt.anchor.row, |r| r.start.row);
        let count = requested_fields_param(p).map_or(1, |v| v.len().max(1));
        (src.headers, date_cols, CellRef::new(top_row, right_col), d.wb.free_object_ids(count))
    };

    let req_fields = match requested_fields_param(p) {
        Some(list) => {
            let mut resolved = Vec::with_capacity(list.len());
            for f in list {
                let h = all_headers
                    .iter()
                    .find(|h| h.eq_ignore_ascii_case(&f))
                    .cloned()
                    .ok_or_else(|| bad("insert.timeline", format!("PivotTable has no field `{f}`")))?;
                resolved.push(h);
            }
            resolved
        }
        None => {
            let first = date_headers.first().cloned().ok_or_else(|| {
                EngineError::Other(
                    "We can't create a Timeline for this report because it doesn't have a field formatted as Date.".into(),
                )
            })?;
            vec![first]
        }
    };

    let at = cell_param_on(s, p, "at", sheet)?.unwrap_or(default_at);
    let style = str_param(p, "style").unwrap_or("TimeSlicerStyleLight1").to_string();
    let caption_override = str_param(p, "caption").map(str::to_string);
    let name_override = str_param(p, "name").map(str::to_string);
    let width = f64_param(p, "width").unwrap_or(320.0).clamp(120.0, 10000.0) as f32;
    let height = f64_param(p, "height").unwrap_or(124.0).clamp(72.0, 10000.0) as f32;

    let mut built = Vec::with_capacity(req_fields.len());
    for (idx, field) in req_fields.into_iter().enumerate() {
        let id = ids.get(idx).copied().unwrap_or(u32::MAX);
        let offset_y = (idx as f32) * (height + 12.0);
        let caption = if idx == 0 {
            caption_override.clone().unwrap_or_else(|| field.clone())
        } else {
            field.clone()
        };
        let name = if idx == 0 {
            name_override
                .clone()
                .unwrap_or_else(|| sanitize_object_name("Timeline", &field, id))
        } else {
            sanitize_object_name("Timeline", &field, id)
        };
        built.push(Timeline {
            id,
            name,
            caption,
            pivots: pivot_names.clone(),
            field,
            level,
            start_date: start_date.clone(),
            end_date: end_date.clone(),
            style: style.clone(),
            anchor: Anchor {
                cell: at,
                dx: 4.0,
                dy: 4.0 + offset_y,
                width,
                height,
                mode: AnchorMode::MoveOnly,
            },
        });
    }

    let created_ids: Vec<u32> = built.iter().map(|tl| tl.id).collect();
    let first_id = created_ids.first().copied().unwrap_or(0);
    let has_initial_range = start_date.is_some() || end_date.is_some();
    edit(s, |cx| {
        cx.sheet_mut(sheet)?.timelines.extend(built);
        if has_initial_range {
            for &id in &created_ids {
                reapply_timeline(&mut cx.wb, sheet, id);
            }
            cx.structural = true;
        }
        Ok(json!({"timeline": first_id, "timelines": created_ids}))
    })
}

fn find_timeline(s: &Session, p: &Json) -> Result<(usize, usize, u32)> {
    let d = s.doc()?;
    let active_si = d.wb.active_sheet();
    if let Some(id) = u32_param(p, "timeline").or_else(|| u32_param(p, "id")) {
        if let Some(sh) = d.wb.sheet(active_si)
            && let Some(idx) = sh.timelines.iter().position(|x| x.id == id)
        {
            return Ok((active_si, idx, id));
        }
        for (si, sh) in d.wb.sheets.iter().enumerate() {
            if let Some(idx) = sh.timelines.iter().position(|x| x.id == id) {
                return Ok((si, idx, id));
            }
        }
        return Err(bad("timeline", "no timeline with that id"));
    }
    let sh = d.wb.sheet(active_si).ok_or(EngineError::NoDocument)?;
    let idx = sh
        .timelines
        .len()
        .checked_sub(1)
        .ok_or_else(|| bad("timeline", "no timeline on this sheet"))?;
    let id = sh.timelines.get(idx).map_or(0, |x| x.id);
    Ok((active_si, idx, id))
}

fn timeline_set_range(s: &mut Session, p: &Json) -> Result<Json> {
    let (sheet, idx, id) = find_timeline(s, p)?;
    let raw_start = if p.get("start").is_some() {
        parse_optional_date_param(p, "start", "timeline.setRange")?
    } else if p.get("startDate").is_some() {
        parse_optional_date_param(p, "startDate", "timeline.setRange")?
    } else {
        s.doc()?
            .wb
            .sheet(sheet)
            .and_then(|sh| sh.timelines.get(idx))
            .and_then(|tl| tl.start_date.clone())
    };
    let raw_end = if p.get("end").is_some() {
        parse_optional_date_param(p, "end", "timeline.setRange")?
    } else if p.get("endDate").is_some() {
        parse_optional_date_param(p, "endDate", "timeline.setRange")?
    } else {
        s.doc()?
            .wb
            .sheet(sheet)
            .and_then(|sh| sh.timelines.get(idx))
            .and_then(|tl| tl.end_date.clone())
    };
    let (start_date, end_date) = normalize_date_bounds(raw_start, raw_end);
    edit(s, |cx| {
        if let Some(tl) = cx.sheet_mut(sheet)?.timelines.get_mut(idx) {
            tl.start_date = start_date;
            tl.end_date = end_date;
        }
        reapply_timeline(&mut cx.wb, sheet, id);
        cx.structural = true;
        Ok(Json::Null)
    })
}

fn timeline_set_level(s: &mut Session, p: &Json) -> Result<Json> {
    let (sheet, idx, _id) = find_timeline(s, p)?;
    let lv_str = str_param(p, "level").ok_or_else(|| bad("timeline.setLevel", "missing `level`"))?;
    let level = TimelineLevel::parse(lv_str)
        .ok_or_else(|| bad("timeline.setLevel", "level must be years, quarters, months or days"))?;
    edit(s, |cx| {
        if let Some(tl) = cx.sheet_mut(sheet)?.timelines.get_mut(idx) {
            tl.level = level;
        }
        Ok(Json::Null)
    })
}

fn timeline_clear(s: &mut Session, p: &Json) -> Result<Json> {
    let (sheet, idx, id) = find_timeline(s, p)?;
    edit(s, |cx| {
        if let Some(tl) = cx.sheet_mut(sheet)?.timelines.get_mut(idx) {
            tl.start_date = None;
            tl.end_date = None;
        }
        reapply_timeline(&mut cx.wb, sheet, id);
        cx.structural = true;
        Ok(Json::Null)
    })
}

fn timeline_update(s: &mut Session, p: &Json) -> Result<Json> {
    let (sheet, idx, id) = find_timeline(s, p)?;
    let at = cell_param_on(s, p, "at", sheet)?;
    let dx = f64_param(p, "dx").map(|v| v.clamp(-100_000.0, 100_000.0) as f32);
    let dy = f64_param(p, "dy").map(|v| v.clamp(-100_000.0, 100_000.0) as f32);
    let width = f64_param(p, "width").map(|v| v.clamp(120.0, 10_000.0) as f32);
    let height = f64_param(p, "height").map(|v| v.clamp(72.0, 10_000.0) as f32);
    let caption = str_param(p, "caption").map(str::to_string);
    let style = str_param(p, "style").map(str::to_string);
    let level = match str_param(p, "level") {
        Some(lv) => Some(
            TimelineLevel::parse(lv)
                .ok_or_else(|| bad("timeline.update", "level must be years, quarters, months or days"))?,
        ),
        None => None,
    };
    let new_pivots = if p.get("pivots").is_some() || p.get("pivot").is_some() {
        pivot_targets_param(&s.doc()?.wb, p, "timeline.update")?.map(|(_, _, names)| names)
    } else {
        None
    };

    edit(s, |cx| {
        if let Some(tl) = cx.sheet_mut(sheet)?.timelines.get_mut(idx) {
            if let Some(c) = at {
                tl.anchor.cell = c;
            }
            if let Some(x) = dx {
                tl.anchor.dx = x;
            }
            if let Some(y) = dy {
                tl.anchor.dy = y;
            }
            if let Some(w) = width {
                tl.anchor.width = w;
            }
            if let Some(h) = height {
                tl.anchor.height = h;
            }
            if let Some(cap) = caption {
                tl.caption = cap;
            }
            if let Some(st) = style {
                tl.style = st;
            }
            if let Some(lv) = level {
                tl.level = lv;
            }
            if let Some(pv) = new_pivots.clone() {
                tl.pivots = pv;
            }
        }
        if new_pivots.is_some() {
            reapply_timeline(&mut cx.wb, sheet, id);
            cx.structural = true;
        }
        Ok(Json::Null)
    })
}

fn timeline_delete(s: &mut Session, p: &Json) -> Result<Json> {
    let (sheet, idx, id) = find_timeline(s, p)?;
    let removed = s
        .doc()?
        .wb
        .sheet(sheet)
        .and_then(|sh| sh.timelines.get(idx))
        .cloned()
        .ok_or_else(|| bad("timeline.delete", "no timeline"))?;
    edit(s, |cx| {
        cx.sheet_mut(sheet)?.timelines.retain(|x| x.id != id);
        cleanup_deleted_timeline(&mut cx.wb, &removed);
        cx.structural = true;
        Ok(Json::Null)
    })
}
