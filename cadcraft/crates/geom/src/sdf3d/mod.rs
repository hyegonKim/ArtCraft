//! Signed distance field (SDF) and constructive solid geometry (CSG) 3D modeling,
//! watertight Dual Surface Nets triangle mesh extraction, STL/OBJ export, and
//! orthographic hidden-line 2D CAD view projection.

mod mesh;
mod project2d;

use serde::{Deserialize, Serialize};

use crate::{Vec2, Vec3};

pub use mesh::{TriMesh3d, extract_isosurface, write_obj, write_stl_ascii, write_stl_binary};
pub use project2d::{OrthoView3d, Projected2d, project_mesh_to_2d, project_solid_to_2d};

/// Maximum CSG / transform tree depth evaluated before clamping to outside distance.
pub const MAX_CSG_DEPTH: usize = 64;

/// Maximum coordinate magnitude accepted by 3D solid modeling helpers.
pub const MAX_COORD_MAGNITUDE: f64 = 1e6;

/// Axis-aligned 3D bounding box.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bounds3 {
    pub min: Vec3,
    pub max: Vec3,
}

impl Bounds3 {
    pub const fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub const fn empty() -> Self {
        Self { min: Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY), max: Vec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY) }
    }

    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x || self.min.y > self.max.y || self.min.z > self.max.z
    }

    pub fn is_finite(&self) -> bool {
        !self.is_empty() && self.min.is_finite() && self.max.is_finite()
    }

    pub fn include(&mut self, p: Vec3) {
        if !p.is_finite() {
            return;
        }
        self.min = Vec3::new(self.min.x.min(p.x), self.min.y.min(p.y), self.min.z.min(p.z));
        self.max = Vec3::new(self.max.x.max(p.x), self.max.y.max(p.y), self.max.z.max(p.z));
    }

    pub fn union(self, other: Self) -> Self {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        Self {
            min: Vec3::new(self.min.x.min(other.min.x), self.min.y.min(other.min.y), self.min.z.min(other.min.z)),
            max: Vec3::new(self.max.x.max(other.max.x), self.max.y.max(other.max.y), self.max.z.max(other.max.z)),
        }
    }

    pub fn intersect(self, other: Self) -> Self {
        if self.is_empty() || other.is_empty() {
            return Self::empty();
        }
        let out = Self {
            min: Vec3::new(self.min.x.max(other.min.x), self.min.y.max(other.min.y), self.min.z.max(other.min.z)),
            max: Vec3::new(self.max.x.min(other.max.x), self.max.y.min(other.max.y), self.max.z.min(other.max.z)),
        };
        if out.is_empty() { Self::empty() } else { out }
    }

    pub fn padded(self, margin: f64) -> Self {
        if !self.is_finite() || !margin.is_finite() {
            return self;
        }
        let m = margin.max(0.0);
        let delta = Vec3::new(m, m, m);
        Self { min: self.min - delta, max: self.max + delta }
    }

    pub fn size(&self) -> Vec3 {
        if !self.is_finite() {
            return Vec3::ZERO;
        }
        self.max - self.min
    }

    pub fn center(&self) -> Vec3 {
        if !self.is_finite() {
            return Vec3::ZERO;
        }
        Vec3::new(self.min.x.midpoint(self.max.x), self.min.y.midpoint(self.max.y), self.min.z.midpoint(self.max.z))
    }

    pub fn max_extent(&self) -> f64 {
        let s = self.size();
        s.x.max(s.y).max(s.z)
    }

    pub fn corners(&self) -> [Vec3; 8] {
        let (a, b) = (self.min, self.max);
        [
            Vec3::new(a.x, a.y, a.z),
            Vec3::new(b.x, a.y, a.z),
            Vec3::new(a.x, b.y, a.z),
            Vec3::new(b.x, b.y, a.z),
            Vec3::new(a.x, a.y, b.z),
            Vec3::new(b.x, a.y, b.z),
            Vec3::new(a.x, b.y, b.z),
            Vec3::new(b.x, b.y, b.z),
        ]
    }
}

fn unit_scale() -> Vec3 {
    Vec3::new(1.0, 1.0, 1.0)
}

fn safe_scale_axis(s: f64) -> f64 {
    if !s.is_finite() {
        1.0
    } else if s.abs() < 1e-6 {
        if s.is_sign_negative() { -1e-6 } else { 1e-6 }
    } else {
        s
    }
}

fn rotate_x(v: Vec3, angle: f64) -> Vec3 {
    let (s, c) = angle.sin_cos();
    Vec3::new(v.x, v.y * c - v.z * s, v.y * s + v.z * c)
}

fn rotate_y(v: Vec3, angle: f64) -> Vec3 {
    let (s, c) = angle.sin_cos();
    Vec3::new(v.x * c + v.z * s, v.y, -v.x * s + v.z * c)
}

fn rotate_z(v: Vec3, angle: f64) -> Vec3 {
    let (s, c) = angle.sin_cos();
    Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
}

/// 3D TRS transform (scale -> Euler XYZ rotation in radians -> translation).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transform3d {
    #[serde(default)]
    pub translation: Vec3,
    /// Euler XYZ rotation angles in radians.
    #[serde(default)]
    pub rotation: Vec3,
    #[serde(default = "unit_scale")]
    pub scale: Vec3,
}

impl Default for Transform3d {
    fn default() -> Self {
        Self::identity()
    }
}

impl Transform3d {
    pub const fn identity() -> Self {
        Self { translation: Vec3::ZERO, rotation: Vec3::ZERO, scale: Vec3::new(1.0, 1.0, 1.0) }
    }

    pub const fn translate(translation: Vec3) -> Self {
        Self { translation, ..Self::identity() }
    }

    pub fn apply_point(&self, local: Vec3) -> Vec3 {
        let sx = safe_scale_axis(self.scale.x);
        let sy = safe_scale_axis(self.scale.y);
        let sz = safe_scale_axis(self.scale.z);
        let scaled = Vec3::new(local.x * sx, local.y * sy, local.z * sz);
        let rx = if self.rotation.x.is_finite() { self.rotation.x } else { 0.0 };
        let ry = if self.rotation.y.is_finite() { self.rotation.y } else { 0.0 };
        let rz = if self.rotation.z.is_finite() { self.rotation.z } else { 0.0 };
        let rotated = rotate_z(rotate_y(rotate_x(scaled, rx), ry), rz);
        let tx = if self.translation.is_finite() { self.translation } else { Vec3::ZERO };
        rotated + tx
    }

    pub fn inverse_point(&self, world: Vec3) -> Vec3 {
        let tx = if self.translation.is_finite() { self.translation } else { Vec3::ZERO };
        let shifted = world - tx;
        let rx = if self.rotation.x.is_finite() { self.rotation.x } else { 0.0 };
        let ry = if self.rotation.y.is_finite() { self.rotation.y } else { 0.0 };
        let rz = if self.rotation.z.is_finite() { self.rotation.z } else { 0.0 };
        let unrotated = rotate_x(rotate_y(rotate_z(shifted, -rz), -ry), -rx);
        let sx = safe_scale_axis(self.scale.x);
        let sy = safe_scale_axis(self.scale.y);
        let sz = safe_scale_axis(self.scale.z);
        Vec3::new(unrotated.x / sx, unrotated.y / sy, unrotated.z / sz)
    }

    /// Minimum absolute axis scale, keeping signed distance Lipschitz-conservative.
    pub fn min_scale(&self) -> f64 {
        let sx = safe_scale_axis(self.scale.x).abs();
        let sy = safe_scale_axis(self.scale.y).abs();
        let sz = safe_scale_axis(self.scale.z).abs();
        sx.min(sy).min(sz)
    }

    pub fn transform_bounds(&self, local: Bounds3) -> Bounds3 {
        if !local.is_finite() {
            return Bounds3::empty();
        }
        let mut out = Bounds3::empty();
        for corner in local.corners() {
            out.include(self.apply_point(corner));
        }
        out
    }

    pub fn then(self, outer: Transform3d) -> Transform3d {
        Transform3d {
            translation: outer.apply_point(self.translation),
            rotation: self.rotation + outer.rotation,
            scale: Vec3::new(self.scale.x * outer.scale.x, self.scale.y * outer.scale.y, self.scale.z * outer.scale.z),
        }
    }
}

/// Analytic 3D signed-distance primitives.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SdfPrimitive {
    /// Axis-aligned box centered at the origin with positive `half_extents`.
    Box3d { half_extents: Vec3 },
    /// Sphere centered at the origin with `radius`.
    Sphere3d { radius: f64 },
    /// Capped cylinder along Z centered at the origin (`z` in `[-height/2, +height/2]`).
    Cylinder3d { radius: f64, height: f64 },
    /// Capped cone or frustum along Z (`z = -height/2` has `base_radius`, `z = +height/2` has `top_radius`).
    Cone3d { base_radius: f64, top_radius: f64, height: f64 },
    /// Torus in the XY plane centered at the origin (`major_radius` ring, `minor_radius` tube).
    Torus3d { major_radius: f64, minor_radius: f64 },
    /// Capsule swept along segment `a..b` with `radius`.
    Capsule3d { a: Vec3, b: Vec3, radius: f64 },
}

impl SdfPrimitive {
    /// Exact Euclidean signed distance at local point `p` (`< 0` inside, `0` on surface, `> 0` outside).
    pub fn eval(&self, p: Vec3) -> f64 {
        match *self {
            SdfPrimitive::Box3d { half_extents } => sdf_box(p, half_extents),
            SdfPrimitive::Sphere3d { radius } => p.len() - radius.max(1e-6),
            SdfPrimitive::Cylinder3d { radius, height } => sdf_cylinder(p, radius, height),
            SdfPrimitive::Cone3d { base_radius, top_radius, height } => sdf_cone(p, base_radius, top_radius, height),
            SdfPrimitive::Torus3d { major_radius, minor_radius } => sdf_torus(p, major_radius, minor_radius),
            SdfPrimitive::Capsule3d { a, b, radius } => sdf_capsule(p, a, b, radius),
        }
    }

    /// Tight local axis-aligned bounding box of the zero-isosurface.
    pub fn local_bounds(&self) -> Bounds3 {
        match *self {
            SdfPrimitive::Box3d { half_extents } => {
                let h = Vec3::new(half_extents.x.abs().max(1e-6), half_extents.y.abs().max(1e-6), half_extents.z.abs().max(1e-6));
                Bounds3::new(-h, h)
            }
            SdfPrimitive::Sphere3d { radius } => {
                let r = radius.abs().max(1e-6);
                let v = Vec3::new(r, r, r);
                Bounds3::new(-v, v)
            }
            SdfPrimitive::Cylinder3d { radius, height } => {
                let r = radius.abs().max(1e-6);
                let hh = (height.abs() * 0.5).max(1e-6);
                Bounds3::new(Vec3::new(-r, -r, -hh), Vec3::new(r, r, hh))
            }
            SdfPrimitive::Cone3d { base_radius, top_radius, height } => {
                let r = base_radius.abs().max(top_radius.abs()).max(1e-6);
                let hh = (height.abs() * 0.5).max(1e-6);
                Bounds3::new(Vec3::new(-r, -r, -hh), Vec3::new(r, r, hh))
            }
            SdfPrimitive::Torus3d { major_radius, minor_radius } => {
                let xy = major_radius.abs() + minor_radius.abs().max(1e-6);
                let z = minor_radius.abs().max(1e-6);
                Bounds3::new(Vec3::new(-xy, -xy, -z), Vec3::new(xy, xy, z))
            }
            SdfPrimitive::Capsule3d { a, b, radius } => {
                let r = radius.abs().max(1e-6);
                let mut bounds = Bounds3::empty();
                bounds.include(a);
                bounds.include(b);
                bounds.padded(r)
            }
        }
    }
}

fn sdf_box(p: Vec3, half_extents: Vec3) -> f64 {
    let hx = half_extents.x.abs().max(1e-6);
    let hy = half_extents.y.abs().max(1e-6);
    let hz = half_extents.z.abs().max(1e-6);
    let q = Vec3::new(p.x.abs() - hx, p.y.abs() - hy, p.z.abs() - hz);
    let outside = Vec3::new(q.x.max(0.0), q.y.max(0.0), q.z.max(0.0)).len();
    let inside = q.x.max(q.y.max(q.z)).min(0.0);
    outside + inside
}

fn sdf_cylinder(p: Vec3, radius: f64, height: f64) -> f64 {
    let r = radius.abs().max(1e-6);
    let hh = (height.abs() * 0.5).max(1e-6);
    let dr = p.x.hypot(p.y) - r;
    let dz = p.z.abs() - hh;
    dr.max(0.0).hypot(dz.max(0.0)) + dr.max(dz).min(0.0)
}

fn sdf_cone(p: Vec3, base_radius: f64, top_radius: f64, height: f64) -> f64 {
    let r1 = base_radius.max(0.0);
    let r2 = top_radius.max(0.0);
    let hh = (height.abs() * 0.5).max(1e-6);
    let q = Vec2::new(p.x.hypot(p.y), p.z);
    let k1 = Vec2::new(r2, hh);
    let k2 = Vec2::new(r2 - r1, 2.0 * hh);
    let cap_r = if q.y < 0.0 { r1 } else { r2 };
    let ca = Vec2::new(q.x - q.x.min(cap_r), q.y.abs() - hh);
    let denom = k2.len2().max(1e-18);
    let t = ((k1 - q).dot(k2) / denom).clamp(0.0, 1.0);
    let cb = q - k1 + k2 * t;
    let sign = if cb.x < 0.0 && ca.y < 0.0 { -1.0 } else { 1.0 };
    sign * ca.len().min(cb.len())
}

fn sdf_torus(p: Vec3, major_radius: f64, minor_radius: f64) -> f64 {
    let major = major_radius.abs().max(1e-6);
    let minor = minor_radius.abs().max(1e-6);
    let qx = p.x.hypot(p.y) - major;
    qx.hypot(p.z) - minor
}

fn sdf_capsule(p: Vec3, a: Vec3, b: Vec3, radius: f64) -> f64 {
    let r = radius.abs().max(1e-6);
    let pa = p - a;
    let ba = b - a;
    let denom = ba.dot(ba);
    let h = if denom > 1e-18 { (pa.dot(ba) / denom).clamp(0.0, 1.0) } else { 0.0 };
    (pa - ba * h).len() - r
}

/// Hard or clay-smooth boolean operation between two 3D solids.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CsgOp {
    Union,
    Subtract,
    Intersect,
}

impl CsgOp {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "union" | "add" | "merge" => Some(Self::Union),
            "subtract" | "sub" | "difference" | "cut" => Some(Self::Subtract),
            "intersect" | "int" | "common" => Some(Self::Intersect),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Union => "union",
            Self::Subtract => "subtract",
            Self::Intersect => "intersect",
        }
    }

    /// Combine distances `a` and `b` using exact boolean (`smooth_k <= 0`) or polynomial smooth-min (`smooth_k > 0`).
    pub fn combine(self, a: f64, b: f64, smooth_k: f64) -> f64 {
        if !smooth_k.is_finite() || smooth_k <= 1e-9 {
            return match self {
                Self::Union => a.min(b),
                Self::Subtract => a.max(-b),
                Self::Intersect => a.max(b),
            };
        }
        match self {
            Self::Union => poly_smin(a, b, smooth_k),
            Self::Subtract => -poly_smin(-a, b, smooth_k),
            Self::Intersect => -poly_smin(-a, -b, smooth_k),
        }
    }
}

/// C1-continuous quadratic polynomial smooth minimum.
pub fn poly_smin(a: f64, b: f64, k: f64) -> f64 {
    let k = k.max(1e-9);
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    (b * (1.0 - h) + a * h) - k * h * (1.0 - h)
}

/// Hierarchical CSG / SDF solid modeling tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "node", rename_all = "camelCase")]
pub enum Solid3dNode {
    Primitive {
        prim: SdfPrimitive,
        #[serde(default)]
        transform: Transform3d,
    },
    Boolean {
        op: CsgOp,
        #[serde(default)]
        smooth_k: f64,
        left: Box<Solid3dNode>,
        right: Box<Solid3dNode>,
    },
    Transform {
        transform: Transform3d,
        child: Box<Solid3dNode>,
    },
}

impl Solid3dNode {
    pub fn primitive(prim: SdfPrimitive) -> Self {
        Self::Primitive { prim, transform: Transform3d::identity() }
    }

    pub fn primitive_at(prim: SdfPrimitive, transform: Transform3d) -> Self {
        Self::Primitive { prim, transform }
    }

    pub fn boolean(op: CsgOp, smooth_k: f64, left: Solid3dNode, right: Solid3dNode) -> Self {
        Self::Boolean { op, smooth_k: if smooth_k.is_finite() { smooth_k.max(0.0) } else { 0.0 }, left: Box::new(left), right: Box::new(right) }
    }

    pub fn transformed(self, transform: Transform3d) -> Self {
        Self::Transform { transform, child: Box::new(self) }
    }

    /// Evaluate the signed distance at world point `p`.
    pub fn eval(&self, p: Vec3) -> f64 {
        if !p.is_finite() {
            return MAX_COORD_MAGNITUDE;
        }
        self.eval_depth(p, 0)
    }

    fn eval_depth(&self, p: Vec3, depth: usize) -> f64 {
        if depth > MAX_CSG_DEPTH || !p.is_finite() {
            return MAX_COORD_MAGNITUDE;
        }
        match self {
            Solid3dNode::Primitive { prim, transform } => prim.eval(transform.inverse_point(p)) * transform.min_scale(),
            Solid3dNode::Boolean { op, smooth_k, left, right } => {
                let da = left.eval_depth(p, depth + 1);
                let db = right.eval_depth(p, depth + 1);
                op.combine(da, db, *smooth_k)
            }
            Solid3dNode::Transform { transform, child } => child.eval_depth(transform.inverse_point(p), depth + 1) * transform.min_scale(),
        }
    }

    /// Tetrahedral finite-difference gradient (4 evaluations), unnormalized.
    pub fn gradient(&self, p: Vec3, eps: f64) -> Vec3 {
        let h = eps.abs().max(1e-6);
        let k0 = Vec3::new(1.0, -1.0, -1.0);
        let k1 = Vec3::new(-1.0, -1.0, 1.0);
        let k2 = Vec3::new(-1.0, 1.0, -1.0);
        let k3 = Vec3::new(1.0, 1.0, 1.0);
        let g = k0 * self.eval(p + k0 * h) + k1 * self.eval(p + k1 * h) + k2 * self.eval(p + k2 * h) + k3 * self.eval(p + k3 * h);
        g * (0.25 / h)
    }

    /// Unit outward surface normal at `p`.
    pub fn normal(&self, p: Vec3, eps: f64) -> Vec3 {
        let n = self.gradient(p, eps).normalized();
        if n == Vec3::ZERO { Vec3::Z } else { n }
    }

    /// Conservative world-space bounding box of the zero-isosurface.
    pub fn bounds(&self) -> Bounds3 {
        self.bounds_depth(0)
    }

    fn bounds_depth(&self, depth: usize) -> Bounds3 {
        if depth > MAX_CSG_DEPTH {
            return Bounds3::empty();
        }
        match self {
            Solid3dNode::Primitive { prim, transform } => transform.transform_bounds(prim.local_bounds()),
            Solid3dNode::Boolean { op, smooth_k, left, right } => {
                let lb = left.bounds_depth(depth + 1);
                let rb = right.bounds_depth(depth + 1);
                let pad = if smooth_k.is_finite() { smooth_k.max(0.0) } else { 0.0 };
                match op {
                    CsgOp::Union => lb.union(rb).padded(pad),
                    CsgOp::Subtract => lb,
                    CsgOp::Intersect => lb.intersect(rb).padded(pad * 0.25),
                }
            }
            Solid3dNode::Transform { transform, child } => transform.transform_bounds(child.bounds_depth(depth + 1)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Bounds3, CsgOp, OrthoView3d, SdfPrimitive, Solid3dNode, Transform3d, extract_isosurface, project_solid_to_2d, write_obj, write_stl_ascii,
        write_stl_binary,
    };
    use crate::Vec3;

    #[test]
    fn all_six_primitives_have_exact_surface_zero_and_sign() {
        let box_s = Solid3dNode::primitive(SdfPrimitive::Box3d { half_extents: Vec3::new(10.0, 6.0, 4.0) });
        assert!((box_s.eval(Vec3::new(10.0, 0.0, 0.0))).abs() < 1e-9);
        assert!(box_s.eval(Vec3::ZERO) < -3.9);
        assert!((box_s.eval(Vec3::new(13.0, 10.0, 4.0)) - 5.0).abs() < 1e-9);

        let sphere = Solid3dNode::primitive(SdfPrimitive::Sphere3d { radius: 5.0 });
        assert!((sphere.eval(Vec3::new(3.0, 4.0, 0.0))).abs() < 1e-9);
        assert!((sphere.eval(Vec3::ZERO) + 5.0).abs() < 1e-9);

        let cyl = Solid3dNode::primitive(SdfPrimitive::Cylinder3d { radius: 4.0, height: 10.0 });
        assert!((cyl.eval(Vec3::new(4.0, 0.0, 0.0))).abs() < 1e-9);
        assert!((cyl.eval(Vec3::new(0.0, 0.0, 5.0))).abs() < 1e-9);
        assert!(cyl.eval(Vec3::ZERO) < -3.9);

        let cone = Solid3dNode::primitive(SdfPrimitive::Cone3d { base_radius: 4.0, top_radius: 0.0, height: 8.0 });
        assert!((cone.eval(Vec3::new(4.0, 0.0, -4.0))).abs() < 1e-9);
        assert!((cone.eval(Vec3::new(0.0, 0.0, 4.0))).abs() < 1e-9);
        assert!(cone.eval(Vec3::ZERO) < -1.0);

        let torus = Solid3dNode::primitive(SdfPrimitive::Torus3d { major_radius: 8.0, minor_radius: 2.0 });
        assert!((torus.eval(Vec3::new(10.0, 0.0, 0.0))).abs() < 1e-9);
        assert!((torus.eval(Vec3::new(8.0, 0.0, 0.0)) + 2.0).abs() < 1e-9);
        assert!(torus.eval(Vec3::ZERO) > 5.9);

        let capsule = Solid3dNode::primitive(SdfPrimitive::Capsule3d { a: Vec3::new(-5.0, 0.0, 0.0), b: Vec3::new(5.0, 0.0, 0.0), radius: 2.0 });
        assert!((capsule.eval(Vec3::new(7.0, 0.0, 0.0))).abs() < 1e-9);
        assert!((capsule.eval(Vec3::ZERO) + 2.0).abs() < 1e-9);
    }

    #[test]
    fn smooth_clay_csg_blends_fillets_and_preserves_boolean_semantics() {
        let a = Solid3dNode::primitive_at(SdfPrimitive::Sphere3d { radius: 5.0 }, Transform3d::translate(Vec3::new(-3.0, 0.0, 0.0)));
        let b = Solid3dNode::primitive_at(SdfPrimitive::Sphere3d { radius: 5.0 }, Transform3d::translate(Vec3::new(3.0, 0.0, 0.0)));
        let hard_union = Solid3dNode::boolean(CsgOp::Union, 0.0, a.clone(), b.clone());
        let clay_union = Solid3dNode::boolean(CsgOp::Union, 2.5, a.clone(), b.clone());
        let seam = Vec3::new(0.0, 4.1, 0.0);
        // At (0, 4.1, 0), distance to (-3,0,0) is sqrt(9 + 16.81) = 5.0803 > 5 -> outside hard union, inside clay union!
        assert!(hard_union.eval(seam) > 0.05);
        assert!(clay_union.eval(seam) < -0.1, "smooth union adds clay fillet material in the crease");

        let cutter = Solid3dNode::primitive(SdfPrimitive::Cylinder3d { radius: 2.0, height: 20.0 });
        let drilled = Solid3dNode::boolean(
            CsgOp::Subtract,
            0.5,
            Solid3dNode::primitive(SdfPrimitive::Box3d { half_extents: Vec3::new(6.0, 6.0, 4.0) }),
            cutter,
        );
        assert!(drilled.eval(Vec3::ZERO) > 1.5, "subtracted bore is outside the solid");
        assert!(drilled.eval(Vec3::new(4.5, 0.0, 0.0)) < -1.0, "flange wall remains solid");
    }

    #[test]
    fn dual_surface_nets_extracts_watertight_outward_meshes_and_exports_stl_obj() {
        let base = Solid3dNode::primitive(SdfPrimitive::Box3d { half_extents: Vec3::new(20.0, 15.0, 8.0) });
        let boss =
            Solid3dNode::primitive_at(SdfPrimitive::Cylinder3d { radius: 8.0, height: 16.0 }, Transform3d::translate(Vec3::new(0.0, 0.0, 8.0)));
        let bore = Solid3dNode::primitive(SdfPrimitive::Cylinder3d { radius: 4.0, height: 40.0 });
        let part = Solid3dNode::boolean(CsgOp::Subtract, 0.0, Solid3dNode::boolean(CsgOp::Union, 2.0, base, boss), bore);

        let mesh = extract_isosurface(&part, 32);
        assert!(!mesh.vertices.is_empty() && mesh.triangles.len() > 100);
        // Every directed half-edge `(a, b)` must have a matching opposite half-edge `(b, a)` in a closed 2-manifold!
        let mut half_edges = std::collections::BTreeSet::new();
        for &[a, b, c] in &mesh.triangles {
            assert!(a != b && b != c && c != a);
            for (u, v) in [(a, b), (b, c), (c, a)] {
                assert!(half_edges.insert((u, v)), "duplicate half-edge ({u}, {v})");
            }
        }
        for &(u, v) in &half_edges {
            assert!(half_edges.contains(&(v, u)), "boundary edge ({u}, {v}) — mesh is not watertight!");
        }

        let bin_stl = write_stl_binary("clay_bracket", &mesh);
        assert_eq!(bin_stl.len(), 84 + mesh.triangles.len() * 50);
        let tri_count = u32::from_le_bytes([bin_stl[80], bin_stl[81], bin_stl[82], bin_stl[83]]) as usize;
        assert_eq!(tri_count, mesh.triangles.len());

        let ascii_stl = write_stl_ascii("clay_bracket", &mesh);
        assert!(ascii_stl.starts_with("solid clay_bracket\n") && ascii_stl.ends_with("endsolid clay_bracket\n"));

        let obj = write_obj("clay_bracket", &mesh);
        assert!(obj.contains("\no clay_bracket\n") && obj.contains("\nv ") && obj.contains("\nvn ") && obj.contains("\nf "));
    }

    #[test]
    fn orthographic_projection_recovers_top_front_right_and_isometric_outlines() {
        let brick = Solid3dNode::primitive(SdfPrimitive::Box3d { half_extents: Vec3::new(25.0, 15.0, 10.0) });
        let top = project_solid_to_2d(&brick, OrthoView3d::Top, 28, 35_f64.to_radians(), false);
        assert!(!top.visible_polylines.is_empty());
        assert!((top.bounds.width() - 50.0).abs() < 0.75, "top width={} expected ~50", top.bounds.width());
        assert!((top.bounds.height() - 30.0).abs() < 0.75, "top height={} expected ~30", top.bounds.height());

        let front = project_solid_to_2d(&brick, OrthoView3d::Front, 28, 35_f64.to_radians(), false);
        assert!((front.bounds.width() - 50.0).abs() < 0.75, "front width={} expected ~50", front.bounds.width());
        assert!((front.bounds.height() - 20.0).abs() < 0.75, "front height={} expected ~20", front.bounds.height());

        let right = project_solid_to_2d(&brick, OrthoView3d::Right, 28, 35_f64.to_radians(), false);
        assert!((right.bounds.width() - 30.0).abs() < 0.75, "right width={} expected ~30", right.bounds.width());
        assert!((right.bounds.height() - 20.0).abs() < 0.75, "right height={} expected ~20", right.bounds.height());

        let iso = project_solid_to_2d(&brick, OrthoView3d::Isometric, 28, 35_f64.to_radians(), true);
        assert!(!iso.visible_polylines.is_empty());
        assert!(!iso.hidden_segments.is_empty(), "isometric view with hidden lines includes back edges");
        assert!(!Bounds3::empty().is_finite());
    }
}
