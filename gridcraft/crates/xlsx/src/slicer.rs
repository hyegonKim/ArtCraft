//! OOXML Slicer (`xl/slicers/`, `xl/slicerCaches/`) and Timeline (`xl/timelines/`,
//! `xl/timelineCaches/`) reading and writing ([MS-XLSX] §2.3.2, §2.3.3, §2.3.5, §2.3.6).

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Arc;

use gridcraft_core::CellRef;
use gridcraft_model::{Anchor, AnchorMode, PivotDateFilter, PivotFilter, Slicer, Timeline, TimelineLevel, Workbook};

use crate::IoError;
use crate::read::Ctx;
use crate::write::{NS_MAIN, Out, Rels, XML_DECL};
use crate::xml::{El, esc_attr, num};

pub const CT_SLICER: &str = "application/vnd.ms-excel.slicer+xml";
pub const CT_SLICER_CACHE: &str = "application/vnd.ms-excel.slicerCache+xml";
pub const CT_TIMELINE: &str = "application/vnd.ms-excel.timeline+xml";
pub const CT_TIMELINE_CACHE: &str = "application/vnd.ms-excel.timelineCache+xml";

pub const REL_SLICER: &str = "http://schemas.microsoft.com/office/2007/relationships/slicer";
pub const REL_SLICER_CACHE: &str = "http://schemas.microsoft.com/office/2007/relationships/slicerCache";
pub const REL_TIMELINE: &str = "http://schemas.microsoft.com/office/2011/relationships/timeline";
pub const REL_TIMELINE_CACHE: &str = "http://schemas.microsoft.com/office/2011/relationships/timelineCache";

const NS_X14: &str = "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main";
const NS_X15: &str = "http://schemas.microsoft.com/office/spreadsheetml/2010/11/main";

const EXT_WB_SLICER_CACHES: &str = "{2F04AC57-C5E5-42C6-A20C-230D21FFEF46}";
const EXT_WB_TIMELINE_CACHES: &str = "{3BE3A9B2-AA1A-44DC-B10B-33CA1CA71E0E}";
const EXT_WS_SLICERS: &str = "{A8765BA9-456A-4DAB-B4F3-ACF838C121DE}";
const EXT_WS_TIMELINES: &str = "{7E03D99C-DC04-49d9-9315-930204A7B6E9}";
const EXT_TABLE_SLICER_CACHE: &str = "{2F2917AC-EB37-4324-AD4E-5DD8C200BD13}";

// ================================================================ writing

#[derive(Default)]
pub struct SlicerWriter {
    slicer_cache_rids: Vec<String>,
    timeline_cache_rids: Vec<String>,
    slicer_parts: u32,
    slicer_caches: u32,
    timeline_parts: u32,
    timeline_caches: u32,
}

impl SlicerWriter {
    pub fn workbook_ext_xml(&self) -> String {
        let mut s = String::new();
        if !self.slicer_cache_rids.is_empty() {
            let _ = write!(s, "<ext uri=\"{EXT_WB_SLICER_CACHES}\" xmlns:x14=\"{NS_X14}\"><x14:slicerCaches>");
            for rid in &self.slicer_cache_rids {
                let _ = write!(s, "<x14:slicerCache r:id=\"{rid}\"/>");
            }
            s.push_str("</x14:slicerCaches></ext>");
        }
        if !self.timeline_cache_rids.is_empty() {
            let _ = write!(s, "<ext uri=\"{EXT_WB_TIMELINE_CACHES}\" xmlns:x15=\"{NS_X15}\"><x15:timelineCacheRefs>");
            for rid in &self.timeline_cache_rids {
                let _ = write!(s, "<x15:timelineCacheRef r:id=\"{rid}\"/>");
            }
            s.push_str("</x15:timelineCacheRefs></ext>");
        }
        s
    }
}

fn anchor_mode_str(mode: AnchorMode) -> &'static str {
    match mode {
        AnchorMode::MoveAndSize => "moveAndSize",
        AnchorMode::MoveOnly => "moveOnly",
        AnchorMode::Absolute => "absolute",
    }
}

fn parse_anchor_mode(s: &str) -> AnchorMode {
    match s {
        "moveAndSize" => AnchorMode::MoveAndSize,
        "absolute" => AnchorMode::Absolute,
        _ => AnchorMode::MoveOnly,
    }
}

fn write_anchor_attrs(w: &mut String, a: &Anchor) {
    let _ = write!(
        w,
        " anchorCell=\"{}\" anchorDx=\"{}\" anchorDy=\"{}\" anchorWidth=\"{}\" anchorHeight=\"{}\" anchorMode=\"{}\"",
        a.cell.a1(),
        num(f64::from(a.dx)),
        num(f64::from(a.dy)),
        num(f64::from(a.width)),
        num(f64::from(a.height)),
        anchor_mode_str(a.mode),
    );
}

fn read_anchor_attrs(el: &El, default_w: f32, default_h: f32) -> Anchor {
    let cell = el.attr("anchorCell").and_then(CellRef::parse).unwrap_or_else(|| CellRef::new(1, 7));
    let dx = el.attr_f64("anchorDx").unwrap_or(4.0).clamp(-100_000.0, 100_000.0) as f32;
    let dy = el.attr_f64("anchorDy").unwrap_or(4.0).clamp(-100_000.0, 100_000.0) as f32;
    let width = el.attr_f64("anchorWidth").map(|v| v.clamp(40.0, 10_000.0) as f32).unwrap_or(default_w);
    let height = el.attr_f64("anchorHeight").map(|v| v.clamp(40.0, 10_000.0) as f32).unwrap_or(default_h);
    let mode = el.attr("anchorMode").map_or(AnchorMode::MoveOnly, parse_anchor_mode);
    Anchor { cell, dx, dy, width, height, mode }
}

pub fn write_sheet_filters(wb: &Workbook, si: usize, out: &mut Out, sheet_rels: &mut Rels, wb_rels: &mut Rels, writer: &mut SlicerWriter) -> String {
    let Some(sheet) = wb.sheet(si) else { return String::new() };
    let mut ext = String::new();

    if !sheet.slicers.is_empty() {
        writer.slicer_parts += 1;
        let part_n = writer.slicer_parts;
        let mut slicers_xml = format!("{XML_DECL}<slicers xmlns=\"{NS_X14}\" xmlns:x=\"{NS_MAIN}\">");

        for sl in &sheet.slicers {
            writer.slicer_caches += 1;
            let cache_n = writer.slicer_caches;
            let obj_name = if sl.name.is_empty() { format!("Slicer_{}", cache_n) } else { sl.name.clone() };
            let cache_name = format!("SlicerCache_{cache_n}");
            let caption = if sl.caption.is_empty() { &sl.column } else { &sl.caption };
            let style = if sl.style.is_empty() { "SlicerStyleLight1" } else { &sl.style };
            let cols = sl.columns.max(1);

            let cache_xml = write_slicer_cache_xml(wb, si, sl, &cache_name);
            let cache_part = format!("xl/slicerCaches/slicerCache{cache_n}.xml");
            out.part(&cache_part, Some(CT_SLICER_CACHE), cache_xml.into_bytes());
            let cache_rid = wb_rels.add_uri(REL_SLICER_CACHE, &format!("slicerCaches/slicerCache{cache_n}.xml"));
            writer.slicer_cache_rids.push(cache_rid);

            let _ = write!(
                slicers_xml,
                "<slicer name=\"{}\" cache=\"{}\" caption=\"{}\" columnCount=\"{cols}\" style=\"{}\" rowHeight=\"228600\" showCaption=\"1\" objectId=\"{}\"{}",
                esc_attr(&obj_name),
                esc_attr(&cache_name),
                esc_attr(caption),
                esc_attr(style),
                sl.id,
                if sl.multi_select { " multiSelect=\"1\"" } else { "" },
            );
            write_anchor_attrs(&mut slicers_xml, &sl.anchor);
            slicers_xml.push_str("/>");
        }
        slicers_xml.push_str("</slicers>");

        let part_name = format!("xl/slicers/slicer{part_n}.xml");
        out.part(&part_name, Some(CT_SLICER), slicers_xml.into_bytes());
        let slicer_rid = sheet_rels.add_uri(REL_SLICER, &format!("../slicers/slicer{part_n}.xml"));
        let _ = write!(
            ext,
            "<ext uri=\"{EXT_WS_SLICERS}\" xmlns:x14=\"{NS_X14}\"><x14:slicerList><x14:slicer r:id=\"{slicer_rid}\"/></x14:slicerList></ext>"
        );
    }

    if !sheet.timelines.is_empty() {
        writer.timeline_parts += 1;
        let part_n = writer.timeline_parts;
        let mut timelines_xml = format!("{XML_DECL}<timelines xmlns=\"{NS_X15}\" xmlns:x=\"{NS_MAIN}\">");

        for tl in &sheet.timelines {
            writer.timeline_caches += 1;
            let cache_n = writer.timeline_caches;
            let obj_name = if tl.name.is_empty() { format!("Timeline_{cache_n}") } else { tl.name.clone() };
            let cache_name = format!("TimelineCache_{cache_n}");
            let caption = if tl.caption.is_empty() { &tl.field } else { &tl.caption };
            let style = if tl.style.is_empty() { "TimeSlicerStyleLight1" } else { &tl.style };
            let lv = tl.level.ooxml_num();

            let cache_xml = write_timeline_cache_xml(wb, tl, &cache_name);
            let cache_part = format!("xl/timelineCaches/timelineCache{cache_n}.xml");
            out.part(&cache_part, Some(CT_TIMELINE_CACHE), cache_xml.into_bytes());
            let cache_rid = wb_rels.add_uri(REL_TIMELINE_CACHE, &format!("timelineCaches/timelineCache{cache_n}.xml"));
            writer.timeline_cache_rids.push(cache_rid);

            let _ = write!(
                timelines_xml,
                "<timeline name=\"{}\" cache=\"{}\" caption=\"{}\" showHeader=\"1\" showSelectionLabel=\"1\" showTimeLevel=\"1\" showHorizontalScrollbar=\"1\" level=\"{lv}\" selectionLevel=\"{lv}\" style=\"{}\" objectId=\"{}\"",
                esc_attr(&obj_name),
                esc_attr(&cache_name),
                esc_attr(caption),
                esc_attr(style),
                tl.id,
            );
            write_anchor_attrs(&mut timelines_xml, &tl.anchor);
            timelines_xml.push_str("/>");
        }
        timelines_xml.push_str("</timelines>");

        let part_name = format!("xl/timelines/timeline{part_n}.xml");
        out.part(&part_name, Some(CT_TIMELINE), timelines_xml.into_bytes());
        let tl_rid = sheet_rels.add_uri(REL_TIMELINE, &format!("../timelines/timeline{part_n}.xml"));
        let _ = write!(
            ext,
            "<ext uri=\"{EXT_WS_TIMELINES}\" xmlns:x15=\"{NS_X15}\"><x15:timelineRefs><x15:timelineRef r:id=\"{tl_rid}\"/></x15:timelineRefs></ext>"
        );
    }

    ext
}

fn write_slicer_cache_xml(wb: &Workbook, sheet_idx: usize, sl: &Slicer, cache_name: &str) -> String {
    let mut x = format!(
        "{XML_DECL}<slicerCacheDefinition xmlns=\"{NS_X14}\" xmlns:x=\"{NS_MAIN}\" name=\"{}\" sourceName=\"{}\">",
        esc_attr(cache_name),
        esc_attr(&sl.column),
    );

    if sl.is_pivot() {
        x.push_str("<pivotTables>");
        for target in &sl.pivots {
            if let Some((pt_si, pt_idx)) = wb.pivot(target)
                && let Some(pt_sh) = wb.sheet(pt_si)
                && let Some(pt) = pt_sh.pivots.get(pt_idx)
            {
                let tab_id = if pt_sh.file_id > 0 { pt_sh.file_id } else { (pt_si + 1) as u32 };
                let _ = write!(x, "<pivotTable tabId=\"{tab_id}\" name=\"{}\" target=\"{}\"/>", esc_attr(&pt.name), esc_attr(target),);
            } else {
                let _ = write!(x, "<pivotTable tabId=\"1\" name=\"{}\" target=\"{}\"/>", esc_attr(target), esc_attr(target),);
            }
        }
        x.push_str("</pivotTables>");
    }

    match &sl.selected {
        None => {
            x.push_str("<data><tabular filtered=\"0\"/></data>");
        }
        Some(items) => {
            let _ = write!(x, "<data><tabular filtered=\"1\"><items count=\"{}\">", items.len());
            for item in items {
                let _ = write!(x, "<i n=\"{}\" s=\"1\"/>", esc_attr(item));
            }
            x.push_str("</items></tabular></data>");
        }
    }

    if !sl.is_pivot() && !sl.table.is_empty() {
        let resolved = wb
            .sheet(sheet_idx)
            .and_then(|sh| sh.tables.iter().position(|t| t.name.eq_ignore_ascii_case(&sl.table)).map(|ti| (sheet_idx, ti)))
            .or_else(|| wb.table(&sl.table));
        let (table_id, col_one_based) = resolved
            .and_then(|(tsi, ti)| {
                let t = wb.sheet(tsi)?.tables.get(ti)?;
                let col = t.column_index(&sl.column).map_or(1, |c| (c + 1) as u32);
                Some((t.id.max(1), col))
            })
            .unwrap_or((1, 1));
        let _ = write!(
            x,
            "<extLst><ext uri=\"{EXT_TABLE_SLICER_CACHE}\"><x15:tableSlicerCache xmlns:x15=\"{NS_X15}\" tableId=\"{table_id}\" tableName=\"{}\" column=\"{col_one_based}\" sortOrder=\"ascending\"/></ext></extLst>",
            esc_attr(&sl.table),
        );
    }

    x.push_str("</slicerCacheDefinition>");
    x
}

fn write_timeline_cache_xml(wb: &Workbook, tl: &Timeline, cache_name: &str) -> String {
    let mut x = format!(
        "{XML_DECL}<timelineCacheDefinition xmlns=\"{NS_X15}\" xmlns:x=\"{NS_MAIN}\" name=\"{}\" sourceName=\"{}\">",
        esc_attr(cache_name),
        esc_attr(&tl.field),
    );
    x.push_str("<pivotTables>");
    for target in &tl.pivots {
        if let Some((pt_si, pt_idx)) = wb.pivot(target)
            && let Some(pt_sh) = wb.sheet(pt_si)
            && let Some(pt) = pt_sh.pivots.get(pt_idx)
        {
            let tab_id = if pt_sh.file_id > 0 { pt_sh.file_id } else { (pt_si + 1) as u32 };
            let _ = write!(x, "<pivotTable tabId=\"{tab_id}\" name=\"{}\" target=\"{}\"/>", esc_attr(&pt.name), esc_attr(target),);
        } else {
            let _ = write!(x, "<pivotTable tabId=\"1\" name=\"{}\" target=\"{}\"/>", esc_attr(target), esc_attr(target),);
        }
    }
    x.push_str("</pivotTables><state>");
    if tl.start_date.is_some() || tl.end_date.is_some() {
        x.push_str("<selection");
        if let Some(st) = &tl.start_date {
            let _ = write!(x, " startDate=\"{}\"", esc_attr(st));
        }
        if let Some(en) = &tl.end_date {
            let _ = write!(x, " endDate=\"{}\"", esc_attr(en));
        }
        x.push_str("/>");
    }
    x.push_str("</state></timelineCacheDefinition>");
    x
}

// ================================================================ reading

#[derive(Clone, Debug, Default)]
struct SlicerCacheIn {
    source_name: String,
    table_name: Option<String>,
    table_id: Option<u32>,
    table_col: Option<u32>,
    pivots: Vec<(Option<u32>, String, Option<String>)>,
    selected: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default)]
struct TimelineCacheIn {
    source_name: String,
    pivots: Vec<(Option<u32>, String, Option<String>)>,
    start_date: Option<String>,
    end_date: Option<String>,
}

fn clean_iso_date(raw: &str) -> Option<String> {
    let prefix = raw.trim().split(['T', 't', ' ']).next()?.trim();
    if prefix.is_empty() {
        return None;
    }
    let mut parts = prefix.split('-');
    let y: i32 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    let d: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1900..=9999).contains(&y) || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(format!("{y:04}-{m:02}-{d:02}"))
}

fn resolve_pivot_link(wb: &Workbook, tab_id: Option<u32>, pt_name: &str, explicit_target: Option<&str>) -> String {
    if let Some(t) = explicit_target
        && !t.is_empty()
        && wb.pivot(t).is_some()
    {
        return t.to_string();
    }
    if let Some(tid) = tab_id {
        let by_file_id = wb.sheets.iter().position(|s| s.file_id == tid);
        let sheet_idx = by_file_id.or_else(|| {
            let idx = (tid as usize).checked_sub(1)?;
            (idx < wb.sheets.len()).then_some(idx)
        });
        if let Some(si) = sheet_idx
            && let Some(sh) = wb.sheet(si)
            && sh.pivots.iter().any(|p| p.name.eq_ignore_ascii_case(pt_name))
        {
            let count = wb.sheets.iter().flat_map(|s| s.pivots.iter()).filter(|p| p.name.eq_ignore_ascii_case(pt_name)).count();
            if count > 1 {
                return format!("{}!{pt_name}", gridcraft_formula::quote_sheet(&sh.name));
            }
            return pt_name.to_string();
        }
    }
    pt_name.to_string()
}

fn resolve_table_link(wb: &Workbook, cache: &SlicerCacheIn) -> (String, String) {
    if let Some(tname) = &cache.table_name
        && !tname.is_empty()
    {
        return (tname.clone(), cache.source_name.clone());
    }
    if let Some(tid) = cache.table_id {
        for sh in &wb.sheets {
            if let Some(t) = sh.tables.iter().find(|t| t.id == tid) {
                let col_name = if !cache.source_name.is_empty() {
                    cache.source_name.clone()
                } else {
                    cache
                        .table_col
                        .and_then(|c| c.checked_sub(1))
                        .and_then(|ci| t.columns.get(ci as usize))
                        .map(|tc| tc.name.clone())
                        .unwrap_or_default()
                };
                return (t.name.clone(), col_name);
            }
        }
    }
    (String::new(), cache.source_name.clone())
}

fn parse_slicer_cache(el: &El) -> Option<(String, SlicerCacheIn)> {
    if el.name != "slicerCacheDefinition" {
        return None;
    }
    let name = el.attr("name")?.to_string();
    let source_name = el.attr("sourceName").unwrap_or("").to_string();
    let mut pivots = Vec::new();
    if let Some(pts) = el.child("pivotTables") {
        for pt in pts.kids("pivotTable") {
            let tab_id = pt.attr_u32("tabId");
            let pt_name = pt.attr("name").unwrap_or("").to_string();
            let target = pt.attr("target").map(str::to_string);
            if !pt_name.is_empty() {
                pivots.push((tab_id, pt_name, target));
            }
        }
    }

    let mut selected: Option<Vec<String>> = None;
    if let Some(data) = el.child("data")
        && let Some(tab) = data.child("tabular")
    {
        let explicit_filtered = tab.attr_bool("filtered");
        let items_el = tab.child("items");
        let all_items: Vec<(&str, bool)> = items_el
            .map(|it| {
                it.kids("i")
                    .filter_map(|i| {
                        let label = i.attr("n").or_else(|| i.attr("c"))?;
                        let is_sel = i.flag("s", false);
                        Some((label, is_sel))
                    })
                    .collect()
            })
            .unwrap_or_default();

        match explicit_filtered {
            Some(false) => {
                selected = None;
            }
            Some(true) => {
                selected = Some(all_items.into_iter().filter(|(_, on)| *on).map(|(lbl, _)| lbl.to_string()).collect());
            }
            None => {
                // External Excel tabular slicer cache: filtered iff at least one item has `s="0"` / omitted `s`.
                if !all_items.is_empty() && all_items.iter().any(|(_, on)| !*on) {
                    selected = Some(all_items.into_iter().filter(|(_, on)| *on).map(|(lbl, _)| lbl.to_string()).collect());
                }
            }
        }
    }

    let mut table_name = None;
    let mut table_id = None;
    let mut table_col = None;
    if let Some(ext_lst) = el.child("extLst") {
        for ext in ext_lst.kids("ext") {
            if let Some(tsc) = ext.child("tableSlicerCache") {
                table_name = tsc.attr("tableName").map(str::to_string);
                table_id = tsc.attr_u32("tableId");
                table_col = tsc.attr_u32("column");
            }
        }
    }

    Some((name, SlicerCacheIn { source_name, table_name, table_id, table_col, pivots, selected }))
}

fn parse_timeline_cache(el: &El) -> Option<(String, TimelineCacheIn)> {
    if el.name != "timelineCacheDefinition" {
        return None;
    }
    let name = el.attr("name")?.to_string();
    let source_name = el.attr("sourceName").unwrap_or("").to_string();
    let mut pivots = Vec::new();
    if let Some(pts) = el.child("pivotTables") {
        for pt in pts.kids("pivotTable") {
            let tab_id = pt.attr_u32("tabId");
            let pt_name = pt.attr("name").unwrap_or("").to_string();
            let target = pt.attr("target").map(str::to_string);
            if !pt_name.is_empty() {
                pivots.push((tab_id, pt_name, target));
            }
        }
    }
    let mut start_date = None;
    let mut end_date = None;
    if let Some(state) = el.child("state")
        && let Some(sel) = state.child("selection")
    {
        start_date = sel.attr("startDate").and_then(clean_iso_date);
        end_date = sel.attr("endDate").and_then(clean_iso_date);
    }
    Some((name, TimelineCacheIn { source_name, pivots, start_date, end_date }))
}

pub fn read_workbook_filters(
    cx: &mut Ctx<'_>,
    wb: &mut Workbook,
    wb_rels: &[crate::package::Rel],
    sheet_parts: &[(usize, String)],
) -> Result<(), IoError> {
    let mut slicer_caches: HashMap<String, SlicerCacheIn> = HashMap::new();
    let mut timeline_caches: HashMap<String, TimelineCacheIn> = HashMap::new();

    for r in wb_rels {
        if r.kind.eq_ignore_ascii_case("slicerCache") {
            if let Some(xml) = cx.optional_xml(&r.target)?
                && let Some((k, v)) = parse_slicer_cache(&xml)
            {
                slicer_caches.insert(k.to_ascii_lowercase(), v);
            }
        } else if r.kind.eq_ignore_ascii_case("timelineCache")
            && let Some(xml) = cx.optional_xml(&r.target)?
            && let Some((k, v)) = parse_timeline_cache(&xml)
        {
            timeline_caches.insert(k.to_ascii_lowercase(), v);
        }
    }

    if slicer_caches.is_empty() && timeline_caches.is_empty() {
        return Ok(());
    }

    let mut used_ids: std::collections::HashSet<u32> = wb
        .sheets
        .iter()
        .flat_map(|s| {
            s.charts
                .iter()
                .map(|o| o.id)
                .chain(s.images.iter().map(|o| o.id))
                .chain(s.shapes.iter().map(|o| o.id))
                .chain(s.tables.iter().map(|o| o.id))
                .chain(s.timelines.iter().map(|o| o.id))
                .chain(s.slicers.iter().map(|o| o.id))
                .chain(s.controls.iter().map(|o| o.id))
        })
        .collect();
    let mut next_free = used_ids.iter().copied().max().unwrap_or(0).saturating_add(1).max(1);
    let mut alloc_id = |preferred: Option<u32>| -> u32 {
        if let Some(id) = preferred.filter(|&id| id > 0 && !used_ids.contains(&id)) {
            used_ids.insert(id);
            next_free = next_free.max(id.saturating_add(1));
            return id;
        }
        while used_ids.contains(&next_free) {
            next_free = next_free.saturating_add(1);
        }
        let id = next_free;
        used_ids.insert(id);
        next_free = next_free.saturating_add(1);
        id
    };

    for (model_si, ws_part) in sheet_parts {
        let ws_rels = cx.pkg.rels(ws_part)?;
        let mut read_slicers = Vec::new();
        let mut read_timelines = Vec::new();

        for r in &ws_rels {
            if r.kind.eq_ignore_ascii_case("slicer") {
                let Some(xml) = cx.optional_xml(&r.target)? else { continue };
                if xml.name != "slicers" {
                    continue;
                }
                for s_el in xml.kids("slicer") {
                    let cache_key = s_el.attr("cache").unwrap_or("").to_ascii_lowercase();
                    let Some(cache) = slicer_caches.get(&cache_key) else { continue };
                    let id = alloc_id(s_el.attr_u32("objectId"));
                    let name = s_el.attr("name").map(str::to_string).unwrap_or_else(|| format!("Slicer_{id}"));
                    let pivots: Vec<String> =
                        cache.pivots.iter().map(|(tid, pname, targ)| resolve_pivot_link(wb, *tid, pname, targ.as_deref())).collect();
                    let (table, column) = if pivots.is_empty() { resolve_table_link(wb, cache) } else { (String::new(), cache.source_name.clone()) };
                    let caption = s_el.attr("caption").map(str::to_string).unwrap_or_else(|| column.clone());
                    let columns = s_el.attr_u32("columnCount").unwrap_or(1).clamp(1, 10);
                    let multi_select = s_el.flag("multiSelect", false);
                    let style = s_el.attr("style").unwrap_or("SlicerStyleLight1").to_string();
                    let anchor = read_anchor_attrs(s_el, 180.0, 210.0);
                    read_slicers.push(Slicer {
                        id,
                        name,
                        table,
                        pivots,
                        column,
                        caption,
                        anchor,
                        selected: cache.selected.clone(),
                        columns,
                        multi_select,
                        style,
                    });
                }
            } else if r.kind.eq_ignore_ascii_case("timeline") {
                let Some(xml) = cx.optional_xml(&r.target)? else { continue };
                if xml.name != "timelines" {
                    continue;
                }
                for t_el in xml.kids("timeline") {
                    let cache_key = t_el.attr("cache").unwrap_or("").to_ascii_lowercase();
                    let Some(cache) = timeline_caches.get(&cache_key) else { continue };
                    let id = alloc_id(t_el.attr_u32("objectId"));
                    let name = t_el.attr("name").map(str::to_string).unwrap_or_else(|| format!("Timeline_{id}"));
                    let pivots: Vec<String> =
                        cache.pivots.iter().map(|(tid, pname, targ)| resolve_pivot_link(wb, *tid, pname, targ.as_deref())).collect();
                    let field = cache.source_name.clone();
                    let caption = t_el.attr("caption").map(str::to_string).unwrap_or_else(|| field.clone());
                    let level = t_el
                        .attr("level")
                        .or_else(|| t_el.attr("selectionLevel"))
                        .and_then(TimelineLevel::from_ooxml)
                        .unwrap_or(TimelineLevel::Months);
                    let style = t_el.attr("style").unwrap_or("TimeSlicerStyleLight1").to_string();
                    let anchor = read_anchor_attrs(t_el, 320.0, 124.0);
                    read_timelines.push(Timeline {
                        id,
                        name,
                        caption,
                        pivots,
                        field,
                        level,
                        start_date: cache.start_date.clone(),
                        end_date: cache.end_date.clone(),
                        style,
                        anchor,
                    });
                }
            }
        }

        if (!read_slicers.is_empty() || !read_timelines.is_empty())
            && let Some(sh) = wb.sheets.get_mut(*model_si)
        {
            let shm = Arc::make_mut(sh);
            shm.slicers.extend(read_slicers);
            shm.timelines.extend(read_timelines);
        }
    }

    // Sync Timeline date_filters & Pivot Slicer axis/report filters onto linked PivotTables.
    let all_timelines: Vec<Timeline> = wb.sheets.iter().flat_map(|s| s.timelines.iter().cloned()).filter(|tl| tl.is_filtered()).collect();
    for tl in all_timelines {
        for target in &tl.pivots {
            if let Some((pt_si, pt_idx)) = wb.pivot(target)
                && let Some(sh) = wb.sheet_mut(pt_si)
                && let Some(pt) = sh.pivots.get_mut(pt_idx)
            {
                pt.date_filters.retain(|df| !df.source_col.eq_ignore_ascii_case(&tl.field));
                pt.date_filters.push(PivotDateFilter {
                    source_col: tl.field.clone(),
                    start_date: tl.start_date.clone(),
                    end_date: tl.end_date.clone(),
                });
            }
        }
    }

    let all_pivot_slicers: Vec<Slicer> =
        wb.sheets.iter().flat_map(|s| s.slicers.iter().cloned()).filter(|sl| sl.is_pivot() && sl.selected.is_some()).collect();
    for sl in all_pivot_slicers {
        for target in &sl.pivots {
            if let Some((pt_si, pt_idx)) = wb.pivot(target)
                && let Some(sh) = wb.sheet_mut(pt_si)
                && let Some(pt) = sh.pivots.get_mut(pt_idx)
            {
                let on_axis = pt.rows.iter().chain(pt.columns.iter()).any(|f| f.source_col.eq_ignore_ascii_case(&sl.column));
                if let Some(pos) = pt.filters.iter().position(|f| f.source_col.eq_ignore_ascii_case(&sl.column)) {
                    if let Some(f) = pt.filters.get_mut(pos) {
                        f.selected = sl.selected.clone();
                    }
                } else if on_axis {
                    pt.filters.push(PivotFilter { source_col: sl.column.clone(), selected: sl.selected.clone() });
                }
            }
        }
    }

    Ok(())
}
