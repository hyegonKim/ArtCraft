//! Bounded flat-AABB BVH accelerator and physical CPU raytracer for 3D & Materials.
//!
//! Features:
//! - Safe, iterative flat-array Surface-Area-Heuristic (SAH) AABB BVH (`Bvh`) capped at
//!   [`MAX_RAYTRACE_TRIANGLES`] with zero `unsafe`, zero recursion, and zero panics.
//! - Crease-angle smooth vertex normal welding (~56° threshold) with sharp-edge curvature detection
//!   for Substance procedural edge wear.
//! - Perspective and orthographic primary rays aligned identically with VectorCraft's 2D/3D page
//!   projection, plus 2x2 Rotated Grid Supersampling (RGSS) on edges/silhouettes.
//! - Cook-Torrance Microfacet GGX BRDF (`D_GGX`, `G_Smith`, `F_Schlick`).
//! - Deterministic multi-ray Vogel-disk soft area-light shadows (both self-shadowing on the 3D
//!   surface and soft penumbra cast onto the `Shadow` ground/back catcher plane).
//! - Deterministic Cranley-Patterson cosine-weighted hemisphere Ambient Occlusion (AO).
//! - 1-bounce specular self-reflection + analytical studio HDRI environment IBL.

use std::collections::HashMap;

use vectorcraft_geom::{Point, Rect};

use crate::material::{PbrSample, SubstanceMaterial, sample_studio_env, tangent_basis};
use crate::{
    Inflate, MAX_SHADOW_FACES, Mesh, Revolve, Shadow, ShadowPosition, TOO_COMPLEX_SHADOW, V3, View, eye_distance, finite, perspective_scale,
};

/// Maximum triangles accepted across all parts of one raytraced 3D scene.
pub const MAX_RAYTRACE_TRIANGLES: usize = MAX_SHADOW_FACES;

/// Maximum pixel dimension on either axis for a raytraced 3D raster.
pub const MAX_RAYTRACE_DIM: u32 = 1280;

const MAX_BVH_DEPTH: u32 = 28;
const LEAF_LIMIT: usize = 4;
const SAH_BINS: usize = 8;
const CREASE_COS: f64 = 0.56;

/// One colored mesh part in a raytraced 3D scene.
#[derive(Clone, Debug)]
pub struct RaytracePart {
    /// Triangle mesh in mesh coordinates (`x` right, `y` up, `z` toward the viewer).
    pub mesh: Mesh,
    /// Display sRGB base color (`0.0..=1.0`).
    pub base_rgb: [f64; 3],
    /// Part opacity (`0.0..=1.0`).
    pub opacity: f64,
}

/// Complete scene specification for [`raytrace_scene`].
#[derive(Clone, Debug)]
pub struct RaytraceScene {
    pub parts: Vec<RaytracePart>,
    /// Document-space point corresponding to `[0, 0, 0]` in mesh space.
    pub origin: Point,
    /// Eye distance along `+z` in points (`None` for orthographic projection).
    pub eye: Option<f64>,
    /// Euler view rotation `[rx, ry, rz]` in degrees, inverted to lock 3D procedural textures to
    /// object space.
    pub rotation: [f64; 3],
    /// Directional lights in view space (`y` down): `(unit_dir_view, rgb_color, intensity, softness)`.
    pub lights: Vec<([f64; 3], [f64; 3], f64, f64)>,
    /// Ambient light intensity (`0.0..=2.0`).
    pub ambient: f64,
    /// Enable lighting (`false` renders unlit procedural albedo).
    pub shade: bool,
    /// Substance procedural PBR material parameters.
    pub material: SubstanceMaterial,
    /// Optional shadow ground/backdrop catcher plane.
    pub shadow: Option<Shadow>,
    /// Raytraced cosine-weighted ambient occlusion strength (`0.0..=1.0`).
    pub ambient_occlusion: f64,
    /// Target resolution in pixels per inch (`72.0` = 1 px per pt).
    pub ppi: f64,
}

/// Straight-alpha RGBA8 output image and its document-space placement bounds.
#[derive(Clone, Debug, PartialEq)]
pub struct RaytraceImage {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub bounds: Rect,
}

/// Closest ray-triangle intersection record returned by [`Bvh::intersect`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    pub t: f64,
    pub position: [f64; 3],
    pub geom_normal: [f64; 3],
    pub smooth_normal: [f64; 3],
    pub curvature: f64,
    pub part_index: usize,
    pub tri_index: usize,
}

#[derive(Clone, Copy, Debug)]
struct RtTri {
    p0: [f64; 3],
    e1: [f64; 3],
    e2: [f64; 3],
    geom_n: [f64; 3],
    n0: [f64; 3],
    n1: [f64; 3],
    n2: [f64; 3],
    c0: f64,
    c1: f64,
    c2: f64,
    part: u32,
    aabb_min: [f64; 3],
    aabb_max: [f64; 3],
    centroid: [f64; 3],
}

#[derive(Clone, Copy, Debug, Default)]
struct BvhNode {
    aabb_min: [f64; 3],
    aabb_max: [f64; 3],
    /// For leaves (`count > 0`): start index into `tri_indices`.
    /// For internal nodes (`count == 0`): index of left child (`right = left + 1`).
    left_or_first: u32,
    count: u32,
}

/// Flat-array bounding volume hierarchy over 3D triangles.
#[derive(Clone, Debug, Default)]
pub struct Bvh {
    tris: Vec<RtTri>,
    tri_indices: Vec<u32>,
    nodes: Vec<BvhNode>,
    bounds_min: [f64; 3],
    bounds_max: [f64; 3],
}

#[derive(Clone, Copy)]
struct ShadowCatcher {
    /// Unit normal in mesh space (`y` up) pointing toward the object.
    n: [f64; 3],
    u: [f64; 3],
    w: [f64; 3],
    level: f64,
    foot: [f64; 3],
    half: f64,
    light_dir: [f64; 3],
    light_softness: f64,
    opacity: f64,
}

fn add3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale3(a: [f64; 3], k: f64) -> [f64; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn norm3(v: [f64; 3], fallback: [f64; 3]) -> [f64; 3] {
    let l2 = dot3(v, v);
    if l2.is_finite() && l2 > 1e-24 {
        let inv = 1.0 / l2.sqrt();
        [v[0] * inv, v[1] * inv, v[2] * inv]
    } else {
        fallback
    }
}

fn min3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])]
}

fn max3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]
}

fn valid_pt3(p: [f64; 3]) -> bool {
    p.iter().all(|v| v.is_finite() && v.abs() <= 1e8)
}

fn collect_view_lights(r: Revolve) -> Vec<([f64; 3], [f64; 3], f64, f64)> {
    if let Some(lights) = r.lights {
        lights.iter().map(|l| (l.direction(), l.color, l.intensity.clamp(0.0, 4.0), l.softness.clamp(0.0, 1.0))).collect()
    } else {
        vec![(r.light_direction(), r.light_color, r.light_intensity.clamp(0.0, 4.0), r.light_softness.clamp(0.0, 1.0))]
    }
}

impl RaytraceScene {
    /// Construct a `RaytraceScene` matching `Extrude & Bevel` or `Rotate` camera projection.
    pub fn for_solid(parts: Vec<RaytracePart>, bounds: Rect, depth: f64, view: View, material: SubstanceMaterial, shadow: Option<Shadow>) -> Self {
        let rev = view.revolve();
        let size = (bounds.width() / 2.0).max(bounds.height() / 2.0).max(depth.max(0.0) / 2.0).max(1.0);
        Self {
            parts,
            origin: bounds.center(),
            eye: eye_distance(rev.perspective, size),
            rotation: [rev.rotation_x, rev.rotation_y, rev.rotation_z],
            lights: collect_view_lights(rev),
            ambient: rev.ambient,
            shade: rev.shade,
            material,
            shadow,
            ambient_occlusion: 0.5,
            ppi: 72.0,
        }
    }

    /// Construct a `RaytraceScene` matching `Inflate`'s puff-adjusted perspective eye distance.
    pub fn for_inflate(
        parts: Vec<RaytracePart>,
        shapes: &[(&vectorcraft_geom::PathData, vectorcraft_geom::FillRule)],
        bounds: Rect,
        options: Inflate,
        view: View,
        material: SubstanceMaterial,
        shadow: Option<Shadow>,
    ) -> Self {
        let mut eye = None;
        for (_, _, proj) in crate::solid::inflate_faces(shapes, bounds, options, view, true).flatten() {
            if let Some(p) = proj {
                eye = p.eye;
                break;
            }
        }
        let rev = view.revolve();
        if eye.is_none() && rev.perspective > 0.0 {
            let size = (bounds.width() / 2.0).max(bounds.height() / 2.0).max(options.depth.max(0.0) / 2.0).max(1.0);
            eye = eye_distance(rev.perspective, size);
        }
        Self {
            parts,
            origin: bounds.center(),
            eye,
            rotation: [rev.rotation_x, rev.rotation_y, rev.rotation_z],
            lights: collect_view_lights(rev),
            ambient: rev.ambient,
            shade: rev.shade,
            material,
            shadow,
            ambient_occlusion: 0.5,
            ppi: 72.0,
        }
    }

    /// Construct a `RaytraceScene` matching `Revolve`'s axis origin and perspective eye distance.
    pub fn for_revolve(parts: Vec<RaytracePart>, bounds: Rect, options: Revolve, material: SubstanceMaterial, shadow: Option<Shadow>) -> Self {
        let o = options.sanitized();
        let proj = crate::revolve_projection(bounds, o);
        Self {
            parts,
            origin: proj.origin,
            eye: proj.eye,
            rotation: [o.rotation_x, o.rotation_y, o.rotation_z],
            lights: collect_view_lights(o),
            ambient: o.ambient,
            shade: o.shade,
            material,
            shadow,
            ambient_occlusion: 0.5,
            ppi: 72.0,
        }
    }
}

impl Bvh {
    /// Build a bounded flat-array SAH BVH from a single [`Mesh`].
    pub fn from_mesh(mesh: &Mesh) -> Result<Self, String> {
        Self::from_parts(&[RaytracePart { mesh: mesh.clone(), base_rgb: [0.8, 0.8, 0.8], opacity: 1.0 }])
    }

    /// Build a bounded flat-array SAH BVH across multiple [`RaytracePart`] meshes.
    pub fn from_parts(parts: &[RaytracePart]) -> Result<Self, String> {
        let mut total_input = 0usize;
        for part in parts {
            total_input = total_input.saturating_add(part.mesh.triangles.len());
            if total_input > MAX_RAYTRACE_TRIANGLES {
                return Err(TOO_COMPLEX_SHADOW.into());
            }
        }

        let mut tris = Vec::with_capacity(total_input);
        for (part_idx, part) in parts.iter().enumerate() {
            append_part_triangles(&part.mesh, part_idx as u32, &mut tris);
        }

        if tris.is_empty() {
            return Ok(Self::default());
        }

        let mut bounds_min = tris[0].aabb_min;
        let mut bounds_max = tris[0].aabb_max;
        for t in &tris[1..] {
            bounds_min = min3(bounds_min, t.aabb_min);
            bounds_max = max3(bounds_max, t.aabb_max);
        }

        let mut tri_indices: Vec<u32> = (0..tris.len() as u32).collect();
        let mut nodes = Vec::with_capacity(tris.len() * 2);
        nodes.push(BvhNode::default());

        let mut stack: Vec<(u32, usize, usize, u32)> = Vec::with_capacity(64);
        stack.push((0, 0, tris.len(), 0));

        while let Some((node_idx, start, end, depth)) = stack.pop() {
            let count = end.saturating_sub(start);
            if count == 0 {
                continue;
            }
            let first_tri = &tris[tri_indices[start] as usize];
            let mut n_min = first_tri.aabb_min;
            let mut n_max = first_tri.aabb_max;
            let mut c_min = first_tri.centroid;
            let mut c_max = first_tri.centroid;
            for &ti in &tri_indices[start + 1..end] {
                let tr = &tris[ti as usize];
                n_min = min3(n_min, tr.aabb_min);
                n_max = max3(n_max, tr.aabb_max);
                c_min = min3(c_min, tr.centroid);
                c_max = max3(c_max, tr.centroid);
            }

            if count <= LEAF_LIMIT || depth >= MAX_BVH_DEPTH {
                if let Some(slot) = nodes.get_mut(node_idx as usize) {
                    *slot = BvhNode { aabb_min: n_min, aabb_max: n_max, left_or_first: start as u32, count: count as u32 };
                }
                continue;
            }

            let mid = partition_sah(&tris, &mut tri_indices[start..end], c_min, c_max) + start;
            let left_idx = nodes.len() as u32;
            nodes.push(BvhNode::default());
            nodes.push(BvhNode::default());
            if let Some(slot) = nodes.get_mut(node_idx as usize) {
                *slot = BvhNode { aabb_min: n_min, aabb_max: n_max, left_or_first: left_idx, count: 0 };
            }
            stack.push((left_idx + 1, mid, end, depth + 1));
            stack.push((left_idx, start, mid, depth + 1));
        }

        Ok(Self { tris, tri_indices, nodes, bounds_min, bounds_max })
    }

    pub fn is_empty(&self) -> bool {
        self.tris.is_empty() || self.nodes.is_empty()
    }

    pub fn triangle_count(&self) -> usize {
        self.tris.len()
    }

    pub fn bounds(&self) -> Option<([f64; 3], [f64; 3])> {
        (!self.is_empty()).then_some((self.bounds_min, self.bounds_max))
    }

    /// Find the closest intersection along `origin + t * dir` with `t in t_min..=t_max`.
    pub fn intersect(&self, origin: [f64; 3], dir: [f64; 3], t_min: f64, t_max: f64) -> Option<RayHit> {
        if self.is_empty() || !valid_pt3(origin) || !dir.iter().all(|v| v.is_finite()) {
            return None;
        }
        let inv_dir = dir.map(safe_rcp);
        let mut closest_t = t_max;
        let mut best: Option<(usize, f64, f64, f64)> = None;

        let mut stack = [0u32; 64];
        let mut sp = 0usize;
        if ray_aabb(origin, inv_dir, self.nodes[0].aabb_min, self.nodes[0].aabb_max, t_min, closest_t).is_some() {
            stack[0] = 0;
            sp = 1;
        }

        while sp > 0 {
            sp -= 1;
            let Some(node) = self.nodes.get(stack[sp] as usize) else { continue };
            if node.count > 0 {
                let first = node.left_or_first as usize;
                let end = (first + node.count as usize).min(self.tri_indices.len());
                for &ti in &self.tri_indices[first..end] {
                    let idx = ti as usize;
                    if let Some((t, u, v)) = ray_tri(origin, dir, &self.tris[idx], t_min, closest_t) {
                        closest_t = t;
                        best = Some((idx, t, u, v));
                    }
                }
            } else {
                let left = node.left_or_first;
                let right = left + 1;
                let tl = self.nodes.get(left as usize).and_then(|n| ray_aabb(origin, inv_dir, n.aabb_min, n.aabb_max, t_min, closest_t));
                let tr = self.nodes.get(right as usize).and_then(|n| ray_aabb(origin, inv_dir, n.aabb_min, n.aabb_max, t_min, closest_t));
                match (tl, tr) {
                    (Some(dl), Some(dr)) if sp + 2 <= stack.len() => {
                        if dl <= dr {
                            stack[sp] = right;
                            stack[sp + 1] = left;
                        } else {
                            stack[sp] = left;
                            stack[sp + 1] = right;
                        }
                        sp += 2;
                    }
                    (Some(_), _) if sp < stack.len() => {
                        stack[sp] = left;
                        sp += 1;
                    }
                    (_, Some(_)) if sp < stack.len() => {
                        stack[sp] = right;
                        sp += 1;
                    }
                    _ => {}
                }
            }
        }

        let (tri_index, t, u, v) = best?;
        let tr = &self.tris[tri_index];
        let w = (1.0 - u - v).max(0.0);
        let smooth_normal = norm3(
            [w * tr.n0[0] + u * tr.n1[0] + v * tr.n2[0], w * tr.n0[1] + u * tr.n1[1] + v * tr.n2[1], w * tr.n0[2] + u * tr.n1[2] + v * tr.n2[2]],
            tr.geom_n,
        );
        let curvature = (w * tr.c0 + u * tr.c1 + v * tr.c2).clamp(0.0, 1.0);
        Some(RayHit {
            t,
            position: add3(origin, scale3(dir, t)),
            geom_normal: tr.geom_n,
            smooth_normal,
            curvature,
            part_index: tr.part as usize,
            tri_index,
        })
    }

    /// Fast shadow / ambient-occlusion any-hit query along `origin + t * dir` in `t_min..=t_max`.
    pub fn occluded(&self, origin: [f64; 3], dir: [f64; 3], t_min: f64, t_max: f64) -> bool {
        if self.is_empty() || !valid_pt3(origin) || !dir.iter().all(|v| v.is_finite()) {
            return false;
        }
        let inv_dir = dir.map(safe_rcp);
        let mut stack = [0u32; 64];
        let mut sp = 0usize;
        if ray_aabb(origin, inv_dir, self.nodes[0].aabb_min, self.nodes[0].aabb_max, t_min, t_max).is_some() {
            stack[0] = 0;
            sp = 1;
        }
        while sp > 0 {
            sp -= 1;
            let Some(node) = self.nodes.get(stack[sp] as usize) else { continue };
            if node.count > 0 {
                let first = node.left_or_first as usize;
                let end = (first + node.count as usize).min(self.tri_indices.len());
                for &ti in &self.tri_indices[first..end] {
                    if ray_tri(origin, dir, &self.tris[ti as usize], t_min, t_max).is_some() {
                        return true;
                    }
                }
            } else {
                let left = node.left_or_first;
                let right = left + 1;
                if sp < stack.len()
                    && self.nodes.get(right as usize).and_then(|n| ray_aabb(origin, inv_dir, n.aabb_min, n.aabb_max, t_min, t_max)).is_some()
                {
                    stack[sp] = right;
                    sp += 1;
                }
                if sp < stack.len()
                    && self.nodes.get(left as usize).and_then(|n| ray_aabb(origin, inv_dir, n.aabb_min, n.aabb_max, t_min, t_max)).is_some()
                {
                    stack[sp] = left;
                    sp += 1;
                }
            }
        }
        false
    }
}

fn safe_rcp(d: f64) -> f64 {
    if d.abs() > 1e-18 {
        1.0 / d
    } else if d >= 0.0 {
        1e18
    } else {
        -1e18
    }
}

fn ray_aabb(origin: [f64; 3], inv_dir: [f64; 3], bmin: [f64; 3], bmax: [f64; 3], mut t_min: f64, mut t_max: f64) -> Option<f64> {
    for a in 0..3 {
        let t1 = (bmin[a] - origin[a]) * inv_dir[a];
        let t2 = (bmax[a] - origin[a]) * inv_dir[a];
        let (near, far) = if t1 <= t2 { (t1, t2) } else { (t2, t1) };
        t_min = t_min.max(near);
        t_max = t_max.min(far);
        if t_max < t_min {
            return None;
        }
    }
    Some(t_min)
}

/// Watertight-epsilon Möller–Trumbore ray-triangle intersection (two-sided).
fn ray_tri(origin: [f64; 3], dir: [f64; 3], tri: &RtTri, t_min: f64, t_max: f64) -> Option<(f64, f64, f64)> {
    let pvec = cross3(dir, tri.e2);
    let det = dot3(tri.e1, pvec);
    if det.abs() < 1e-15 {
        return None;
    }
    let inv_det = 1.0 / det;
    let tvec = sub3(origin, tri.p0);
    let u = dot3(tvec, pvec) * inv_det;
    if !(-1e-6..=1.0 + 1e-6).contains(&u) {
        return None;
    }
    let qvec = cross3(tvec, tri.e1);
    let v = dot3(dir, qvec) * inv_det;
    if v < -1e-6 || u + v > 1.0 + 1e-6 {
        return None;
    }
    let t = dot3(tri.e2, qvec) * inv_det;
    if t.is_finite() && t >= t_min && t <= t_max { Some((t, u.clamp(0.0, 1.0), v.clamp(0.0, 1.0))) } else { None }
}

fn append_part_triangles(mesh: &Mesh, part: u32, out: &mut Vec<RtTri>) {
    let Some((lo, hi)) = mesh.bounds() else { return };
    if !valid_pt3(lo) || !valid_pt3(hi) {
        return;
    }
    let diag = sub3(hi, lo);
    let weld_cell = ((diag[0].max(diag[1]).max(diag[2])) * 1e-5).max(1e-5);
    let inv_cell = 1.0 / weld_cell;
    let quant = |p: [f64; 3]| -> (i64, i64, i64) {
        (((p[0] - lo[0]) * inv_cell).round() as i64, ((p[1] - lo[1]) * inv_cell).round() as i64, ((p[2] - lo[2]) * inv_cell).round() as i64)
    };

    struct RawTri {
        p0: [f64; 3],
        p1: [f64; 3],
        p2: [f64; 3],
        nf: [f64; 3],
    }

    let mut raw: Vec<RawTri> = Vec::with_capacity(mesh.triangles.len());
    let mut vertex_normals: HashMap<(i64, i64, i64), Vec<[f64; 3]>> = HashMap::new();

    for &[i0, i1, i2] in &mesh.triangles {
        let (Some(&p0), Some(&p1), Some(&p2)) = (mesh.positions.get(i0 as usize), mesh.positions.get(i1 as usize), mesh.positions.get(i2 as usize))
        else {
            continue;
        };
        if !valid_pt3(p0) || !valid_pt3(p1) || !valid_pt3(p2) {
            continue;
        }
        let e1 = sub3(p1, p0);
        let e2 = sub3(p2, p0);
        let cr = cross3(e1, e2);
        let cr_l2 = dot3(cr, cr);
        if !cr_l2.is_finite() || cr_l2 < 1e-22 {
            continue;
        }
        let given = mesh.normals.get(i0 as usize).copied().unwrap_or([0.0; 3]);
        let mut nf = scale3(cr, 1.0 / cr_l2.sqrt());
        if valid_pt3(given) && dot3(given, given) > 0.25 && dot3(nf, given) < 0.0 {
            nf = scale3(nf, -1.0);
        }
        for p in [p0, p1, p2] {
            vertex_normals.entry(quant(p)).or_default().push(nf);
        }
        raw.push(RawTri { p0, p1, p2, nf });
    }

    for RawTri { p0, p1, p2, nf } in raw {
        let smooth_corner = |p: [f64; 3]| -> ([f64; 3], f64) {
            let Some(list) = vertex_normals.get(&quant(p)) else { return (nf, 0.0) };
            let mut sum = [0.0; 3];
            let mut sharp = false;
            let mut max_spread = 0.0f64;
            for &nj in list {
                let d = dot3(nf, nj);
                if d >= CREASE_COS {
                    sum = add3(sum, nj);
                    max_spread = max_spread.max((1.0 - d.min(1.0)).sqrt());
                } else {
                    sharp = true;
                }
            }
            let sn = norm3(sum, nf);
            let curv = if sharp { 0.85 } else { (max_spread * 1.4).clamp(0.0, 0.75) };
            (sn, curv)
        };
        let (n0, c0) = smooth_corner(p0);
        let (n1, c1) = smooth_corner(p1);
        let (n2, c2) = smooth_corner(p2);
        let aabb_min = min3(p0, min3(p1, p2));
        let aabb_max = max3(p0, max3(p1, p2));
        let centroid = [(p0[0] + p1[0] + p2[0]) / 3.0, (p0[1] + p1[1] + p2[1]) / 3.0, (p0[2] + p1[2] + p2[2]) / 3.0];
        out.push(RtTri { p0, e1: sub3(p1, p0), e2: sub3(p2, p0), geom_n: nf, n0, n1, n2, c0, c1, c2, part, aabb_min, aabb_max, centroid });
    }
}

fn surface_area(bmin: [f64; 3], bmax: [f64; 3]) -> f64 {
    let dx = (bmax[0] - bmin[0]).max(0.0);
    let dy = (bmax[1] - bmin[1]).max(0.0);
    let dz = (bmax[2] - bmin[2]).max(0.0);
    2.0 * (dx * dy + dy * dz + dz * dx)
}

fn partition_sah(tris: &[RtTri], indices: &mut [u32], c_min: [f64; 3], c_max: [f64; 3]) -> usize {
    let ext = sub3(c_max, c_min);
    let axis = if ext[0] >= ext[1] && ext[0] >= ext[2] {
        0
    } else if ext[1] >= ext[2] {
        1
    } else {
        2
    };
    let span = ext[axis];
    let half_n = indices.len() / 2;
    if !span.is_finite() || span <= 1e-12 {
        return half_n.max(1);
    }

    let mut bin_counts = [0usize; SAH_BINS];
    let mut bin_min = [[f64::INFINITY; 3]; SAH_BINS];
    let mut bin_max = [[f64::NEG_INFINITY; 3]; SAH_BINS];
    let scale = SAH_BINS as f64 / span;

    for &ti in indices.iter() {
        let tr = &tris[ti as usize];
        let b = (((tr.centroid[axis] - c_min[axis]) * scale) as usize).min(SAH_BINS - 1);
        bin_counts[b] += 1;
        bin_min[b] = min3(bin_min[b], tr.aabb_min);
        bin_max[b] = max3(bin_max[b], tr.aabb_max);
    }

    let mut left_area = [0.0f64; SAH_BINS - 1];
    let mut left_count = [0usize; SAH_BINS - 1];
    let (mut l_min, mut l_max, mut l_cnt) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3], 0usize);
    for i in 0..SAH_BINS - 1 {
        if bin_counts[i] > 0 {
            l_cnt += bin_counts[i];
            l_min = min3(l_min, bin_min[i]);
            l_max = max3(l_max, bin_max[i]);
        }
        left_count[i] = l_cnt;
        left_area[i] = if l_cnt > 0 { surface_area(l_min, l_max) } else { 0.0 };
    }

    let (mut r_min, mut r_max, mut r_cnt) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3], 0usize);
    let mut best_cost = f64::INFINITY;
    let mut best_split = SAH_BINS / 2;
    for i in (0..SAH_BINS - 1).rev() {
        if bin_counts[i + 1] > 0 {
            r_cnt += bin_counts[i + 1];
            r_min = min3(r_min, bin_min[i + 1]);
            r_max = max3(r_max, bin_max[i + 1]);
        }
        if left_count[i] > 0 && r_cnt > 0 {
            let cost = left_count[i] as f64 * left_area[i] + r_cnt as f64 * surface_area(r_min, r_max);
            if cost < best_cost {
                best_cost = cost;
                best_split = i + 1;
            }
        }
    }

    let split_pos = c_min[axis] + (best_split as f64 / SAH_BINS as f64) * span;
    let (mut i, mut j) = (0usize, indices.len());
    while i < j {
        if tris[indices[i] as usize].centroid[axis] < split_pos {
            i += 1;
        } else {
            j -= 1;
            indices.swap(i, j);
        }
    }
    if i == 0 || i == indices.len() { half_n.max(1) } else { i }
}

fn inv_rotate(rot_deg: [f64; 3], v_mesh: [f64; 3]) -> [f64; 3] {
    // Convert mesh space (y-up) to view space (y-down), apply inverse of Revolve::rotate (Z^T Y^T X^T),
    // then return in y-up local object coordinates.
    let v = [v_mesh[0], -v_mesh[1], v_mesh[2]];
    let (sx, cx) = rot_deg[0].to_radians().sin_cos();
    let (sy, cy) = rot_deg[1].to_radians().sin_cos();
    let (sz, cz) = rot_deg[2].to_radians().sin_cos();
    // Inverse Z
    let z = [cz * v[0] + sz * v[1], -sz * v[0] + cz * v[1], v[2]];
    // Inverse Y
    let y = [cy * z[0] - sy * z[2], z[1], sy * z[0] + cy * z[2]];
    // Inverse X
    let x = [y[0], cx * y[1] + sx * y[2], -sx * y[1] + cx * y[2]];
    [x[0], -x[1], x[2]]
}

fn build_shadow_catcher(scene: &RaytraceScene, bvh: &Bvh) -> Option<ShadowCatcher> {
    let settings = scene.shadow?;
    let (lo, hi) = bvh.bounds()?;
    let &(l_view, _, inten, softness) = scene.lights.first()?;
    if !scene.shade || inten <= 0.0 {
        return None;
    }
    let light_dir = norm3([l_view[0], -l_view[1], l_view[2]], [0.0, 0.0, 1.0]);
    let s_dist = finite(settings.distance, 0.0, 0.0, 1.0);
    let s_bounds = finite(settings.bounds, 1.0, 0.1, 4.0);
    let size = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(hi[2] - lo[2]).max(1e-6);

    let rot =
        Revolve { rotation_x: scene.rotation[0], rotation_y: scene.rotation[1], rotation_z: scene.rotation[2], ..Revolve::default() }.sanitized();
    let to_mesh = |v: V3| {
        let r = rot.rotate(v).unit();
        [r.0, -r.1, r.2]
    };
    let (n, u, w) = match settings.position {
        ShadowPosition::Behind => ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0]),
        ShadowPosition::Below => (to_mesh(V3(0.0, -1.0, 0.0)), to_mesh(V3(1.0, 0.0, 0.0)), to_mesh(V3(0.0, 0.0, 1.0))),
    };
    let reach = dot3(n, light_dir);
    if !reach.is_finite() || reach < 1e-3 {
        return None;
    }
    let mut min_proj = f64::INFINITY;
    for tr in &bvh.tris {
        for p in [tr.p0, add3(tr.p0, tr.e1), add3(tr.p0, tr.e2)] {
            min_proj = min_proj.min(dot3(n, p));
        }
    }
    if !min_proj.is_finite() {
        return None;
    }
    let level = min_proj - s_dist * size;
    let center = [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, (lo[2] + hi[2]) * 0.5];
    let foot = add3(center, scale3(n, level - dot3(n, center)));
    let max_opacity = scene.parts.iter().map(|p| p.opacity.clamp(0.0, 1.0)).fold(0.0f64, f64::max);
    if max_opacity <= 0.0 {
        return None;
    }
    Some(ShadowCatcher { n, u, w, level, foot, half: 2.0 * s_bounds * size, light_dir, light_softness: softness, opacity: 0.5 * max_opacity })
}

fn intersect_shadow_catcher(origin: [f64; 3], dir: [f64; 3], c: &ShadowCatcher) -> Option<(f64, [f64; 3])> {
    let denom = dot3(c.n, dir);
    if denom.abs() < 1e-9 {
        return None;
    }
    let t = (c.level - dot3(c.n, origin)) / denom;
    if !t.is_finite() || t <= 1e-6 {
        return None;
    }
    let pos = add3(origin, scale3(dir, t));
    let rel = sub3(pos, c.foot);
    if dot3(c.u, rel).abs() <= c.half && dot3(c.w, rel).abs() <= c.half { Some((t, pos)) } else { None }
}

fn project_mesh_pt(p: [f64; 3], origin: Point, eye: Option<f64>) -> Point {
    let f = perspective_scale(eye, p[2]);
    Point::new(origin.x + p[0] * f, origin.y - p[1] * f)
}

fn scene_page_bounds(scene: &RaytraceScene, bvh: &Bvh, catcher: Option<&ShadowCatcher>) -> Option<Rect> {
    let mut acc: Option<Rect> = None;
    let mut include = |pt: Point| {
        if pt.x.is_finite() && pt.y.is_finite() {
            acc = Some(match acc {
                Some(r) => r.union_pt(pt),
                None => Rect::from_points(pt, pt),
            });
        }
    };
    for tr in &bvh.tris {
        for p in [tr.p0, add3(tr.p0, tr.e1), add3(tr.p0, tr.e2)] {
            include(project_mesh_pt(p, scene.origin, scene.eye));
        }
    }
    // Include projected shadow bounds on the catcher plane so cast shadows are never clipped.
    if let Some(c) = catcher {
        let reach = dot3(c.n, c.light_dir).max(1e-3);
        let pad_cone = 0.42 * c.light_softness;
        for tr in &bvh.tris {
            for p in [tr.p0, add3(tr.p0, tr.e1), add3(tr.p0, tr.e2)] {
                let dist = (dot3(c.n, p) - c.level).max(0.0);
                let q = add3(p, scale3(c.light_dir, -dist / reach));
                let rel = sub3(q, c.foot);
                let expand = dist * pad_cone / reach;
                for (su, sw) in [(-1.0, -1.0), (1.0, 1.0)] {
                    let du = (dot3(c.u, rel) + su * expand).clamp(-c.half, c.half);
                    let dw = (dot3(c.w, rel) + sw * expand).clamp(-c.half, c.half);
                    let on_plane = add3(c.foot, add3(scale3(c.u, du), scale3(c.w, dw)));
                    include(project_mesh_pt(on_plane, scene.origin, scene.eye));
                }
            }
        }
    }
    let r = acc?.inflate(1.5, 1.5);
    (r.width() > 0.0 && r.height() > 0.0).then_some(r)
}

fn spatial_hash(p: [f64; 3], salt: u32) -> f64 {
    let qx = (p[0] * 4096.0).round() as i32;
    let qy = (p[1] * 4096.0).round() as i32;
    let qz = (p[2] * 4096.0).round() as i32;
    let mut h = salt
        .wrapping_add((qx as u32).wrapping_mul(0x9e37_79b1))
        .wrapping_add((qy as u32).wrapping_mul(0x85eb_ca77))
        .wrapping_add((qz as u32).wrapping_mul(0xc2b2_ae3d));
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    f64::from(h & 0x00ff_ffff) / 16_777_216.0
}

/// Evaluate soft or hard raytraced visibility (`0.0` fully occluded .. `1.0` fully lit) toward `light_dir`.
fn light_visibility(bvh: &Bvh, origin: [f64; 3], light_dir: [f64; 3], softness: f64, eps: f64, far: f64, jitter: f64) -> f64 {
    if softness <= 0.015 {
        return if bvh.occluded(origin, light_dir, eps, far) { 0.0 } else { 1.0 };
    }
    const SAMPLES: usize = 6;
    const GOLDEN_ANGLE: f64 = 2.399_963_229_728_653;
    let radius = 0.025 + 0.36 * softness.clamp(0.0, 1.0);
    let (lt, lb) = tangent_basis(light_dir);
    let rot_offset = jitter * std::f64::consts::TAU;
    let mut unoccluded = 0usize;
    for i in 0..SAMPLES {
        let r = ((i as f64 + 0.5) / SAMPLES as f64).sqrt() * radius;
        let theta = i as f64 * GOLDEN_ANGLE + rot_offset;
        let (s, c) = theta.sin_cos();
        let dir = norm3(add3(light_dir, add3(scale3(lt, r * c), scale3(lb, r * s))), light_dir);
        if !bvh.occluded(origin, dir, eps, far) {
            unoccluded += 1;
        }
    }
    unoccluded as f64 / SAMPLES as f64
}

/// Evaluate cosine-weighted hemisphere Ambient Occlusion (`1.0` open sky .. `1.0 - strength` occluded).
fn ambient_occlusion_factor(bvh: &Bvh, origin: [f64; 3], normal: [f64; 3], strength: f64, ao_radius: f64, eps: f64, jitter: f64) -> f64 {
    let strength = strength.clamp(0.0, 1.0);
    if strength <= 1e-3 || ao_radius <= eps {
        return 1.0;
    }
    const AO_SAMPLES: usize = 6;
    const GOLDEN_ANGLE: f64 = 2.399_963_229_728_653;
    let (t, bt) = tangent_basis(normal);
    let phi_off = jitter * std::f64::consts::TAU;
    let mut occ = 0.0;
    for i in 0..AO_SAMPLES {
        let u1 = (i as f64 + 0.5) / AO_SAMPLES as f64;
        let sin_theta = u1.sqrt();
        let cos_theta = (1.0 - u1).max(0.0).sqrt();
        let phi = i as f64 * GOLDEN_ANGLE + phi_off;
        let (sp, cp) = phi.sin_cos();
        let dir = norm3(add3(scale3(normal, cos_theta), add3(scale3(t, sin_theta * cp), scale3(bt, sin_theta * sp))), normal);
        if let Some(hit) = bvh.intersect(origin, dir, eps, ao_radius) {
            let falloff = (1.0 - hit.t / ao_radius).clamp(0.0, 1.0);
            occ += 0.35 + 0.65 * falloff;
        }
    }
    (1.0 - strength * (occ / AO_SAMPLES as f64)).clamp(0.08, 1.0)
}

fn ggx_direct_brdf(sample: &PbrSample, v: [f64; 3], l: [f64; 3], l_col: [f64; 3], intensity: f64, softness: f64) -> [f64; 3] {
    let n = sample.normal;
    let raw_nl = dot3(n, l);
    let wrap_nl = ((raw_nl + softness * 0.45) / (1.0 + softness * 0.45)).max(0.0);
    if wrap_nl <= 0.0 || intensity <= 0.0 {
        return [0.0; 3];
    }
    let n_dot_l = raw_nl.max(0.0);
    let n_dot_v = dot3(n, v).max(1e-4);
    let m = sample.metallic.clamp(0.0, 1.0);
    let rough = sample.roughness.clamp(0.045, 1.0);
    let alpha = rough * rough;
    let alpha2 = alpha * alpha;
    let k_geom = ((rough + 1.0) * (rough + 1.0)) / 8.0;
    let g_v = n_dot_v / (n_dot_v * (1.0 - k_geom) + k_geom);

    let h = norm3(add3(v, l), n);
    let n_dot_h = dot3(n, h).max(0.0);
    let v_dot_h = dot3(v, h).clamp(0.0, 1.0);
    let denom_d = n_dot_h * n_dot_h * (alpha2 - 1.0) + 1.0;
    let d_ggx = alpha2 / (std::f64::consts::PI * denom_d * denom_d).max(1e-9);
    let g_l = if n_dot_l > 0.0 { n_dot_l / (n_dot_l * (1.0 - k_geom) + k_geom) } else { 0.0 };
    let fresnel_pow = (1.0 - v_dot_h).powi(5);

    std::array::from_fn(|ch| {
        let f0 = 0.04 * (1.0 - m) + sample.albedo[ch] * m;
        let f_ch = f0 + (1.0 - f0) * fresnel_pow;
        let spec = if n_dot_l > 0.0 { (d_ggx * g_v * g_l * f_ch) / (4.0 * n_dot_v * n_dot_l).max(1e-6) * n_dot_l } else { 0.0 };
        let kd = (1.0 - f_ch) * (1.0 - m);
        let diff = kd * sample.albedo[ch] * wrap_nl;
        (diff + spec) * l_col[ch] * intensity
    })
}

struct ShadeContext<'a> {
    scene: &'a RaytraceScene,
    bvh: &'a Bvh,
    catcher: Option<&'a ShadowCatcher>,
    inv_radius: f64,
    ray_eps: f64,
    shadow_far: f64,
    ao_radius: f64,
    z_start: f64,
}

impl ShadeContext<'_> {
    fn eval_sample(&self, hit: &RayHit, v: [f64; 3]) -> (PbrSample, f64) {
        let part = &self.scene.parts[hit.part_index];
        let mut geom_n = hit.geom_normal;
        let mut smooth_n = hit.smooth_normal;
        if dot3(geom_n, v) < 0.0 {
            geom_n = scale3(geom_n, -1.0);
        }
        if dot3(smooth_n, geom_n) < 0.0 {
            smooth_n = scale3(smooth_n, -1.0);
        }
        // Pull grazing smooth normal gently above horizon of V.
        let nv = dot3(smooth_n, v);
        if nv < 0.02 {
            smooth_n = norm3(add3(smooth_n, scale3(v, 0.03 - nv)), geom_n);
        }
        let obj_pos = scale3(inv_rotate(self.scene.rotation, hit.position), self.inv_radius);
        let obj_n = inv_rotate(self.scene.rotation, smooth_n);
        let mut sample = self.scene.material.sample_3d(obj_pos, obj_n, smooth_n, part.base_rgb, hit.curvature);
        let p_nv = dot3(sample.normal, v);
        if p_nv < 0.02 {
            sample.normal = norm3(add3(sample.normal, scale3(v, 0.03 - p_nv)), smooth_n);
        }
        (sample, part.opacity.clamp(0.0, 1.0))
    }

    fn shade_secondary(&self, hit: &RayHit, ray_dir: [f64; 3]) -> [f64; 3] {
        let v = scale3(ray_dir, -1.0);
        let (sample, _) = self.eval_sample(hit, v);
        if !self.scene.shade {
            return sample.albedo;
        }
        let m = sample.metallic;
        let amb = self.scene.ambient.clamp(0.0, 2.0);
        let mut col = sample.albedo.map(|a| a * amb * (1.0 - 0.65 * m));
        for &(l_view, l_col, inten, softness) in &self.scene.lights {
            let l = norm3([l_view[0], -l_view[1], l_view[2]], [0.0, 0.0, 1.0]);
            let add = ggx_direct_brdf(&sample, v, l, l_col, inten, softness);
            col = add3(col, add);
        }
        let r_dir = sub3(scale3(sample.normal, 2.0 * dot3(sample.normal, v)), v);
        let env = sample_studio_env(r_dir, sample.roughness);
        for ch in 0..3 {
            let f0 = 0.04 * (1.0 - m) + sample.albedo[ch] * m;
            col[ch] += env[ch] * f0 * (0.30 + 0.45 * m);
        }
        col
    }

    fn trace_ray(&self, page_pt: Point, allow_bounce: bool) -> [f64; 4] {
        let dx = page_pt.x - self.scene.origin.x;
        let dy = -(page_pt.y - self.scene.origin.y);
        let (ray_o, ray_d) = match self.scene.eye {
            Some(e) => {
                let o = [0.0, 0.0, e];
                let d = norm3([dx, dy, -e], [0.0, 0.0, -1.0]);
                (o, d)
            }
            None => ([dx, dy, self.z_start], [0.0, 0.0, -1.0]),
        };

        let mesh_hit = self.bvh.intersect(ray_o, ray_d, 1e-5, self.shadow_far);
        let catcher_hit = self.catcher.and_then(|c| intersect_shadow_catcher(ray_o, ray_d, c).map(|h| (c, h.0, h.1)));

        if let Some((c, t_plane, p_plane)) = catcher_hit
            && mesh_hit.is_none_or(|mh| t_plane < mh.t)
        {
            let jitter = spatial_hash(p_plane, 0x51a7_0013);
            let bias_p = add3(p_plane, scale3(c.n, self.ray_eps));
            let vis = light_visibility(self.bvh, bias_p, c.light_dir, c.light_softness, self.ray_eps, self.shadow_far, jitter);
            let occ = (1.0 - vis).clamp(0.0, 1.0);
            if occ > 1e-4 {
                return [0.0, 0.0, 0.0, (c.opacity * occ).clamp(0.0, 1.0)];
            }
            return [0.0; 4];
        }

        let Some(hit) = mesh_hit else { return [0.0; 4] };
        let v = scale3(ray_d, -1.0);
        let (sample, opacity) = self.eval_sample(&hit, v);
        if !self.scene.shade {
            return [sample.albedo[0], sample.albedo[1], sample.albedo[2], opacity];
        }

        let orient_geom = if dot3(hit.geom_normal, v) >= 0.0 { hit.geom_normal } else { scale3(hit.geom_normal, -1.0) };
        let p_bias = add3(hit.position, scale3(orient_geom, self.ray_eps));
        let jitter = spatial_hash(hit.position, self.scene.material.seed.wrapping_add(0x9e37));

        let ao = ambient_occlusion_factor(self.bvh, p_bias, sample.normal, self.scene.ambient_occlusion, self.ao_radius, self.ray_eps, jitter);

        let m = sample.metallic;
        let amb = self.scene.ambient.clamp(0.0, 2.0);
        let sky_irr = sample_studio_env(sample.normal, 0.92);
        let mut linear_rgb = std::array::from_fn(|ch| {
            let hemi = 0.72 + 0.28 * sky_irr[ch];
            sample.albedo[ch] * amb * (1.0 - 0.72 * m) * ao * hemi
        });

        for (li, &(l_view, l_col, inten, softness)) in self.scene.lights.iter().enumerate() {
            if inten <= 0.0 {
                continue;
            }
            let l = norm3([l_view[0], -l_view[1], l_view[2]], [0.0, 0.0, 1.0]);
            let l_jit = (jitter + li as f64 * 0.381_966).fract();
            let vis = light_visibility(self.bvh, p_bias, l, softness, self.ray_eps, self.shadow_far, l_jit);
            if vis <= 0.0 {
                continue;
            }
            let direct = ggx_direct_brdf(&sample, v, l, l_col, inten, softness);
            linear_rgb = add3(linear_rgb, scale3(direct, vis));
        }

        // 1-bounce specular self-reflection + Studio HDRI IBL
        let n_dot_v = dot3(sample.normal, v).clamp(0.0, 1.0);
        let r_dir = norm3(sub3(scale3(sample.normal, 2.0 * n_dot_v), v), sample.normal);
        let env_rgb = sample_studio_env(r_dir, sample.roughness);
        let refl_rgb = if allow_bounce && sample.roughness < 0.72 {
            if let Some(sec_hit) = self.bvh.intersect(p_bias, r_dir, self.ray_eps * 2.0, self.shadow_far) {
                let sec_col = self.shade_secondary(&sec_hit, r_dir);
                let env_mix = (sample.roughness / 0.72).powi(2) * 0.80;
                [
                    sec_col[0] * (1.0 - env_mix) + env_rgb[0] * env_mix,
                    sec_col[1] * (1.0 - env_mix) + env_rgb[1] * env_mix,
                    sec_col[2] * (1.0 - env_mix) + env_rgb[2] * env_mix,
                ]
            } else {
                env_rgb
            }
        } else {
            env_rgb
        };

        let gloss = 1.0 - sample.roughness;
        let fresnel = (1.0 - n_dot_v).powi(5);
        let spec_ao = (ao + (1.0 - ao) * gloss * 0.55).clamp(0.15, 1.0);
        let ibl_weight = if self.scene.material.is_none() && m < 0.05 { 0.14 } else { 0.28 + 0.44 * m };
        for ch in 0..3 {
            let f0 = 0.04 * (1.0 - m) + sample.albedo[ch] * m;
            let f_ibl = f0 + (gloss.max(f0) - f0) * fresnel;
            linear_rgb[ch] += refl_rgb[ch] * f_ibl * ibl_weight * spec_ao;
        }

        let mapped = tone_map_neutral(linear_rgb);
        [mapped[0], mapped[1], mapped[2], opacity]
    }
}

/// Neutral highlight-preserving shoulder curve: near-linear in `0..0.7` so base colors stay
/// faithful, rolling specular highlights smoothly into `0..1` without channel hue shifts.
fn tone_map_neutral(rgb: [f64; 3]) -> [f64; 3] {
    let peak = rgb[0].max(rgb[1]).max(rgb[2]);
    if !peak.is_finite() || peak <= 0.0 {
        return [0.0; 3];
    }
    if peak <= 0.75 {
        return rgb.map(|v| v.clamp(0.0, 1.0));
    }
    let excess = peak - 0.75;
    let compressed_peak = 0.75 + 0.25 * (excess / (excess + 0.35));
    let scale = compressed_peak / peak;
    // Slight desaturation toward white only at extreme specular peaks, like Khronos PBR Neutral.
    let white_bleed = ((peak - 1.2) / (peak + 1.5)).clamp(0.0, 0.35);
    rgb.map(|v| {
        let c = (v.max(0.0) * scale).clamp(0.0, 1.0);
        (c + (compressed_peak - c) * white_bleed).clamp(0.0, 1.0)
    })
}

/// Render `scene` with physical BVH raytracing into an RGBA8 image and document placement `Rect`.
pub fn raytrace_scene(scene: &RaytraceScene) -> Result<RaytraceImage, String> {
    let bvh = Bvh::from_parts(&scene.parts)?;
    if bvh.is_empty() {
        return Ok(RaytraceImage {
            rgba: vec![0, 0, 0, 0],
            width: 1,
            height: 1,
            bounds: Rect::new(scene.origin.x, scene.origin.y, scene.origin.x + 1.0, scene.origin.y + 1.0),
        });
    }

    let catcher = build_shadow_catcher(scene, &bvh);
    let Some(bounds) = scene_page_bounds(scene, &bvh, catcher.as_ref()) else {
        return Ok(RaytraceImage {
            rgba: vec![0, 0, 0, 0],
            width: 1,
            height: 1,
            bounds: Rect::new(scene.origin.x, scene.origin.y, scene.origin.x + 1.0, scene.origin.y + 1.0),
        });
    };

    let ppi = finite(scene.ppi, 72.0, 18.0, 300.0);
    let scale_px = ppi / 72.0;
    let raw_w = (bounds.width() * scale_px).ceil().max(1.0);
    let raw_h = (bounds.height() * scale_px).ceil().max(1.0);
    let downscale = (raw_w / f64::from(MAX_RAYTRACE_DIM)).max(raw_h / f64::from(MAX_RAYTRACE_DIM)).max(1.0);
    let width = ((raw_w / downscale).round() as u32).clamp(1, MAX_RAYTRACE_DIM);
    let height = ((raw_h / downscale).round() as u32).clamp(1, MAX_RAYTRACE_DIM);

    let (lo, hi) = bvh.bounds().unwrap_or(([0.0; 3], [1.0; 3]));
    let ext = sub3(hi, lo);
    let scene_radius = (ext[0].max(ext[1]).max(ext[2]) * 0.5).max(1.0);
    let ctx = ShadeContext {
        scene,
        bvh: &bvh,
        catcher: catcher.as_ref(),
        inv_radius: 1.0 / scene_radius,
        ray_eps: (scene_radius * 2e-4).max(1e-4),
        shadow_far: scene_radius * 64.0 + scene.eye.unwrap_or(0.0) * 2.0 + 1000.0,
        ao_radius: scene_radius * 0.55,
        z_start: hi[2] + scene_radius * 4.0 + 50.0,
    };

    let w_usize = width as usize;
    let h_usize = height as usize;
    let px_w = bounds.width() / f64::from(width);
    let px_h = bounds.height() / f64::from(height);

    // Pass 1: 1 center sample per pixel.
    let mut center_samples = vec![[0.0f64; 4]; w_usize * h_usize];
    for y in 0..h_usize {
        let py = bounds.y0 + (y as f64 + 0.5) * px_h;
        for x in 0..w_usize {
            let px = bounds.x0 + (x as f64 + 0.5) * px_w;
            center_samples[y * w_usize + x] = ctx.trace_ray(Point::new(px, py), true);
        }
    }

    // Pass 2: Adaptive 2x2 Rotated Grid Supersampling (RGSS) on silhouette and high-contrast pixels.
    const RGSS: [(f64, f64); 4] = [(0.125, 0.375), (0.375, 0.875), (0.625, 0.125), (0.875, 0.625)];
    let mut rgba = vec![0u8; w_usize * h_usize * 4];
    for y in 0..h_usize {
        for x in 0..w_usize {
            let idx = y * w_usize + x;
            let c = center_samples[idx];
            let needs_aa = pixel_needs_aa(&center_samples, w_usize, h_usize, x, y, c);
            let final_px = if needs_aa {
                let mut premul = [0.0f64; 4];
                for &(ox, oy) in &RGSS {
                    let pt = Point::new(bounds.x0 + (x as f64 + ox) * px_w, bounds.y0 + (y as f64 + oy) * px_h);
                    let s = ctx.trace_ray(pt, false);
                    premul[0] += s[0] * s[3];
                    premul[1] += s[1] * s[3];
                    premul[2] += s[2] * s[3];
                    premul[3] += s[3];
                }
                let a = (premul[3] * 0.25).clamp(0.0, 1.0);
                if a > 1e-6 {
                    let inv_a = 0.25 / a;
                    [(premul[0] * inv_a).clamp(0.0, 1.0), (premul[1] * inv_a).clamp(0.0, 1.0), (premul[2] * inv_a).clamp(0.0, 1.0), a]
                } else {
                    [0.0; 4]
                }
            } else {
                c
            };
            let out = &mut rgba[idx * 4..idx * 4 + 4];
            out[0] = (final_px[0].clamp(0.0, 1.0) * 255.0).round() as u8;
            out[1] = (final_px[1].clamp(0.0, 1.0) * 255.0).round() as u8;
            out[2] = (final_px[2].clamp(0.0, 1.0) * 255.0).round() as u8;
            out[3] = (final_px[3].clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }

    Ok(RaytraceImage { rgba, width, height, bounds })
}

fn pixel_needs_aa(buf: &[[f64; 4]], w: usize, h: usize, x: usize, y: usize, c: [f64; 4]) -> bool {
    let neighbors = [(x.saturating_sub(1), y), ((x + 1).min(w - 1), y), (x, y.saturating_sub(1)), (x, (y + 1).min(h - 1))];
    for (nx, ny) in neighbors {
        let n = buf[ny * w + nx];
        let da = (c[3] - n[3]).abs();
        let dc = (c[0] - n[0]).abs() + (c[1] - n[1]).abs() + (c[2] - n[2]).abs();
        if da > 0.04 || dc > 0.16 {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::SubstanceMaterialPreset;
    use crate::{Extrude, Shadow, ShadowPosition, View, extrude_mesh};
    use vectorcraft_geom::{BezPath, FillRule, PathData};

    fn rect_path(x0: f64, y0: f64, x1: f64, y1: f64) -> PathData {
        let mut bp = BezPath::new();
        bp.move_to((x0, y0));
        bp.line_to((x1, y0));
        bp.line_to((x1, y1));
        bp.line_to((x0, y1));
        bp.close_path();
        PathData::from_bezpath(&bp)
    }

    #[test]
    fn bvh_raytracer_hits_and_misses_accurately() {
        let path = rect_path(0.0, 0.0, 60.0, 60.0);
        let bounds = path.bounds().unwrap();
        let view = View { rotation_x: 0.0, rotation_y: 0.0, rotation_z: 0.0, ..View::default() };
        let mesh = extrude_mesh(&path, FillRule::NonZero, bounds, Extrude { depth: 40.0, ..Extrude::default() }, view).unwrap();
        let bvh = Bvh::from_mesh(&mesh).unwrap();
        assert!(!bvh.is_empty());

        // Center ray along -z hits front cap at z = +20.0
        let hit = bvh.intersect([0.0, 0.0, 100.0], [0.0, 0.0, -1.0], 1e-4, 500.0).expect("center ray hits cube");
        assert!((hit.position[2] - 20.0).abs() < 1e-4, "hit z = {}", hit.position[2]);
        assert!(hit.smooth_normal[2] > 0.99);
        assert!(bvh.occluded([0.0, 0.0, 100.0], [0.0, 0.0, -1.0], 1e-4, 500.0));

        // Ray outside the 60x60 cube misses cleanly
        assert!(bvh.intersect([80.0, 0.0, 100.0], [0.0, 0.0, -1.0], 1e-4, 500.0).is_none());
        assert!(!bvh.occluded([80.0, 0.0, 100.0], [0.0, 0.0, -1.0], 1e-4, 500.0));
    }

    #[test]
    fn self_shadowing_occlusion_darkens_occluded_regions() {
        // Build two meshes in mesh space: a receiver floor/back plate and an overhanging blocker
        // between the receiver and the top-front key light.
        let receiver = Mesh {
            positions: vec![[-40.0, -40.0, 0.0], [40.0, -40.0, 0.0], [40.0, 40.0, 0.0], [-40.0, 40.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        };
        let blocker = Mesh {
            positions: vec![[-15.0, 5.0, 20.0], [15.0, 5.0, 20.0], [15.0, 35.0, 20.0], [-15.0, 35.0, 20.0]],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        };
        // Light from top-front (in view space y-down: [0.0, -0.6, 0.8] -> mesh y-up: [0.0, +0.6, +0.8]),
        // so the blocker at y=[5..35], z=20 casts a shadow downward onto y ~ [5 - 15 = -10 .. +20] on z=0!
        let make_scene = |parts: Vec<RaytracePart>| RaytraceScene {
            parts,
            origin: Point::new(50.0, 50.0),
            eye: None,
            rotation: [0.0; 3],
            lights: vec![([0.0, -0.6, 0.8], [1.0, 1.0, 1.0], 0.9, 0.0)],
            ambient: 0.15,
            shade: true,
            material: SubstanceMaterial { roughness: 0.8, metallic: 0.0, ..SubstanceMaterial::default() },
            shadow: None,
            ambient_occlusion: 0.4,
            ppi: 72.0,
        };
        let unoccluded =
            raytrace_scene(&make_scene(vec![RaytracePart { mesh: receiver.clone(), base_rgb: [0.85, 0.85, 0.85], opacity: 1.0 }])).unwrap();
        let occluded = raytrace_scene(&make_scene(vec![
            RaytracePart { mesh: receiver, base_rgb: [0.85, 0.85, 0.85], opacity: 1.0 },
            RaytracePart { mesh: blocker, base_rgb: [0.85, 0.85, 0.85], opacity: 1.0 },
        ]))
        .unwrap();

        // Sample document point (50.0, 52.0) -> mesh (0.0, -2.0, 0.0), directly under the blocker's shadow!
        let sample_luma = |img: &RaytraceImage, pt: Point| -> f64 {
            let x = (((pt.x - img.bounds.x0) / img.bounds.width()) * f64::from(img.width)) as usize;
            let y = (((pt.y - img.bounds.y0) / img.bounds.height()) * f64::from(img.height)) as usize;
            let idx = (y.min(img.height as usize - 1) * img.width as usize + x.min(img.width as usize - 1)) * 4;
            0.2126 * f64::from(img.rgba[idx]) + 0.7152 * f64::from(img.rgba[idx + 1]) + 0.0722 * f64::from(img.rgba[idx + 2])
        };
        let lit_luma = sample_luma(&unoccluded, Point::new(50.0, 52.0));
        let shadowed_luma = sample_luma(&occluded, Point::new(50.0, 52.0));
        assert!(
            shadowed_luma + 35.0 < lit_luma,
            "self-shadowed region must be significantly darker than unoccluded: shadowed={shadowed_luma} vs lit={lit_luma}"
        );
    }

    #[test]
    fn ggx_metallic_vs_dielectric_specular_response() {
        let path = rect_path(10.0, 10.0, 70.0, 70.0);
        let bounds = path.bounds().unwrap();
        let view = View { rotation_x: 0.0, rotation_y: 0.0, rotation_z: 0.0, light_rotation: 0.0, light_height: 90.0, ..View::default() };
        let mesh = extrude_mesh(&path, FillRule::NonZero, bounds, Extrude { depth: 30.0, ..Extrude::default() }, view).unwrap();
        let base_rgb = [0.92, 0.68, 0.22]; // warm gold-orange

        let render_with = |metallic: f64, roughness: f64| {
            let mut scene = RaytraceScene::for_solid(
                vec![RaytracePart { mesh: mesh.clone(), base_rgb, opacity: 1.0 }],
                bounds,
                30.0,
                view,
                SubstanceMaterial { metallic, roughness, ..SubstanceMaterial::default() },
                None,
            );
            scene.ppi = 36.0;
            raytrace_scene(&scene).unwrap()
        };

        let dielectric = render_with(0.0, 0.85);
        let metal = render_with(1.0, 0.12);
        let center_rgb = |img: &RaytraceImage| -> [u8; 3] {
            let cx = (img.width as usize) / 2;
            let cy = (img.height as usize) / 2;
            let i = (cy * img.width as usize + cx) * 4;
            [img.rgba[i], img.rgba[i + 1], img.rgba[i + 2]]
        };
        let d_col = center_rgb(&dielectric);
        let m_col = center_rgb(&metal);
        // Glossy gold metal reflects colored specular peak stronger than rough matte dielectric,
        // and preserves warm R > B tint in specular reflection.
        assert!(m_col[0] > d_col[0], "glossy metal peak R exceeds rough dielectric: {m_col:?} vs {d_col:?}");
        assert!(m_col[0] > m_col[2] + 15, "colored metallic F0 tints normal-incidence reflection: {m_col:?}");
    }

    #[test]
    fn all_substance_presets_raytrace_distinct_finite_images_and_shadow_plane_works() {
        let path = rect_path(20.0, 20.0, 68.0, 68.0);
        let bounds = path.bounds().unwrap();
        let view = View::default();
        let mesh =
            extrude_mesh(&path, FillRule::NonZero, bounds, Extrude { depth: 36.0, bevel: Some(crate::Bevel::default()), ..Extrude::default() }, view)
                .unwrap();

        let mut digests: Vec<(SubstanceMaterialPreset, Vec<u8>)> = Vec::new();
        for preset in SubstanceMaterialPreset::ALL {
            let mut scene = RaytraceScene::for_solid(
                vec![RaytracePart { mesh: mesh.clone(), base_rgb: [0.72, 0.38, 0.26], opacity: 1.0 }],
                bounds,
                36.0,
                view,
                SubstanceMaterial { preset, wear: 0.35, scale: 1.1, ..SubstanceMaterial::default() },
                Some(Shadow { position: ShadowPosition::Behind, distance: 0.1, bounds: 1.0 }),
            );
            scene.ppi = 36.0;
            let img = raytrace_scene(&scene).unwrap();
            assert!(img.width > 8 && img.height > 8);
            assert_eq!(img.rgba.len(), (img.width * img.height * 4) as usize);
            assert!(img.rgba.iter().any(|&b| b > 0), "{preset:?} produced blank image");
            digests.push((preset, img.rgba));
        }

        for i in 0..digests.len() {
            for j in (i + 1)..digests.len() {
                assert_ne!(digests[i].1, digests[j].1, "{:?} and {:?} rendered identical RGBA buffers", digests[i].0, digests[j].0);
            }
        }
    }

    #[test]
    fn hostile_empty_and_nan_meshes_never_panic() {
        let empty = RaytraceScene::for_solid(vec![], Rect::new(0.0, 0.0, 10.0, 10.0), 10.0, View::default(), SubstanceMaterial::default(), None);
        let out_empty = raytrace_scene(&empty).unwrap();
        assert_eq!(out_empty.rgba, vec![0, 0, 0, 0]);

        let hostile_mesh = Mesh {
            positions: vec![
                [f64::NAN, 0.0, 0.0],
                [f64::INFINITY, 1.0, 0.0],
                [0.0, f64::NEG_INFINITY, 1.0],
                [5.0, 5.0, 5.0],
                [5.0, 5.0, 5.0], // degenerate zero-area
                [5.0, 5.0, 5.0],
            ],
            normals: vec![[f64::NAN; 3]; 6],
            triangles: vec![[0, 1, 2], [3, 4, 5], [99, 100, 101]],
        };
        let mut hostile_scene = empty;
        hostile_scene.parts.push(RaytracePart { mesh: hostile_mesh, base_rgb: [f64::NAN, -5.0, 99.0], opacity: f64::NAN });
        let out_hostile = raytrace_scene(&hostile_scene).unwrap();
        assert_eq!(out_hostile.rgba, vec![0, 0, 0, 0]);
    }
}
