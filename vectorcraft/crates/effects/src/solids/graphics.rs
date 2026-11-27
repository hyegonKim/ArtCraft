//! Graphics mapped onto Extrude & Bevel caps, Rotate planes, and Revolve swept sides.

use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_doc::color::{Color, Gradient, GradientGeom, GradientPaint, Paint};
use vectorcraft_doc::{Appearance, AppearanceItem, Document, FillLayer, Node, NodeId, NodeKind, OpacityMask};
use vectorcraft_geom::kurbo::PathEl;
use vectorcraft_geom::{Affine, BezPath, FillRule, PathData, Point, Rect};
use vectorcraft_pathops::{BoolOp, boolean};
use vectorcraft_three_d::{Face, FaceMap, Facet, PlanarFace, SurfaceCell};

use crate::GeomContext;

/// Parameter key storing an effect's mapped graphics array.
pub const GRAPHICS: &str = "graphics";
/// Maximum mapped graphics allowed on one 3D effect.
pub const MAX_GRAPHICS: usize = 64;
/// Maximum `x` / `y` offset (in document points) of a mapped graphic.
pub const MAX_OFFSET: f64 = 100_000.0;
/// Allowed `(min, max)` percentage range for `scale`.
pub const SCALE_RANGE: (f64, f64) = (1.0, 10_000.0);
/// Refusal message when mapping graphics onto an unsupported 3D effect (Inflate or Classic 3D).
pub const NOT_TAKEN: &str = "Graphics map onto the Plane, Extrude and Revolve object types";

/// Maximum Bezier path elements drawn across all planar-cap graphics of one 3D effect.
const MAX_GRAPHIC_ELEMENTS: usize = 65_536;
/// Maximum flattened points allowed across scaled symbols mapped onto a Revolve surface.
const MAX_SURFACE_POINTS: usize = 65_536;
/// Maximum clipped pieces emitted across a Revolve surface's graphics.
const MAX_SURFACE_PARTS: usize = 32_768;
/// Maximum contour-vs-cell boolean clip calls allowed before refusing a pathological surface graphic.
const MAX_SURFACE_CLIPS: usize = 48_000;
/// Maximum total contour anchors tested in boolean clips across all faces of a Revolve surface.
const MAX_SURFACE_CLIP_WORK: usize = 4_000_000;

/// One symbol mapped onto a planar face or revolved side of a 3D effect.
#[derive(Clone, Debug, PartialEq)]
pub struct MappedGraphic {
    pub symbol: String,
    pub face: PlanarFace,
    pub x: f64,
    pub y: f64,
    pub scale: f64,
    pub rotation: f64,
}

impl MappedGraphic {
    /// Serializes this mapped graphic back into an effect parameter JSON object.
    pub fn to_value(&self) -> Value {
        json!({
            "symbol": self.symbol,
            "face": self.face.id(),
            "x": self.x,
            "y": self.y,
            "scale": self.scale,
            "rotation": self.rotation,
        })
    }

    fn local_transform(&self) -> Affine {
        Affine::translate((self.x, self.y)) * Affine::rotate((-self.rotation).to_radians()) * Affine::scale(self.scale / 100.0)
    }
}

/// Frame of a 3D effect's mapped graphics: the 2D bounds its solids are built with and the
/// projected [`FaceMap`]s of its front and back planar faces (when it has planar faces).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GraphicFrame {
    pub bounds: Rect,
    front: Option<FaceMap>,
    back: Option<FaceMap>,
}

impl GraphicFrame {
    pub fn new(bounds: Rect, front: Option<FaceMap>, back: Option<FaceMap>) -> Self {
        Self { bounds, front, back }
    }

    fn map_of(&self, face: PlanarFace) -> Option<&FaceMap> {
        match face {
            PlanarFace::Front => self.front.as_ref(),
            PlanarFace::Back => self.back.as_ref(),
        }
    }

    /// Projected page point of `g`'s handle and whether its face is turned toward the viewer.
    pub fn handle(&self, g: &MappedGraphic) -> Option<(Point, bool)> {
        let map = self.map_of(g.face)?;
        Some((map.project_offset(g.x, g.y)?, map.seen))
    }

    /// Face-local `(x, y)` offset corresponding to `page` on `face`.
    pub fn offset_at(&self, face: PlanarFace, page: Point) -> Option<(f64, f64)> {
        self.map_of(face)?.offset_at(page)
    }
}

/// Whether 3D effect `id` supports mapped graphics (`Extrude & Bevel`, `Rotate`, or `Revolve`).
pub fn takes_graphics(id: &str) -> bool {
    id == super::EXTRUDE || id == super::ROTATE || id == crate::REVOLVE
}

/// Parses the valid [`MappedGraphic`] entries from `p["graphics"]`.
pub fn mapped_graphics(p: &Value) -> Vec<MappedGraphic> {
    let Some(list) = p.get(GRAPHICS).and_then(Value::as_array) else {
        return Vec::new();
    };
    list.iter().take(MAX_GRAPHICS).filter_map(parse_entry_lenient).collect()
}

fn parse_entry_lenient(v: &Value) -> Option<MappedGraphic> {
    let obj = v.as_object()?;
    let symbol = obj.get("symbol")?.as_str()?.trim();
    if symbol.is_empty() {
        return None;
    }
    let face = obj.get("face").and_then(Value::as_str).and_then(PlanarFace::parse).unwrap_or(PlanarFace::Front);
    let num_or = |k: &str, d: f64| obj.get(k).and_then(Value::as_f64).filter(|x| x.is_finite()).unwrap_or(d);
    Some(MappedGraphic {
        symbol: symbol.to_string(),
        face,
        x: num_or("x", 0.0).clamp(-MAX_OFFSET, MAX_OFFSET),
        y: num_or("y", 0.0).clamp(-MAX_OFFSET, MAX_OFFSET),
        scale: num_or("scale", 100.0).clamp(SCALE_RANGE.0, SCALE_RANGE.1),
        rotation: num_or("rotation", 0.0).clamp(-360.0, 360.0),
    })
}

/// Returns `params` with `graphic` appended to its `graphics` array.
pub fn with_graphic(params: &Value, graphic: MappedGraphic) -> Value {
    let mut map = params.as_object().cloned().unwrap_or_default();
    let mut list = map.get(GRAPHICS).and_then(Value::as_array).cloned().unwrap_or_default();
    list.push(graphic.to_value());
    map.insert(GRAPHICS.into(), Value::Array(list));
    Value::Object(map)
}

/// Distinct symbol names referenced in `p["graphics"]`, in order of appearance.
fn param_graphic_symbols(p: &Value) -> Vec<String> {
    let mut names = Vec::new();
    for g in mapped_graphics(p) {
        if !names.contains(&g.symbol) {
            names.push(g.symbol);
        }
    }
    names
}

/// Symbol names referenced by any 3D effect on `node` (object-level or item-level).
pub fn graphic_symbols(node: &Node) -> Vec<String> {
    appearance_graphic_symbols(&node.appearance)
}

/// Symbol names referenced by any 3D effect in `app` (object-level or item-level).
pub fn appearance_graphic_symbols(app: &Appearance) -> Vec<String> {
    let mut out = Vec::new();
    let mut collect = |fx: &[vectorcraft_doc::Effect]| {
        for e in fx {
            if super::is_three_d(&e.id) {
                for name in param_graphic_symbols(&e.params) {
                    if !out.contains(&name) {
                        out.push(name);
                    }
                }
            }
        }
    };
    collect(&app.effects);
    for item in &app.items {
        collect(item.effects());
    }
    out
}

/// First symbol name in `params` that does not exist in `doc.symbols` (ignoring any already
/// missing symbol that was already present in `previous`).
pub fn missing_symbol(doc: &Document, params: &Value, previous: Option<&Value>) -> Option<String> {
    let prev_names = previous.map(param_graphic_symbols).unwrap_or_default();
    param_graphic_symbols(params).into_iter().find(|name| !doc.symbols.iter().any(|s| s.name == *name) && !prev_names.contains(name))
}

/// Renames symbol `from` to `to` inside a single 3D effect parameter JSON `params` (`true` if changed).
pub fn rename_graphic_symbol(params: &mut Value, from: &str, to: &str) -> bool {
    let Some(list) = params.get_mut(GRAPHICS).and_then(Value::as_array_mut) else {
        return false;
    };
    let mut changed = false;
    for entry in list {
        if let Some(obj) = entry.as_object_mut()
            && obj.get("symbol").and_then(Value::as_str) == Some(from)
        {
            obj.insert("symbol".into(), json!(to));
            changed = true;
        }
    }
    changed
}

/// Renames symbol `from` to `to` across all 3D effects in `app` (`true` if changed).
pub fn rename_appearance_graphic_symbol(app: &mut Appearance, from: &str, to: &str) -> bool {
    let mut changed = false;
    for e in &mut app.effects {
        if super::is_three_d(&e.id) {
            changed |= rename_graphic_symbol(&mut e.params, from, to);
        }
    }
    if app.items.iter().any(|i| i.effects().iter().any(|e| super::is_three_d(&e.id) && param_graphic_symbols(&e.params).iter().any(|s| s == from))) {
        for item in &mut app.items {
            for e in item.effects_mut() {
                if super::is_three_d(&e.id) {
                    changed |= rename_graphic_symbol(&mut e.params, from, to);
                }
            }
        }
    }
    changed
}

/// Whether `node` (or any descendant or opacity mask inside it) maps graphic symbol `name`.
pub fn names_graphic_symbol(node: &Node, name: &str) -> bool {
    if appearance_graphic_symbols(&node.appearance).iter().any(|s| s == name) {
        return true;
    }
    if let Some(mask) = &node.extras().mask
        && names_graphic_symbol(&mask.art, name)
    {
        return true;
    }
    node.children().is_some_and(|ch| ch.iter().any(|c| names_graphic_symbol(c, name)))
}

/// Renames graphic symbol `from` to `to` throughout `node` and its descendants / opacity masks.
pub fn rename_graphic_symbol_in(node: &mut Node, from: &str, to: &str) -> bool {
    if !names_graphic_symbol(node, from) {
        return false;
    }
    let mut changed = rename_appearance_graphic_symbol(&mut node.appearance, from, to);
    if node.extras().mask.as_ref().is_some_and(|m| names_graphic_symbol(&m.art, from))
        && let Some(mask) = node.extras_mut().mask.as_mut()
    {
        changed |= rename_graphic_symbol_in(Arc::make_mut(&mut mask.art), from, to);
    }
    if let Some(children) = node.children_mut() {
        for child in children {
            if names_graphic_symbol(child, from) {
                changed |= rename_graphic_symbol_in(Arc::make_mut(child), from, to);
            }
        }
    }
    changed
}

/// Validates `p["graphics"]` for 3D effect `id`.
pub(crate) fn validate(id: &str, p: &Value) -> Result<(), String> {
    let Some(raw) = p.get(GRAPHICS) else {
        return Ok(());
    };
    if raw.is_null() {
        return Ok(());
    }
    if !takes_graphics(id) {
        let non_empty = raw.as_array().is_none_or(|a| !a.is_empty());
        if non_empty {
            return Err(NOT_TAKEN.into());
        }
        return Ok(());
    }
    let Some(list) = raw.as_array() else {
        return Err(format!("`{GRAPHICS}` must be an array"));
    };
    if list.len() > MAX_GRAPHICS {
        return Err(format!("`{GRAPHICS}` takes at most {MAX_GRAPHICS} graphics"));
    }
    for entry in list {
        let Some(obj) = entry.as_object() else {
            return Err(format!("each of `{GRAPHICS}` must be an object"));
        };
        for k in obj.keys() {
            if !matches!(k.as_str(), "symbol" | "face" | "x" | "y" | "scale" | "rotation") {
                return Err(format!("`{GRAPHICS}` entries take symbol, face, x, y, scale and rotation, not `{k}`"));
            }
        }
        let Some(sym) = obj.get("symbol").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()) else {
            return Err(format!("each of `{GRAPHICS}` needs `symbol`, the name of a symbol"));
        };
        let _ = sym;
        if let Some(fv) = obj.get("face") {
            let Some(face) = fv.as_str().and_then(PlanarFace::parse) else {
                return Err(format!("`{GRAPHICS}` face must be front or back"));
            };
            if id == crate::REVOLVE && face == PlanarFace::Back {
                return Err("Revolve maps graphics onto its side: map them onto the front".into());
            }
            if id == super::ROTATE && face == PlanarFace::Back {
                return Err("A plane has one face: map its graphics onto the front".into());
            }
        }
        for (key, lo, hi) in
            [("x", -MAX_OFFSET, MAX_OFFSET), ("y", -MAX_OFFSET, MAX_OFFSET), ("scale", SCALE_RANGE.0, SCALE_RANGE.1), ("rotation", -360.0, 360.0)]
        {
            if let Some(v) = obj.get(key) {
                let Some(n) = v.as_f64().filter(|n| n.is_finite()) else {
                    return Err(format!("`{GRAPHICS}` {key} must be a number from {lo} to {hi}"));
                };
                if n < lo || n > hi {
                    return Err(format!("`{GRAPHICS}` {key} must be a number from {lo} to {hi}"));
                }
            }
        }
    }
    Ok(())
}

/// Validates that the symbols mapped in `p` stay within the path element limit.
pub(crate) fn validate_symbols(doc: &Document, p: &Value) -> Result<(), String> {
    let list = mapped_graphics(p);
    if list.is_empty() {
        return Ok(());
    }
    let mut total_els = 0usize;
    for g in &list {
        let Some(shapes) = symbol_shapes(doc, &g.symbol, 0) else {
            continue;
        };
        let els: usize = shapes.iter().map(|s| s.path.to_bezpath().elements().len()).sum();
        total_els = total_els.saturating_add(els);
        if total_els > MAX_GRAPHIC_ELEMENTS {
            return Err("The graphics are too complex: simplify the symbols' art".into());
        }
    }
    Ok(())
}

/// Validates that the scaled flattened curves of symbols mapped onto a Revolve surface stay within
/// [`MAX_SURFACE_POINTS`].
pub(crate) fn validate_surface_symbols(doc: &Document, p: &Value) -> Result<(), String> {
    let list = mapped_graphics(p);
    let mut total_pts = 0usize;
    for g in &list {
        let Some(shapes) = symbol_shapes(doc, &g.symbol, 0) else {
            continue;
        };
        let xf = g.local_transform();
        for s in &shapes {
            let mut tp = s.path.clone();
            tp.transform(xf);
            vectorcraft_geom::kurbo::flatten(tp.to_bezpath(), 0.25, |el| {
                if matches!(el, PathEl::MoveTo(_) | PathEl::LineTo(_)) {
                    total_pts = total_pts.saturating_add(1);
                }
            });
            if total_pts > MAX_SURFACE_POINTS {
                return Err("The graphics are too complex for this surface: simplify the symbols' art or reduce Segments".into());
            }
        }
    }
    Ok(())
}

/// Bounds of symbol `name`'s graphic artwork in centered natural coordinates.
pub(crate) fn art_bounds(doc: &Document, name: &str) -> Option<Rect> {
    let shapes = symbol_shapes(doc, name, 0)?;
    shapes.iter().filter_map(|s| s.path.bounds()).reduce(|a, b| a.union(b))
}

#[derive(Clone)]
struct GraphicShape {
    path: PathData,
    rule: FillRule,
    paint: Paint,
    opacity: f32,
    blend: vectorcraft_doc::color::BlendMode,
}

fn natural_symbol_node(doc: &Document, name: &str) -> Option<Node> {
    let sym = doc.symbols.iter().find(|s| s.name == name)?;
    let mut art = (*sym.art).clone();
    if let Some(sizes) = doc.unknown.get(vectorcraft_doc::SYMBOL_SIZES).and_then(Value::as_object)
        && let Some([w, h]) = sizes.get(name).and_then(Value::as_array).and_then(|a| Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?]))
        && w > 0.0
        && h > 0.0
    {
        // Normalized symbols are already centered on `(0, 0)` inside `-10..10` with strokes untouched.
        art.transform(Affine::scale_non_uniform(w / 20.0, h / 20.0), false);
    } else if let Some(b) = art.visual_bounds().or_else(|| art.geometric_bounds()) {
        let c = b.center();
        art.transform(Affine::translate((-c.x, -c.y)), false);
    }
    Some(art)
}

fn symbol_shapes(doc: &Document, name: &str, depth: usize) -> Option<Vec<GraphicShape>> {
    if depth > 6 {
        return None;
    }
    let root = natural_symbol_node(doc, name)?;
    let mut out = Vec::new();
    collect_graphic_shapes(doc, &root, 1.0, depth, &mut out);
    (!out.is_empty()).then_some(out)
}

fn collect_graphic_shapes(doc: &Document, n: &Node, parent_opacity: f32, depth: usize, out: &mut Vec<GraphicShape>) {
    if depth > 6 || !n.visible || n.is_guide() {
        return;
    }
    let opacity = (parent_opacity * n.opacity).clamp(0.0, 1.0);
    if opacity <= 0.0 {
        return;
    }
    if let NodeKind::SymbolInstance { symbol, xf } = &n.kind {
        if let Some(sym) = doc.symbols.iter().find(|s| s.name == *symbol) {
            let art = vectorcraft_doc::symbol_overrides::dynamic_art(&sym.art, n);
            if let Some(placed) = doc.place_symbol_art(symbol, &art, *xf) {
                collect_graphic_shapes(doc, &placed, opacity, depth + 1, out);
            }
        }
        return;
    }
    if let NodeKind::Group { children, clip: true } = &n.kind {
        if let Some(clip_node) = children.first()
            && let Some((mask_bez, mask_rule)) = crate::clip_outline(clip_node)
        {
            let mask = PathData::from_bezpath(&mask_bez);
            let mut inner = Vec::new();
            for c in children.iter().skip(1) {
                collect_graphic_shapes(doc, c, opacity, depth + 1, &mut inner);
            }
            for mut s in inner {
                let clipped = boolean(&s.path, s.rule, &mask, mask_rule, BoolOp::Intersect);
                if clipped.bounds().is_some() {
                    s.path = clipped;
                    s.rule = FillRule::NonZero;
                    out.push(s);
                }
            }
        }
        return;
    }
    if let Some(children) = n.children()
        && !matches!(n.kind, NodeKind::Compound { .. })
    {
        for c in children {
            collect_graphic_shapes(doc, c, opacity, depth + 1, out);
        }
        return;
    }
    let Some((raw_path, raw_rule)) = super::profile(n) else {
        return;
    };
    let ctx = GeomContext { doc: Some(doc), ..GeomContext::of(n) };
    let shaped = crate::apply_geometry_with(&n.appearance.effects, &raw_path, raw_path.bounds().unwrap_or(Rect::ZERO), &ctx);
    for item in super::solid_paints(n) {
        match item {
            AppearanceItem::Fill(fill) if fill.opacity > 0.0 && !matches!(fill.paint, Paint::None) => {
                out.push(GraphicShape {
                    path: shaped.clone(),
                    rule: raw_rule,
                    paint: fill.paint,
                    opacity: opacity * fill.opacity,
                    blend: fill.blend,
                });
            }
            AppearanceItem::Stroke(stroke) if stroke.opacity > 0.0 && stroke.width > 0.0 && !matches!(stroke.paint, Paint::None) => {
                let region = crate::stroke::outline_region(&shaped, raw_rule, &stroke);
                if region.bounds().is_some() {
                    out.push(GraphicShape {
                        path: region,
                        rule: FillRule::EvenOdd,
                        paint: stroke.paint,
                        opacity: opacity * stroke.opacity,
                        blend: stroke.blend,
                    });
                }
            }
            _ => {}
        }
    }
}

fn transform_paint(paint: &Paint, xf: Affine) -> Paint {
    match paint {
        Paint::Gradient(gp) => {
            let mut next = (**gp).clone();
            next.transform(xf);
            Paint::Gradient(Box::new(next))
        }
        other => other.clone(),
    }
}

fn opaque_stops(paint: &Paint) -> bool {
    match paint {
        Paint::Gradient(gp) => gp.gradient.stops.iter().all(|s| s.opacity >= 1.0 - 1e-6),
        _ => true,
    }
}

/// Projects and clips the graphics mapped onto planar cap `face` (`map`), shaded by `map`'s
/// directional illumination and clipped to the union of visible cap facets `caps`.
pub(crate) fn face_art(
    doc: Option<&Document>,
    id: NodeId,
    list: &[MappedGraphic],
    (face, map): (PlanarFace, &FaceMap),
    _center: Point,
    caps: &[&Facet],
    elements: &mut usize,
) -> Option<Node> {
    let doc = doc?;
    if !map.seen || caps.is_empty() {
        return None;
    }
    let on_face: Vec<&MappedGraphic> = list.iter().filter(|g| g.face == face).collect();
    if on_face.is_empty() {
        return None;
    }
    let mut cap_mask = caps.first()?.path();
    for cap in caps.iter().skip(1) {
        let u = boolean(&cap_mask, FillRule::NonZero, &cap.path(), FillRule::NonZero, BoolOp::Union);
        if u.bounds().is_some() {
            cap_mask = u;
        }
    }
    let colored = map.illumination.windows(2).any(|w| (w[0] - w[1]).abs() > 1e-6);
    let mut drawn = Vec::new();
    for g in on_face {
        let Some(shapes) = symbol_shapes(doc, &g.symbol, 0) else {
            continue;
        };
        let local_xf = g.local_transform();
        let page_xf = planar_affine(map).map(|a| a * local_xf);
        for s in shapes {
            let mut local_path = s.path;
            local_path.transform(local_xf);
            let Some(projected) = map.project_path(&local_path) else {
                continue;
            };
            let count = projected.to_bezpath().elements().len();
            if elements.saturating_add(count) > MAX_GRAPHIC_ELEMENTS {
                return None;
            }
            let clipped = boolean(&projected, s.rule, &cap_mask, FillRule::NonZero, BoolOp::Intersect);
            if clipped.bounds().is_none() {
                continue;
            }
            *elements = elements.saturating_add(clipped.to_bezpath().elements().len());
            if *elements > MAX_GRAPHIC_ELEMENTS {
                return None;
            }
            let placed_paint = page_xf.map_or_else(|| s.paint.clone(), |xf| transform_paint(&s.paint, xf));
            let items = crate::lighting::fills(&placed_paint, map.illumination, map.brightness, colored);
            let mut node = crate::group::shape_node(id, clipped, FillRule::NonZero, Appearance { items, ..Default::default() });
            node.opacity = s.opacity;
            node.blend = s.blend;
            drawn.push(Arc::new(node));
        }
    }
    (!drawn.is_empty()).then(|| Node::group(id, drawn))
}

fn planar_affine(map: &FaceMap) -> Option<Affine> {
    let o = map.project_offset(0.0, 0.0)?;
    let px = map.project_offset(1.0, 0.0)?;
    let py = map.project_offset(0.0, 1.0)?;
    Some(Affine::new([px.x - o.x, px.y - o.y, py.x - o.x, py.y - o.y, o.x, o.y]))
}

/// Maps `list` onto Revolve's `faces`, returning per-face graphic child nodes in face order.
/// Returns `None` when the graphics exceed complexity or paint-copy budgets.
pub(crate) fn surface_parts(
    doc: Option<&Document>,
    id: NodeId,
    list: &[MappedGraphic],
    faces: &[Face],
    shade: &dyn Fn(&Face, Color) -> Paint,
    copies: &mut usize,
) -> Option<Vec<Vec<Arc<Node>>>> {
    let Some(doc) = doc else {
        return Some(Vec::new());
    };
    let on_side: Vec<&MappedGraphic> = list.iter().filter(|g| g.face == PlanarFace::Front).collect();
    if on_side.is_empty() {
        return Some(Vec::new());
    }
    let cells: Vec<Option<(Rect, PathData, &SurfaceCell)>> = faces
        .iter()
        .map(|f| {
            let cell = f.surface.as_ref().filter(|c| c.outward && c.uv.len() >= 3 && c.uv.len() == c.page.len())?;
            let uv_pts: Vec<Point> = if f.outline.is_some() && f.points.len() >= 3 {
                let mapped: Option<Vec<Point>> = f.points.iter().map(|&p| map_page_point_to_uv(p, cell)).collect();
                mapped.unwrap_or_else(|| cell.uv.clone())
            } else {
                cell.uv.clone()
            };
            let bounds = uv_bounds(&uv_pts)?;
            let poly = polygon_path(&uv_pts);
            Some((bounds, poly, cell))
        })
        .collect();
    if !cells.iter().any(Option::is_some) {
        return Some(Vec::new());
    }

    let mut out: Vec<Vec<Arc<Node>>> = vec![Vec::new(); faces.len()];
    let mut total_parts = 0usize;
    let mut clip_calls = 0usize;
    let mut clip_work = 0usize;

    for g in on_side {
        let Some(shapes) = symbol_shapes(doc, &g.symbol, 0) else {
            continue;
        };
        let xf = g.local_transform();
        for s in shapes {
            let mut uv_path = s.path;
            uv_path.transform(xf);
            let uv_paint = transform_paint(&s.paint, xf);
            let Some(shape_bounds) = uv_path.bounds() else {
                continue;
            };
            // Pre-group subpaths or test individual contours whose bounding boxes overlap a cell,
            // so contours far from a face are never boolean-clipped against it.
            let contours: Vec<(Rect, usize, PathData)> = uv_path
                .subpaths
                .iter()
                .filter_map(|sp| {
                    let anchors = sp.anchors.len().max(1);
                    let single = PathData::new(vec![sp.clone()]);
                    Some((single.bounds()?, anchors, single))
                })
                .collect();
            if contours.is_empty() {
                continue;
            }
            let multi_contour = contours.len() > 1;
            let has_seam = s.opacity >= 1.0 - 1e-6 && opaque_stops(&uv_paint);

            for (face_idx, (face, cell_entry)) in faces.iter().zip(&cells).enumerate() {
                let Some((cell_rect, cell_poly, cell)) = cell_entry else {
                    continue;
                };
                if !rects_touch(shape_bounds, *cell_rect) {
                    continue;
                }
                let overlapping: Vec<(usize, &PathData)> =
                    contours.iter().filter(|(cb, _, _)| rects_touch(*cb, *cell_rect)).map(|(_, n_pts, p)| (*n_pts, p)).collect();
                if overlapping.is_empty() {
                    continue;
                }
                let work: usize = overlapping.iter().map(|(n_pts, _)| *n_pts).sum();
                clip_calls = clip_calls.saturating_add(overlapping.len());
                clip_work = clip_work.saturating_add(work);
                if clip_calls > MAX_SURFACE_CLIPS || clip_work > MAX_SURFACE_CLIP_WORK {
                    return None;
                }
                let candidate = if multi_contour && overlapping.len() < contours.len() {
                    let subs = overlapping.into_iter().flat_map(|(_, p)| p.subpaths.iter().cloned()).collect();
                    PathData::new(subs)
                } else if multi_contour {
                    uv_path.clone()
                } else {
                    overlapping[0].1.clone()
                };
                let clipped_uv = boolean(&candidate, s.rule, cell_poly, FillRule::NonZero, BoolOp::Intersect);
                if clipped_uv.subpaths.is_empty() {
                    continue;
                }
                let Some(mut page_path) = map_uv_path_to_cell(&clipped_uv, cell) else {
                    continue;
                };
                if face.outline.is_some() {
                    page_path = boolean(&page_path, FillRule::NonZero, &face.path(), FillRule::NonZero, BoolOp::Intersect);
                }
                if page_path.bounds().is_none() {
                    continue;
                }
                total_parts = total_parts.saturating_add(1);
                if total_parts > MAX_SURFACE_PARTS {
                    return None;
                }
                let copy_mul = if has_seam { 2 } else { 1 };
                if !crate::revolve::paint_copies_add(copies, copy_mul, &uv_paint) {
                    return None;
                }
                let style = PartStyle { paint: &uv_paint, opacity: s.opacity, blend: s.blend, has_seam };
                let node = build_surface_part_node(id, face, cell, page_path, style, shade);
                out[face_idx].push(Arc::new(node));
            }
        }
    }
    Some(out)
}

struct PartStyle<'a> {
    paint: &'a Paint,
    opacity: f32,
    blend: vectorcraft_doc::color::BlendMode,
    has_seam: bool,
}

fn rgb_clipped_part(id: NodeId, clip_path: &PathData, paint: &Paint, light: vectorcraft_three_d::RgbGradient) -> Node {
    use vectorcraft_doc::color::BlendMode;
    use vectorcraft_geom::shapes;

    let (base, mask) = crate::lighting::opaque_paint(paint);
    let clip = Node::path(id, clip_path.clone(), Appearance::default());
    let bounds = clip_path.bounds().unwrap_or(Rect::ZERO).inflate(1.0, 1.0);
    let cover = |p: Paint| Node::path(id, shapes::rectangle(bounds), Appearance::basic(p, Paint::None, 0.0));
    let channels: Vec<Arc<Node>> = light
        .channels
        .iter()
        .enumerate()
        .map(|(channel, gradient)| {
            let mut color = [0.0; 3];
            if let Some(v) = color.get_mut(channel) {
                *v = 1.0;
            }
            let ramp = vectorcraft_three_d::LightGradient { diffuse: *gradient, ambient: 0.0, color };
            let mut layer = cover(crate::lighting::smooth_paint(ramp, Color::WHITE));
            layer.blend = BlendMode::Screen;
            Arc::new(layer)
        })
        .collect();
    let mut illumination = Node::group(id, channels);
    illumination.isolate = true;
    illumination.blend = BlendMode::Multiply;
    let mut colors = Node::group(id, vec![Arc::new(cover(base)), Arc::new(illumination)]);
    colors.isolate = true;
    let mut part = Node::new(id, NodeKind::Group { children: vec![Arc::new(clip), Arc::new(colors)], clip: true });
    if let Some(mask_paint) = mask {
        part.extras_mut().mask = Some(Box::new(OpacityMask::new(cover(mask_paint), true)));
    }
    part
}

fn build_surface_part_node(
    id: NodeId,
    face: &Face,
    cell: &SurfaceCell,
    page_path: PathData,
    style: PartStyle<'_>,
    shade: &dyn Fn(&Face, Color) -> Paint,
) -> Node {
    let uv_paint = style.paint;
    let cell_xf = cell_uv_to_page_affine(cell);
    let placed_paint = cell_xf.map_or_else(|| uv_paint.clone(), |xf| transform_paint(uv_paint, xf));
    if let Some(rgb_grad) = face.rgb_gradient
        && (uv_paint.color().is_some() || matches!(uv_paint, Paint::Gradient(_)))
    {
        if !style.has_seam {
            let mut node = rgb_clipped_part(id, &page_path, &placed_paint, rgb_grad);
            node.opacity = style.opacity;
            node.blend = style.blend;
            return node;
        }
        let seam_bez = vectorcraft_geom::kurbo::stroke(
            page_path.to_bezpath().iter(),
            &vectorcraft_geom::kurbo::Stroke::new(0.35).with_join(vectorcraft_geom::kurbo::Join::Round),
            &vectorcraft_geom::kurbo::StrokeOpts::default(),
            0.05,
        );
        let seam_path = PathData::from_bezpath(&seam_bez);
        let seam_art = rgb_clipped_part(id, &seam_path, &placed_paint, rgb_grad);
        let fill_art = rgb_clipped_part(id, &page_path, &placed_paint, rgb_grad);
        let mut group = Node::group(id, vec![Arc::new(seam_art), Arc::new(fill_art)]);
        group.opacity = style.opacity;
        group.blend = style.blend;
        return group;
    }
    let colored = face.light_gradient.is_some() || face.illumination.windows(2).any(|w| (w[0] - w[1]).abs() > 1e-6);
    if let Some(c) = uv_paint.color() {
        let lit_paint = shade(face, c);
        let fill = FillLayer::new(lit_paint.clone());
        let mut items = vec![AppearanceItem::Fill(fill)];
        if style.has_seam {
            items.push(AppearanceItem::Stroke(crate::lighting::seam_stroke(lit_paint, 0.35)));
        }
        let mut node = Node::path(id, page_path, Appearance { items, ..Default::default() });
        node.opacity = style.opacity;
        node.blend = style.blend;
        return node;
    }
    let (opaque_base, mask) = crate::lighting::opaque_paint(&placed_paint);
    let fills = if colored {
        crate::lighting::fills(&opaque_base, face.illumination, face.brightness, true)
    } else {
        let base = FillLayer::new(opaque_base);
        let mut dark = FillLayer::new(Paint::solid(Color::BLACK));
        dark.opacity = (1.0 - face.brightness).clamp(0.0, 1.0) as f32;
        if let Some(g) = face.gradient {
            dark.opacity = 1.0;
            dark.paint = gradient_shadow_paint(g);
        }
        vec![AppearanceItem::Fill(base), AppearanceItem::Fill(dark)]
    };
    let items = if style.has_seam {
        fills
            .into_iter()
            .flat_map(|item| {
                let AppearanceItem::Fill(fill) = &item else { return vec![item] };
                let mut edge = crate::lighting::seam_stroke(fill.paint.clone(), 0.35);
                edge.opacity = fill.opacity;
                vec![item, AppearanceItem::Stroke(edge)]
            })
            .collect()
    } else {
        fills
    };
    let mut node = Node::path(id, page_path, Appearance { items, ..Default::default() });
    node.opacity = style.opacity;
    node.blend = style.blend;
    if let Some(mask_paint) = mask {
        let bounds = node.visual_bounds().unwrap_or(Rect::ZERO).inflate(1.0, 1.0);
        let cov = Node::path(id, vectorcraft_geom::shapes::rectangle(bounds), Appearance::basic(mask_paint, Paint::None, 0.0));
        node.extras_mut().mask = Some(Box::new(OpacityMask::new(cov, true)));
    }
    node
}

fn gradient_shadow_paint(g: vectorcraft_three_d::ShadeGradient) -> Paint {
    use vectorcraft_doc::color::{GradientKind, GradientStop};
    let stops = g
        .brightness
        .into_iter()
        .enumerate()
        .map(|(i, b)| GradientStop { opacity: (1.0 - b).clamp(0.0, 1.0) as f32, ..GradientStop::new(i as f32, Color::BLACK) })
        .collect();
    let mut gp = GradientPaint::new(Gradient::new(GradientKind::Linear, stops));
    gp.geom = Some(GradientGeom { start: g.start, end: g.end, aspect: 1.0, focal: None });
    Paint::Gradient(Box::new(gp))
}

fn uv_bounds(pts: &[Point]) -> Option<Rect> {
    let mut it = pts.iter();
    let first = *it.next()?;
    Some(it.fold(Rect::from_points(first, first), |r, &p| r.union_pt(p)))
}

fn rects_touch(a: Rect, b: Rect) -> bool {
    a.x0 <= b.x1 + 1e-6 && b.x0 <= a.x1 + 1e-6 && a.y0 <= b.y1 + 1e-6 && b.y0 <= a.y1 + 1e-6
}

fn polygon_path(pts: &[Point]) -> PathData {
    let mut bp = BezPath::new();
    if let Some((&first, rest)) = pts.split_first() {
        bp.move_to(first);
        for &p in rest {
            bp.line_to(p);
        }
        bp.close_path();
    }
    PathData::from_bezpath(&bp)
}

fn map_uv_point(q: Point, cell: &SurfaceCell) -> Option<Point> {
    let uv = &cell.uv;
    let pg = &cell.page;
    if uv.len() == 4 && pg.len() == 4 {
        let (va, vb) = (uv[0].y, uv[1].y);
        let dy = vb - va;
        if dy.is_finite() && dy.abs() > 1e-12 {
            let t = (q.y - va) / dy;
            let u_lo = uv[0].x + (uv[1].x - uv[0].x) * t;
            let u_hi = uv[3].x + (uv[2].x - uv[3].x) * t;
            let du = u_hi - u_lo;
            if du.is_finite() && du.abs() > 1e-12 {
                let s = (q.x - u_lo) / du;
                let out = pg[0].lerp(pg[1], t).lerp(pg[3].lerp(pg[2], t), s);
                if out.x.is_finite() && out.y.is_finite() {
                    return Some(out);
                }
            }
        }
        let t1 = barycentric(q, [uv[0], uv[1], uv[2]], [pg[0], pg[1], pg[2]]);
        let t2 = barycentric(q, [uv[0], uv[2], uv[3]], [pg[0], pg[2], pg[3]]);
        return match (t1, t2) {
            (Some((p1, e1)), Some((p2, e2))) => Some(if e1 <= e2 { p1 } else { p2 }),
            (Some((p1, _)), None) => Some(p1),
            (None, Some((p2, _))) => Some(p2),
            (None, None) => None,
        };
    }
    if uv.len() >= 3 {
        return barycentric(q, [uv[0], uv[1], uv[2]], [pg[0], pg[1], pg[2]]).map(|(p, _)| p);
    }
    None
}

fn map_page_point_to_uv(p: Point, cell: &SurfaceCell) -> Option<Point> {
    let uv = &cell.uv;
    let pg = &cell.page;
    if uv.len() == 4 && pg.len() == 4 {
        let es = pg[3] - pg[0];
        let et = pg[1] - pg[0];
        let est = (pg[0] - pg[1]) + (pg[2] - pg[3]);
        let (mut s, mut t) = (0.5, 0.5);
        for _ in 0..6 {
            let cur = pg[0] + es * s + et * t + est * (s * t);
            let r = p - cur;
            if r.hypot() <= 1e-12 {
                break;
            }
            let dfs = es + est * t;
            let dft = et + est * s;
            let det = dfs.cross(dft);
            if !det.is_finite() || det.abs() <= 1e-12 {
                return None;
            }
            s += r.cross(dft) / det;
            t += dfs.cross(r) / det;
        }
        let u_lo = uv[0].x + (uv[1].x - uv[0].x) * t;
        let u_hi = uv[3].x + (uv[2].x - uv[3].x) * t;
        let out = Point::new(u_lo + s * (u_hi - u_lo), uv[0].y + t * (uv[1].y - uv[0].y));
        return (out.x.is_finite() && out.y.is_finite()).then_some(out);
    }
    let xf = cell_uv_to_page_affine(cell)?;
    let det = xf.determinant();
    if !det.is_finite() || det.abs() <= 1e-12 {
        return None;
    }
    let out = xf.inverse() * p;
    (out.x.is_finite() && out.y.is_finite()).then_some(out)
}

fn barycentric(q: Point, u: [Point; 3], p: [Point; 3]) -> Option<(Point, f64)> {
    let v0 = u[1] - u[0];
    let v1 = u[2] - u[0];
    let det = v0.cross(v1);
    if !det.is_finite() || det.abs() <= 1e-12 {
        return None;
    }
    let d = q - u[0];
    let w1 = d.cross(v1) / det;
    let w2 = v0.cross(d) / det;
    let w0 = 1.0 - w1 - w2;
    let outside = (-w0).max(0.0) + (-w1).max(0.0) + (-w2).max(0.0);
    let out = Point::new(w0 * p[0].x + w1 * p[1].x + w2 * p[2].x, w0 * p[0].y + w1 * p[1].y + w2 * p[2].y);
    (out.x.is_finite() && out.y.is_finite()).then_some((out, outside))
}

fn cell_uv_to_page_affine(cell: &SurfaceCell) -> Option<Affine> {
    if cell.uv.len() < 3 || cell.page.len() < 3 {
        return None;
    }
    let (u0, u1, u2) = (cell.uv[0], cell.uv[1], cell.uv[2]);
    let (p0, p1, p2) = (cell.page[0], cell.page[1], cell.page[2]);
    let du1 = u1 - u0;
    let du2 = u2 - u0;
    let det = du1.cross(du2);
    if !det.is_finite() || det.abs() <= 1e-12 {
        return None;
    }
    let dp1 = p1 - p0;
    let dp2 = p2 - p0;
    let a = (dp1.x * du2.y - dp2.x * du1.y) / det;
    let b = (dp1.y * du2.y - dp2.y * du1.y) / det;
    let c = (du1.x * dp2.x - du2.x * dp1.x) / det;
    let d = (du1.x * dp2.y - du2.x * dp1.y) / det;
    let tx = p0.x - a * u0.x - c * u0.y;
    let ty = p0.y - b * u0.x - d * u0.y;
    Some(Affine::new([a, b, c, d, tx, ty]))
}

fn map_uv_path_to_cell(uv_path: &PathData, cell: &SurfaceCell) -> Option<PathData> {
    let mut bp = BezPath::new();
    vectorcraft_geom::kurbo::flatten(uv_path.to_bezpath(), 0.2, |el| match el {
        PathEl::MoveTo(p) => {
            if let Some(q) = map_uv_point(p, cell) {
                bp.move_to(q);
            }
        }
        PathEl::LineTo(p) => {
            if let Some(q) = map_uv_point(p, cell) {
                bp.line_to(q);
            }
        }
        PathEl::ClosePath => bp.close_path(),
        _ => {}
    });
    let out = PathData::from_bezpath(&bp);
    (!out.subpaths.is_empty()).then_some(out)
}
