//! Reshaping board connectors by dragging their middle runs or curves (`board.reshape`).

use figcraft_doc::{BoardItem, Document, End, NodeId, Routing, attached_sides, path_handles, reshaped};
use figcraft_geom::Point;
use serde_json::{Value, json};

use super::derived::{place_route, route, route_ends};
use super::{END_HANDLE_PX, Gesture, PointerEvent, Result, Session, bad, get, id, require};

const MIN_RUN_PX: f64 = 32.0;
const MAX_COORD: f64 = 1e9;

/// Path handles `(portion, page_point)` of the single selected connector, when it can be reshaped
/// at `zoom`.
pub fn selected_connector_handles(doc: &Document, sel: &[NodeId], zoom: f64) -> Vec<(usize, Point)> {
    let [cid] = sel else { return Vec::new() };
    if crate::tools::locked_in(doc, *cid) {
        return Vec::new();
    }
    let Some(n) = doc.node(*cid) else { return Vec::new() };
    if !n.visible {
        return Vec::new();
    }
    let Some(BoardItem::Connector(c)) = &n.board else { return Vec::new() };
    if c.routing == Routing::Straight {
        return Vec::new();
    }
    let Some(pts) = route(doc, c) else { return Vec::new() };
    let z = zoom.max(1e-6);
    let end_first = pts.first().copied();
    let end_last = pts.last().copied();

    path_handles(&pts, c.routing)
        .into_iter()
        .filter(|&(portion, hp)| {
            if c.routing == Routing::Elbow
                && let (Some(&a), Some(&b)) = (pts.get(portion), pts.get(portion + 1))
                && (b - a).hypot() * z < MIN_RUN_PX
            {
                return false;
            }
            let near_start = end_first.is_some_and(|s| (hp - s).hypot() * z <= END_HANDLE_PX);
            let near_end = end_last.is_some_and(|e| (hp - e).hypot() * z <= END_HANDLE_PX);
            !(near_start || near_end)
        })
        .collect()
}

/// Hit-test the selected connector's path handles at `p`.
pub(crate) fn path_handle_at(doc: &Document, sel: &[NodeId], p: Point, zoom: f64) -> Option<(NodeId, usize)> {
    let [cid] = sel else { return None };
    let tol = END_HANDLE_PX / zoom.max(1e-6);
    selected_connector_handles(doc, sel, zoom)
        .into_iter()
        .filter(|&(_, hp)| (hp - p).hypot() <= tol)
        .min_by(|a, b| (a.1 - p).hypot().total_cmp(&(b.1 - p).hypot()))
        .map(|(portion, _)| (*cid, portion))
}

fn apply_reshape(doc: &mut Document, cmd: &str, cid: NodeId, portion: usize, target: Point) -> Result<()> {
    let n = doc.get(cid)?;
    let Some(BoardItem::Connector(mut c)) = n.board.clone() else {
        return Err(bad(cmd, "layer is not a connector"));
    };
    if c.routing == Routing::Straight {
        return Err(bad(cmd, "straight connectors have no middle runs to reshape"));
    }
    let (from_re, to_re) = route_ends(doc, &c).ok_or_else(|| bad(cmd, "connector ends can't be placed"))?;
    let (fs, ts) = attached_sides(from_re, to_re);
    if let End::Node { side, .. } = &mut c.from
        && let Some(fs) = fs
    {
        *side = fs;
    }
    if let End::Node { side, .. } = &mut c.to
        && let Some(ts) = ts
    {
        *side = ts;
    }
    let (from_re, to_re) = route_ends(doc, &c).ok_or_else(|| bad(cmd, "connector ends can't be placed"))?;
    let new_path = reshaped(from_re, to_re, c.routing, c.path.as_ref(), portion, target)
        .ok_or_else(|| bad(cmd, format!("no portion {portion} on this connector")))?;
    c.path = Some(new_path);
    if let Some(pts) = route(doc, &c) {
        place_route(doc, cid, &pts, c.routing);
    }
    if let Some(m) = doc.node_mut(cid) {
        m.board = Some(BoardItem::Connector(c));
    }
    Ok(())
}

pub(super) fn reshape(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "board.reshape";
    require(s, cmd)?;
    let cid = match id(p, "id") {
        Some(v) => v,
        None => s.selection().first().copied().ok_or_else(|| bad(cmd, "select a connector"))?,
    };
    if get(p, "id").is_some_and(|v| !v.is_null()) && id(p, "id").is_none() {
        return Err(bad(cmd, "`id` must be a layer id"));
    }
    let portion = match get(p, "portion") {
        Some(v) => {
            if let Some(u) = v.as_u64() {
                usize::try_from(u).map_err(|_| bad(cmd, "`portion` out of range"))?
            } else if let Some(f) = v.as_f64().filter(|f| f.is_finite() && *f >= 0.0 && f.fract() == 0.0 && *f <= usize::MAX as f64) {
                f as usize
            } else {
                return Err(bad(cmd, "`portion` must be a non-negative integer"));
            }
        }
        None => return Err(bad(cmd, "`portion` is required")),
    };
    let x = get(p, "x")
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite() && (-MAX_COORD..=MAX_COORD).contains(v))
        .ok_or_else(|| bad(cmd, "`x` must be a finite coordinate"))?;
    let y = get(p, "y")
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite() && (-MAX_COORD..=MAX_COORD).contains(v))
        .ok_or_else(|| bad(cmd, "`y` must be a finite coordinate"))?;

    let doc = s.doc_mut()?;
    apply_reshape(doc, cmd, cid, portion, Point::new(x, y))?;
    Ok(json!({"id": cid}))
}

impl Session {
    pub(crate) fn reshape_down(&mut self, id: NodeId, portion: usize, start: Point) -> Result<()> {
        self.tool.gesture = Gesture::Reshape { id, portion, start };
        Ok(())
    }

    pub(crate) fn reshape_drag(&mut self, ev: &PointerEvent, id: NodeId, portion: usize, start: Point) {
        self.tool.gesture = Gesture::Reshape { id, portion, start };
        let moved = (ev.p() - start).hypot() * ev.zoom.max(1e-6) >= 4.0;
        let has_interaction = self.ds().is_ok_and(|d| d.interaction.is_some());
        if !moved && !has_interaction {
            return;
        }
        if !has_interaction && self.begin_interaction("Reshape connector").is_err() {
            return;
        }
        let Some(base_doc) = self.ds().ok().and_then(|d| d.interaction.as_ref().map(|it| (*it.doc).clone())) else {
            return;
        };
        let target = ev.p();
        let _ = self.preview(|doc, _| {
            *doc = base_doc;
            apply_reshape(doc, "board.reshape", id, portion, target)
        });
    }

    pub(crate) fn reshape_up(&mut self, ev: &PointerEvent, id: NodeId, portion: usize, start: Point) -> Result<()> {
        let had_interaction = self.ds().is_ok_and(|d| d.interaction.is_some());
        if had_interaction {
            self.cancel_interaction()?;
        }
        if !had_interaction && (ev.p() - start).hypot() * ev.zoom.max(1e-6) < 4.0 {
            return Ok(());
        }
        self.execute("board.reshape", &json!({"id": id, "portion": portion, "x": ev.x, "y": ev.y}))?;
        Ok(())
    }
}
