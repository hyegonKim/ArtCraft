//! 2.5D perspective projection (`3D Rotation` and `3D Translation` / XFL `matrix3D`).
//!
//! Projects planar 2D points, bounding rectangles, and Bézier paths tilted in 3D space
//! (`rot_x_deg`, `rot_y_deg`, `z_offset`) through a pinhole camera (`fov_deg`) onto 2D stage
//! coordinates, and converts cleanly to and from 4×4 column-major XFL `matrix3D` arrays.

use crate::{Affine, Decomposed, Path, PathCmd, Rect, Vec2, lerp};
use serde::{Deserialize, Serialize};

/// Default field-of-view angle in degrees (matches classic 2.5D stage perspective).
pub const DEFAULT_FOV_DEG: f32 = 55.0;
/// Allowed FOV range in degrees.
pub const MIN_FOV_DEG: f32 = 5.0;
pub const MAX_FOV_DEG: f32 = 160.0;
/// Allowed 3D tilt angle range in degrees.
pub const MAX_TILT_DEG: f32 = 3600.0;
/// Allowed Z-offset range in stage pixels.
pub const MIN_Z_OFFSET: f32 = -5_000.0;
pub const MAX_Z_OFFSET: f32 = 20_000.0;
/// Reference half-width used to convert `fov_deg` into a perspective focal length in pixels.
const REF_HALF_WIDTH: f32 = 275.0;

fn default_fov() -> f32 {
    DEFAULT_FOV_DEG
}

/// Postcard-in-space 2.5D perspective parameters for an element on a keyframe.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Perspective3D {
    /// Rotation around the local X axis in degrees (tilts top/bottom toward or away from viewer).
    #[serde(default)]
    pub rot_x_deg: f32,
    /// Rotation around the local Y axis in degrees (tilts left/right toward or away from viewer).
    #[serde(default)]
    pub rot_y_deg: f32,
    /// Translation along the Z axis in pixels (positive pushes away from viewer, negative pulls closer).
    #[serde(default)]
    pub z_offset: f32,
    /// Perspective camera field-of-view in degrees (`5..=160`, default `55`).
    #[serde(default = "default_fov")]
    pub fov_deg: f32,
}

impl Default for Perspective3D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl PartialEq for Perspective3D {
    fn eq(&self, other: &Self) -> bool {
        let a = self.sanitized();
        let b = other.sanitized();
        (a.rot_x_deg - b.rot_x_deg).abs() <= 1e-4
            && (a.rot_y_deg - b.rot_y_deg).abs() <= 1e-4
            && (a.z_offset - b.z_offset).abs() <= 1e-4
            && (a.fov_deg - b.fov_deg).abs() <= 1e-4
    }
}

impl Perspective3D {
    /// Neutral 2.5D perspective (no 3D tilt or Z shift, 55° FOV).
    pub const IDENTITY: Self = Self { rot_x_deg: 0.0, rot_y_deg: 0.0, z_offset: 0.0, fov_deg: DEFAULT_FOV_DEG };

    /// True when this perspective leaves 2D coordinates unchanged.
    pub fn is_identity(&self) -> bool {
        let s = self.sanitized();
        s.rot_x_deg.abs() <= 1e-4 && s.rot_y_deg.abs() <= 1e-4 && s.z_offset.abs() <= 1e-4
    }

    /// Clamps non-finite and out-of-range parameters into safe bounds.
    pub fn sanitized(self) -> Self {
        let clean = |v: f32, lo: f32, hi: f32, fallback: f32| {
            if v.is_finite() { v.clamp(lo, hi) } else { fallback }
        };
        Self {
            rot_x_deg: clean(self.rot_x_deg, -MAX_TILT_DEG, MAX_TILT_DEG, 0.0),
            rot_y_deg: clean(self.rot_y_deg, -MAX_TILT_DEG, MAX_TILT_DEG, 0.0),
            z_offset: clean(self.z_offset, MIN_Z_OFFSET, MAX_Z_OFFSET, 0.0),
            fov_deg: clean(self.fov_deg, MIN_FOV_DEG, MAX_FOV_DEG, DEFAULT_FOV_DEG),
        }
    }

    /// Linearly interpolates between two 2.5D perspective states.
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        let a = self.sanitized();
        let b = other.sanitized();
        let u = if t.is_finite() { t } else { 0.0 };
        Self {
            rot_x_deg: lerp(a.rot_x_deg, b.rot_x_deg, u),
            rot_y_deg: lerp(a.rot_y_deg, b.rot_y_deg, u),
            z_offset: lerp(a.z_offset, b.z_offset, u),
            fov_deg: lerp(a.fov_deg, b.fov_deg, u),
        }
        .sanitized()
    }

    /// Pinhole focal distance in pixels corresponding to `fov_deg`.
    pub fn focal_length(&self) -> f32 {
        let s = self.sanitized();
        let half_rad = (s.fov_deg * 0.5).to_radians();
        let tan = half_rad.tan().max(1e-3);
        (REF_HALF_WIDTH / tan).clamp(32.0, 50_000.0)
    }

    /// Projects a 2D point `pt` tilted around `pivot` onto the `z = 0` plane.
    pub fn project_point(&self, pt: Vec2, pivot: Vec2) -> Vec2 {
        if !pt.is_finite() || !pivot.is_finite() {
            return pt;
        }
        let s = self.sanitized();
        if s.is_identity() {
            return pt;
        }
        let focal = s.focal_length();
        let dx = pt.x - pivot.x;
        let dy = pt.y - pivot.y;

        let (sx, cx) = s.rot_x_deg.to_radians().sin_cos();
        let (sy, cy) = s.rot_y_deg.to_radians().sin_cos();

        // Rotate around X first: (dx, dy*cx, dy*sx)
        let y1 = dy * cx;
        let z1 = dy * sx;
        // Then rotate around Y: (dx*cy + z1*sy, y1, -dx*sy + z1*cy)
        let x2 = dx * cy + z1 * sy;
        let y2 = y1;
        let z2 = -dx * sy + z1 * cy + s.z_offset;

        // Near-plane clamp prevents sign inversion if a corner tilts behind the eye.
        let denom = (focal + z2).max(focal * 0.05);
        let k = focal / denom;
        let out = Vec2::new(pivot.x + x2 * k, pivot.y + y2 * k);
        if out.is_finite() { out } else { pt }
    }

    /// Projects the four corners of `rect` around `pivot` and returns the enclosing 2D box.
    pub fn project_bounds(&self, rect: &Rect, pivot: Vec2) -> Rect {
        if !(rect.min.x <= rect.max.x && rect.min.y <= rect.max.y) {
            return *rect;
        }
        if self.is_identity() {
            return *rect;
        }
        let mut out = Rect::EMPTY;
        for c in [rect.min, Vec2::new(rect.max.x, rect.min.y), rect.max, Vec2::new(rect.min.x, rect.max.y)] {
            out.include(self.project_point(c, pivot));
        }
        out
    }

    /// Projects a 2D vector path through this 2.5D perspective transform around `pivot`.
    ///
    /// Straight segments remain exact straight lines under projective homography; quadratic and
    /// cubic Bézier segments are subdivided once before control-point projection so foreshortened
    /// curves stay faithful even at steep tilts.
    pub fn project_path(&self, path: &Path, pivot: Vec2) -> Path {
        if self.is_identity() || path.cmds.is_empty() {
            return path.clone();
        }
        let mut cmds = Vec::with_capacity(path.cmds.len() * 2);
        let mut cur = Vec2::ZERO;
        for &cmd in &path.cmds {
            match cmd {
                PathCmd::MoveTo(p) => {
                    cur = p;
                    cmds.push(PathCmd::MoveTo(self.project_point(p, pivot)));
                }
                PathCmd::LineTo(p) => {
                    cur = p;
                    cmds.push(PathCmd::LineTo(self.project_point(p, pivot)));
                }
                PathCmd::QuadTo(c, p) => {
                    let p0 = cur;
                    let q0 = p0.lerp(c, 0.5);
                    let q1 = c.lerp(p, 0.5);
                    let mid = q0.lerp(q1, 0.5);
                    cmds.push(PathCmd::QuadTo(self.project_point(q0, pivot), self.project_point(mid, pivot)));
                    cmds.push(PathCmd::QuadTo(self.project_point(q1, pivot), self.project_point(p, pivot)));
                    cur = p;
                }
                PathCmd::CubicTo(c1, c2, p) => {
                    let p0 = cur;
                    let p01 = p0.lerp(c1, 0.5);
                    let p12 = c1.lerp(c2, 0.5);
                    let p23 = c2.lerp(p, 0.5);
                    let p012 = p01.lerp(p12, 0.5);
                    let p123 = p12.lerp(p23, 0.5);
                    let mid = p012.lerp(p123, 0.5);
                    cmds.push(PathCmd::CubicTo(self.project_point(p01, pivot), self.project_point(p012, pivot), self.project_point(mid, pivot)));
                    cmds.push(PathCmd::CubicTo(self.project_point(p123, pivot), self.project_point(p23, pivot), self.project_point(p, pivot)));
                    cur = p;
                }
                PathCmd::Close => cmds.push(PathCmd::Close),
            }
        }
        Path { cmds }
    }

    /// Encodes `(affine, self)` into a 16-element row-vector 4×4 matrix matching XFL `matrix3D`.
    ///
    /// Layout (`p' = p · M`, Flash `Matrix3D.rawData` order):
    /// - row 0 (`m[0..4]`): X basis `(r00, r01, r02, 0)`
    /// - row 1 (`m[4..8]`): Y basis `(r10, r11, r12, 0)`
    /// - row 2 (`m[8..12]`): Z basis `(r20, r21, r22, 0)`
    /// - row 3 (`m[12..16]`): translation `(tx, ty, z_offset, 1)` (with `m[15]` storing `fov_deg / 55` if non-default? No: `m[15]` is always `1.0` in valid homogeneous XFL `matrix3D`!).
    pub fn to_matrix3d(&self, affine: &Affine, pivot: Vec2) -> [f32; 16] {
        let s = self.sanitized();
        let d = affine.decompose();
        let sx = if d.sx.is_finite() { d.sx } else { 1.0 };
        let sy = if d.sy.is_finite() { d.sy } else { 1.0 };
        let rz = if d.rotation.is_finite() { d.rotation } else { 0.0 };
        let rx = s.rot_x_deg.to_radians();
        let ry = s.rot_y_deg.to_radians();

        let (sz, cz) = rz.sin_cos();
        let (sx_r, cx_r) = rx.sin_cos();
        let (sy_r, cy_r) = ry.sin_cos();

        // Local 2D scale + Z-rotation first, followed by X-tilt, then Y-tilt (matching project_point):
        // v0 = (x * sx * cz - y * sy * sz, x * sx * sz + y * sy * cz, 0)
        // v1 = R_x(rx) * v0 = (v0.x, v0.y * cx_r, v0.y * sx_r)
        // v2 = R_y(ry) * v1 = (v1.x * cy_r + v1.z * sy_r, v1.y, -v1.x * sy_r + v1.z * cy_r)
        let r00 = sx * (cz * cy_r + sz * sx_r * sy_r);
        let r01 = sx * (sz * cx_r);
        let r02 = sx * (-cz * sy_r + sz * sx_r * cy_r);

        let r10 = sy * (-sz * cy_r + cz * sx_r * sy_r);
        let r11 = sy * (cz * cx_r);
        let r12 = sy * (sz * sy_r + cz * sx_r * cy_r);

        let r20 = cx_r * sy_r;
        let r21 = -sx_r;
        let r22 = cx_r * cy_r;

        // Compensate for local pivot so rotating around `pivot` keeps `affine.apply(pivot)` fixed in XY.
        let px = if pivot.x.is_finite() { pivot.x } else { 0.0 };
        let py = if pivot.y.is_finite() { pivot.y } else { 0.0 };
        let target_pivot = affine.apply(Vec2::new(px, py));
        let tx = target_pivot.x - (px * r00 + py * r10);
        let ty = target_pivot.y - (px * r01 + py * r11);
        let tz = s.z_offset - (px * r02 + py * r12);

        [r00, r01, r02, 0.0, r10, r11, r12, 0.0, r20, r21, r22, 0.0, tx, ty, tz, 1.0]
    }

    /// Decodes a 16-float XFL `matrix3D` attribute back into `(Affine, Perspective3D)`.
    pub fn from_matrix3d(m: &[f32; 16], pivot: Vec2) -> (Affine, Self) {
        if !m.iter().all(|v| v.is_finite()) {
            return (Affine::IDENTITY, Self::IDENTITY);
        }
        let sx = (m[0] * m[0] + m[1] * m[1] + m[2] * m[2]).sqrt().max(1e-4);
        let sy = (m[4] * m[4] + m[5] * m[5] + m[6] * m[6]).sqrt().max(1e-4);

        // Extract Z-axis unit basis (or reconstruct via cross product if row 2 was zeroed).
        let z_norm = (m[8] * m[8] + m[9] * m[9] + m[10] * m[10]).sqrt();
        let (r20, r21, r22) = if z_norm > 1e-5 {
            (m[8] / z_norm, m[9] / z_norm, m[10] / z_norm)
        } else {
            let u0 = [m[0] / sx, m[1] / sx, m[2] / sx];
            let u1 = [m[4] / sy, m[5] / sy, m[6] / sy];
            let cx = u0[1] * u1[2] - u0[2] * u1[1];
            let cy = u0[2] * u1[0] - u0[0] * u1[2];
            let cz = u0[0] * u1[1] - u0[1] * u1[0];
            let cn = (cx * cx + cy * cy + cz * cz).sqrt().max(1e-5);
            (cx / cn, cy / cn, cz / cn)
        };

        let rx = (-r21).clamp(-1.0, 1.0).asin();
        let cx_r = rx.cos();
        let (ry, rz) = if cx_r.abs() > 1e-4 {
            let ry = r20.atan2(r22);
            let rz = (m[1] / sx).atan2(m[5] / sy);
            (ry, rz)
        } else {
            let rz = (-(m[4] / sy)).atan2(m[0] / sx);
            (0.0, rz)
        };

        let px = if pivot.x.is_finite() { pivot.x } else { 0.0 };
        let py = if pivot.y.is_finite() { pivot.y } else { 0.0 };
        let stage_pivot_x = px * m[0] + py * m[4] + m[12];
        let stage_pivot_y = px * m[1] + py * m[5] + m[13];
        let stage_pivot_z = px * m[2] + py * m[6] + m[14];

        let mut dec = Decomposed { tx: 0.0, ty: 0.0, sx, sy, rotation: rz, skew: 0.0 };
        let lin = dec.compose();
        let lin_p = lin.apply(Vec2::new(px, py));
        dec.tx = stage_pivot_x - lin_p.x;
        dec.ty = stage_pivot_y - lin_p.y;
        let affine = dec.compose();

        let persp = Self { rot_x_deg: rx.to_degrees(), rot_y_deg: ry.to_degrees(), z_offset: stage_pivot_z, fov_deg: DEFAULT_FOV_DEG }.sanitized();
        (if affine.is_finite() { affine } else { Affine::IDENTITY }, persp)
    }

    /// Parses a whitespace- or comma-separated XFL `matrix3D="..."` attribute string.
    pub fn parse_matrix3d_attr(attr: &str, pivot: Vec2) -> Option<(Affine, Self)> {
        let mut vals = [0.0f32; 16];
        let mut count = 0usize;
        for tok in attr.split(|c: char| c.is_ascii_whitespace() || c == ',') {
            let t = tok.trim();
            if t.is_empty() {
                continue;
            }
            if count >= 16 {
                return None;
            }
            let v: f32 = t.parse().ok()?;
            if !v.is_finite() {
                return None;
            }
            vals[count] = v;
            count += 1;
        }
        if count != 16 {
            return None;
        }
        Some(Self::from_matrix3d(&vals, pivot))
    }

    /// Formats `to_matrix3d` as a space-separated 16-float XFL `matrix3D` attribute value.
    pub fn format_matrix3d_attr(&self, affine: &Affine, pivot: Vec2) -> String {
        let m = self.to_matrix3d(affine, pivot);
        let mut out = String::with_capacity(128);
        for (i, v) in m.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            let clean = if v.abs() < 1e-6 { 0.0 } else { *v };
            use std::fmt::Write as _;
            let _ = write!(out, "{clean:.6}");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_leaves_points_and_bounds_unchanged() {
        let p = Perspective3D::IDENTITY;
        assert!(p.is_identity());
        let r = Rect::new(10.0, 20.0, 110.0, 80.0);
        assert_eq!(p.project_bounds(&r, r.center()), r);
    }

    #[test]
    fn positive_z_offset_shrinks_around_pivot_and_negative_enlarges() {
        let pivot = Vec2::new(100.0, 100.0);
        let r = Rect::new(50.0, 50.0, 150.0, 150.0);
        let far = Perspective3D { z_offset: 500.0, ..Perspective3D::IDENTITY };
        let near = Perspective3D { z_offset: -200.0, ..Perspective3D::IDENTITY };
        let b_far = far.project_bounds(&r, pivot);
        let b_near = near.project_bounds(&r, pivot);
        assert!(b_far.width() < r.width());
        assert!(b_near.width() > r.width());
        assert!((b_far.center() - pivot).length() < 1e-3);
    }

    #[test]
    fn y_axis_tilt_produces_foreshortened_keystoning() {
        let pivot = Vec2::ZERO;
        let tilt = Perspective3D { rot_y_deg: 35.0, ..Perspective3D::IDENTITY };
        // At +35° around Y: z2 = -dx * sin(35°). Right edge (dx = +50) has negative z (closer -> taller),
        // left edge (dx = -50) has positive z (farther -> shorter).
        let left_top = tilt.project_point(Vec2::new(-50.0, -40.0), pivot);
        let left_bot = tilt.project_point(Vec2::new(-50.0, 40.0), pivot);
        let right_top = tilt.project_point(Vec2::new(50.0, -40.0), pivot);
        let right_bot = tilt.project_point(Vec2::new(50.0, 40.0), pivot);
        let left_h = left_bot.y - left_top.y;
        let right_h = right_bot.y - right_top.y;
        assert!(right_h > left_h, "near side ({right_h}) must project taller than far side ({left_h})");
    }

    #[test]
    fn matrix3d_round_trips_affine_and_perspective() {
        let aff = Decomposed { tx: 320.0, ty: 180.0, sx: 1.25, sy: 0.85, rotation: 22.0_f32.to_radians(), skew: 0.0 }.compose();
        let persp = Perspective3D { rot_x_deg: 24.0, rot_y_deg: -31.5, z_offset: 140.0, fov_deg: DEFAULT_FOV_DEG };
        let pivot = Vec2::new(15.0, -10.0);
        let attr = persp.format_matrix3d_attr(&aff, pivot);
        let (aff2, persp2) = Perspective3D::parse_matrix3d_attr(&attr, pivot).expect("16 floats");
        assert!((persp2.rot_x_deg - persp.rot_x_deg).abs() < 0.05, "{persp2:?}");
        assert!((persp2.rot_y_deg - persp.rot_y_deg).abs() < 0.05, "{persp2:?}");
        assert!((persp2.z_offset - persp.z_offset).abs() < 0.05, "{persp2:?}");
        let d1 = aff.decompose();
        let d2 = aff2.decompose();
        assert!((d1.tx - d2.tx).abs() < 0.05 && (d1.ty - d2.ty).abs() < 0.05);
        assert!((d1.sx - d2.sx).abs() < 0.01 && (d1.sy - d2.sy).abs() < 0.01);
        assert!((d1.rotation - d2.rotation).abs() < 0.01);
    }
}
