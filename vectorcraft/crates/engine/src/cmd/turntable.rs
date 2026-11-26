//! Object → Turntable (`object.turntable` — Illustrator 2027 `31.0` `TurntableGR6` / `TurntableV8`):
//! generates a multi-angle 2D/3D vector turnaround sheet from the selected artwork with optional
//! orthographic/isometric/perspective projection, PBR metallic-roughness shading, degree labels,
//! and dedicated turnaround artboard framing in a single Undo step.

use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{
    Appearance, AppearanceItem, Artboard, CharStyle, Document, FillLayer, Node, NodeKind,
    StrokeLayer, TextObject,
};
use vectorcraft_geom::{Affine, FillRule, PathData, Point, Rect, Vec2};
use vectorcraft_three_d::{Extrude, Facet, PbrMaterial, View, extrude};

use super::edit::{outermost, selected_roots};
use super::{
    CommandSpec, EngineError, Result, Session, bad, cmd, given_objects, has_selection,
    number_or, switch_or, text_param, unique_name,
};

const CMD: &str = "object.turntable";
const ISO_PITCH_DEG: f64 = 35.264_389_682_754_65;
const KEY_LIGHT_DIR: [f64; 3] = [-0.5, -0.707_106_781_186_547_5, 0.5];

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "object.turntable",
        "Turntable…",
        ["Object"],
        None,
        "{ids?|id?, angles?: int=8, elevation?: deg=20, projection?: \"orthographic\"|\"isometric\"|\"perspective\", extrudeDepth?: pt=24, metallic?: %=0, roughness?: %=50, spacing?: pt=32, showLabels?: bool=true, createArtboard?: bool=false, keepOriginal?: bool=true} generate a multi-angle 2D/3D turnaround sheet for the selected artwork → {id, views, artboard?}",
        has_selection,
        run_turntable
    )]
}

#[derive(Clone, Copy)]
struct TurntableOpts {
    angles: usize,
    elevation: f64,
    projection: ProjKind,
    extrude_depth: f64,
    metallic: f64,
    roughness: f64,
    spacing: f64,
    show_labels: bool,
    create_artboard: bool,
    keep_original: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProjKind {
    Orthographic,
    Isometric,
    Perspective,
}

struct ProfilePart {
    path: PathData,
    rule: FillRule,
    base_rgb: [f64; 3],
    stroke_width: Option<f64>,
}

fn parse_opts(p: &Value) -> Result<TurntableOpts> {
    let angles_f = number_or(p, "angles", 8.0, CMD)?;
    if !angles_f.is_finite() || !(2.0..=24.0).contains(&angles_f) || angles_f.fract() != 0.0 {
        return Err(bad(CMD, "`angles` must be an integer between 2 and 24"));
    }
    let elevation = number_or(p, "elevation", 20.0, CMD)?;
    if !elevation.is_finite() || !(-89.0..=89.0).contains(&elevation) {
        return Err(bad(CMD, "`elevation` must be between -89 and 89 degrees"));
    }
    let projection = match text_param(p, "projection", CMD)?.unwrap_or("orthographic") {
        "orthographic" => ProjKind::Orthographic,
        "isometric" => ProjKind::Isometric,
        "perspective" => ProjKind::Perspective,
        other => {
            return Err(bad(
                CMD,
                format!("unknown projection `{other}` (orthographic|isometric|perspective)"),
            ));
        }
    };
    let extrude_depth = number_or(p, "extrudeDepth", 24.0, CMD)?;
    if !extrude_depth.is_finite() || !(0.0..=500.0).contains(&extrude_depth) {
        return Err(bad(CMD, "`extrudeDepth` must be between 0 and 500 pt"));
    }
    let metallic = number_or(p, "metallic", 0.0, CMD)?;
    if !metallic.is_finite() || !(0.0..=100.0).contains(&metallic) {
        return Err(bad(CMD, "`metallic` must be between 0 and 100"));
    }
    let roughness = number_or(p, "roughness", 50.0, CMD)?;
    if !roughness.is_finite() || !(0.0..=100.0).contains(&roughness) {
        return Err(bad(CMD, "`roughness` must be between 0 and 100"));
    }
    let spacing = number_or(p, "spacing", 32.0, CMD)?;
    if !spacing.is_finite() || !(0.0..=1000.0).contains(&spacing) {
        return Err(bad(CMD, "`spacing` must be between 0 and 1000 pt"));
    }
    Ok(TurntableOpts {
        angles: angles_f as usize,
        elevation,
        projection,
        extrude_depth,
        metallic,
        roughness,
        spacing,
        show_labels: switch_or(p, "showLabels", true, CMD)?,
        create_artboard: switch_or(p, "createArtboard", false, CMD)?,
        keep_original: switch_or(p, "keepOriginal", true, CMD)?,
    })
}

fn paint_rgb(paint: &Paint) -> Option<[f64; 3]> {
    match paint {
        Paint::None => None,
        Paint::Solid { color: c, .. } => {
            let [r, g, b] = c.to_rgb();
            Some([f64::from(r), f64::from(g), f64::from(b)])
        }
        Paint::Gradient(g) => g.gradient.stops.first().map(|st| {
            let [r, gr, b] = st.color.to_rgb();
            [f64::from(r), f64::from(gr), f64::from(b)]
        }),
        Paint::Pattern { .. } => Some([0.55, 0.58, 0.64]),
    }
}

fn appearance_rgb_and_stroke(app: &Appearance) -> ([f64; 3], Option<f64>) {
    if let Some(f) = app.fill().filter(|f| f.visible)
        && let Some(rgb) = paint_rgb(&f.paint)
    {
        return (rgb, None);
    }
    if let Some(s) = app.stroke().filter(|s| s.visible && s.width > 0.0)
        && let Some(rgb) = paint_rgb(&s.paint)
    {
        return (rgb, Some(s.width));
    }
    ([0.52, 0.56, 0.64], None)
}

fn collect_parts(node: &Node, out: &mut Vec<ProfilePart>) {
    if !node.visible {
        return;
    }
    match &node.kind {
        NodeKind::Path { path, rule, clipping: false, guide: false, .. } => {
            if !path.subpaths.is_empty() {
                let (base_rgb, stroke_width) = appearance_rgb_and_stroke(&node.appearance);
                out.push(ProfilePart { path: path.clone(), rule: *rule, base_rgb, stroke_width });
            }
        }
        NodeKind::Compound { children, rule } => {
            let mut merged = PathData::new(Vec::new());
            for ch in children {
                if let NodeKind::Path { path, .. } = &ch.kind {
                    merged.subpaths.extend(path.subpaths.iter().cloned());
                }
            }
            if !merged.subpaths.is_empty() {
                let (base_rgb, stroke_width) = appearance_rgb_and_stroke(&node.appearance);
                out.push(ProfilePart { path: merged, rule: *rule, base_rgb, stroke_width });
            }
        }
        _ => {
            if let Some(children) = node.children() {
                for ch in children {
                    collect_parts(ch, out);
                }
            }
        }
    }
}

fn build_facet_appearance(rgb: [f64; 3]) -> Appearance {
    let col = Color::rgb(rgb[0] as f32, rgb[1] as f32, rgb[2] as f32);
    Appearance {
        items: vec![
            AppearanceItem::Fill(FillLayer::new(Paint::solid(col))),
            AppearanceItem::Stroke(StrokeLayer::new(Paint::solid(col), 0.25)),
        ],
        ..Appearance::default()
    }
}

fn shade_facet(facet: &Facet, base_rgb: [f64; 3], pbr: PbrMaterial, view: View) -> [f64; 3] {
    if pbr.is_default() {
        pbr.shade_lit_rgb(base_rgb, facet.illumination, view.ambient)
    } else {
        let lights = [(KEY_LIGHT_DIR, view.light_color, view.light_intensity)];
        pbr.shade_normal_rgb(base_rgb, facet.normal, &lights, view.ambient)
    }
}

fn foreshortened_fallback(part: &ProfilePart, center: Point, yaw_deg: f64, pitch_deg: f64) -> (PathData, Appearance) {
    let sx = yaw_deg.to_radians().cos().clamp(-1.0, 1.0);
    let sx = if sx.abs() < 0.08 { 0.08 * sx.signum() } else { sx };
    let sy = pitch_deg.to_radians().cos().clamp(0.15, 1.0);
    let xf = Affine::translate(Vec2::new(center.x, center.y))
        * Affine::scale_non_uniform(sx, sy)
        * Affine::translate(Vec2::new(-center.x, -center.y));
    let path = part.path.transformed(xf);
    let col = Color::rgb(part.base_rgb[0] as f32, part.base_rgb[1] as f32, part.base_rgb[2] as f32);
    let app = match part.stroke_width {
        Some(w) => Appearance::basic(Paint::None, Paint::solid(col), w),
        None => Appearance { items: vec![AppearanceItem::Fill(FillLayer::new(Paint::solid(col)))], ..Appearance::default() },
    };
    (path, app)
}

fn build_view_shapes(
    parts: &[ProfilePart],
    orig_bounds: Rect,
    opts: TurntableOpts,
    yaw_deg: f64,
) -> Vec<(PathData, FillRule, Appearance)> {
    let wrapped_yaw = if yaw_deg > 180.0 { yaw_deg - 360.0 } else { yaw_deg };
    let (pitch, persp) = match opts.projection {
        ProjKind::Isometric => (ISO_PITCH_DEG, 0.0),
        ProjKind::Perspective => (opts.elevation, 35.0),
        ProjKind::Orthographic => (opts.elevation, 0.0),
    };
    let view = View {
        rotation_x: -pitch,
        rotation_y: wrapped_yaw,
        rotation_z: 0.0,
        perspective: persp,
        shade: true,
        ..View::default()
    };
    let ext = Extrude { depth: opts.extrude_depth, cap: true, ..Extrude::default() };
    let pbr = PbrMaterial::new(opts.metallic / 100.0, opts.roughness / 100.0);
    let mut shapes_out = Vec::new();

    for part in parts {
        if part.stroke_width.is_none()
            && let Ok(facets) = extrude(&part.path, part.rule, orig_bounds, ext, view)
            && !facets.is_empty()
        {
            for facet in &facets {
                let rgb = shade_facet(facet, part.base_rgb, pbr, view);
                shapes_out.push((facet.path(), FillRule::NonZero, build_facet_appearance(rgb)));
            }
        } else {
            let (p, app) = foreshortened_fallback(part, orig_bounds.center(), wrapped_yaw, pitch);
            shapes_out.push((p, part.rule, app));
        }
    }
    shapes_out
}

fn shapes_bounds(shapes: &[(PathData, FillRule, Appearance)], fallback: Rect) -> Rect {
    let mut acc: Option<Rect> = None;
    for (path, ..) in shapes {
        if let Some(b) = path.bounds() {
            acc = Some(match acc {
                Some(r) => r.union(b),
                None => b,
            });
        }
    }
    acc.unwrap_or(fallback)
}

fn make_angle_label(d: &mut Document, center_x: f64, baseline_y: f64, deg: i32) -> Arc<Node> {
    let style = CharStyle {
        size: 9.0,
        fill: Paint::solid(Color::rgb(0.32, 0.34, 0.38)),
        ..CharStyle::default()
    };
    let label = format!("{deg}°");
    let mut txt = TextObject::point(Point::new(center_x, baseline_y), &label, style);
    txt.para.justify = vectorcraft_doc::Justify::Center;
    super::typecmd::refresh_bounds(&mut txt);
    Arc::new(Node::new(d.alloc_id(), NodeKind::Text(Box::new(txt))))
}

fn run_turntable(s: &mut Session, p: &Value) -> Result<Value> {
    let opts = parse_opts(p)?;
    let roots = match given_objects(s, p, CMD)? {
        Some(ids) => outermost(&s.doc()?.doc, ids),
        None => selected_roots(s)?,
    };
    let st = s.doc()?;
    let orig_bounds = st.doc.bounds_of(&roots, true).ok_or_else(|| bad(CMD, "the selection has no bounds"))?;
    if orig_bounds.width() <= 0.0 && orig_bounds.height() <= 0.0 {
        return Err(bad(CMD, "the selection has no area"));
    }

    let mut parts = Vec::new();
    for id in &roots {
        if let Some(node) = st.doc.node(*id) {
            collect_parts(node, &mut parts);
        }
    }
    if parts.is_empty() {
        return Err(bad(CMD, "the selection contains no vector paths for Turntable"));
    }

    type TurntableView = (i32, Vec<(PathData, FillRule, Appearance)>, Rect);
    let prebuilt: Vec<TurntableView> = (0..opts.angles)
        .map(|i| {
            let yaw = (i as f64) * 360.0 / (opts.angles as f64);
            let shapes = build_view_shapes(&parts, orig_bounds, opts, yaw);
            let b = shapes_bounds(&shapes, orig_bounds);
            (yaw.round() as i32, shapes, b)
        })
        .collect();

    let max_h = prebuilt.iter().map(|(_, _, b)| b.height()).fold(orig_bounds.height().max(12.0), f64::max);
    let start_x = orig_bounds.x0;
    let row_center_y = if opts.keep_original {
        orig_bounds.y1 + opts.spacing.max(24.0) + max_h * 0.5
    } else {
        orig_bounds.y0 + max_h * 0.5
    };
    let top_root = *roots.last().ok_or_else(|| bad(CMD, "nothing selected"))?;

    let (group_id, ab_index) = s.edit("Turntable", |d, sel| {
        let mut cursor_x = start_x;
        let mut view_groups = Vec::with_capacity(prebuilt.len());
        let mut sheet_bounds: Option<Rect> = None;

        for (deg, shapes, vb) in prebuilt {
            let cell_w = vb.width().max(20.0);
            let target_cx = cursor_x + cell_w * 0.5;
            let dx = target_cx - vb.center().x;
            let dy = row_center_y - vb.center().y;
            let shift = Affine::translate(Vec2::new(dx, dy));

            let mut children = Vec::with_capacity(shapes.len() + 1);
            for (mut path, rule, app) in shapes {
                path.transform(shift);
                let mut n = Node::path(d.alloc_id(), path, app);
                if let NodeKind::Path { rule: r, .. } = &mut n.kind {
                    *r = rule;
                }
                children.push(Arc::new(n));
            }
            let moved_rect = Rect::new(vb.x0 + dx, vb.y0 + dy, vb.x1 + dx, vb.y1 + dy);
            let mut cell_rect = moved_rect;
            if opts.show_labels {
                let label_y = row_center_y + max_h * 0.5 + 14.0;
                children.push(make_angle_label(d, target_cx, label_y, deg));
                cell_rect = cell_rect.union(Rect::new(target_cx - 16.0, label_y - 10.0, target_cx + 16.0, label_y + 4.0));
            }
            sheet_bounds = Some(match sheet_bounds {
                Some(r) => r.union(cell_rect),
                None => cell_rect,
            });
            let mut vg = Node::group(d.alloc_id(), children);
            vg.name = Some(format!("View {deg}°"));
            view_groups.push(Arc::new(vg));
            cursor_x += cell_w + opts.spacing;
        }

        let gid = d.alloc_id();
        let mut root_group = Node::group(gid, view_groups);
        root_group.name = Some("Turntable".into());

        if opts.keep_original {
            let (par, idx, _) = d.position(top_root).ok_or(EngineError::NoNode(top_root))?;
            d.insert(par, idx + 1, root_group)?;
        } else {
            d.wrap(&roots, |_| root_group)?;
        }

        let ab_idx = if opts.create_artboard {
            let pad = opts.spacing.max(24.0);
            let rect = sheet_bounds.unwrap_or(orig_bounds).inflate(pad, pad);
            let ab_id = d.next_artboard_id();
            let ab_name = unique_name("Turntable", |cand| d.artboards.iter().any(|a| a.name == cand));
            d.artboards.push(Artboard {
                id: ab_id,
                name: ab_name,
                rect,
                locked: false,
                show_center_mark: false,
                show_cross_hairs: false,
                show_video_safe_areas: false,
                video_ruler_pixel_aspect_ratio: 1.0,
                color: None,
                guide_label: None,
            });
            Some(d.artboards.len() - 1)
        } else {
            None
        };

        sel.set([gid]);
        Ok((gid, ab_idx))
    })?;

    let mut out = json!({ "id": group_id.0, "views": opts.angles });
    if let Some(idx) = ab_index {
        out["artboard"] = json!(idx);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use crate::Session;

    #[test]
    fn turntable_generates_multi_angle_views_artboard_and_supports_undo_redo() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 800, "height": 600})).unwrap();
        let r = s.execute("shape.star", &json!({"cx": 120, "cy": 120, "radius1": 50, "radius2": 24, "points": 5})).unwrap();
        let star_id = r["id"].as_u64().unwrap();
        s.execute("paint.setFill", &json!({"ids": [star_id], "color": "#e06c3a"})).unwrap();

        for angles in [4, 6, 8, 12] {
            let res = s
                .execute(
                    "object.turntable",
                    &json!({
                        "ids": [star_id],
                        "angles": angles,
                        "projection": if angles == 8 { "isometric" } else { "orthographic" },
                        "extrudeDepth": 20,
                        "metallic": 65,
                        "roughness": 25,
                        "createArtboard": angles == 8,
                        "keepOriginal": true
                    }),
                )
                .unwrap();
            assert_eq!(res["views"], angles);
            let gid = vectorcraft_doc::NodeId(res["id"].as_u64().unwrap());
            let group = s.active().unwrap().doc.node(gid).unwrap();
            assert_eq!(group.children().unwrap().len(), angles as usize);
            if angles == 8 {
                assert_eq!(res["artboard"], 1);
                assert_eq!(s.active().unwrap().doc.artboards.len(), 2);
            }
            s.execute("edit.undo", &json!({})).unwrap();
            assert!(s.active().unwrap().doc.node(gid).is_none());
            s.execute("edit.redo", &json!({})).unwrap();
            assert!(s.active().unwrap().doc.node(gid).is_some());
            s.execute("edit.undo", &json!({})).unwrap();
        }
    }
}
