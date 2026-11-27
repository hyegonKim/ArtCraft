//! Extracts deduplicated wireframe edge paths from any 3D effect's projected facets.

use std::collections::{HashMap, HashSet};

use vectorcraft_doc::{Document, Effect, Node};
use vectorcraft_geom::kurbo::{CubicBez, ParamCurve, PathEl};
use vectorcraft_geom::{BezPath, FillRule, PathData, Point, Rect};
use vectorcraft_three_d::MAX_OBJECT_FACES;

use super::{
    CLASSIC_EXTRUDE, INFLATE, Profile, built_solids, facets, geometry, has_solid, is_container, is_plane, label_of, members, object_profile, region,
    reshape_members, resolve, solid, solid_paint_sources, solid_parts, union_bounds,
};
use crate::{GeomContext, apply_geometry_with};

/// Maximum unique wireframe edges allowed for one object.
const MAX_WIRE_EDGES: usize = 1_000_000;

/// Quantization grid for matching shared polygon/curve endpoints (`1e-4` pt).
const SNAP: f64 = 10_000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct KeyPt(i64, i64);

impl KeyPt {
    fn of(p: Point) -> Self {
        Self((p.x * SNAP).round() as i64, (p.y * SNAP).round() as i64)
    }
}

#[derive(Clone, Copy, Debug)]
enum SegGeom {
    Line(Point, Point),
    Cubic(CubicBez),
}

impl SegGeom {
    fn start(self) -> Point {
        match self {
            Self::Line(a, _) => a,
            Self::Cubic(c) => c.p0,
        }
    }

    fn end(self) -> Point {
        match self {
            Self::Line(_, b) => b,
            Self::Cubic(c) => c.p3,
        }
    }

    fn reversed(self) -> Self {
        match self {
            Self::Line(a, b) => Self::Line(b, a),
            Self::Cubic(c) => Self::Cubic(CubicBez::new(c.p3, c.p2, c.p1, c.p0)),
        }
    }

    fn canonical_mid_key(self, forward: bool) -> (u8, KeyPt, KeyPt) {
        let g = if forward { self } else { self.reversed() };
        match g {
            Self::Line(a, b) => {
                let mid = a.lerp(b, 0.5);
                (0, KeyPt::of(mid), KeyPt::of(mid))
            }
            Self::Cubic(c) => (1, KeyPt::of(c.eval(0.25)), KeyPt::of(c.eval(0.75))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct EdgeKey {
    a: KeyPt,
    b: KeyPt,
    kind: u8,
    m1: KeyPt,
    m2: KeyPt,
}

/// Applies the geometry effects in `fx` across `paths` as one shared bounding box (just as effects
/// after a 3D effect reshape the whole projected 3D group together).
pub(crate) fn reshaped(paths: Vec<PathData>, fx: &[Effect], ctx: &GeomContext) -> Vec<PathData> {
    if paths.is_empty() || !crate::has_geometry(fx) {
        return paths;
    }
    let bounds = paths.iter().filter_map(PathData::bounds).reduce(|a, b| a.union(b)).unwrap_or(Rect::ZERO);
    paths.into_iter().map(|p| apply_geometry_with(fx, &p, bounds, ctx)).collect()
}

/// Extracts the deduplicated wireframe edge paths of `n`'s visible 3D effect.
pub fn three_d_wireframe_in(doc: Option<&Document>, n: &Node) -> Result<Vec<PathData>, String> {
    if !crate::has_revolve(n) && !has_solid(n) {
        return Err("the object has no 3D effect (Extrude & Bevel, Revolve, Inflate or Rotate)".into());
    }
    let label = n.appearance.effects.iter().find(|e| e.visible && super::is_three_d(&e.id)).map_or("This effect", |e| label_of(&e.id));
    let resolved = resolve(doc, n, label)?;
    let n = &*resolved;
    let face_paths = if crate::has_revolve(n) {
        let raw = crate::revolve::revolve_face_paths(doc, n)?;
        if let Some(index) = n.appearance.effects.iter().position(|e| e.visible && crate::revolve::is_revolve(&e.id)) {
            let ctx = GeomContext { doc, ..GeomContext::of(n) };
            reshaped(raw, n.appearance.effects.get(index + 1..).unwrap_or_default(), &ctx)
        } else {
            raw
        }
    } else {
        solid_face_paths(doc, n, label)?
    };
    if face_paths.is_empty() {
        return Err(format!("{label} draws no faces for this object"));
    }
    chain_wireframe_edges(&face_paths, label)
}

fn solid_face_paths(doc: Option<&Document>, n: &Node, label: &str) -> Result<Vec<PathData>, String> {
    let ctx = GeomContext { doc, ..GeomContext::of(n) };
    if is_container(n) {
        let (index, fx) = solid(&n.appearance.effects).ok_or("a 3D effect on a group's fill or stroke draws nothing")?;
        let fx_label = label_of(&fx.id);
        let mut shapes = members(n, fx_label)?;
        reshape_members(n, n.appearance.effects.get(..index).unwrap_or_default(), &mut shapes);
        let raw = collect_solid_faces(fx, &shapes)?;
        return Ok(reshaped(raw, n.appearance.effects.get(index + 1..).unwrap_or_default(), &ctx));
    }
    let (path, rule) = object_profile(n, label)?;
    let (sources, paints) = solid_paint_sources(n);
    if let Some((index, fx)) = solid(&n.appearance.effects) {
        let shaped = geometry(&path, n.appearance.effects.get(..index).unwrap_or_default(), &ctx);
        let profiles = [Profile { path: shaped, rule, items: paints, opacity: 1.0, scopes: vec![] }];
        let raw = collect_solid_faces(fx, &profiles)?;
        return Ok(reshaped(raw, n.appearance.effects.get(index + 1..).unwrap_or_default(), &ctx));
    }
    let shaped = geometry(&path, &n.appearance.effects, &ctx);
    let mut out = Vec::new();
    for (source, item) in sources.into_iter().zip(&paints) {
        if let Some((index, fx)) = solid(item.effects()) {
            let item_ctx = ctx.item(source.unwrap_or(item));
            let p = geometry(&shaped, item.effects().get(..index).unwrap_or_default(), &item_ctx);
            let profiles = [Profile { path: p, rule, items: vec![item.clone()], opacity: 1.0, scopes: vec![] }];
            let raw = collect_solid_faces(fx, &profiles)?;
            out.extend(reshaped(raw, item.effects().get(index + 1..).unwrap_or_default(), &ctx));
        }
    }
    Ok(out)
}

fn collect_solid_faces(fx: &Effect, shapes: &[Profile]) -> Result<Vec<PathData>, String> {
    let label = label_of(&fx.id);
    let b = union_bounds(shapes).ok_or_else(|| format!("{label} requires a shape with at least two anchors"))?;
    let p = crate::merged_params(&fx.id, &fx.params);
    let mut out = Vec::new();
    if is_plane(&fx.id) {
        for s in shapes {
            for item in &s.items {
                let (area, rule) = region(&s.path, s.rule, item);
                for f in facets(super::ROTATE, &area, rule, b, &p)? {
                    out.push(f.path());
                }
            }
        }
        return Ok(out);
    }
    let (parts, bounds) = solid_parts(shapes, b);
    for res in built_solids(&fx.id, &p, &parts, bounds, MAX_OBJECT_FACES) {
        if let Some(facets) = res? {
            let _ = (CLASSIC_EXTRUDE, INFLATE, FillRule::NonZero);
            out.extend(facets.iter().map(vectorcraft_three_d::Facet::path));
        }
    }
    Ok(out)
}

fn extract_segments(path: &PathData, seen: &mut HashSet<EdgeKey>, edges: &mut Vec<SegGeom>) -> Result<(), String> {
    let mut start = Point::ZERO;
    let mut cur = Point::ZERO;
    let mut push = |seg: SegGeom| -> Result<(), String> {
        let a = seg.start();
        let b = seg.end();
        if a.distance(b) <= 1e-6 {
            return Ok(());
        }
        let ka = KeyPt::of(a);
        let kb = KeyPt::of(b);
        if ka == kb {
            return Ok(());
        }
        let (lo, hi, forward) = if ka <= kb { (ka, kb, true) } else { (kb, ka, false) };
        let (kind, m1, m2) = seg.canonical_mid_key(forward);
        let key = EdgeKey { a: lo, b: hi, kind, m1, m2 };
        if seen.insert(key) {
            if edges.len() >= MAX_WIRE_EDGES {
                return Err(format!("the wireframe has more than {MAX_WIRE_EDGES} edges"));
            }
            edges.push(seg);
        }
        Ok(())
    };
    for el in path.to_bezpath().elements() {
        match *el {
            PathEl::MoveTo(p) => {
                start = p;
                cur = p;
            }
            PathEl::LineTo(p) => {
                push(SegGeom::Line(cur, p))?;
                cur = p;
            }
            PathEl::QuadTo(p1, p2) => {
                let c = CubicBez::new(cur, cur + (p1 - cur) * (2.0 / 3.0), p2 + (p1 - p2) * (2.0 / 3.0), p2);
                push(SegGeom::Cubic(c))?;
                cur = p2;
            }
            PathEl::CurveTo(p1, p2, p3) => {
                push(SegGeom::Cubic(CubicBez::new(cur, p1, p2, p3)))?;
                cur = p3;
            }
            PathEl::ClosePath => {
                push(SegGeom::Line(cur, start))?;
                cur = start;
            }
        }
    }
    Ok(())
}

fn chain_wireframe_edges(face_paths: &[PathData], label: &str) -> Result<Vec<PathData>, String> {
    let mut seen = HashSet::new();
    let mut edges = Vec::new();
    for path in face_paths {
        extract_segments(path, &mut seen, &mut edges)?;
    }
    if edges.is_empty() {
        return Err(format!("{label} draws no faces for this object"));
    }
    let mut adj: HashMap<KeyPt, Vec<(usize, bool)>> = HashMap::new();
    for (idx, seg) in edges.iter().enumerate() {
        adj.entry(KeyPt::of(seg.start())).or_default().push((idx, true));
        adj.entry(KeyPt::of(seg.end())).or_default().push((idx, false));
    }
    let mut used = vec![false; edges.len()];
    let mut out = Vec::with_capacity(edges.len());

    // First walk chains starting at junctions / endpoints (degree != 2), then closed loops (degree == 2).
    for pass in 0..2 {
        for seed in 0..edges.len() {
            if used[seed] {
                continue;
            }
            let k_start = KeyPt::of(edges[seed].start());
            let k_end = KeyPt::of(edges[seed].end());
            let deg_start = adj.get(&k_start).map_or(0, Vec::len);
            let deg_end = adj.get(&k_end).map_or(0, Vec::len);
            if pass == 0 && deg_start == 2 && deg_end == 2 {
                continue;
            }
            used[seed] = true;
            let initial = if pass == 0 && deg_start == 2 && deg_end != 2 { edges[seed].reversed() } else { edges[seed] };
            let mut chain = vec![initial];
            // Extend forward while vertex has degree == 2.
            while let Some(tail_seg) = chain.last().copied() {
                let tail_key = KeyPt::of(tail_seg.end());
                let Some(neighbors) = adj.get(&tail_key) else { break };
                if neighbors.len() != 2 {
                    break;
                }
                let Some(&(next_idx, forward)) = neighbors.iter().find(|&&(i, _)| !used[i]) else {
                    break;
                };
                used[next_idx] = true;
                let oriented = if forward { edges[next_idx] } else { edges[next_idx].reversed() };
                chain.push(oriented);
            }
            let closed = chain.len() >= 2 && chain.first().zip(chain.last()).is_some_and(|(a, b)| KeyPt::of(a.start()) == KeyPt::of(b.end()));
            out.push(segments_to_path(&chain, closed));
        }
    }
    Ok(out)
}

fn segments_to_path(chain: &[SegGeom], closed: bool) -> PathData {
    let mut bp = BezPath::new();
    let Some(first) = chain.first() else {
        return PathData::new(vec![]);
    };
    bp.move_to(first.start());
    for (i, seg) in chain.iter().enumerate() {
        let is_last_closed_line = closed && i + 1 == chain.len() && matches!(seg, SegGeom::Line(..));
        if is_last_closed_line {
            break;
        }
        match *seg {
            SegGeom::Line(_, b) => bp.line_to(b),
            SegGeom::Cubic(c) => bp.curve_to(c.p1, c.p2, c.p3),
        }
    }
    if closed {
        bp.close_path();
    }
    PathData::from_bezpath(&bp)
}
