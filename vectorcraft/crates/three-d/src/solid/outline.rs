//! Curved outlines for extruded caps, rotated planes, and the cap-meeting edges of side walls.
//!
//! Where a step moves the art affinely without inset, consecutive sampled spans of one source
//! cubic Bezier collapse back into a single exact transformed cubic. Under perspective or bevel
//! inset, each sampled span is approximated by one or more Hermite cubics within `along.tolerance`,
//! with shared wall-to-cap edges producing the exact reversed cubic sequence so adjacent facets
//! meet without gaps.

use vectorcraft_geom::kurbo::{CubicBez, ParamCurve};
use vectorcraft_geom::{BezPath, PathData, Point};

use super::rings::{Ring, Span};

/// Maximum midpoint distance (in document points) allowed when fitting a projected curve span.
pub(crate) const CURVE_TOLERANCE: f64 = 0.05;

/// Total curved segments fitted for one solid's outlines before falling back to polygon contours.
const MAX_OUTLINE_SEGMENTS: usize = 65_536;

/// Tracks how many cubic segments have been emitted across one solid's outlines.
#[derive(Default)]
pub(super) struct Budget {
    used: usize,
}

impl Budget {
    fn take(&mut self, count: usize) -> bool {
        let next = self.used.saturating_add(count);
        if next > MAX_OUTLINE_SEGMENTS {
            return false;
        }
        self.used = next;
        true
    }
}

/// Evaluates a projected curve at parameter `t` along a [`Span`], or maps points affinely when
/// the step is neither inset nor under perspective.
pub(super) struct Along<'a> {
    pub(super) at: &'a dyn Fn(&Span, f64) -> Point,
    pub(super) map: Option<&'a dyn Fn(Point) -> Point>,
    pub(super) affine: bool,
    pub(super) tolerance: f64,
}

/// Directed subsegment of `curve` from `t0` to `t1` (handling `t1 < t0` when a ring was reversed).
fn directed_subsegment(curve: CubicBez, t0: f64, t1: f64) -> CubicBez {
    if t0 <= t1 {
        curve.subsegment(t0..t1)
    } else {
        let rev = curve.subsegment(t1..t0);
        CubicBez::new(rev.p3, rev.p2, rev.p1, rev.p0)
    }
}

fn same_curve(a: CubicBez, b: CubicBez) -> bool {
    a.p0.distance(b.p0) <= 1e-9 && a.p1.distance(b.p1) <= 1e-9 && a.p2.distance(b.p2) <= 1e-9 && a.p3.distance(b.p3) <= 1e-9
}

fn spans_continue(a: Option<Span>, b: Option<Span>) -> Option<Span> {
    let (a, b) = (a?, b?);
    if !same_curve(a.curve, b.curve) || (a.t1 - b.t0).abs() > 1e-7 {
        return None;
    }
    let dir_a = a.t1 - a.t0;
    let dir_b = b.t1 - b.t0;
    if dir_a * dir_b < 0.0 {
        return None;
    }
    Some(Span { curve: a.curve, t0: a.t0, t1: b.t1 })
}

/// Evaluates `along.at` at `u` in `0..=1` across `[t0, t1]`, blended linearly at the ends so the
/// fitted curve lands on the exact projected contour endpoints `p0` and `p3`.
fn eval_pinned(span: &Span, t0: f64, t1: f64, u: f64, p0: Point, p3: Point, along: &Along<'_>) -> Point {
    let raw0 = (along.at)(span, t0);
    let raw1 = (along.at)(span, t1);
    let raw = (along.at)(span, t0 + (t1 - t0) * u);
    let d0 = p0 - raw0;
    let d1 = p3 - raw1;
    Point::new(raw.x + (1.0 - u) * d0.x + u * d1.x, raw.y + (1.0 - u) * d0.y + u * d1.y)
}

/// Evaluates the derivative of `along.at` with respect to `t` via central finite differences.
/// Because `span.curve` and camera projection are smooth, two spans meeting at a smooth source
/// point share the exact same tangent direction.
fn raw_tangent(span: &Span, t: f64, along: &Along<'_>) -> vectorcraft_geom::Vec2 {
    const EPS: f64 = 1e-5;
    let a = (along.at)(span, t - EPS);
    let b = (along.at)(span, t + EPS);
    (b - a) / (2.0 * EPS)
}

fn fit_range(
    span: &Span,
    (t0, t1): (f64, f64),
    (p0, p3): (Point, Point),
    along: &Along<'_>,
    depth: u8,
    out: &mut Vec<CubicBez>,
    budget: &mut Budget,
) -> bool {
    if let (true, Some(map)) = (along.affine, along.map) {
        if !budget.take(1) {
            return false;
        }
        let sub = directed_subsegment(span.curve, t0, t1);
        let c1 = map(sub.p1);
        let c2 = map(sub.p2);
        if ![c1.x, c1.y, c2.x, c2.y].iter().all(|v| v.is_finite()) {
            return false;
        }
        out.push(CubicBez::new(p0, c1, c2, p3));
        return true;
    }
    let dt = t1 - t0;
    let dp0 = raw_tangent(span, t0, along) * dt;
    let dp3 = raw_tangent(span, t1, along) * dt;
    if ![dp0.x, dp0.y, dp3.x, dp3.y].iter().all(|v| v.is_finite()) {
        return false;
    }
    let cubic = CubicBez::new(p0, p0 + dp0 / 3.0, p3 - dp3 / 3.0, p3);
    let tol = along.tolerance.max(1e-3);
    if depth < 6 {
        let mid_p = eval_pinned(span, t0, t1, 0.5, p0, p3, along);
        let q_lo = eval_pinned(span, t0, t1, 0.25, p0, p3, along);
        let q_hi = eval_pinned(span, t0, t1, 0.75, p0, p3, along);
        let err = cubic.eval(0.5).distance(mid_p).max(cubic.eval(0.25).distance(q_lo)).max(cubic.eval(0.75).distance(q_hi));
        if err > tol && mid_p.x.is_finite() && mid_p.y.is_finite() {
            let tm = 0.5 * (t0 + t1);
            return fit_range(span, (t0, tm), (p0, mid_p), along, depth + 1, out, budget)
                && fit_range(span, (tm, t1), (mid_p, p3), along, depth + 1, out, budget);
        }
    }
    if !budget.take(1) {
        return false;
    }
    out.push(cubic);
    true
}

fn fit_span(span: &Span, p0: Point, p3: Point, along: &Along<'_>, budget: &mut Budget) -> Option<Vec<CubicBez>> {
    if !span.curve.p0.x.is_finite() {
        return None;
    }
    let mut out = Vec::new();
    fit_range(span, (span.t0, span.t1), (p0, p3), along, 0, &mut out, budget).then_some(out)
}

/// Builds the curved outline of a planar face (cap or rotated plane) from its traced `rings` and
/// projected `contours`. Returns `None` if no edge is curved or if the segment budget is exceeded.
pub(super) fn face(rings: &[&Ring], contours: &[Vec<Point>], along: &Along<'_>, budget: &mut Budget) -> Option<PathData> {
    if rings.len() != contours.len() || !rings.iter().any(|r| r.spans.iter().any(Option::is_some)) {
        return None;
    }
    let mut bp = BezPath::new();
    for (ring, contour) in rings.iter().zip(contours) {
        let n = contour.len();
        if n < 3 || ring.spans.len() != n {
            return None;
        }
        if along.affine && along.map.is_some() {
            // Start at an anchor boundary where the previous edge does not continue into this edge,
            // so a cubic that wraps across index 0 still merges into a single segment.
            let start = (0..n).find(|&i| spans_continue(ring.spans[(i + n - 1) % n], ring.spans[i]).is_none()).unwrap_or(0);
            bp.move_to(contour[start]);
            let mut step = 0usize;
            while step < n {
                let idx = (start + step) % n;
                if let Some(mut merged) = ring.spans[idx] {
                    let mut run = 1usize;
                    while step + run < n {
                        let next_idx = (start + step + run) % n;
                        if let Some(next_merged) = spans_continue(Some(merged), ring.spans[next_idx]) {
                            merged = next_merged;
                            run += 1;
                        } else {
                            break;
                        }
                    }
                    let end_pt = contour[(idx + run) % n];
                    let cubics = fit_span(&merged, contour[idx], end_pt, along, budget)?;
                    for c in cubics {
                        bp.curve_to(c.p1, c.p2, c.p3);
                    }
                    step += run;
                } else {
                    bp.line_to(contour[(idx + 1) % n]);
                    step += 1;
                }
            }
            bp.close_path();
        } else {
            bp.move_to(contour[0]);
            for i in 0..n {
                let next = contour[(i + 1) % n];
                if let Some(span) = ring.spans[i] {
                    for c in fit_span(&span, contour[i], next, along, budget)? {
                        bp.curve_to(c.p1, c.p2, c.p3);
                    }
                } else {
                    bp.line_to(next);
                }
            }
            bp.close_path();
        }
    }
    Some(PathData::from_bezpath(&bp))
}

/// Builds the outline of a 4-point wall quad `[a0, a1, b1, b0]` where `a0 -> a1` may follow `near`
/// and `b1 -> b0` may follow `far` (reversed so it shares the exact cubics of `b0 -> b1`).
pub(super) fn wall(span: &Span, quad: &[Point], near: Option<&Along<'_>>, far: Option<&Along<'_>>, budget: &mut Budget) -> Option<PathData> {
    let [a0, a1, b1, b0] = *quad else { return None };
    if near.is_none() && far.is_none() {
        return None;
    }
    let mut bp = BezPath::new();
    bp.move_to(a0);
    if let Some(along) = near {
        for c in fit_span(span, a0, a1, along, budget)? {
            bp.curve_to(c.p1, c.p2, c.p3);
        }
    } else {
        bp.line_to(a1);
    }
    bp.line_to(b1);
    if let Some(along) = far {
        let forward = fit_span(span, b0, b1, along, budget)?;
        for c in forward.into_iter().rev() {
            bp.curve_to(c.p2, c.p1, c.p0);
        }
    } else {
        bp.line_to(b0);
    }
    bp.line_to(a0);
    bp.close_path();
    Some(PathData::from_bezpath(&bp))
}
