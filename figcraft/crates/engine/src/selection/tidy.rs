//! Tidy up (`arrange.tidy`): evens gaps across a 1D row, 1D column or full 2D grid while keeping
//! layer order, sizes and the first layer's anchor.

use std::collections::BTreeMap;

use figcraft_doc::{Document, NodeId};
use figcraft_geom::Rect;

#[derive(Clone, Copy, Debug)]
struct Item {
    id: NodeId,
    rect: Rect,
}

fn movable_items(doc: &Document, ids: &[NodeId]) -> Vec<Item> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for &id in ids {
        if !seen.insert(id) {
            continue;
        }
        let Some(n) = doc.node(id) else { continue };
        if n.locked || n.is_page() {
            continue;
        }
        // Auto-layout flow children are positioned by their parent frame unless absolute.
        let in_flow = n
            .parent
            .and_then(|p| doc.node(p))
            .is_some_and(|pn| pn.layout.is_some() && !n.child_layout.absolute);
        if in_flow {
            continue;
        }
        let Some(rect) = doc.world_bounds(id) else { continue };
        out.push(Item { id, rect });
    }
    out
}

fn round_half_up(v: f64) -> f64 {
    (v + 0.5).floor()
}

fn overlaps_1d(a0: f64, a1: f64, b0: f64, b1: f64) -> bool {
    let lo = a0.max(b0);
    let hi = a1.min(b1);
    hi > lo || ((a1 - a0).abs() <= 1e-9 && (b1 - b0).abs() <= 1e-9 && (a0 - b0).abs() <= 1e-9)
}

fn is_row(items: &[Item]) -> bool {
    for i in 0..items.len() {
        for j in (i + 1)..items.len() {
            if !overlaps_1d(items[i].rect.y0, items[i].rect.y1, items[j].rect.y0, items[j].rect.y1) {
                return false;
            }
        }
    }
    let mut sorted = items.to_vec();
    sorted.sort_by(|a, b| a.rect.x0.total_cmp(&b.rect.x0).then_with(|| a.rect.x1.total_cmp(&b.rect.x1)));
    for i in 0..sorted.len().saturating_sub(2) {
        if sorted[i + 2].rect.x0 < sorted[i].rect.x1 - 1e-9 {
            return false;
        }
    }
    true
}

fn is_column(items: &[Item]) -> bool {
    for i in 0..items.len() {
        for j in (i + 1)..items.len() {
            if !overlaps_1d(items[i].rect.x0, items[i].rect.x1, items[j].rect.x0, items[j].rect.x1) {
                return false;
            }
        }
    }
    let mut sorted = items.to_vec();
    sorted.sort_by(|a, b| a.rect.y0.total_cmp(&b.rect.y0).then_with(|| a.rect.y1.total_cmp(&b.rect.y1)));
    for i in 0..sorted.len().saturating_sub(2) {
        if sorted[i + 2].rect.y0 < sorted[i].rect.y1 - 1e-9 {
            return false;
        }
    }
    true
}

/// Connected interval bands along one axis (`(lo, hi, indices)`), sorted by `lo`.
fn axis_bands(items: &[Item], horizontal: bool) -> Vec<(f64, f64, Vec<usize>)> {
    let mut order: Vec<(usize, f64, f64)> = items
        .iter()
        .enumerate()
        .map(|(i, it)| if horizontal { (i, it.rect.x0, it.rect.x1) } else { (i, it.rect.y0, it.rect.y1) })
        .collect();
    order.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.2.total_cmp(&b.2)));

    let mut bands: Vec<(f64, f64, Vec<usize>)> = Vec::new();
    for (idx, lo, hi) in order {
        if let Some(last) = bands.last_mut()
            && overlaps_1d(last.0, last.1, lo, hi)
        {
            last.1 = last.1.max(hi);
            last.2.push(idx);
            continue;
        }
        bands.push((lo, hi, vec![idx]));
    }
    bands
}

/// Returns `(rows, cols, grid_cells)` where `grid_cells[r][c]` is the item index at `(r, c)`.
fn full_grid(items: &[Item]) -> Option<(usize, usize, Vec<Vec<usize>>)> {
    let y_bands = axis_bands(items, false);
    let x_bands = axis_bands(items, true);
    let rows = y_bands.len();
    let cols = x_bands.len();
    if rows < 2 || cols < 2 || rows.checked_mul(cols) != Some(items.len()) {
        return None;
    }
    let mut row_of = vec![0usize; items.len()];
    for (r, (_, _, idxs)) in y_bands.iter().enumerate() {
        if idxs.len() != cols {
            return None;
        }
        for &i in idxs {
            row_of[i] = r;
        }
    }
    let mut col_of = vec![0usize; items.len()];
    for (c, (_, _, idxs)) in x_bands.iter().enumerate() {
        if idxs.len() != rows {
            return None;
        }
        for &i in idxs {
            col_of[i] = c;
        }
    }
    let mut cells = vec![vec![usize::MAX; cols]; rows];
    for i in 0..items.len() {
        let cell = &mut cells[row_of[i]][col_of[i]];
        if *cell != usize::MAX {
            return None;
        }
        *cell = i;
    }
    Some((rows, cols, cells))
}

#[derive(Debug)]
enum Arrangement {
    Row,
    Column,
    Grid { rows: usize, cols: usize, cells: Vec<Vec<usize>> },
}

fn classify(items: &[Item]) -> Option<Arrangement> {
    if is_row(items) {
        Some(Arrangement::Row)
    } else if is_column(items) {
        Some(Arrangement::Column)
    } else {
        full_grid(items).map(|(rows, cols, cells)| Arrangement::Grid { rows, cols, cells })
    }
}

/// Pick the modal rounded gap (ties go to the smaller gap), or when every rounded gap occurs once,
/// the mean rounded half-up.
fn chosen_gap(gaps: &[f64]) -> f64 {
    if gaps.is_empty() {
        return 0.0;
    }
    let mut counts: BTreeMap<i64, usize> = BTreeMap::new();
    for &g in gaps {
        let k = round_half_up(g) as i64;
        *counts.entry(k).or_insert(0) += 1;
    }
    let max_count = counts.values().copied().max().unwrap_or(0);
    if max_count >= 2
        && let Some((&best_gap, _)) = counts.iter().find(|&(_, &c)| c == max_count)
    {
        return best_gap as f64;
    }
    let sum: f64 = gaps.iter().sum();
    round_half_up(sum / (gaps.len() as f64))
}

/// Raise a negative inferred gap so adjacent layers stay in order (`sizes[0] + g >= 1`) and no
/// layer reaches the one two places before it (`sizes[i + 1] + 2 * g >= 2`).
fn clamp_negative_gap(mut gap: f64, sizes: &[f64]) -> f64 {
    if gap >= 0.0 {
        return gap;
    }
    if let Some(&first) = sizes.first() {
        gap = gap.max((1.0 - first).ceil());
    }
    if sizes.len() >= 3 {
        for &mid in &sizes[1..sizes.len() - 1] {
            gap = gap.max(((2.0 - mid) / 2.0).ceil());
        }
    }
    gap
}

fn snap_coord(v: f64, snap: bool) -> f64 {
    if snap { round_half_up(v) } else { v }
}

/// Whether the Design panel and menu should enable Tidy up for `ids` (requires at least 3 movable
/// layers forming a row, column or full grid).
pub fn can_tidy(doc: &Document, ids: &[NodeId]) -> bool {
    let items = movable_items(doc, ids);
    items.len() >= 3 && classify(&items).is_some()
}

/// Compute `(id, new_world_x, new_world_y)` for layers in `ids` whose position changes when tidied.
pub fn tidy_moves(doc: &Document, ids: &[NodeId], spacing: Option<f64>, snap: bool) -> Vec<(NodeId, f64, f64)> {
    let items = movable_items(doc, ids);
    let min_len = if spacing.is_some() { 2 } else { 3 };
    if items.len() < min_len {
        return Vec::new();
    }
    let Some(arr) = classify(&items) else {
        return Vec::new();
    };

    let mut targets: BTreeMap<NodeId, (f64, f64)> = BTreeMap::new();
    match arr {
        Arrangement::Row => {
            let mut sorted = items.clone();
            sorted.sort_by(|a, b| a.rect.x0.total_cmp(&b.rect.x0).then_with(|| a.rect.x1.total_cmp(&b.rect.x1)));
            let gaps: Vec<f64> = sorted.windows(2).map(|w| w[1].rect.x0 - w[0].rect.x1).collect();
            let widths: Vec<f64> = sorted.iter().map(|it| it.rect.width()).collect();
            let gap = spacing.unwrap_or_else(|| clamp_negative_gap(chosen_gap(&gaps), &widths));
            let mut x = sorted[0].rect.x0;
            for it in &sorted {
                targets.insert(it.id, (x, it.rect.y0));
                x += it.rect.width() + gap;
            }
        }
        Arrangement::Column => {
            let mut sorted = items.clone();
            sorted.sort_by(|a, b| a.rect.y0.total_cmp(&b.rect.y0).then_with(|| a.rect.y1.total_cmp(&b.rect.y1)));
            let gaps: Vec<f64> = sorted.windows(2).map(|w| w[1].rect.y0 - w[0].rect.y1).collect();
            let heights: Vec<f64> = sorted.iter().map(|it| it.rect.height()).collect();
            let gap = spacing.unwrap_or_else(|| clamp_negative_gap(chosen_gap(&gaps), &heights));
            let mut y = sorted[0].rect.y0;
            for it in &sorted {
                targets.insert(it.id, (it.rect.x0, y));
                y += it.rect.height() + gap;
            }
        }
        Arrangement::Grid { rows, cols, cells } => {
            if cols >= rows {
                // Column-major tidy: columns centred on their max width and evenly spaced; within
                // each column, items stack from the top row's min_y with a shared vertical gap.
                let mut col_widths = vec![0.0_f64; cols];
                let mut col_boxes = vec![(0.0_f64, 0.0_f64); cols];
                for c in 0..cols {
                    let mut min_x = f64::INFINITY;
                    let mut max_x = f64::NEG_INFINITY;
                    let mut max_w = 0.0_f64;
                    for r in 0..rows {
                        let rect = items[cells[r][c]].rect;
                        min_x = min_x.min(rect.x0);
                        max_x = max_x.max(rect.x1);
                        max_w = max_w.max(rect.width());
                    }
                    let centre = (min_x + max_x) * 0.5;
                    col_widths[c] = max_w;
                    col_boxes[c] = (centre - max_w * 0.5, centre + max_w * 0.5);
                }
                let col_gaps: Vec<f64> = col_boxes.windows(2).map(|w| w[1].0 - w[0].1).collect();
                let col_gap = spacing.unwrap_or_else(|| chosen_gap(&col_gaps));

                let mut v_gaps = Vec::new();
                for c in 0..cols {
                    for r in 0..(rows - 1) {
                        let upper = items[cells[r][c]].rect;
                        let lower = items[cells[r + 1][c]].rect;
                        v_gaps.push(lower.y0 - upper.y1);
                    }
                }
                let row_gap = spacing.unwrap_or_else(|| chosen_gap(&v_gaps));
                let top_y = (0..cols)
                    .map(|c| items[cells[0][c]].rect.y0)
                    .fold(f64::INFINITY, f64::min);

                let mut col_x = snap_coord(col_boxes[0].0, snap);
                for c in 0..cols {
                    let cw = col_widths[c];
                    let mut y = top_y;
                    for r in 0..rows {
                        let it = items[cells[r][c]];
                        let ix = snap_coord(col_x + (cw - it.rect.width()) * 0.5, snap);
                        let iy = snap_coord(y, snap);
                        targets.insert(it.id, (ix, iy));
                        y += it.rect.height() + row_gap;
                    }
                    col_x += cw + col_gap;
                }
            } else {
                // Row-major tidy (transposed): rows centred on their max height and evenly spaced;
                // within each row, items stack from the first column's min_x with a shared horizontal gap.
                let mut row_heights = vec![0.0_f64; rows];
                let mut row_boxes = vec![(0.0_f64, 0.0_f64); rows];
                for r in 0..rows {
                    let mut min_y = f64::INFINITY;
                    let mut max_y = f64::NEG_INFINITY;
                    let mut max_h = 0.0_f64;
                    for c in 0..cols {
                        let rect = items[cells[r][c]].rect;
                        min_y = min_y.min(rect.y0);
                        max_y = max_y.max(rect.y1);
                        max_h = max_h.max(rect.height());
                    }
                    let centre = (min_y + max_y) * 0.5;
                    row_heights[r] = max_h;
                    row_boxes[r] = (centre - max_h * 0.5, centre + max_h * 0.5);
                }
                let row_gaps: Vec<f64> = row_boxes.windows(2).map(|w| w[1].0 - w[0].1).collect();
                let row_gap = spacing.unwrap_or_else(|| chosen_gap(&row_gaps));

                let mut h_gaps = Vec::new();
                for r in 0..rows {
                    for c in 0..(cols - 1) {
                        let left = items[cells[r][c]].rect;
                        let right = items[cells[r][c + 1]].rect;
                        h_gaps.push(right.x0 - left.x1);
                    }
                }
                let col_gap = spacing.unwrap_or_else(|| chosen_gap(&h_gaps));
                let left_x = (0..rows)
                    .map(|r| items[cells[r][0]].rect.x0)
                    .fold(f64::INFINITY, f64::min);

                let mut row_y = snap_coord(row_boxes[0].0, snap);
                for r in 0..rows {
                    let rh = row_heights[r];
                    let mut x = left_x;
                    for c in 0..cols {
                        let it = items[cells[r][c]];
                        let ix = snap_coord(x, snap);
                        let iy = snap_coord(row_y + (rh - it.rect.height()) * 0.5, snap);
                        targets.insert(it.id, (ix, iy));
                        x += it.rect.width() + col_gap;
                    }
                    row_y += rh + row_gap;
                }
            }
        }
    }

    items
        .into_iter()
        .filter_map(|it| {
            let &(tx, ty) = targets.get(&it.id)?;
            if (tx - it.rect.x0).abs() > 1e-9 || (ty - it.rect.y0).abs() > 1e-9 {
                Some((it.id, tx - it.rect.x0, ty - it.rect.y0))
            } else {
                None
            }
        })
        .collect()
}
