//! Raytraced 3D evaluation and vector-facet Substance material shading for `vectorcraft-effects`.

use std::io::Cursor;
use std::sync::Arc;

use serde_json::Value;
use vectorcraft_doc::color::{BlendMode, Color, Paint};
use vectorcraft_doc::{Appearance, AppearanceItem, Document, Effect, FillLayer, ImageBlob, ImageObject, Node, NodeId, NodeKind};
use vectorcraft_geom::{Affine, FillRule, PathData, Point, Rect};
use vectorcraft_three_d::{
    Facet, Mesh, RaytraceImage, RaytracePart, RaytraceScene, Revolve, SubstanceMaterial, View, raytrace_scene, revolve_mesh, shade_facet_substance,
};

use crate::solids::{EXTRUDE, INFLATE, Profile, ROTATE, extrude_options, inflate_options, mesh_color, union_bounds, view_options};
use crate::util::{flag, num};

pub(crate) fn is_raytraced(p: &Value) -> bool {
    flag(p, "raytrace", false)
}

pub(crate) fn ao_strength(p: &Value) -> f64 {
    (num(p, "ambientOcclusion", 50.0) / 100.0).clamp(0.0, 1.0)
}

pub(crate) fn doc_ppi(doc: Option<&Document>) -> f64 {
    doc.map_or(72.0, |d| d.raster_effects_ppi).clamp(36.0, 300.0)
}

/// Encode a [`RaytraceImage`] into a standalone [`NodeKind::Image`] holding PNG pixels.
pub(crate) fn image_node(id: NodeId, img: RaytraceImage) -> Option<Node> {
    let w = img.width.max(1);
    let h = img.height.max(1);
    let rgba = image::RgbaImage::from_raw(w, h, img.rgba)?;
    let mut png = Vec::new();
    rgba.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).ok()?;
    let blob = ImageBlob::png(png);
    let sx = img.bounds.width() / f64::from(w);
    let sy = img.bounds.height() / f64::from(h);
    let xf = Affine::translate((img.bounds.x0, img.bounds.y0)) * Affine::scale_non_uniform(sx, sy);
    let obj =
        ImageObject { key: blob.content_key(), width: w, height: h, xf, link: None, placement: Default::default(), pixels: Some(Arc::new(blob)) };
    Some(Node::new(id, NodeKind::Image(Box::new(obj))))
}

/// Raytrace `EXTRUDE`, `INFLATE`, or `ROTATE` for `shapes` into an image node (with optional
/// planar mapped graphics layered above it).
pub(crate) fn raytrace_solid(doc: Option<&Document>, n: &Node, fx: &Effect, shapes: &[Profile], solid_meshes: Vec<crate::SolidMesh>) -> Option<Node> {
    let b = union_bounds(shapes)?;
    let p = crate::merged_params(&fx.id, &fx.params);
    let view = view_options(&p);
    let material = crate::lighting::substance_material(&p);
    let shadow = crate::solids::shadow::options(&p);
    let parts: Vec<RaytracePart> = solid_meshes
        .into_iter()
        .filter(|m| !m.mesh.is_empty())
        .map(|m| RaytracePart {
            mesh: m.mesh,
            base_rgb: [f64::from(m.color[0]), f64::from(m.color[1]), f64::from(m.color[2])],
            opacity: f64::from(m.color[3]),
        })
        .collect();
    if parts.is_empty() {
        return None;
    }

    let (_, part_bounds) = crate::solids::solid_parts_bounds(shapes, b);
    let mut scene = if fx.id == INFLATE {
        let pairs: Vec<(PathData, FillRule)> = crate::solids::inflate_part_regions(shapes);
        let refs: Vec<(&PathData, FillRule)> = pairs.iter().map(|(pa, r)| (pa, *r)).collect();
        RaytraceScene::for_inflate(parts, &refs, part_bounds, inflate_options(&p), view, material, shadow)
    } else if fx.id == ROTATE {
        RaytraceScene::for_solid(parts, b, 0.0, view, material, shadow)
    } else {
        let ext = extrude_options(&p);
        RaytraceScene::for_solid(parts, part_bounds, ext.depth, view, material, shadow)
    };
    scene.ambient_occlusion = ao_strength(&p);
    scene.ppi = doc_ppi(doc);

    let img = raytrace_scene(&scene).ok()?;
    image_node(n.id, img)
}

/// Raytrace `REVOLVE` on a single path and appearance item.
#[allow(clippy::too_many_arguments)]
pub(crate) fn raytrace_revolve_single(
    doc: Option<&Document>,
    id: NodeId,
    path: &PathData,
    rule: FillRule,
    bounds: Rect,
    options: Revolve,
    fx: &Effect,
    paint: &Paint,
    opacity: f32,
    blend: BlendMode,
) -> Option<Node> {
    let p = crate::merged_params(&fx.id, &fx.params);
    let mesh: Mesh = revolve_mesh(path, rule, bounds, options).ok()?;
    if mesh.is_empty() {
        return None;
    }
    let col = mesh_color(paint, 1.0);
    let part = RaytracePart { mesh, base_rgb: [f64::from(col[0]), f64::from(col[1]), f64::from(col[2])], opacity: f64::from(col[3]) };
    let material = crate::lighting::substance_material(&p);
    let shadow = crate::solids::shadow::options(&p);
    let mut scene = RaytraceScene::for_revolve(vec![part], bounds, options, material, shadow);
    scene.ambient_occlusion = ao_strength(&p);
    scene.ppi = doc_ppi(doc);
    let img = raytrace_scene(&scene).ok()?;
    let mut node = image_node(id, img)?;
    node.opacity = opacity;
    node.blend = blend;
    Some(node)
}

/// Raytrace `REVOLVE` on a group's members sharing one revolve axis.
pub(crate) fn raytrace_revolve_group(
    doc: Option<&Document>,
    id: NodeId,
    shapes: &[Profile],
    bounds: Rect,
    options: Revolve,
    fx: &Effect,
) -> Option<Node> {
    let p = crate::merged_params(&fx.id, &fx.params);
    let mut parts = Vec::new();
    for s in shapes {
        let Some(item) = crate::revolve::surface_item(&s.items) else { continue };
        let (paint, item_op, _) = match item {
            AppearanceItem::Fill(f) => (&f.paint, f.opacity, f.blend),
            AppearanceItem::Stroke(st) => (&st.paint, st.opacity, st.blend),
        };
        let mesh = revolve_mesh(&s.path, s.rule, bounds, options).ok()?;
        if mesh.is_empty() {
            continue;
        }
        let col = mesh_color(paint, item_op * s.opacity);
        parts.push(RaytracePart { mesh, base_rgb: [f64::from(col[0]), f64::from(col[1]), f64::from(col[2])], opacity: f64::from(col[3]) });
    }
    if parts.is_empty() {
        return None;
    }
    let material = crate::lighting::substance_material(&p);
    let shadow = crate::solids::shadow::options(&p);
    let mut scene = RaytraceScene::for_revolve(parts, bounds, options, material, shadow);
    scene.ambient_occlusion = ao_strength(&p);
    scene.ppi = doc_ppi(doc);
    let img = raytrace_scene(&scene).ok()?;
    image_node(id, img)
}

fn inv_rotate_view(rot: [f64; 3], v: [f64; 3]) -> [f64; 3] {
    let (sx, cx) = rot[0].to_radians().sin_cos();
    let (sy, cy) = rot[1].to_radians().sin_cos();
    let (sz, cz) = rot[2].to_radians().sin_cos();
    let z = [cz * v[0] + sz * v[1], -sz * v[0] + cz * v[1], v[2]];
    let y = [cy * z[0] - sy * z[2], z[1], sy * z[0] + cy * z[2]];
    [y[0], cx * y[1] + sx * y[2], -sx * y[1] + cx * y[2]]
}

fn active_lights_view(view: View) -> Vec<([f64; 3], [f64; 3], f64)> {
    if let Some(lights) = view.lights {
        lights.iter().map(|l| (l.direction(), l.color, l.intensity)).collect()
    } else {
        let rev =
            Revolve { light_rotation: view.light_rotation, light_height: view.light_height, light_behind: view.light_behind, ..Revolve::default() };
        vec![(rev.light_direction(), view.light_color, view.light_intensity)]
    }
}

/// Shade vector facets with [`SubstanceMaterial`] when `substancePreset != "none"` and
/// `raytrace == false`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn substance_vector_surface(
    id: NodeId,
    facets: Vec<Facet>,
    paint: &Paint,
    opacity: f32,
    blend: BlendMode,
    bounds: Rect,
    p: &Value,
    material: SubstanceMaterial,
) -> Node {
    let view = view_options(p);
    let lights = active_lights_view(view);
    let col = mesh_color(paint, 1.0);
    let base_rgb = [f64::from(col[0]), f64::from(col[1]), f64::from(col[2])];
    let origin = bounds.center();
    let inv_scale = 1.0 / (bounds.width().max(bounds.height()) * 0.5).max(1.0);
    let rot = [view.rotation_x, view.rotation_y, view.rotation_z];

    let children = facets
        .into_iter()
        .map(|face| {
            let mut sum_x = 0.0;
            let mut sum_y = 0.0;
            let mut cnt = 0usize;
            for pt in face.contours.iter().flatten() {
                sum_x += pt.x;
                sum_y += pt.y;
                cnt += 1;
            }
            let center = if cnt > 0 { Point::new(sum_x / cnt as f64, sum_y / cnt as f64) } else { origin };
            let z = if face.depth.is_finite() && face.depth.abs() < 1e6 { face.depth } else { 0.0 };
            let v_pos = [center.x - origin.x, center.y - origin.y, z];
            let o_pos_down = inv_rotate_view(rot, v_pos);
            let o_pos = [o_pos_down[0] * inv_scale, -o_pos_down[1] * inv_scale, o_pos_down[2] * inv_scale];
            let o_n_down = inv_rotate_view(rot, face.normal);
            let o_n = [o_n_down[0], -o_n_down[1], o_n_down[2]];

            let rgb = if view.shade {
                shade_facet_substance(material, base_rgb, o_pos, o_n, face.normal, &lights, view.ambient)
            } else {
                material.sample_3d(o_pos, o_n, face.normal, base_rgb, 0.35).albedo
            };
            let shaded = Paint::solid(Color::rgb(rgb[0] as f32, rgb[1] as f32, rgb[2] as f32));
            let seam = (face.thickness() / 2.0).clamp(0.05, 0.75);
            let edge = crate::lighting::seam_stroke(shaded.clone(), seam);
            let items = vec![AppearanceItem::Fill(FillLayer::new(shaded)), AppearanceItem::Stroke(edge)];
            Arc::new(Node::path(id, face.path(), Appearance { items, ..Default::default() }))
        })
        .collect();

    let mut group = Node::group(id, children);
    group.opacity = opacity * col[3];
    group.blend = blend;
    let _ = EXTRUDE;
    group
}
