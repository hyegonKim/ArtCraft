//! Orthographic 3D-to-2D CAD view projection (`Top`, `Front`, `Right`, `Isometric`)
//! with silhouette & sharp-crease feature detection, hidden-line occlusion culling,
//! and collinear polyline chaining for direct 2D CAD dimensioning.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::Solid3dNode;
use super::mesh::{TriMesh3d, extract_isosurface};
use crate::{Bounds2, Vec2, Vec3};

const BUCKET_SIDE: usize = 16;
const SUB_SEGMENTS: usize = 8;

/// Standard CAD orthographic and isometric projection views.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OrthoView3d {
    /// Plan view from `+Z` (`u = +X`, `v = +Y`).
    Top,
    /// Elevation from `-Y` (`u = +X`, `v = +Z`).
    Front,
    /// Right-side elevation from `+X` (`u = +Y`, `v = +Z`).
    Right,
    /// Southeast isometric view from `(1, -1, 1) / sqrt(3)` with `+Z` projected upward.
    Isometric,
}

impl OrthoView3d {
    pub const ALL: [OrthoView3d; 4] = [Self::Top, Self::Front, Self::Right, Self::Isometric];

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "top" | "plan" | "xy" => Some(Self::Top),
            "front" | "xz" => Some(Self::Front),
            "right" | "side" | "yz" => Some(Self::Right),
            "isometric" | "iso" | "seiso" | "3d" => Some(Self::Isometric),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Front => "front",
            Self::Right => "right",
            Self::Isometric => "isometric",
        }
    }

    /// Orthonormal camera basis `(right, up, view_dir)` where `view_dir` points toward the viewer.
    pub fn basis(self) -> (Vec3, Vec3, Vec3) {
        match self {
            Self::Top => (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)),
            Self::Front => (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, -1.0, 0.0)),
            Self::Right => (Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 0.0)),
            Self::Isometric => {
                let view_dir = Vec3::new(1.0, -1.0, 1.0).normalized();
                let right = Vec3::new(1.0, 1.0, 0.0).normalized();
                let up = view_dir.cross(right).normalized();
                (right, up, view_dir)
            }
        }
    }

    /// Project a 3D world point into `(2D view coordinates, depth toward viewer)`.
    pub fn project(self, p: Vec3) -> (Vec2, f64) {
        let (right, up, view_dir) = self.basis();
        (Vec2::new(p.dot(right), p.dot(up)), p.dot(view_dir))
    }
}

/// 2D CAD geometry projected from a 3D solid or triangle mesh.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Projected2d {
    pub visible_polylines: Vec<Vec<Vec2>>,
    pub hidden_segments: Vec<[Vec2; 2]>,
    pub bounds: Bounds2,
}

#[derive(Clone, Copy)]
struct TriFace {
    verts: [u32; 3],
    normal: Vec3,
    front: bool,
}

#[derive(Clone, Copy)]
struct FrontOccluder {
    tri_index: usize,
    uv: [Vec2; 3],
    depth: [f64; 3],
    bbox: Bounds2,
}

struct SpatialGrid {
    bounds: Bounds2,
    buckets: Vec<Vec<usize>>,
}

impl SpatialGrid {
    fn build(occluders: &[FrontOccluder], overall: Bounds2) -> Self {
        let mut buckets = vec![Vec::new(); BUCKET_SIDE * BUCKET_SIDE];
        if overall.is_empty() {
            return Self { bounds: overall, buckets };
        }
        for (idx, occ) in occluders.iter().enumerate() {
            let (x0, y0) = bucket_coords(overall, occ.bbox.min);
            let (x1, y1) = bucket_coords(overall, occ.bbox.max);
            for by in y0..=y1 {
                for bx in x0..=x1 {
                    if let Some(cell) = buckets.get_mut(by * BUCKET_SIDE + bx) {
                        cell.push(idx);
                    }
                }
            }
        }
        Self { bounds: overall, buckets }
    }

    fn candidates(&self, p: Vec2) -> &[usize] {
        if self.bounds.is_empty() {
            return &[];
        }
        let (bx, by) = bucket_coords(self.bounds, p);
        self.buckets.get(by * BUCKET_SIDE + bx).map_or(&[], Vec::as_slice)
    }
}

fn bucket_coords(bounds: Bounds2, p: Vec2) -> (usize, usize) {
    let w = bounds.width().max(1e-9);
    let h = bounds.height().max(1e-9);
    let tx = ((p.x - bounds.min.x) / w).clamp(0.0, 0.999_999);
    let ty = ((p.y - bounds.min.y) / h).clamp(0.0, 0.999_999);
    ((tx * BUCKET_SIDE as f64) as usize, (ty * BUCKET_SIDE as f64) as usize)
}

#[derive(Default)]
struct EdgeFaces {
    count: u8,
    faces: [usize; 2],
}

impl EdgeFaces {
    fn push(&mut self, face_idx: usize) {
        if let Some(slot) = self.faces.get_mut(self.count as usize) {
            *slot = face_idx;
            self.count += 1;
        }
    }
}

struct CandidateEdge {
    u: u32,
    v: u32,
    faces: EdgeFaces,
    front_candidate: bool,
}

/// Extract an SDF isosurface at `resolution` and project its silhouette and sharp creases to 2D.
pub fn project_solid_to_2d(solid: &Solid3dNode, view: OrthoView3d, resolution: usize, crease_angle_rad: f64, include_hidden: bool) -> Projected2d {
    let mesh = extract_isosurface(solid, resolution);
    project_mesh_to_2d(&mesh, view, crease_angle_rad, include_hidden)
}

/// Project `mesh` onto `view`, classifying visible feature polylines and optional hidden edges.
pub fn project_mesh_to_2d(mesh: &TriMesh3d, view: OrthoView3d, crease_angle_rad: f64, include_hidden: bool) -> Projected2d {
    if mesh.vertices.is_empty() || mesh.triangles.is_empty() {
        return Projected2d { bounds: Bounds2::EMPTY, ..Projected2d::default() };
    }
    let projected_verts: Vec<(Vec2, f64)> = mesh.vertices.iter().map(|&p| view.project(p)).collect();
    let mut view_bounds = Bounds2::EMPTY;
    for &(uv, _) in &projected_verts {
        view_bounds.add(uv);
    }
    let (_, _, view_dir) = view.basis();
    let (faces, occluders) = classify_faces(mesh, &projected_verts, view_dir);
    let grid = SpatialGrid::build(&occluders, view_bounds);
    let candidates = collect_feature_edges(&faces, crease_angle_rad.cos(), include_hidden);
    let depth_tol = estimate_depth_tolerance(mesh, view_bounds);

    let mut visible_segs = Vec::new();
    let mut hidden_segs = Vec::new();
    for edge in &candidates {
        partition_edge_visibility(edge, &projected_verts, &occluders, &grid, depth_tol, include_hidden, (&mut visible_segs, &mut hidden_segs));
    }

    let merge_tol = (view_bounds.width().max(view_bounds.height()) * 1e-4).max(1e-6);
    let visible_polylines = chain_and_simplify_segments(&visible_segs, merge_tol);
    let mut out_bounds = Bounds2::EMPTY;
    for poly in &visible_polylines {
        for &pt in poly {
            out_bounds.add(pt);
        }
    }
    for &[a, b] in &hidden_segs {
        out_bounds.add(a);
        out_bounds.add(b);
    }
    Projected2d { visible_polylines, hidden_segments: hidden_segs, bounds: out_bounds }
}

fn classify_faces(mesh: &TriMesh3d, projected: &[(Vec2, f64)], view_dir: Vec3) -> (Vec<TriFace>, Vec<FrontOccluder>) {
    let mut faces = Vec::with_capacity(mesh.triangles.len());
    let mut occluders = Vec::new();
    for &verts in &mesh.triangles {
        let (Some(&p0), Some(&p1), Some(&p2)) =
            (mesh.vertices.get(verts[0] as usize), mesh.vertices.get(verts[1] as usize), mesh.vertices.get(verts[2] as usize))
        else {
            continue;
        };
        let raw_n = (p1 - p0).cross(p2 - p0);
        let normal = raw_n.normalized();
        let front = normal.dot(view_dir) > 1e-6;
        let face_id = faces.len();
        faces.push(TriFace { verts, normal, front });
        if front
            && let (Some(&(uv0, z0)), Some(&(uv1, z1)), Some(&(uv2, z2))) =
                (projected.get(verts[0] as usize), projected.get(verts[1] as usize), projected.get(verts[2] as usize))
            && (uv1 - uv0).cross(uv2 - uv0).abs() > 1e-12
        {
            let mut bbox = Bounds2::EMPTY;
            bbox.add(uv0);
            bbox.add(uv1);
            bbox.add(uv2);
            occluders.push(FrontOccluder { tri_index: face_id, uv: [uv0, uv1, uv2], depth: [z0, z1, z2], bbox });
        }
    }
    (faces, occluders)
}

fn collect_feature_edges(faces: &[TriFace], cos_crease: f64, include_hidden: bool) -> Vec<CandidateEdge> {
    let mut adj: BTreeMap<(u32, u32), EdgeFaces> = BTreeMap::new();
    for (idx, face) in faces.iter().enumerate() {
        let [a, b, c] = face.verts;
        for (u, v) in [(a, b), (b, c), (c, a)] {
            let key = if u < v { (u, v) } else { (v, u) };
            adj.entry(key).or_default().push(idx);
        }
    }
    let mut out = Vec::new();
    for ((u, v), ef) in adj {
        match ef.count {
            1 => {
                if let Some(f0) = faces.get(ef.faces[0])
                    && (f0.front || include_hidden)
                {
                    out.push(CandidateEdge { u, v, faces: ef, front_candidate: f0.front });
                }
            }
            2 => {
                let (Some(f0), Some(f1)) = (faces.get(ef.faces[0]), faces.get(ef.faces[1])) else { continue };
                let silhouette = f0.front != f1.front;
                let sharp_crease = f0.normal.dot(f1.normal) < cos_crease;
                if silhouette || (sharp_crease && (f0.front || include_hidden)) {
                    out.push(CandidateEdge { u, v, faces: ef, front_candidate: f0.front || f1.front });
                }
            }
            _ => {}
        }
    }
    out
}

fn estimate_depth_tolerance(mesh: &TriMesh3d, view_bounds: Bounds2) -> f64 {
    let span = view_bounds.width().max(view_bounds.height()).max(1e-3);
    // Estimate average triangle edge length to prevent curved-surface self-occlusion on silhouettes.
    let mut edge_sum = 0.0;
    let mut count = 0usize;
    for &[a, b, _] in mesh.triangles.iter().take(64) {
        if let (Some(&pa), Some(&pb)) = (mesh.vertices.get(a as usize), mesh.vertices.get(b as usize)) {
            edge_sum += (pb - pa).len();
            count += 1;
        }
    }
    let avg_edge = if count > 0 { edge_sum / count as f64 } else { span * 0.02 };
    (avg_edge * 0.85).max(span * 1e-3)
}

fn partition_edge_visibility(
    edge: &CandidateEdge,
    projected: &[(Vec2, f64)],
    occluders: &[FrontOccluder],
    grid: &SpatialGrid,
    depth_tol: f64,
    include_hidden: bool,
    out: (&mut Vec<[Vec2; 2]>, &mut Vec<[Vec2; 2]>),
) {
    let (Some(&(p0, z0)), Some(&(p1, z1))) = (projected.get(edge.u as usize), projected.get(edge.v as usize)) else {
        return;
    };
    if p0.near(p1, 1e-9) {
        return;
    }
    let (visible_out, hidden_out) = out;
    if !edge.front_candidate {
        if include_hidden {
            hidden_out.push([p0, p1]);
        }
        return;
    }
    let mut run_start: Option<(f64, bool)> = None;
    for step in 0..SUB_SEGMENTS {
        let t0 = step as f64 / SUB_SEGMENTS as f64;
        let t1 = (step + 1) as f64 / SUB_SEGMENTS as f64;
        let tm = 0.5 * (t0 + t1);
        let mid_uv = p0.lerp(p1, tm);
        let mid_z = z0 + (z1 - z0) * tm;
        let vis = !is_point_occluded(mid_uv, mid_z, edge, occluders, grid, depth_tol);
        match run_start {
            None => run_start = Some((t0, vis)),
            Some((s_t, prev_vis)) if prev_vis != vis => {
                push_sub_segment(p0, p1, s_t, t0, prev_vis, include_hidden, (visible_out, hidden_out));
                run_start = Some((t0, vis));
            }
            Some(_) => {}
        }
    }
    if let Some((s_t, vis)) = run_start {
        push_sub_segment(p0, p1, s_t, 1.0, vis, include_hidden, (visible_out, hidden_out));
    }
}

fn push_sub_segment(p0: Vec2, p1: Vec2, t0: f64, t1: f64, visible: bool, include_hidden: bool, out: (&mut Vec<[Vec2; 2]>, &mut Vec<[Vec2; 2]>)) {
    let a = p0.lerp(p1, t0);
    let b = p0.lerp(p1, t1);
    if a.near(b, 1e-9) {
        return;
    }
    if visible {
        out.0.push([a, b]);
    } else if include_hidden {
        out.1.push([a, b]);
    }
}

fn is_point_occluded(uv: Vec2, depth: f64, edge: &CandidateEdge, occluders: &[FrontOccluder], grid: &SpatialGrid, depth_tol: f64) -> bool {
    for &occ_idx in grid.candidates(uv) {
        let Some(occ) = occluders.get(occ_idx) else { continue };
        if (edge.faces.count >= 1 && occ.tri_index == edge.faces.faces[0]) || (edge.faces.count >= 2 && occ.tri_index == edge.faces.faces[1]) {
            continue;
        }
        if !occ.bbox.contains(uv) {
            continue;
        }
        if let Some(occ_depth) = barycentric_depth(uv, occ.uv, occ.depth)
            && occ_depth > depth + depth_tol
        {
            return true;
        }
    }
    false
}

fn barycentric_depth(p: Vec2, tri: [Vec2; 3], depths: [f64; 3]) -> Option<f64> {
    let v0 = tri[1] - tri[0];
    let v1 = tri[2] - tri[0];
    let v2 = p - tri[0];
    let denom = v0.cross(v1);
    if denom.abs() <= 1e-12 {
        return None;
    }
    let inv = 1.0 / denom;
    let w1 = v2.cross(v1) * inv;
    let w2 = v0.cross(v2) * inv;
    let w0 = 1.0 - w1 - w2;
    if w0 >= -1e-5 && w1 >= -1e-5 && w2 >= -1e-5 { Some(w0 * depths[0] + w1 * depths[1] + w2 * depths[2]) } else { None }
}

fn chain_and_simplify_segments(segments: &[[Vec2; 2]], tol: f64) -> Vec<Vec<Vec2>> {
    let mut used = vec![false; segments.len()];
    let mut polylines = Vec::new();
    for start_idx in 0..segments.len() {
        if used[start_idx] {
            continue;
        }
        used[start_idx] = true;
        let [a, b] = segments[start_idx];
        let mut chain = vec![a, b];
        extend_chain(&mut chain, segments, &mut used, tol, true);
        extend_chain(&mut chain, segments, &mut used, tol, false);
        let simplified = simplify_collinear(&chain, tol * 2.5);
        if simplified.len() >= 2 {
            polylines.push(simplified);
        }
    }
    polylines
}

fn extend_chain(chain: &mut Vec<Vec2>, segments: &[[Vec2; 2]], used: &mut [bool], tol: f64, at_tail: bool) {
    while let Some(&anchor) = if at_tail { chain.last() } else { chain.first() } {
        let found = segments.iter().enumerate().find_map(|(i, &[a, b])| {
            if used[i] {
                None
            } else if a.near(anchor, tol) {
                Some((i, b))
            } else if b.near(anchor, tol) {
                Some((i, a))
            } else {
                None
            }
        });
        let Some((idx, next_pt)) = found else {
            break;
        };
        used[idx] = true;
        if at_tail {
            chain.push(next_pt);
        } else {
            chain.insert(0, next_pt);
        }
    }
}

fn simplify_collinear(points: &[Vec2], chord_tol: f64) -> Vec<Vec2> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    let mut out = Vec::with_capacity(points.len());
    for &p in points {
        while out.len() >= 2 {
            let a = out[out.len() - 2];
            let b = out[out.len() - 1];
            let ac = p - a;
            let len = ac.len();
            if len <= chord_tol {
                break;
            }
            let perp_dist = (b - a).cross(ac).abs() / len;
            let forward = (b - a).dot(p - b) >= 0.0;
            if forward && perp_dist <= chord_tol {
                out.pop();
            } else {
                break;
            }
        }
        out.push(p);
    }
    out
}
