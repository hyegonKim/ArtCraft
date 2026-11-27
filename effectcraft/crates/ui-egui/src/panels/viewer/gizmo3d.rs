//! Interactive 3D Viewport Transform Gizmo (Gap 12): projects a selected 3D layer's anchor
//! origin, local/world X/Y/Z translation arrows, and X/Y/Z rotation arc handles onto the
//! Composition viewport, hit-tests pointer hovers/drags, and computes axis-constrained translation
//! and rotation deltas for `Session::execute`.

use effectcraft_engine::geom::{Mat4, Vec3, vec3};
use effectcraft_engine::project::{Layer, LayerId};
use effectcraft_engine::render::EvalCtx;
use egui::{Color32, Pos2, Rect, Stroke, pos2, vec2};

use super::geometry::{cam_state, project_segment};
use super::tools::{axis_drag, parent_world_inverse};
use super::{EffectcraftApp, ViewerMap};
use crate::state::GizmoSpace;

/// Principal axis of the 3D transform gizmo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GizmoAxis {
    X,
    Y,
    Z,
}

impl GizmoAxis {
    pub const ALL: [GizmoAxis; 3] = [GizmoAxis::X, GizmoAxis::Y, GizmoAxis::Z];

    pub fn name(self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Y => "y",
            Self::Z => "z",
        }
    }

    pub fn color(self) -> Color32 {
        match self {
            Self::X => Color32::from_rgb(0xe0, 0x50, 0x50),
            Self::Y => Color32::from_rgb(0x60, 0xd0, 0x60),
            Self::Z => Color32::from_rgb(0x50, 0x8c, 0xf0),
        }
    }

    pub fn unit_vec(self) -> Vec3 {
        match self {
            Self::X => vec3(1.0, 0.0, 0.0),
            Self::Y => vec3(0.0, 1.0, 0.0),
            Self::Z => vec3(0.0, 0.0, 1.0),
        }
    }

    /// Transform property path driven when rotating around this axis.
    pub fn prop_path(self) -> &'static str {
        match self {
            Self::X => "transform/rotationX",
            Self::Y => "transform/rotationY",
            Self::Z => "transform/rotation",
        }
    }

    pub fn prop_match_id(self) -> &'static str {
        match self {
            Self::X => "rotationX",
            Self::Y => "rotationY",
            Self::Z => "rotation",
        }
    }
}

/// Grabbable 3D rotation arc & knob on the Composition viewport.
#[derive(Clone, Debug)]
pub(crate) struct RotAxisHit {
    pub layer: LayerId,
    pub axis: GizmoAxis,
    pub polyline: Vec<Pos2>,
    pub handle: Pos2,
    /// Unit screen-space vector along which dragging increases the axis rotation in degrees.
    pub screen_dir: [f32; 2],
    pub start_deg: f64,
}

///Projected 3D gizmo geometry in screen coordinates (used by rendering and unit tests).
#[derive(Clone, Debug)]
pub(crate) struct ProjectedGizmo {
    pub origin: Pos2,
    /// `(axis, shaft_start, tip, parent_delta_per_comp_x, parent_delta_per_comp_y)`
    pub translate: Vec<(GizmoAxis, Pos2, Pos2, [f64; 3], [f64; 3])>,
    pub rotate: Vec<RotAxisHit>,
}

/// Whether `layer` displays the 3D transform gizmo when selected.
pub(super) fn has_3d_gizmo(layer: &Layer) -> bool {
    (layer.switches.three_d || layer.source.is_model()) && !layer.is_camera() && !layer.is_light()
}

/// Hit-tests the 3D rotation arcs and midpoint knobs under screen point `p`.
pub(super) fn rot_axis_at(hits: &[RotAxisHit], p: Pos2) -> Option<&RotAxisHit> {
    let dist_hit = |h: &RotAxisHit| -> f32 {
        let mut best = h.handle.distance(p);
        for seg in h.polyline.windows(2) {
            let (a, b) = (seg[0], seg[1]);
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
            best = best.min(p.distance(a + ab * t));
        }
        best
    };
    hits.iter().map(|h| (h, dist_hit(h))).filter(|(_, d)| *d < 6.5).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(h, _)| h)
}

/// Computes the projected 3D gizmo for `layer` in `space` (`Local` or `World`).
pub(crate) fn project_gizmo(ectx: &EvalCtx, layer: &Layer, map: &ViewerMap, space: GizmoSpace) -> Option<ProjectedGizmo> {
    let tr = layer.transform()?;
    let anchor = ectx.v3(layer, tr, "anchor", [0.0; 3]);
    let a3 = vec3(anchor[0], anchor[1], anchor[2]);
    let w = ectx.world_matrix(layer);
    let origin_w = w.apply(a3);

    let cam = cam_state(ectx);
    let (cw, ch) = (ectx.comp.width as f64, ectx.comp.height as f64);
    let pc = cam.projection(cw, ch);
    let [origin_comp, _] = project_segment(&pc, origin_w, origin_w)?;
    let origin_screen = map.to_screen(origin_comp);

    let zoom = map.zoom.max(1e-4) as f64;
    let arrow_len = 60.0 / zoom;
    let pinv = parent_world_inverse(ectx, layer);
    let axes_world = world_axes(&w, space);

    let mut translate = Vec::with_capacity(3);
    for (idx, axis) in GizmoAxis::ALL.into_iter().enumerate() {
        let world_vec = axes_world[idx] * arrow_len;
        let Some([o, e]) = project_segment(&pc, origin_w, origin_w + world_vec) else {
            continue;
        };
        let mut s = [e[0] - o[0], e[1] - o[1]];
        if (s[0] * s[0] + s[1] * s[1]).sqrt() * zoom < 12.0 {
            let k = 24.0 / zoom;
            s = [k, -k];
        }
        let (so, se) = (map.to_screen(o), map.to_screen([o[0] + s[0], o[1] + s[1]]));
        let dir = pinv.apply_vec(world_vec);
        let (ax, ay) = axis_drag([dir.x, dir.y, dir.z], s);
        translate.push((axis, so, se, ax, ay));
    }

    let rotate = build_rotation_arcs(ectx, layer, &pc, origin_w, origin_screen, &axes_world, map);
    Some(ProjectedGizmo { origin: origin_screen, translate, rotate })
}

fn world_axes(w: &Mat4, space: GizmoSpace) -> [Vec3; 3] {
    match space {
        GizmoSpace::World => [vec3(1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0), vec3(0.0, 0.0, 1.0)],
        GizmoSpace::Local => GizmoAxis::ALL.map(|ax| {
            let u = ax.unit_vec();
            let v = w.apply_vec(u);
            if v.length() > 1e-6 { v.normalize() } else { u }
        }),
    }
}

fn build_rotation_arcs(ectx: &EvalCtx, layer: &Layer, pc: &Mat4, origin_w: Vec3, origin_screen: Pos2, axes_w: &[Vec3; 3], map: &ViewerMap) -> Vec<RotAxisHit> {
    let Some(tr) = layer.transform() else { return Vec::new() };
    let zoom = map.zoom.max(1e-4) as f64;
    let radius = 42.0 / zoom;
    // Pick clean camera-facing quadrants so X, Y, Z arcs stay separated in front and oblique views:
    // - Z arc in (+X, +Y) SE quadrant
    // - X arc in (+Y, -Z) bowed to SW when edge-on
    // - Y arc in (-X, -Z) bowed to NW when edge-on
    let specs: [(GizmoAxis, Vec3, Vec3, [f32; 2]); 3] = [
        (GizmoAxis::X, axes_w[1], -axes_w[2], [-18.0, 0.0]),
        (GizmoAxis::Y, -axes_w[0], -axes_w[2], [0.0, -18.0]),
        (GizmoAxis::Z, axes_w[0], axes_w[1], [0.0, 0.0]),
    ];

    let mut out = Vec::with_capacity(3);
    const SEGMENTS: usize = 12;
    let a0 = 20.0_f64.to_radians();
    let a1 = 70.0_f64.to_radians();

    for (axis, b1, b2, edge_bow) in specs {
        // Measure minor-axis projection so edge-on rings bow cleanly away from the translation shaft.
        let b2_screen = project_segment(pc, origin_w, origin_w + b2 * radius).map_or(0.0f32, |[o, e]| map.to_screen(e).distance(map.to_screen(o)));
        let bow_weight = ((14.0 - b2_screen) / 14.0).clamp(0.0, 1.0);

        let mut pts = Vec::with_capacity(SEGMENTS + 1);
        for i in 0..=SEGMENTS {
            let t = i as f64 / SEGMENTS as f64;
            let ang = a0 + (a1 - a0) * t;
            let (s, c) = ang.sin_cos();
            let wp = origin_w + (b1 * c + b2 * s) * radius;
            if let Some([p_comp, _]) = project_segment(pc, wp, wp) {
                let sp = map.to_screen(p_comp);
                let arch = (std::f32::consts::PI * t as f32).sin() * bow_weight;
                pts.push(pos2(sp.x + edge_bow[0] * arch, sp.y + edge_bow[1] * arch));
            }
        }
        if pts.len() < 2 {
            continue;
        }
        let mid_idx = pts.len() / 2;
        let handle = pts[mid_idx];
        let screen_dir = rotation_screen_dir(ectx, layer, pc, origin_w, origin_screen, (b1 + b2).normalize() * radius, axis, map, &pts);
        let start_deg = ectx.f(layer, tr, axis.prop_match_id(), 0.0);
        out.push(RotAxisHit { layer: layer.id, axis, polyline: pts, handle, screen_dir, start_deg });
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn rotation_screen_dir(
    ectx: &EvalCtx,
    layer: &Layer,
    pc: &Mat4,
    origin_w: Vec3,
    origin_screen: Pos2,
    arm_w: Vec3,
    axis: GizmoAxis,
    map: &ViewerMap,
    pts: &[Pos2],
) -> [f32; 2] {
    // Evaluate how +1° on the layer's local rotation axis moves `origin_w + arm_w` on screen.
    let w = ectx.world_matrix(layer);
    let rot_axis_w = w.apply_vec(axis.unit_vec()).normalize();
    let rad = 1.0_f64.to_radians();
    // Right-hand differential in EffectCraft's `Mat4::rotation_xyz` space.
    let p0 = origin_w + arm_w;
    let p1 = origin_w + arm_w + rot_axis_w.cross(arm_w) * rad;
    if let Some([c0, c1]) = project_segment(pc, p0, p1) {
        let d = map.to_screen(c1) - map.to_screen(c0);
        if d.length() > 1e-3 {
            let n = d.normalized();
            return [n.x, n.y];
        }
    }
    // Fallback to geometric tangent along the drawn arc.
    if let (Some(&first), Some(&last)) = (pts.first(), pts.last()) {
        let d = last - first;
        if d.length() > 1e-3 {
            let n = d.normalized();
            return [n.x, n.y];
        }
    }
    let radial = pts.get(pts.len() / 2).copied().unwrap_or(origin_screen) - origin_screen;
    let perp = vec2(-radial.y, radial.x);
    if perp.length() > 1e-3 {
        let n = perp.normalized();
        [n.x, n.y]
    } else {
        [1.0, 0.0]
    }
}

/// Draws the 3D anchor crosshair, translation arrows, and rotation rings for a selected 3D layer,
/// recording hit regions and automation IDs.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_layer_gizmo(
    app: &mut EffectcraftApp,
    painter: &egui::Painter,
    ectx: &EvalCtx,
    layer: &Layer,
    map: &ViewerMap,
    draw_anchor: bool,
    axis_hits: &mut Vec<(LayerId, [Pos2; 2], [f64; 3], [f64; 3])>,
    rot_hits: &mut Vec<RotAxisHit>,
) {
    let space = app.ui.viewer.gizmo_space;
    let Some(gizmo) = project_gizmo(ectx, layer, map, space) else {
        return;
    };
    let s = gizmo.origin;
    if draw_anchor {
        painter.circle_stroke(s, 5.0, Stroke::new(1.0, Color32::WHITE));
        painter.line_segment([s - vec2(9.0, 0.0), s + vec2(9.0, 0.0)], Stroke::new(1.0, Color32::WHITE));
        painter.line_segment([s - vec2(0.0, 9.0), s + vec2(0.0, 9.0)], Stroke::new(1.0, Color32::WHITE));
        app.auto.add(&format!("viewer.anchor.{}", layer.id.0), Rect::from_center_size(s, vec2(12.0, 12.0)), "anchor point");
    }
    app.auto.add(&format!("viewer.gizmo.{}.origin", layer.id.0), Rect::from_center_size(s, vec2(12.0, 12.0)), "3D gizmo origin");

    // Draw rotation arcs first so translation arrow tips sit crisply on top.
    for rh in gizmo.rotate {
        let col = rh.axis.color();
        painter.add(egui::Shape::line(rh.polyline.clone(), Stroke::new(1.75, col.gamma_multiply(0.9))));
        painter.circle_filled(rh.handle, 3.2, col);
        painter.circle_stroke(rh.handle, 3.2, Stroke::new(1.0, Color32::from_black_alpha(140)));
        if !layer.switches.locked {
            let name = rh.axis.name();
            app.auto.add(&format!("viewer.gizmo.{}.rotate.{name}", layer.id.0), Rect::from_center_size(rh.handle, vec2(12.0, 12.0)), "3D rotation axis");
            rot_hits.push(rh);
        }
    }

    // Draw translation arrows.
    for (axis, so, se, ax, ay) in gizmo.translate {
        let col = axis.color();
        let name = axis.name();
        painter.arrow(so, se - so, Stroke::new(2.0, col));
        if !layer.switches.locked {
            axis_hits.push((layer.id, [so + (se - so) * 0.35, se], ax, ay));
            let tip_rect = Rect::from_center_size(se, vec2(12.0, 12.0));
            app.auto.add(&format!("viewer.gizmo.{}.{name}", layer.id.0), tip_rect, "3D axis");
            app.auto.add(&format!("viewer.gizmo.{}.translate.{name}", layer.id.0), tip_rect, "3D translation axis");
        }
    }
}
