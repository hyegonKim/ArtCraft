//! Mind maps on boards: `board.mindMap`, `board.mindMapAdd` and the normalize layout pass.

use std::collections::{BTreeMap, BTreeSet};

use figcraft_color::{Color, Paint};
use figcraft_doc::{
    AutoLayout, BoardItem, Connector, CrossAlign, Document, End, LayoutMode, LetterSpacing, LineHeight, MainAlign, MindBox, NodeId, NodeKind,
    Routing, Side, Sizing, StrokeCap, TextAutoResize, TextData, mind_layout,
};
use figcraft_geom::{Affine, Point};
use serde_json::{Value, json};

use super::connectors::make;
use super::items::{add, owner_of_text, text_of};
use super::{EngineError, INK, Result, Session, bad, boolean, get, id, is_board, parent_of, require, string};

const MAX_COORD: f64 = 1e9;
const MAX_CHILDREN: usize = 20;
const MAX_TEXT_CHARS: usize = 500;
const BRANCH_INK: Color = Color::from_rgb8_const(0x75, 0x75, 0x75);
const BRANCH_WEIGHT: f64 = 4.0;

fn truncate_text(s: &str) -> String {
    s.chars().take(MAX_TEXT_CHARS).collect()
}

fn coord_param(cmd: &str, p: &Value, key: &str) -> Result<Option<f64>> {
    match get(p, key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => {
            let x = v.as_f64().filter(|x| x.is_finite() && (-MAX_COORD..=MAX_COORD).contains(x)).ok_or_else(|| {
                bad(cmd, format!("`{key}` must be a finite coordinate within ±{MAX_COORD}"))
            })?;
            Ok(Some(x))
        }
    }
}

fn make_mind_node(doc: &mut Document, parent: NodeId, pos: (f64, f64), text: &str, root: bool) -> Result<NodeId> {
    let clean = truncate_text(text);
    let mut td = TextData::new(clean.clone());
    td.style.font_size = if root { 24.0 } else { 16.0 };
    td.auto_resize = TextAutoResize::WidthAndHeight;
    td.style.font_style = if root { "Bold".into() } else { "Medium".into() };
    td.style.line_height = LineHeight::Percent(150.0);
    td.style.letter_spacing = LetterSpacing::Percent(if root { -1.9 } else { -1.1 });

    let node_id = add(doc, parent, NodeKind::Frame, if clean.is_empty() { "Mind map node" } else { &clean }, pos, (80.0, 32.0), |n| {
        n.board = Some(BoardItem::MindNode);
        n.fills.clear();
        n.clip = false;
        n.layout = Some(AutoLayout {
            mode: LayoutMode::Horizontal,
            gap: 0.0,
            padding: [0.0; 4],
            main_align: MainAlign::Min,
            cross_align: CrossAlign::Center,
            ..AutoLayout::default()
        });
        n.child_layout.sizing_h = Sizing::Hug;
        n.child_layout.sizing_v = Sizing::Hug;
    })?;

    let tid = add(doc, node_id, NodeKind::Text(td), "Text", (0.0, 0.0), (80.0, 32.0), |t| {
        t.fills = vec![Paint::solid(INK)];
    })?;

    if let Some((tw, th)) = doc.node(tid).and_then(figcraft_text::auto_size) {
        if let Some(tn) = doc.node_mut(tid) {
            (tn.width, tn.height) = (tw, th);
        }
        if let Some(fn_) = doc.node_mut(node_id) {
            (fn_.width, fn_.height) = (tw, th);
        }
    }
    Ok(node_id)
}

fn make_branch(doc: &mut Document, page: NodeId, from: NodeId, to: NodeId, left: bool) -> Result<NodeId> {
    let (from_side, to_side) = if left { (Side::Left, Side::Right) } else { (Side::Right, Side::Left) };
    let c = Connector {
        from: End::Node { node: from, side: from_side, at: None },
        to: End::Node { node: to, side: to_side, at: None },
        routing: Routing::Curved,
        path: None,
        branch: true,
    };
    let cid = make(doc, page, c, (StrokeCap::Round, StrokeCap::Round))?;
    if let Some(n) = doc.node_mut(cid) {
        n.strokes = vec![Paint::solid(BRANCH_INK)];
        n.stroke.weight = BRANCH_WEIGHT;
        n.stroke.cap_start = StrokeCap::Round;
        n.stroke.cap_end = StrokeCap::Round;
    }
    Ok(cid)
}

pub(super) fn mind_map_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "board.mindMap";
    require(s, cmd)?;
    let parent = parent_of(cmd, s, p)?;
    let x_opt = coord_param(cmd, p, "x")?;
    let y_opt = coord_param(cmd, p, "y")?;

    let child_texts: Vec<String> = match get(p, "children") {
        None | Some(Value::Null) => vec!["Idea 1".into(), "Idea 2".into(), "Idea 3".into()],
        Some(Value::Array(arr)) => {
            if arr.len() > MAX_CHILDREN {
                return Err(bad(cmd, format!("`children` takes at most {MAX_CHILDREN} items")));
            }
            let mut out = Vec::with_capacity(arr.len());
            for item in arr {
                let sv = item.as_str().ok_or_else(|| bad(cmd, "`children` must be an array of strings"))?;
                out.push(truncate_text(sv));
            }
            out
        }
        Some(_) => return Err(bad(cmd, "`children` must be an array of strings")),
    };

    let root_text = truncate_text(&string(p, "text").unwrap_or_else(|| "Topic".into()));
    let edit = boolean(p, "edit").unwrap_or(false);
    if edit && s.ds()?.text_edit.is_some() {
        crate::textedit::end(s)?;
    }

    let (vx, vy) = s.ds().map(|d| (d.view.cx, d.view.cy)).unwrap_or((0.0, 0.0));
    let page = s.page()?;
    let doc = s.doc_mut()?;
    let default_local = doc.world_transform(parent).inverse() * Point::new(vx, vy);
    let rx = x_opt.unwrap_or(default_local.x);
    let ry = y_opt.unwrap_or(default_local.y);

    let root = make_mind_node(doc, parent, (rx, ry), &root_text, true)?;
    let mut children_out = Vec::with_capacity(child_texts.len());
    for (i, ct) in child_texts.iter().enumerate() {
        let cy = ry + (i as f64) * 60.0;
        let child = make_mind_node(doc, parent, (rx + 240.0, cy), ct, false)?;
        let conn = make_branch(doc, page, root, child, false)?;
        children_out.push(json!({
            "id": child,
            "text": text_of(doc, child),
            "connector": conn,
        }));
    }

    let final_root_text = text_of(doc, root);
    s.set_selection(vec![root]);
    if edit {
        s.request_ui("board.editText", json!({"id": root}));
    }
    Ok(json!({
        "id": root,
        "text": final_root_text,
        "children": children_out,
    }))
}

/// Incoming `(parent_node, left_side, connector_id)` for each mind node on `page`.
fn incoming_branches(doc: &Document, page: NodeId) -> BTreeMap<NodeId, (NodeId, bool, NodeId)> {
    let mut map: BTreeMap<NodeId, (NodeId, bool, NodeId)> = BTreeMap::new();
    for n in doc.nodes() {
        let Some(BoardItem::Connector(c)) = &n.board else { continue };
        if !c.branch || doc.page_of(n.id) != Some(page) {
            continue;
        }
        let (End::Node { node: from, side: from_side, .. }, End::Node { node: to, .. }) = (&c.from, &c.to) else {
            continue;
        };
        let valid = doc.node(*from).is_some_and(|m| m.board == Some(BoardItem::MindNode) && doc.page_of(*from) == Some(page))
            && doc.node(*to).is_some_and(|m| m.board == Some(BoardItem::MindNode) && doc.page_of(*to) == Some(page))
            && *from != *to;
        if !valid {
            continue;
        }
        let left = match from_side {
            Side::Left => true,
            Side::Right => false,
            _ => doc
                .world_bounds(*to)
                .zip(doc.world_bounds(*from))
                .is_some_and(|(tb, fb)| tb.center().x < fb.center().x),
        };
        map.entry(*to).or_insert((*from, left, n.id));
    }
    map
}

pub(super) fn mind_map_add(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "board.mindMapAdd";
    require(s, cmd)?;
    let relation = string(p, "relation").unwrap_or_else(|| "child".into());
    if !matches!(relation.as_str(), "child" | "sibling") {
        return Err(bad(cmd, "`relation` is child or sibling"));
    }

    let target = match id(p, "id") {
        Some(v) => v,
        None => {
            let ds = s.ds()?;
            let cand = ds
                .text_edit
                .as_ref()
                .map(|t| t.id)
                .or_else(|| ds.selection.first().copied())
                .and_then(|i| owner_of_text(&ds.doc, i).or(Some(i)));
            cand.ok_or_else(|| bad(cmd, "select a mind map node"))?
        }
    };

    let edit = boolean(p, "edit").unwrap_or(false);
    if edit && s.ds()?.text_edit.is_some() {
        crate::textedit::end(s)?;
    }

    let page = s.page()?;
    let text = truncate_text(&string(p, "text").unwrap_or_default());
    let doc = s.doc_mut()?;
    if doc.get(target)?.board != Some(BoardItem::MindNode) {
        return Err(bad(cmd, "target must be a mind map node"));
    }
    if doc.page_of(target) != Some(page) {
        return Err(bad(cmd, "target must be on the active page"));
    }

    let incoming = incoming_branches(doc, page);
    let (branch_parent, left, place_world) = if relation == "sibling" {
        let Some(&(par, left, _)) = incoming.get(&target) else {
            return Err(EngineError::Disabled(cmd.into(), "the first node of a mind map has no parent to add a sibling under".into()));
        };
        let target_cy = doc.world_bounds(target).map_or(0.0, |b| b.center().y);
        let mut sib_cys: Vec<(f64, NodeId)> = incoming
            .iter()
            .filter(|&(_, &(p_id, l, _))| p_id == par && l == left)
            .map(|(&cid, _)| (doc.world_bounds(cid).map_or(0.0, |b| b.center().y), cid))
            .collect();
        sib_cys.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        let pos_idx = sib_cys.iter().position(|(_, id)| *id == target).unwrap_or(0);
        let desired_cy = match sib_cys.get(pos_idx + 1) {
            Some((next_cy, _)) if (*next_cy - target_cy).abs() > 1e-3 => (target_cy + *next_cy) * 0.5,
            _ => target_cy + 80.0,
        };
        let tx = doc.world_bounds(target).map_or(0.0, |b| b.x0);
        (par, left, Point::new(tx, desired_cy - 16.0))
    } else {
        let left = incoming.get(&target).is_some_and(|&(_, l, _)| l);
        let max_cy = incoming
            .iter()
            .filter(|&(_, &(p_id, l, _))| p_id == target && l == left)
            .filter_map(|(&cid, _)| doc.world_bounds(cid).map(|b| b.center().y))
            .fold(f64::NEG_INFINITY, f64::max);
        let tb = doc.world_bounds(target).unwrap_or_default();
        let desired_cy = if max_cy.is_finite() { max_cy + 80.0 } else { tb.center().y };
        let dx = if left { -220.0 } else { 220.0 };
        (target, left, Point::new(tb.x0 + dx, desired_cy - 16.0))
    };

    let node_parent = doc.node(branch_parent).and_then(|n| n.parent).unwrap_or(page);
    let local = doc.world_transform(node_parent).inverse() * place_world;
    let new_node = make_mind_node(doc, node_parent, (local.x, local.y), &text, false)?;
    let conn = make_branch(doc, page, branch_parent, new_node, left)?;
    let final_text = text_of(doc, new_node);

    s.set_selection(vec![new_node]);
    if edit {
        s.request_ui("board.editText", json!({"id": new_node}));
    }
    Ok(json!({
        "id": new_node,
        "text": final_text,
        "connector": conn,
    }))
}

/// Re-flow mind maps whose nodes or branches changed, and clean up branches whose mind nodes were
/// deleted or detached.
pub(crate) fn settle_mind_maps(doc: &mut Document, before: &Document, touched: &BTreeSet<NodeId>) {
    if !is_board(doc) {
        return;
    }

    // 1. Delete branches whose attached MindNode was deleted; clear `branch` on connectors no
    // longer joining two distinct MindNodes on the same page.
    let mut to_remove = Vec::new();
    let mut to_unbranch = Vec::new();
    for n in doc.nodes() {
        let Some(BoardItem::Connector(c)) = &n.board else { continue };
        if !c.branch {
            continue;
        }
        let had_mind_end_deleted = before.node(n.id).and_then(|bn| match &bn.board {
            Some(BoardItem::Connector(bc)) => Some(
                [bc.from.node(), bc.to.node()]
                    .into_iter()
                    .flatten()
                    .any(|end_id| !doc.contains(end_id) && before.node(end_id).is_some_and(|m| m.board == Some(BoardItem::MindNode))),
            ),
            _ => None,
        });
        let missing_end = [c.from.node(), c.to.node()].into_iter().flatten().any(|end_id| !doc.contains(end_id));
        if had_mind_end_deleted == Some(true) || missing_end {
            to_remove.push(n.id);
            continue;
        }
        let page = doc.page_of(n.id);
        let valid = match (&c.from, &c.to) {
            (End::Node { node: a, .. }, End::Node { node: b, .. }) => {
                *a != *b
                    && doc.node(*a).is_some_and(|m| m.board == Some(BoardItem::MindNode) && doc.page_of(*a) == page)
                    && doc.node(*b).is_some_and(|m| m.board == Some(BoardItem::MindNode) && doc.page_of(*b) == page)
            }
            _ => false,
        };
        if !valid {
            to_unbranch.push(n.id);
        }
    }

    let mut extra_dirty: BTreeSet<NodeId> = BTreeSet::new();
    for cid in to_remove {
        if let Some(BoardItem::Connector(c)) = doc.node(cid).and_then(|n| n.board.clone()) {
            for e in [c.from.node(), c.to.node()].into_iter().flatten() {
                if doc.contains(e) {
                    extra_dirty.insert(e);
                }
            }
        }
        let _ = doc.remove(cid);
    }
    for cid in to_unbranch {
        if let Some(bn) = before.node(cid)
            && let Some(BoardItem::Connector(bc)) = &bn.board
        {
            for e in [bc.from.node(), bc.to.node()].into_iter().flatten() {
                if doc.contains(e) {
                    extra_dirty.insert(e);
                }
            }
        }
        let mut cur_ends = [None, None];
        if let Some(n) = doc.node_mut(cid)
            && let Some(BoardItem::Connector(mut c)) = n.board.clone()
        {
            c.branch = false;
            cur_ends = [c.from.node(), c.to.node()];
            n.board = Some(BoardItem::Connector(c));
        }
        for e in cur_ends.into_iter().flatten() {
            if doc.contains(e) {
                extra_dirty.insert(e);
            }
        }
    }

    // Also mark parents/siblings when a mind node or branch present in `before` was removed or reattached.
    for &cid in touched {
        if let Some(BoardItem::Connector(bc)) = before.node(cid).and_then(|bn| bn.board.as_ref())
            && bc.branch
        {
            for e in [bc.from.node(), bc.to.node()].into_iter().flatten() {
                if doc.contains(e) {
                    extra_dirty.insert(e);
                }
            }
        }
        if let Some(BoardItem::Connector(c)) = doc.node(cid).and_then(|n| n.board.as_ref())
            && c.branch
        {
            for e in [c.from.node(), c.to.node()].into_iter().flatten() {
                if doc.contains(e) {
                    extra_dirty.insert(e);
                }
            }
        }
    }
    for bn in before.nodes() {
        if let Some(BoardItem::Connector(bc)) = &bn.board
            && bc.branch
            && !doc.contains(bn.id)
        {
            for e in [bc.from.node(), bc.to.node()].into_iter().flatten() {
                if doc.contains(e) {
                    extra_dirty.insert(e);
                }
            }
        }
    }

    if touched.is_empty() && extra_dirty.is_empty() {
        return;
    }

    let is_hit = |doc: &Document, id: NodeId| {
        touched.contains(&id)
            || extra_dirty.contains(&id)
            || doc.ancestors(id).iter().any(|a| touched.contains(a))
            || doc.node(id).is_some_and(|n| n.children.iter().any(|k| touched.contains(k)))
    };

    for &page in &doc.pages.clone() {
        let incoming = incoming_branches(doc, page);
        let mut children_of: BTreeMap<NodeId, Vec<(NodeId, bool, NodeId)>> = BTreeMap::new();
        for (&child, &(par, left, cid)) in &incoming {
            children_of.entry(par).or_default().push((child, left, cid));
        }

        let roots: Vec<NodeId> = doc
            .nodes()
            .filter(|n| n.board == Some(BoardItem::MindNode) && doc.page_of(n.id) == Some(page) && !incoming.contains_key(&n.id))
            .map(|n| n.id)
            .collect();

        for root in roots {
            // Collect BFS tree (acyclic and bounded).
            let mut tree = Vec::new();
            let mut queue = std::collections::VecDeque::from([(root, None, false, 0usize)]);
            let mut visited = BTreeSet::from([root]);
            let mut tree_dirty = is_hit(doc, root);

            while let Some((nid, par, left, depth)) = queue.pop_front() {
                if tree.len() >= figcraft_doc::MIND_MAX_NODES || depth > figcraft_doc::MIND_MAX_DEPTH {
                    break;
                }
                let Some(rect) = doc.world_bounds(nid) else { continue };
                let my_idx = tree.len();
                tree.push(MindBox { id: nid, rect, parent: par, left });
                if let Some(kids) = children_of.get(&nid) {
                    for &(kid, kleft, cid) in kids {
                        if !visited.insert(kid) {
                            continue;
                        }
                        if is_hit(doc, kid) || touched.contains(&cid) {
                            tree_dirty = true;
                        }
                        queue.push_back((kid, Some(my_idx), kleft, depth + 1));
                    }
                }
            }

            if !tree_dirty || tree.len() <= 1 {
                continue;
            }

            // Detect whether a child was dragged across its parent's horizontal centre to flip sides.
            let mut side_updates: Vec<(NodeId, bool)> = Vec::new();
            for item in &tree {
                let Some(par_idx) = item.parent else { continue };
                let Some(par_box) = tree.get(par_idx) else { continue };
                if !touched.contains(&item.id) {
                    continue;
                }
                let moved = before
                    .node(item.id)
                    .zip(doc.node(item.id))
                    .is_some_and(|(b, n)| b.transform != n.transform);
                if moved
                    && let Some(&(_, old_left, cid)) = incoming.get(&item.id)
                {
                    let now_left = item.rect.center().x < par_box.rect.center().x;
                    if now_left != old_left {
                        side_updates.push((cid, now_left));
                    }
                }
            }
            if !side_updates.is_empty() {
                for (cid, now_left) in side_updates {
                    if let Some(n) = doc.node_mut(cid)
                        && let Some(BoardItem::Connector(mut c)) = n.board.clone()
                    {
                        let (fs, ts) = if now_left { (Side::Left, Side::Right) } else { (Side::Right, Side::Left) };
                        if let End::Node { side, .. } = &mut c.from {
                            *side = fs;
                        }
                        if let End::Node { side, .. } = &mut c.to {
                            *side = ts;
                        }
                        n.board = Some(BoardItem::Connector(c));
                    }
                }
                // Re-read left/right with updated branch sides.
                let updated_incoming = incoming_branches(doc, page);
                for item in &mut tree {
                    if let Some(&(_, l, _)) = updated_incoming.get(&item.id) {
                        item.left = l;
                    }
                }
            }

            for (nid, target_origin) in mind_layout(&tree) {
                let Some(cur_bounds) = doc.world_bounds(nid) else { continue };
                if (cur_bounds.x0 - target_origin.x).abs() <= 1e-6 && (cur_bounds.y0 - target_origin.y).abs() <= 1e-6 {
                    continue;
                }
                let parent_tf = doc
                    .node(nid)
                    .and_then(|n| n.parent)
                    .map_or(Affine::IDENTITY, |p| doc.world_transform(p));
                let local = parent_tf.inverse() * target_origin;
                if let Some(n) = doc.node_mut(nid) {
                    n.transform = Affine::translate((local.x, local.y));
                }
            }
        }
    }
}
