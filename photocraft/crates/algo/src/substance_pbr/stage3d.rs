//! Suite B — Substance 3D Stager + SDF Clay Modeler (`3D Stage & SDF Clay Mockup`):
//! - Analytic 3D SDF primitives (`Sphere`, `RoundedBox`, `Bottle`, `Can`, `Cylinder`, `Torus`, `DisplayCard`)
//!   plus bounded pure-Rust Wavefront `.obj` mesh parser (`CustomObj`, capped at 65,536 vertices/faces).
//! - SDF Clay smooth boolean operators (`SmoothUnion`, `SmoothSubtract`, `SmoothIntersect`).
//! - 64-step sphere-tracing raymarcher with 2×2 silhouette supersampling, tetrahedral central-difference
//!   normals, 3-step SDF ambient occlusion, Laplacian curvature edge wear (`weathering`), Cook-Torrance
//!   GGX BRDF, background-matched Equirectangular IBL, Active Layer Decal UV wrapping, and a soft
//!   ground-plane shadow catcher (`y = ground_y`).

use std::f32::consts::{PI, TAU};

use photocraft_color::PixelFormat;
use photocraft_geom::Rect;
use photocraft_raster::{Surface, from_rgba_into};
use serde::{Deserialize, Serialize};

use super::brdf::{
    EquirectProbe, Mat3, PbrSurfacePoint, linear3_to_srgb, sanitize_f32, shade_ggx_direct, srgb3_to_linear, tone_map_filmic, v3_add, v3_cross, v3_dot, v3_len,
    v3_lerp, v3_norm, v3_scale, v3_sub,
};
use super::sampler::extract_ibl_environment_rgba;
use crate::photo_util::{bilinear, par_rows};

/// Maximum vertices or faces accepted by [`parse_obj_mesh`].
pub const MAX_OBJ_ELEMENTS: usize = 65_536;

/// Primary or secondary 3D primitive shape in Substance 3D Stager / Modeler.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum StageShape {
    #[default]
    Sphere,
    RoundedBox,
    Bottle,
    Can,
    Cylinder,
    Torus,
    DisplayCard,
    CustomObj,
}

impl StageShape {
    pub const ALL: [Self; 8] = [Self::Sphere, Self::RoundedBox, Self::Bottle, Self::Can, Self::Cylinder, Self::Torus, Self::DisplayCard, Self::CustomObj];

    pub fn id(self) -> &'static str {
        match self {
            Self::Sphere => "sphere",
            Self::RoundedBox => "roundedBox",
            Self::Bottle => "bottle",
            Self::Can => "can",
            Self::Cylinder => "cylinder",
            Self::Torus => "torus",
            Self::DisplayCard => "displayCard",
            Self::CustomObj => "customObj",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Sphere => "Sphere",
            Self::RoundedBox => "Rounded Box",
            Self::Bottle => "Bottle",
            Self::Can => "Beverage Can",
            Self::Cylinder => "Cylinder",
            Self::Torus => "Torus",
            Self::DisplayCard => "Display Card",
            Self::CustomObj => "Custom OBJ Mesh",
        }
    }

    pub fn from_id(s: &str) -> Option<Self> {
        match s.trim() {
            "sphere" | "Sphere" => Some(Self::Sphere),
            "roundedBox" | "box" | "RoundedBox" => Some(Self::RoundedBox),
            "bottle" | "Bottle" => Some(Self::Bottle),
            "can" | "Can" => Some(Self::Can),
            "cylinder" | "Cylinder" => Some(Self::Cylinder),
            "torus" | "Torus" => Some(Self::Torus),
            "displayCard" | "card" | "DisplayCard" => Some(Self::DisplayCard),
            "customObj" | "obj" | "CustomObj" => Some(Self::CustomObj),
            _ => None,
        }
    }
}

/// SDF Clay boolean combination operator (`Substance 3D Modeler`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SdfBooleanOp {
    #[default]
    None,
    SmoothUnion,
    SmoothSubtract,
    SmoothIntersect,
}

impl SdfBooleanOp {
    pub const ALL: [Self; 4] = [Self::None, Self::SmoothUnion, Self::SmoothSubtract, Self::SmoothIntersect];

    pub fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::SmoothUnion => "smoothUnion",
            Self::SmoothSubtract => "smoothSubtract",
            Self::SmoothIntersect => "smoothIntersect",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "Single Shape",
            Self::SmoothUnion => "Clay Smooth Union",
            Self::SmoothSubtract => "Clay Smooth Subtract",
            Self::SmoothIntersect => "Clay Smooth Intersect",
        }
    }

    pub fn from_id(s: &str) -> Option<Self> {
        match s.trim() {
            "none" | "None" => Some(Self::None),
            "smoothUnion" | "union" | "SmoothUnion" => Some(Self::SmoothUnion),
            "smoothSubtract" | "subtract" | "SmoothSubtract" => Some(Self::SmoothSubtract),
            "smoothIntersect" | "intersect" | "SmoothIntersect" => Some(Self::SmoothIntersect),
            _ => None,
        }
    }
}

/// Surface material preset for the 3D Stage object (`Stager` + `Painter`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum StageMaterialPreset {
    #[default]
    MattePlastic,
    BrushedMetal,
    PolishedGold,
    FrostedGlass,
    Ceramic,
    Marble,
    WeatheredCopper,
    ActiveLayerDecal,
}

impl StageMaterialPreset {
    pub const ALL: [Self; 8] = [
        Self::MattePlastic,
        Self::BrushedMetal,
        Self::PolishedGold,
        Self::FrostedGlass,
        Self::Ceramic,
        Self::Marble,
        Self::WeatheredCopper,
        Self::ActiveLayerDecal,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::MattePlastic => "mattePlastic",
            Self::BrushedMetal => "brushedMetal",
            Self::PolishedGold => "polishedGold",
            Self::FrostedGlass => "frostedGlass",
            Self::Ceramic => "ceramic",
            Self::Marble => "marble",
            Self::WeatheredCopper => "weatheredCopper",
            Self::ActiveLayerDecal => "activeLayerDecal",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::MattePlastic => "Matte Plastic",
            Self::BrushedMetal => "Brushed Metal",
            Self::PolishedGold => "Polished Gold",
            Self::FrostedGlass => "Frosted Glass",
            Self::Ceramic => "Glazed Ceramic",
            Self::Marble => "Veined Marble",
            Self::WeatheredCopper => "Weathered Copper",
            Self::ActiveLayerDecal => "Wrap Active Layer as Decal",
        }
    }

    pub fn from_id(s: &str) -> Option<Self> {
        match s.trim() {
            "mattePlastic" | "plastic" | "MattePlastic" => Some(Self::MattePlastic),
            "brushedMetal" | "metal" | "BrushedMetal" => Some(Self::BrushedMetal),
            "polishedGold" | "gold" | "PolishedGold" => Some(Self::PolishedGold),
            "frostedGlass" | "glass" | "FrostedGlass" => Some(Self::FrostedGlass),
            "ceramic" | "Ceramic" => Some(Self::Ceramic),
            "marble" | "Marble" => Some(Self::Marble),
            "weatheredCopper" | "copper" | "WeatheredCopper" => Some(Self::WeatheredCopper),
            "activeLayerDecal" | "decal" | "ActiveLayerDecal" => Some(Self::ActiveLayerDecal),
            _ => None,
        }
    }
}

/// Complete scene, clay modeling, material, camera, and lighting parameters for 3D Stager.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Stage3dParams {
    pub shape: StageShape,
    pub sdf_op: SdfBooleanOp,
    pub secondary_shape: StageShape,
    pub smooth_k: f32,
    pub secondary_offset: [f32; 3],
    pub material: StageMaterialPreset,
    pub base_color: [f32; 3],
    pub metallic: f32,
    pub roughness: f32,
    pub weathering: f32,
    pub rot_deg: [f32; 3],
    pub scale: f32,
    pub center: [f32; 2],
    pub fov_deg: f32,
    pub match_background_ibl: bool,
    pub light_dir: [f32; 3],
    pub light_intensity: f32,
    pub shadow_catcher: f32,
    pub ground_y: f32,
}

impl Default for Stage3dParams {
    fn default() -> Self {
        Self {
            shape: StageShape::Bottle,
            sdf_op: SdfBooleanOp::None,
            secondary_shape: StageShape::Torus,
            smooth_k: 0.16,
            secondary_offset: [0.32, 0.05, 0.0],
            material: StageMaterialPreset::Ceramic,
            base_color: [0.84, 0.86, 0.90],
            metallic: 0.08,
            roughness: 0.24,
            weathering: 0.15,
            rot_deg: [12.0, -24.0, 0.0],
            scale: 1.0,
            center: [0.5, 0.5],
            fov_deg: 42.0,
            match_background_ibl: true,
            light_dir: [0.52, 0.72, 0.46],
            light_intensity: 2.2,
            shadow_catcher: 0.65,
            ground_y: -0.64,
        }
    }
}

impl Stage3dParams {
    pub fn sanitized(&self) -> Self {
        let clean3 =
            |v: [f32; 3], d: [f32; 3], lo: f32, hi: f32| [sanitize_f32(v[0], d[0], lo, hi), sanitize_f32(v[1], d[1], lo, hi), sanitize_f32(v[2], d[2], lo, hi)];
        let raw_ld = clean3(self.light_dir, [0.52, 0.72, 0.46], -10.0, 10.0);
        let light_dir = if v3_len(raw_ld) > 1e-4 { v3_norm(raw_ld) } else { v3_norm([0.52, 0.72, 0.46]) };
        Self {
            shape: self.shape,
            sdf_op: self.sdf_op,
            secondary_shape: self.secondary_shape,
            smooth_k: sanitize_f32(self.smooth_k, 0.16, 0.02, 0.8),
            secondary_offset: clean3(self.secondary_offset, [0.32, 0.05, 0.0], -2.5, 2.5),
            material: self.material,
            base_color: clean3(self.base_color, [0.84, 0.86, 0.90], 0.0, 1.0),
            metallic: sanitize_f32(self.metallic, 0.08, 0.0, 1.0),
            roughness: sanitize_f32(self.roughness, 0.24, 0.04, 1.0),
            weathering: sanitize_f32(self.weathering, 0.15, 0.0, 1.0),
            rot_deg: clean3(self.rot_deg, [12.0, -24.0, 0.0], -3600.0, 3600.0),
            scale: sanitize_f32(self.scale, 1.0, 0.2, 2.5),
            center: [sanitize_f32(self.center[0], 0.5, -0.5, 1.5), sanitize_f32(self.center[1], 0.5, -0.5, 1.5)],
            fov_deg: sanitize_f32(self.fov_deg, 42.0, 20.0, 90.0),
            match_background_ibl: self.match_background_ibl,
            light_dir,
            light_intensity: sanitize_f32(self.light_intensity, 2.2, 0.0, 10.0),
            shadow_catcher: sanitize_f32(self.shadow_catcher, 0.65, 0.0, 1.0),
            ground_y: sanitize_f32(self.ground_y, -0.64, -1.2, 1.2),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct PrecomputedTri {
    a: [f32; 3],
    ba: [f32; 3],
    cb: [f32; 3],
    ac: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
    nor: [f32; 3],
}

/// Auto-centered, unit-scaled Wavefront `.obj` triangle mesh bounded at 65,536 vertices & faces.
#[derive(Clone, Debug)]
pub struct ObjMesh {
    pub vertices: Vec<[f32; 3]>,
    pub faces: Vec<[u32; 3]>,
    pub bound_radius: f32,
    tris: Vec<PrecomputedTri>,
}

fn parse_obj_vertex_index(tok: &str, vert_count: usize) -> Option<u32> {
    let head = tok.split('/').next()?.trim();
    if head.is_empty() {
        return None;
    }
    let raw: i64 = head.parse().ok()?;
    let idx = if raw > 0 {
        usize::try_from(raw - 1).ok()?
    } else if raw < 0 {
        let rel = usize::try_from(raw.checked_neg()?).ok()?;
        vert_count.checked_sub(rel)?
    } else {
        return None;
    };
    (idx < vert_count).then_some(idx as u32)
}

/// Bounded pure-Rust Wavefront `.obj` parser (capped at 65,536 vertices and 65,536 triangles,
/// auto-centered and unit-scaled to radius `0.62`, never panics on malformed/hostile input).
pub fn parse_obj_mesh(obj_text: &str) -> Result<ObjMesh, String> {
    let mut raw_verts: Vec<[f32; 3]> = Vec::new();
    let mut faces: Vec<[u32; 3]> = Vec::new();

    for line in obj_text.lines().take(262_144) {
        let trimmed = line.split('#').next().unwrap_or("").trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let Some(tag) = parts.next() else { continue };
        match tag {
            "v" if raw_verts.len() < MAX_OBJ_ELEMENTS => {
                let (Some(xs), Some(ys), Some(zs)) = (parts.next(), parts.next(), parts.next()) else {
                    continue;
                };
                let (Ok(x), Ok(y), Ok(z)) = (xs.parse::<f32>(), ys.parse::<f32>(), zs.parse::<f32>()) else {
                    continue;
                };
                if x.is_finite() && y.is_finite() && z.is_finite() {
                    raw_verts.push([x, y, z]);
                }
            }
            "f" if faces.len() < MAX_OBJ_ELEMENTS => {
                let mut poly: Vec<u32> = Vec::with_capacity(8);
                for tok in parts.take(32) {
                    if let Some(vi) = parse_obj_vertex_index(tok, raw_verts.len()) {
                        poly.push(vi);
                    }
                }
                if poly.len() >= 3 {
                    let v0 = poly[0];
                    for pair in poly[1..].windows(2) {
                        if faces.len() >= MAX_OBJ_ELEMENTS {
                            break;
                        }
                        if let [v1, v2] = *pair
                            && v0 != v1
                            && v1 != v2
                            && v2 != v0
                        {
                            faces.push([v0, v1, v2]);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if raw_verts.len() < 3 || faces.is_empty() {
        return Err("OBJ mesh contains no valid triangles".into());
    }

    let mut bmin = [f32::MAX; 3];
    let mut bmax = [f32::MIN; 3];
    for v in &raw_verts {
        for c in 0..3 {
            bmin[c] = bmin[c].min(v[c]);
            bmax[c] = bmax[c].max(v[c]);
        }
    }
    let center = [0.5 * (bmin[0] + bmax[0]), 0.5 * (bmin[1] + bmax[1]), 0.5 * (bmin[2] + bmax[2])];
    let half_span = ((bmax[0] - bmin[0]).max(bmax[1] - bmin[1]).max(bmax[2] - bmin[2]) * 0.5).max(1e-5);
    let inv_scale = 0.60 / half_span;

    let mut vertices = Vec::with_capacity(raw_verts.len());
    let mut max_r2 = 0.04f32;
    for v in &raw_verts {
        let p = [(v[0] - center[0]) * inv_scale, (v[1] - center[1]) * inv_scale, (v[2] - center[2]) * inv_scale];
        max_r2 = max_r2.max(v3_dot(p, p));
        vertices.push(p);
    }

    let mut tris = Vec::with_capacity(faces.len());
    for &[i0, i1, i2] in &faces {
        let (Some(&a), Some(&b), Some(&c)) = (vertices.get(i0 as usize), vertices.get(i1 as usize), vertices.get(i2 as usize)) else {
            continue;
        };
        let ba = v3_sub(b, a);
        let cb = v3_sub(c, b);
        let ac = v3_sub(a, c);
        let raw_n = v3_cross(ba, v3_sub(c, a));
        let nor = v3_norm(raw_n);
        tris.push(PrecomputedTri { a, ba, cb, ac, b, c, nor });
    }
    if tris.is_empty() {
        return Err("OBJ mesh faces are degenerate".into());
    }

    Ok(ObjMesh { vertices, faces, bound_radius: max_r2.sqrt() + 0.06, tris })
}

/// Optional image/mesh inputs for [`render_stage_3d_rgba`] and [`render_stage_3d_surface`].
#[derive(Clone, Copy, Default)]
pub struct Stage3dInputs<'a> {
    pub backdrop_rgba: Option<(&'a [[f32; 4]], usize, usize)>,
    pub decal_rgba: Option<(&'a [[f32; 4]], usize, usize)>,
    pub custom_mesh: Option<&'a ObjMesh>,
}

#[inline]
fn dot2_clamped(e: [f32; 3], p: [f32; 3]) -> f32 {
    let t = (v3_dot(p, e) / v3_dot(e, e).max(1e-8)).clamp(0.0, 1.0);
    let d = v3_sub(v3_scale(e, t), p);
    v3_dot(d, d)
}

/// Signed/thickened distance to a triangle mesh (Quilez exact `udTriangle` with shell thickness `0.022`).
fn sdf_obj_mesh(p: [f32; 3], mesh: &ObjMesh) -> f32 {
    let r = v3_len(p);
    if r > mesh.bound_radius + 0.25 {
        return r - mesh.bound_radius;
    }
    let mut best_d2 = f32::MAX;
    let mut best_sign = 1.0f32;
    for t in &mesh.tris {
        let pa = v3_sub(p, t.a);
        let pb = v3_sub(p, t.b);
        let pc = v3_sub(p, t.c);
        let s1 = v3_dot(v3_cross(t.ba, t.nor), pa).signum();
        let s2 = v3_dot(v3_cross(t.cb, t.nor), pb).signum();
        let s3 = v3_dot(v3_cross(t.ac, t.nor), pc).signum();
        let d2 = if s1 + s2 + s3 < 2.0 {
            dot2_clamped(t.ba, pa).min(dot2_clamped(t.cb, pb)).min(dot2_clamped(t.ac, pc))
        } else {
            let dn = v3_dot(t.nor, pa);
            dn * dn
        };
        if d2 < best_d2 {
            best_d2 = d2;
            best_sign = if v3_dot(pa, t.nor) < 0.0 { -1.0 } else { 1.0 };
        }
    }
    // Closed meshes get interior negative distance; open shells still render cleanly via thin shell offset.
    best_d2.max(0.0).sqrt() * best_sign.max(-0.5) - 0.015
}

#[inline]
fn sd_round_box(p: [f32; 3], b: [f32; 3], r: f32) -> f32 {
    let q = [p[0].abs() - b[0], p[1].abs() - b[1], p[2].abs() - b[2]];
    let out = v3_len([q[0].max(0.0), q[1].max(0.0), q[2].max(0.0)]);
    let inside = q[0].max(q[1].max(q[2])).min(0.0);
    out + inside - r
}

#[inline]
fn sd_rounded_cylinder(p: [f32; 3], ra: f32, rb: f32, h: f32) -> f32 {
    let dx = (p[0] * p[0] + p[2] * p[2]).sqrt() - ra + rb;
    let dy = p[1].abs() - h + rb;
    (dx.max(0.0).hypot(dy.max(0.0))) + dx.max(dy).min(0.0) - rb
}

fn sd_bottle(p: [f32; 3]) -> f32 {
    let r = (p[0] * p[0] + p[2] * p[2]).sqrt();
    let y = p[1];
    // Smooth studio bottle silhouette: base [-0.60..0.10], shoulder [0.10..0.34], neck [0.34..0.56], lip [0.56..0.62]
    let profile_r = if y < 0.10 {
        0.31
    } else if y < 0.34 {
        let t = ((y - 0.10) / 0.24).clamp(0.0, 1.0);
        let s = t * t * (3.0 - 2.0 * t);
        0.31 + (0.115 - 0.31) * s
    } else if y < 0.55 {
        0.115
    } else {
        0.135
    };
    let dy = y.abs() - 0.61;
    let dr = r - profile_r;
    // Scale by conservative Lipschitz factor (0.82) so sphere-tracing never overshoots shoulder slope!
    ((dr.max(0.0).hypot(dy.max(0.0))) + dr.max(dy).min(0.0) - 0.018) * 0.82
}

fn sd_can(p: [f32; 3]) -> f32 {
    let body = sd_rounded_cylinder(p, 0.34, 0.045, 0.54);
    let r = (p[0] * p[0] + p[2] * p[2]).sqrt();
    // Recessed aluminum top chime indent
    let top_indent = ((r - 0.26).max(0.0)).hypot((p[1] - 0.54).abs()) - 0.025;
    let rim = sd_torus([p[0], p[1] - 0.525, p[2]], 0.295, 0.022);
    let with_rim = smin_poly(body, rim, 0.02);
    smax_poly(with_rim, -top_indent, 0.015)
}

#[inline]
fn sd_torus(p: [f32; 3], major: f32, minor: f32) -> f32 {
    let qx = (p[0] * p[0] + p[2] * p[2]).sqrt() - major;
    qx.hypot(p[1]) - minor
}

fn sd_octahedron_gem(p: [f32; 3], s: f32) -> f32 {
    let q = [p[0].abs(), p[1].abs(), p[2].abs()];
    (q[0] + q[1] + q[2] - s) * 0.577_350_26 - 0.02
}

fn eval_primitive(p: [f32; 3], shape: StageShape, mesh: Option<&ObjMesh>) -> f32 {
    match shape {
        StageShape::Sphere => v3_len(p) - 0.58,
        StageShape::RoundedBox => sd_round_box(p, [0.40, 0.40, 0.40], 0.08),
        StageShape::Bottle => sd_bottle(p),
        StageShape::Can => sd_can(p),
        StageShape::Cylinder => sd_rounded_cylinder(p, 0.37, 0.04, 0.54),
        StageShape::Torus => sd_torus(p, 0.42, 0.16),
        StageShape::DisplayCard => sd_round_box(p, [0.48, 0.60, 0.035], 0.025),
        StageShape::CustomObj => match mesh {
            Some(m) => sdf_obj_mesh(p, m),
            None => sd_octahedron_gem(p, 0.78),
        },
    }
}

#[inline]
fn smin_poly(a: f32, b: f32, k: f32) -> f32 {
    let kk = k.max(1e-4);
    let h = (0.5 + 0.5 * (b - a) / kk).clamp(0.0, 1.0);
    b + (a - b) * h - kk * h * (1.0 - h)
}

#[inline]
fn smax_poly(a: f32, b: f32, k: f32) -> f32 {
    -smin_poly(-a, -b, k)
}

struct SceneCtx<'a> {
    params: Stage3dParams,
    rot_inv: Mat3,
    rot_fwd: Mat3,
    mesh: Option<&'a ObjMesh>,
    probe: EquirectProbe,
    backdrop_flat: Vec<f32>,
    backdrop_dim: (usize, usize),
    decal_flat: Vec<f32>,
    decal_dim: (usize, usize),
}

impl<'a> SceneCtx<'a> {
    fn eval_local(&self, p_local: [f32; 3]) -> f32 {
        let d1 = eval_primitive(p_local, self.params.shape, self.mesh);
        match self.params.sdf_op {
            SdfBooleanOp::None => d1,
            op => {
                let p2 = v3_sub(p_local, self.params.secondary_offset);
                let d2 = eval_primitive(p2, self.params.secondary_shape, self.mesh);
                match op {
                    SdfBooleanOp::None => d1,
                    SdfBooleanOp::SmoothUnion => smin_poly(d1, d2, self.params.smooth_k),
                    SdfBooleanOp::SmoothSubtract => smax_poly(d1, -d2, self.params.smooth_k),
                    SdfBooleanOp::SmoothIntersect => smax_poly(d1, d2, self.params.smooth_k),
                }
            }
        }
    }

    #[inline]
    fn world_to_local(&self, p_world: [f32; 3]) -> [f32; 3] {
        let inv_s = self.params.scale.recip();
        self.rot_inv.mul_vec(v3_scale(p_world, inv_s))
    }

    #[inline]
    fn eval_world(&self, p_world: [f32; 3]) -> f32 {
        self.eval_local(self.world_to_local(p_world)) * self.params.scale
    }

    /// Tetrahedral 4-tap SDF normal + Laplacian curvature in world space.
    fn normal_and_curvature(&self, p_world: [f32; 3], d_center: f32) -> ([f32; 3], f32) {
        let eps = 0.0022 * self.params.scale;
        let k0 = [1.0f32, -1.0, -1.0];
        let k1 = [-1.0f32, -1.0, 1.0];
        let k2 = [-1.0f32, 1.0, -1.0];
        let k3 = [1.0f32, 1.0, 1.0];

        let d0 = self.eval_world(v3_add(p_world, v3_scale(k0, eps)));
        let d1 = self.eval_world(v3_add(p_world, v3_scale(k1, eps)));
        let d2 = self.eval_world(v3_add(p_world, v3_scale(k2, eps)));
        let d3 = self.eval_world(v3_add(p_world, v3_scale(k3, eps)));

        let grad = v3_add(v3_add(v3_scale(k0, d0), v3_scale(k1, d1)), v3_add(v3_scale(k2, d2), v3_scale(k3, d3)));
        let normal = v3_norm(grad);
        let lap = ((d0 + d1 + d2 + d3) - 4.0 * d_center) / (4.0 * eps).max(1e-5);
        (normal, lap.clamp(-2.0, 2.0))
    }

    /// 3-step SDF Ambient Occlusion along surface normal.
    fn eval_sdf_ao(&self, p_world: [f32; 3], normal: [f32; 3]) -> f32 {
        let mut occ = 0.0f32;
        let mut weight = 1.0f32;
        for step_idx in 1..=3 {
            let h = (step_idx as f32) * 0.065 * self.params.scale;
            let d = self.eval_world(v3_add(p_world, v3_scale(normal, h)));
            occ += (h - d).max(0.0) * weight;
            weight *= 0.55;
        }
        (1.0 - 2.6 * occ / self.params.scale).clamp(0.08, 1.0)
    }

    /// Soft SDF raymarched cast shadow factor in `[0, 1]` (`1` = fully lit, `0` = occluded).
    fn soft_shadow(&self, origin: [f32; 3], dir: [f32; 3], max_t: f32, penumbra_k: f32) -> f32 {
        let mut res = 1.0f32;
        let mut t = 0.025 * self.params.scale;
        for _ in 0..28 {
            if t >= max_t {
                break;
            }
            let h = self.eval_world(v3_add(origin, v3_scale(dir, t)));
            if h < 0.0008 {
                return 0.0;
            }
            res = res.min(penumbra_k * h / t.max(1e-4));
            t += h.clamp(0.015, 0.22);
        }
        res.clamp(0.0, 1.0)
    }
}

/// Raymarch result along one camera ray.
struct RayHit {
    hit: bool,
    t: f32,
    d_last: f32,
    min_d: f32,
}

fn raymarch_object(scene: &SceneCtx<'_>, ro: [f32; 3], rd: [f32; 3]) -> RayHit {
    let bound_r = 1.55 * scene.params.scale;
    // Analytic ray-sphere bounding check around origin to skip empty sky rays immediately!
    let b = v3_dot(ro, rd);
    let c = v3_dot(ro, ro) - bound_r * bound_r;
    let disc = b * b - c;
    if disc < 0.0 {
        return RayHit { hit: false, t: 0.0, d_last: 10.0, min_d: (-disc).sqrt() };
    }
    let sqrt_d = disc.sqrt();
    let mut t = (-b - sqrt_d).max(0.01);
    let t_end = (-b + sqrt_d).min(12.0);
    if t_end <= 0.0 {
        return RayHit { hit: false, t: 0.0, d_last: 10.0, min_d: 10.0 };
    }

    let mut min_d = f32::MAX;
    let mut d_last = 1.0f32;
    for _ in 0..64 {
        if t > t_end {
            break;
        }
        let p = v3_add(ro, v3_scale(rd, t));
        let d = scene.eval_world(p);
        d_last = d;
        min_d = min_d.min(d.max(0.0));
        if d < 0.0012 {
            return RayHit { hit: true, t, d_last: d, min_d: 0.0 };
        }
        t += d.max(0.002);
    }
    RayHit { hit: false, t, d_last, min_d }
}

/// Procedural 3D value hash for marble veins & brushed grooves in local object space.
#[inline]
fn hash3(ix: i32, iy: i32, iz: i32) -> f32 {
    let mut n = (ix as u32).wrapping_mul(73_856_093) ^ (iy as u32).wrapping_mul(19_349_663) ^ (iz as u32).wrapping_mul(83_492_791);
    n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
    ((n ^ (n >> 16)) & 0x00FF_FFFF) as f32 / 16_777_215.0
}

fn value_noise3(p: [f32; 3]) -> f32 {
    let (ix, iy, iz) = (p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32);
    let (fx, fy, fz) = (p[0] - ix as f32, p[1] - iy as f32, p[2] - iz as f32);
    let (ux, uy, uz) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy), fz * fz * (3.0 - 2.0 * fz));
    let c000 = hash3(ix, iy, iz);
    let c100 = hash3(ix + 1, iy, iz);
    let c010 = hash3(ix, iy + 1, iz);
    let c110 = hash3(ix + 1, iy + 1, iz);
    let c001 = hash3(ix, iy, iz + 1);
    let c101 = hash3(ix + 1, iy, iz + 1);
    let c011 = hash3(ix, iy + 1, iz + 1);
    let c111 = hash3(ix + 1, iy + 1, iz + 1);
    let x00 = c000 + (c100 - c000) * ux;
    let x10 = c010 + (c110 - c010) * ux;
    let x01 = c001 + (c101 - c001) * ux;
    let x11 = c011 + (c111 - c011) * ux;
    let y0 = x00 + (x10 - x00) * uy;
    let y1 = x01 + (x11 - x01) * uy;
    y0 + (y1 - y0) * uz
}

fn fbm3(p: [f32; 3]) -> f32 {
    let n1 = value_noise3(p);
    let n2 = value_noise3(v3_scale(p, 2.07));
    let n3 = value_noise3(v3_scale(p, 4.19));
    0.58 * n1 + 0.28 * n2 + 0.14 * n3
}

/// UV mapping of local 3D surface point onto `[0, 1] × [0, 1]` for Active Layer Decal wrapping.
fn local_surface_uv(p_local: [f32; 3], shape: StageShape) -> Option<[f32; 2]> {
    match shape {
        StageShape::DisplayCard | StageShape::RoundedBox => {
            // Front-focused planar wrap
            let u = (p_local[0] / 0.90 + 0.5).clamp(0.0, 1.0);
            let v = (0.5 - p_local[1] / 1.10).clamp(0.0, 1.0);
            Some([u, v])
        }
        StageShape::Sphere => {
            let n = v3_norm(p_local);
            let phi = n[0].atan2(n[2]);
            let theta = n[1].clamp(-1.0, 1.0).asin();
            Some([(phi / TAU + 0.5).clamp(0.0, 1.0), (0.5 - theta / PI).clamp(0.0, 1.0)])
        }
        _ => {
            // Cylindrical wrap around Y axis (Bottle, Can, Cylinder, Torus, CustomObj)
            let phi = p_local[0].atan2(p_local[2]);
            let u = (phi / (PI * 1.35) + 0.5).clamp(0.0, 1.0);
            let v = (0.5 - p_local[1] / 1.12).clamp(0.0, 1.0);
            if (-0.68 * PI..=0.68 * PI).contains(&phi) { Some([u, v]) } else { None }
        }
    }
}

fn evaluate_stage_material(scene: &SceneCtx<'_>, p_local: [f32; 3], mut normal: [f32; 3], curvature: f32, ao: f32) -> PbrSurfacePoint {
    let user_albedo = srgb3_to_linear(scene.params.base_color);
    let mut albedo_lin = user_albedo;
    let mut metallic = scene.params.metallic;
    let mut roughness = scene.params.roughness;

    match scene.params.material {
        StageMaterialPreset::MattePlastic => {
            metallic = (metallic * 0.15).clamp(0.0, 0.2);
            roughness = roughness.max(0.52);
        }
        StageMaterialPreset::BrushedMetal => {
            albedo_lin = v3_lerp([0.68, 0.71, 0.75], user_albedo, 0.35);
            metallic = metallic.max(0.92);
            let groove = (p_local[1] * 140.0 + fbm3(v3_scale(p_local, 12.0)) * 4.0).sin() * 0.04;
            let n_pert = scene.rot_fwd.mul_vec([groove, 0.0, 0.0]);
            normal = v3_norm(v3_add(normal, n_pert));
            roughness = (roughness * 0.85).clamp(0.14, 0.48);
        }
        StageMaterialPreset::PolishedGold => {
            albedo_lin = [0.98, 0.74, 0.28];
            metallic = 1.0;
            roughness = roughness.clamp(0.06, 0.26);
        }
        StageMaterialPreset::FrostedGlass => {
            albedo_lin = v3_lerp([0.86, 0.93, 0.98], user_albedo, 0.35);
            metallic = 0.0;
            roughness = roughness.clamp(0.18, 0.45);
        }
        StageMaterialPreset::Ceramic => {
            metallic = 0.0;
            roughness = roughness.clamp(0.06, 0.22);
        }
        StageMaterialPreset::Marble => {
            let n = fbm3(v3_scale(p_local, 5.2));
            let vein = ((p_local[0] * 7.5 + p_local[1] * 4.2 + n * 6.5).sin().abs()).powf(0.35);
            albedo_lin = v3_lerp([0.14, 0.16, 0.19], v3_lerp([0.86, 0.85, 0.82], user_albedo, 0.35), vein);
            metallic = 0.0;
            roughness = (0.12 + (1.0 - vein) * 0.22).clamp(0.08, 0.5);
        }
        StageMaterialPreset::WeatheredCopper => {
            let pat = fbm3(v3_scale(p_local, 6.5));
            let cavity_pat = ((1.0 - ao) * 1.4 + (-curvature).max(0.0) * 0.8 + pat * 0.45).clamp(0.0, 1.0);
            let copper = [0.92, 0.42, 0.28];
            let verdigris = [0.16, 0.52, 0.40];
            albedo_lin = v3_lerp(copper, verdigris, cavity_pat);
            metallic = (1.0 - cavity_pat * 0.92).clamp(0.05, 1.0);
            roughness = (0.22 + cavity_pat * 0.62).clamp(0.12, 0.92);
        }
        StageMaterialPreset::ActiveLayerDecal => {
            let (dw, dh) = scene.decal_dim;
            if dw > 0
                && dh > 0
                && let Some([u, v]) = local_surface_uv(p_local, scene.params.shape)
            {
                let sx = u * (dw.saturating_sub(1) as f32);
                let sy = v * (dh.saturating_sub(1) as f32);
                let dr = bilinear(&scene.decal_flat, dw, dh, 4, 0, sx, sy);
                let dg = bilinear(&scene.decal_flat, dw, dh, 4, 1, sx, sy);
                let db = bilinear(&scene.decal_flat, dw, dh, 4, 2, sx, sy);
                let da = bilinear(&scene.decal_flat, dw, dh, 4, 3, sx, sy).clamp(0.0, 1.0);
                let decal_lin = srgb3_to_linear([dr, dg, db]);
                albedo_lin = v3_lerp(user_albedo, decal_lin, da);
                roughness = roughness * (1.0 - 0.25 * da) + 0.22 * da;
            }
        }
    }

    // Substance Painter curvature edge wear & cavity grime (`weathering`)
    let wear = scene.params.weathering;
    if wear > 1e-3 {
        let noise_w = fbm3(v3_scale(p_local, 9.0));
        let edge_mask = ((curvature * 1.35 + (noise_w - 0.45) * 0.55) * wear * 1.8).clamp(0.0, 1.0);
        let dirt_mask = (((1.0 - ao) * 1.25 + (-curvature).max(0.0) * 0.6) * wear).clamp(0.0, 0.85);
        // Exposed bright brushed alloy on convex edges; dark dust in crevices
        albedo_lin = v3_lerp(albedo_lin, [0.76, 0.78, 0.80], edge_mask * 0.75);
        albedo_lin = v3_lerp(albedo_lin, [0.07, 0.065, 0.06], dirt_mask * 0.65);
        metallic = (metallic + edge_mask * 0.65).clamp(0.0, 1.0);
        roughness = (roughness + dirt_mask * 0.35 - edge_mask * 0.12).clamp(0.05, 1.0);
    }

    PbrSurfacePoint { albedo_lin, normal, roughness, metallic, ao }
}

fn shade_ray_sample(scene: &SceneCtx<'_>, ro: [f32; 3], rd: [f32; 3], screen_uv: [f32; 2]) -> ([f32; 4], f32) {
    let hit = raymarch_object(scene, ro, rd);
    if hit.hit {
        let p_world = v3_add(ro, v3_scale(rd, hit.t));
        let (normal, curvature) = scene.normal_and_curvature(p_world, hit.d_last);
        let ao = scene.eval_sdf_ao(p_world, normal);
        let p_local = scene.world_to_local(p_world);
        let surf = evaluate_stage_material(scene, p_local, normal, curvature, ao);

        let view_dir = v3_scale(rd, -1.0);
        let shadow = scene.soft_shadow(v3_add(p_world, v3_scale(normal, 0.008 * scene.params.scale)), scene.params.light_dir, 4.5, 10.0);
        let key_rad = v3_scale([1.0, 0.98, 0.94], scene.params.light_intensity * shadow);
        let direct = shade_ggx_direct(&surf, view_dir, scene.params.light_dir, key_rad);
        let ibl = scene.probe.shade_ibl(&surf, view_dir);

        let mut total_lin = v3_add(direct, ibl);
        let mut out_alpha = 1.0f32;

        // FrostedGlass: physically plausible screen-space backdrop refraction + Fresnel rim!
        if scene.params.material == StageMaterialPreset::FrostedGlass {
            let (bw, bh) = scene.backdrop_dim;
            let refracted_lin = if bw > 0 && bh > 0 {
                let rx = ((screen_uv[0] - normal[0] * 0.065).clamp(0.0, 1.0)) * (bw.saturating_sub(1) as f32);
                let ry = ((screen_uv[1] + normal[1] * 0.065).clamp(0.0, 1.0)) * (bh.saturating_sub(1) as f32);
                let br = bilinear(&scene.backdrop_flat, bw, bh, 4, 0, rx, ry);
                let bg = bilinear(&scene.backdrop_flat, bw, bh, 4, 1, rx, ry);
                let bb = bilinear(&scene.backdrop_flat, bw, bh, 4, 2, rx, ry);
                srgb3_to_linear([br, bg, bb])
            } else {
                scene.probe.sample_dir(rd, surf.roughness)
            };
            let fresnel = (1.0 - v3_dot(normal, view_dir).clamp(0.0, 1.0)).powi(3);
            let tinted = [refracted_lin[0] * surf.albedo_lin[0], refracted_lin[1] * surf.albedo_lin[1], refracted_lin[2] * surf.albedo_lin[2]];
            total_lin = v3_add(v3_scale(tinted, 0.72 * (1.0 - fresnel)), total_lin);
            out_alpha = (0.78 + 0.22 * fresnel).clamp(0.0, 1.0);
        }

        let mapped = linear3_to_srgb(tone_map_filmic(total_lin));
        return ([mapped[0], mapped[1], mapped[2], out_alpha], 0.0);
    }

    // Ground-plane Shadow Catcher (`y = ground_y`) where primary ray misses object!
    if scene.params.shadow_catcher > 1e-4 && rd[1] < -1e-4 {
        let t_ground = (scene.params.ground_y * scene.params.scale - ro[1]) / rd[1];
        if (0.05..14.0).contains(&t_ground) {
            let gp = v3_add(ro, v3_scale(rd, t_ground));
            let radial = (gp[0] * gp[0] + gp[2] * gp[2]).sqrt() / scene.params.scale.max(0.2);
            if radial < 2.8 {
                let lit = scene.soft_shadow([gp[0], gp[1] + 0.008, gp[2]], scene.params.light_dir, 4.5, 7.5);
                let d_obj = scene.eval_world([gp[0], gp[1] + 0.02 * scene.params.scale, gp[2]]).max(0.0);
                let contact = (1.0 - (d_obj / (0.28 * scene.params.scale)).clamp(0.0, 1.0)).powi(2) * 0.55;
                let fade = (1.0 - (radial / 2.8).powi(2)).clamp(0.0, 1.0);
                let shadow_amt = (((1.0 - lit) * 0.78 + contact) * fade * scene.params.shadow_catcher).clamp(0.0, 0.92);
                return ([0.0, 0.0, 0.0, shadow_amt], hit.min_d);
            }
        }
    }

    ([0.0, 0.0, 0.0, 0.0], hit.min_d)
}

fn flatten_rgba(opt: Option<(&[[f32; 4]], usize, usize)>) -> (Vec<f32>, (usize, usize)) {
    let Some((px, w, h)) = opt else {
        return (Vec::new(), (0, 0));
    };
    let n = w.saturating_mul(h);
    if n == 0 || px.len() < n {
        return (Vec::new(), (0, 0));
    }
    let mut flat = Vec::with_capacity(n * 4);
    for q in px.iter().take(n) {
        flat.extend_from_slice(&[q[0].clamp(0.0, 1.0), q[1].clamp(0.0, 1.0), q[2].clamp(0.0, 1.0), q[3].clamp(0.0, 1.0)]);
    }
    (flat, (w, h))
}

#[inline]
fn camera_ray(px_x: f32, px_y: f32, w: f32, h: f32, center: [f32; 2], tan_half_fov: f32) -> ([f32; 3], [f32; 3]) {
    let min_dim = w.min(h).max(1.0);
    let ndc_x = (px_x - center[0] * w) / (0.5 * min_dim);
    let ndc_y = (center[1] * h - px_y) / (0.5 * min_dim);
    let ro = [0.0, 0.06, 2.55];
    let rd = v3_norm([ndc_x * tan_half_fov, ndc_y * tan_half_fov - 0.02, -1.0]);
    (ro, rd)
}

/// Renders a 3D Stager / SDF Clay scene into straight sRGB RGBA (`w × h`), with 2×2 silhouette
/// edge supersampling, Cook-Torrance GGX shading, backdrop-matched IBL, decal wrapping, and
/// semi-transparent ground-plane shadow catcher alpha.
pub fn render_stage_3d_rgba(w: usize, h: usize, params: &Stage3dParams, inputs: &Stage3dInputs<'_>) -> Vec<[f32; 4]> {
    let n = w.saturating_mul(h);
    if n == 0 {
        return Vec::new();
    }
    let p = params.sanitized();
    let probe = if p.match_background_ibl
        && let Some((bg_px, bw, bh)) = inputs.backdrop_rgba
    {
        let env = extract_ibl_environment_rgba(bg_px, bw, bh, 96, 48);
        EquirectProbe::from_equirect_rgba(&env, 96, 48)
    } else {
        EquirectProbe::neutral_studio()
    };

    let (backdrop_flat, backdrop_dim) = flatten_rgba(inputs.backdrop_rgba);
    let (decal_flat, decal_dim) = flatten_rgba(inputs.decal_rgba);
    let rot_fwd = Mat3::from_euler_deg(p.rot_deg);
    let rot_inv = rot_fwd.transpose();

    let scene = SceneCtx { params: p, rot_inv, rot_fwd, mesh: inputs.custom_mesh, probe, backdrop_flat, backdrop_dim, decal_flat, decal_dim };

    let wf = w as f32;
    let hf = h as f32;
    let tan_half_fov = (0.5 * p.fov_deg.to_radians()).tan();
    let edge_band = (4.5 / wf.min(hf).max(16.0)) * p.scale;

    let mut out = vec![[0.0f32; 4]; n];
    par_rows(&mut out, w, 1, |y, row| {
        for (x, dst) in row.iter_mut().enumerate() {
            let cx = x as f32 + 0.5;
            let cy = y as f32 + 0.5;
            let (ro, rd) = camera_ray(cx, cy, wf, hf, p.center, tan_half_fov);
            let uv = [cx / wf, cy / hf];
            let (c0, min_d) = shade_ray_sample(&scene, ro, rd, uv);

            // Check whether ray grazed near object silhouette (`min_d < edge_band`) or hit near grazing angle:
            // run 2x2 premultiplied-alpha supersampling on silhouette border pixels!
            let needs_ssaa = min_d < edge_band
                && (min_d > 0.0 || {
                    // Probe one neighbor ray to see if pixel straddles object silhouette
                    let (ro_r, rd_r) = camera_ray(cx + 0.65, cy, wf, hf, p.center, tan_half_fov);
                    let (ro_u, rd_u) = camera_ray(cx, cy + 0.65, wf, hf, p.center, tan_half_fov);
                    !raymarch_object(&scene, ro_r, rd_r).hit || !raymarch_object(&scene, ro_u, rd_u).hit
                });

            if !needs_ssaa {
                *dst = c0;
                continue;
            }

            let offsets = [(-0.25f32, -0.25f32), (0.25, -0.25), (-0.25, 0.25), (0.25, 0.25)];
            let mut pm = [0.0f32; 4];
            for (ox, oy) in offsets {
                let sx = cx + ox;
                let sy = cy + oy;
                let (s_ro, s_rd) = camera_ray(sx, sy, wf, hf, p.center, tan_half_fov);
                let (sc, _) = shade_ray_sample(&scene, s_ro, s_rd, [sx / wf, sy / hf]);
                let a = sc[3];
                pm[0] += sc[0] * a;
                pm[1] += sc[1] * a;
                pm[2] += sc[2] * a;
                pm[3] += a;
            }
            let out_a = (pm[3] * 0.25).clamp(0.0, 1.0);
            if out_a > 1e-5 {
                let inv_a = pm[3].recip();
                *dst = [(pm[0] * inv_a).clamp(0.0, 1.0), (pm[1] * inv_a).clamp(0.0, 1.0), (pm[2] * inv_a).clamp(0.0, 1.0), out_a];
            } else {
                *dst = [0.0; 4];
            }
        }
    });
    out
}

/// Renders a 3D Stager / SDF Clay scene directly into a depth-agnostic `Surface` (`8`/`16`/`32f`).
pub fn render_stage_3d_surface(fmt: PixelFormat, rect: Rect, params: &Stage3dParams, inputs: &Stage3dInputs<'_>) -> Surface {
    if rect.is_empty() {
        return Surface::new(fmt);
    }
    let w = rect.width() as usize;
    let h = rect.height() as usize;
    let ch = fmt.channels();
    let rgba = render_stage_3d_rgba(w, h, params, inputs);
    let mut raw = vec![0.0f32; w * h * ch];
    for (q, dst) in rgba.iter().zip(raw.chunks_exact_mut(ch)) {
        from_rgba_into(&fmt, *q, dst);
    }
    let mut surf = Surface::new(fmt);
    surf.write_region(rect, &raw);
    surf
}
