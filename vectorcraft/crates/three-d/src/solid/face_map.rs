//! Projects 2D artwork offsets onto the front and back planar caps of Extrude & Bevel or onto the
//! rotated plane of Rotate, and inverts page points back to planar face offsets for handle dragging.

use vectorcraft_geom::kurbo::{CubicBez, ParamCurve, PathEl};
use vectorcraft_geom::{BezPath, PathData, Point, Rect};

use super::build::{Camera, Placing, Turn, check_bounds};
use super::{Extrude, PlanarFace, V3, View, perspective_scale};

/// Maps 2D coordinates `(x, y)` relative to the planar face's center onto the projected page, and
/// carries the face's visibility and directional lighting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceMap {
    x: V3,
    y: V3,
    center: V3,
    normal: V3,
    origin: Point,
    eye: Option<f64>,
    /// Whether the planar face is turned toward the viewer.
    pub seen: bool,
    /// Scalar brightness (`0..=1`) of the face under the active light setup.
    pub brightness: f64,
    /// Per-channel RGB illumination of the face under the active light setup.
    pub illumination: [f64; 3],
}

impl FaceMap {
    fn from_placing(camera: &Camera, m: Placing, normal: V3, flip_x: bool, seen: bool) -> Self {
        let (raw_x, y, _) = m.frame();
        let x = if flip_x { V3(-raw_x.0, -raw_x.1, -raw_x.2) } else { raw_x };
        let center = m.place(camera.center_and_eye().0);
        let n = normal.unit();
        let (origin, eye) = camera.center_and_eye();
        Self {
            x,
            y,
            center,
            normal: n,
            origin,
            eye,
            seen,
            brightness: camera.brightness(n),
            illumination: crate::illumination(&camera.view, n, camera.light),
        }
    }

    fn v3_at(&self, u: f64, v: f64) -> V3 {
        V3(self.center.0 + self.x.0 * u + self.y.0 * v, self.center.1 + self.x.1 * u + self.y.1 * v, self.center.2 + self.x.2 * u + self.y.2 * v)
    }

    /// Projects a face-local offset `(u, v)` from the face's center onto the document page.
    /// Returns `None` if the point lies at or behind the perspective eye.
    pub fn project_offset(&self, u: f64, v: f64) -> Option<Point> {
        let p = self.v3_at(u, v);
        if let Some(e) = self.eye
            && e - p.2 <= e * 1e-3
        {
            return None;
        }
        let f = perspective_scale(self.eye, p.2);
        let out = Point::new(self.origin.x + p.0 * f, self.origin.y + p.1 * f);
        (out.x.is_finite() && out.y.is_finite()).then_some(out)
    }

    /// Inverts a document page point back to its `(u, v)` offset on this planar face.
    /// Returns `None` if the face is edge-on to the ray through `page`.
    pub fn offset_at(&self, page: Point) -> Option<(f64, f64)> {
        let (dx, dy) = (page.x - self.origin.x, page.y - self.origin.y);
        let hit = match self.eye {
            None => {
                if self.normal.2.abs() <= 1e-9 {
                    return None;
                }
                let dz = (self.normal.dot(self.center) - self.normal.0 * dx - self.normal.1 * dy) / self.normal.2;
                V3(dx, dy, dz)
            }
            Some(e) => {
                let eye = V3(0.0, 0.0, e);
                let dir = V3(dx, dy, -e);
                let denom = self.normal.dot(dir);
                if denom.abs() <= 1e-9 {
                    return None;
                }
                let s = self.normal.dot(self.center.sub(eye)) / denom;
                if !s.is_finite() || s <= 0.0 {
                    return None;
                }
                V3(eye.0 + dir.0 * s, eye.1 + dir.1 * s, eye.2 + dir.2 * s)
            }
        };
        let rel = hit.sub(self.center);
        let a11 = self.x.dot(self.x);
        let a12 = self.x.dot(self.y);
        let a22 = self.y.dot(self.y);
        let det = a11 * a22 - a12 * a12;
        if !det.is_finite() || det.abs() <= 1e-12 {
            return None;
        }
        let b1 = self.x.dot(rel);
        let b2 = self.y.dot(rel);
        let u = (b1 * a22 - b2 * a12) / det;
        let v = (a11 * b2 - a12 * b1) / det;
        (u.is_finite() && v.is_finite()).then_some((u, v))
    }

    /// Projects a `PathData` given in face-local `(u, v)` coordinates onto the document page.
    pub fn project_path(&self, path: &PathData) -> Option<PathData> {
        if self.eye.is_none() {
            let mut out = path.clone();
            for sp in &mut out.subpaths {
                for a in &mut sp.anchors {
                    a.p = self.project_offset(a.p.x, a.p.y)?;
                    a.h_in = self.project_offset(a.h_in.x, a.h_in.y)?;
                    a.h_out = self.project_offset(a.h_out.x, a.h_out.y)?;
                }
            }
            return Some(out);
        }
        let mut bp = BezPath::new();
        let mut cur = Point::ZERO;
        let mut sub_start = Point::ZERO;
        for el in path.to_bezpath().elements() {
            match *el {
                PathEl::MoveTo(p) => {
                    cur = p;
                    sub_start = p;
                    bp.move_to(self.project_offset(p.x, p.y)?);
                }
                PathEl::LineTo(p) => {
                    cur = p;
                    bp.line_to(self.project_offset(p.x, p.y)?);
                }
                PathEl::QuadTo(p1, p2) => {
                    let c = CubicBez::new(cur, cur + (p1 - cur) * (2.0 / 3.0), p2 + (p1 - p2) * (2.0 / 3.0), p2);
                    project_cubic(self, c, 0, &mut bp)?;
                    cur = p2;
                }
                PathEl::CurveTo(p1, p2, p3) => {
                    project_cubic(self, CubicBez::new(cur, p1, p2, p3), 0, &mut bp)?;
                    cur = p3;
                }
                PathEl::ClosePath => {
                    cur = sub_start;
                    bp.close_path();
                }
            }
        }
        Some(PathData::from_bezpath(&bp))
    }
}

fn project_cubic(map: &FaceMap, c: CubicBez, depth: u8, bp: &mut BezPath) -> Option<()> {
    let q0 = map.project_offset(c.p0.x, c.p0.y)?;
    let q3 = map.project_offset(c.p3.x, c.p3.y)?;
    let s1 = c.eval(1.0 / 3.0);
    let s2 = c.eval(2.0 / 3.0);
    let m1 = map.project_offset(s1.x, s1.y)?;
    let m2 = map.project_offset(s2.x, s2.y)?;
    let c1 = Point::new((-5.0 * q0.x + 18.0 * m1.x - 9.0 * m2.x + 2.0 * q3.x) / 6.0, (-5.0 * q0.y + 18.0 * m1.y - 9.0 * m2.y + 2.0 * q3.y) / 6.0);
    let c2 = Point::new((2.0 * q0.x - 9.0 * m1.x + 18.0 * m2.x - 5.0 * q3.x) / 6.0, (2.0 * q0.y - 9.0 * m1.y + 18.0 * m2.y - 5.0 * q3.y) / 6.0);
    let fitted = CubicBez::new(q0, c1, c2, q3);
    if depth < 3 {
        let sm = c.eval(0.5);
        let mm = map.project_offset(sm.x, sm.y)?;
        if fitted.eval(0.5).distance(mm) > 0.05 {
            project_cubic(map, c.subsegment(0.0..0.5), depth + 1, bp)?;
            return project_cubic(map, c.subsegment(0.5..1.0), depth + 1, bp);
        }
    }
    bp.curve_to(c1, c2, q3);
    Some(())
}

/// Face map of a flat 2D plane rotated in 3D about the center of `bounds`.
pub fn plane_face_map(bounds: Rect, view: View) -> Option<FaceMap> {
    check_bounds(bounds).ok()?;
    let camera = Camera::new(view, bounds, 0.0);
    let up = camera.rotate(V3(0.0, 0.0, 1.0));
    let back = camera.facing(V3::default(), up) < 0.0;
    let normal = if back { V3(-up.0, -up.1, -up.2) } else { up };
    let m = Placing::new(&camera, bounds.center(), Turn::new(bounds, 0.0, 1.0, 0.0), 0.0);
    Some(FaceMap::from_placing(&camera, m, normal, false, true))
}

/// Face map of the front or back planar cap of an extruded solid built about `bounds`.
/// Returns `None` when caps are turned off or when `face` is `Back` on a zero-depth extrusion.
pub fn extrude_face_map(bounds: Rect, options: Extrude, view: View, face: PlanarFace) -> Option<FaceMap> {
    check_bounds(bounds).ok()?;
    let o = options.sanitized();
    if !o.cap {
        return None;
    }
    if o.depth <= 0.0 {
        return (face == PlanarFace::Front).then(|| plane_face_map(bounds, view)).flatten();
    }
    let camera = Camera::new(view, bounds, o.depth);
    let (t, z_sign, flip_x) = match face {
        PlanarFace::Front => (0.0, 1.0, false),
        // Seen from behind, a graphic on the back cap reads left to right rather than mirrored.
        PlanarFace::Back => (1.0, -1.0, true),
    };
    let z = o.depth / 2.0 - o.depth * t;
    let turn = Turn::new(bounds, o.twist, o.taper, t);
    let m = Placing::new(&camera, bounds.center(), turn, z);
    let normal = camera.rotate(V3(0.0, 0.0, z_sign)).unit();
    let center = m.place(bounds.center());
    let seen = camera.facing(center, normal) > 0.0;
    Some(FaceMap::from_placing(&camera, m, normal, flip_x, seen))
}
