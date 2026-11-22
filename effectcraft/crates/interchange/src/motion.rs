//! Apple Motion (`.motn`, `.moti`, `.motr`, `.moef`) `<ozml>` project import and export.
//!
//! Bridges Apple Motion 5 `<ozml>` projects — including those exported from
//! `photocraft-doc::motion` and `vectorcraft-doc::motion` — into EffectCraft [`Comp`]s:
//! - `<layer type="Group" | "Text" | "Shape" | "Solid" | "Footage" | "DropZone">`
//! - `<shape>` with `<fill>`, `<stroke>`, and cubic Bézier `<path><knot .../></path>`
//! - `<text>` with `<string>`, `<font>`, `<size>`, `<alignment>`, `<color>`, `<tracking>`, `<leading>`
//! - `<behavior type="Oscillate" | "Spin" | "Throw" | "FadeInFadeOut">` → procedural expressions
//!   and keyframes on target layer transform channels
//! - `<publishedParameters>` and `DropZone` footage layers → [`EssentialGraphics`] controls
//!   ([`EgControl`], [`EgKind::Property`], [`EgKind::Media`]) for Final Cut Pro / FilmCraft
//!   template rigging.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::sync::Arc;

use effectcraft_color::{BlendMode, Label};
use effectcraft_keyframe::{Justify, Keyframe, ShapePath, TextDoc, Value};
use effectcraft_project::build::{
    Ids, layer as build_layer, mask as build_mask, shape_fill, shape_group, shape_path,
    shape_stroke,
};
use effectcraft_project::essential::{EgControl, EgKind};
use effectcraft_project::props::{Expression, GroupKind, MaskMode, Node, PropGroup};
use effectcraft_project::{
    AlphaMode, Comp, Footage, FootageKind, ItemId, ItemKind, Layer, LayerId, LayerSource, Project,
    Solid, Uid,
};
use effectcraft_time::{FrameRate, Tick};

use crate::import::{ImportOptions, ImportResult, MAX_DEPTH, xml_depth};
use crate::{Error, Result};

/// Returns `true` when `bytes` looks like an Apple Motion `<ozml>` XML document
/// (`.motn`, `.moti`, `.motr`, `.moef`).
#[must_use]
pub fn is_motion_ozml(bytes: &[u8]) -> bool {
    let head_len = bytes.len().min(2048);
    let Ok(head) = std::str::from_utf8(&bytes[..head_len]) else {
        return false;
    };
    head.contains("<ozml") || head.contains("<!DOCTYPE ozxmlscene")
}

/// Import an Apple Motion `<ozml>` document into `project`.
pub fn import_motion_ozml(
    project: &mut Project,
    bytes: &[u8],
    opts: &ImportOptions,
) -> Result<ImportResult> {
    if xml_depth(bytes) > MAX_DEPTH {
        return Err(Error::TooDeep(MAX_DEPTH));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| Error::UnknownFormat)?;
    let root = parse_xml(text).ok_or(Error::UnknownFormat)?;
    let ozml = if root.tag == "ozml" {
        &root
    } else {
        root.child("ozml").ok_or(Error::UnknownFormat)?
    };
    let scene = ozml.child("scene").unwrap_or(ozml);

    let settings = scene.child("sceneSettings");
    let width = settings
        .and_then(|s| s.child_text("width"))
        .and_then(|v| v.trim().parse::<u32>().ok())
        .unwrap_or(1920)
        .max(1);
    let height = settings
        .and_then(|s| s.child_text("height"))
        .and_then(|v| v.trim().parse::<u32>().ok())
        .unwrap_or(1080)
        .max(1);
    let fps_f64 = settings
        .and_then(|s| s.child_text("frameRate"))
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(60.0);
    let rate = FrameRate::from_f64(fps_f64);
    let dur_frames = settings
        .and_then(|s| s.child_text("duration"))
        .and_then(|v| v.trim().parse::<i64>().ok())
        .unwrap_or(300)
        .max(1);
    let duration = Tick(dur_frames.saturating_mul(rate.frame_duration().0));

    let folder_name = if opts.name.is_empty() {
        "Apple Motion Import".to_string()
    } else {
        opts.name.clone()
    };
    let mut res = ImportResult {
        folder: project.add_item(&folder_name, Label::None, None, ItemKind::Folder),
        ..Default::default()
    };

    let mut comp = Comp::new(width, height, rate, duration);
    if let Some(bg) = settings.and_then(|s| s.child("backgroundColor")) {
        let r = bg.attr_f32("red").unwrap_or(0.0);
        let g = bg.attr_f32("green").unwrap_or(0.0);
        let b = bg.attr_f32("blue").unwrap_or(0.0);
        comp.background = [r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0)];
    }

    let comp_name = scene
        .child("layer")
        .and_then(|l| l.attr("name"))
        .filter(|n| !n.is_empty())
        .unwrap_or(&folder_name)
        .to_string();
    comp.essential.get_or_insert_with(Default::default).name = comp_name.clone();

    let mut ozml_to_layer: HashMap<u64, LayerId> = HashMap::new();
    let mut eg_next_id = 1_u64;

    for child in &scene.children {
        if child.tag == "layer" {
            import_ozml_layer(
                project,
                &mut comp,
                child,
                res.folder,
                &mut res,
                &mut ozml_to_layer,
                &mut eg_next_id,
            );
        }
    }

    // In Apple Motion / PhotoCraft / VectorCraft `<ozml>`, layers are emitted bottom-to-top inside
    // a group, whereas EffectCraft's `comp.layers[0]` is the topmost layer. Reverse so the top
    // visual layer renders on top.
    comp.layers.reverse();

    if let Some(pub_params) = scene.child("publishedParameters") {
        for param in &pub_params.children {
            if param.tag != "parameter" {
                continue;
            }
            let Some(target_ozml_id) = param.attr_u64("targetLayer") else {
                continue;
            };
            let Some(&layer_id) = ozml_to_layer.get(&target_ozml_id) else {
                continue;
            };
            let channel = param.attr("channel").unwrap_or("");
            let param_name = param
                .attr("name")
                .filter(|s| !s.is_empty())
                .unwrap_or("Published Parameter")
                .to_string();
            if let Some(layer) = comp.layer(layer_id)
                && let Some(prop_uid) = resolve_channel_prop_uid(layer, channel)
            {
                let id = eg_next_id;
                eg_next_id += 1;
                comp.essential
                    .get_or_insert_with(Default::default)
                    .controls
                    .push(EgControl {
                        id,
                        name: param_name,
                        kind: EgKind::Property {
                            layer: layer_id,
                            prop: prop_uid,
                            links: Vec::new(),
                        },
                        as_type: None,
                    });
            }
        }
    }

    let comp_item = project.add_item(
        &comp_name,
        Label::Sandstone,
        Some(res.folder),
        ItemKind::Comp(Arc::new(comp)),
    );
    res.comps.push(comp_item);
    res.all_comps.push(comp_item);
    Ok(res)
}

#[allow(clippy::too_many_arguments)]
fn import_ozml_layer(
    project: &mut Project,
    comp: &mut Comp,
    node: &XmlNode,
    folder: ItemId,
    res: &mut ImportResult,
    ozml_to_layer: &mut HashMap<u64, LayerId>,
    eg_next_id: &mut u64,
) {
    let layer_type = node.attr("type").unwrap_or("Group");
    let name = node.attr("name").unwrap_or("Layer");
    let ozml_id = node.attr_u64("id").unwrap_or(0);

    // Recurse into Group layers that contain child `<layer>` nodes.
    let has_child_layers = node.children.iter().any(|c| c.tag == "layer");
    if layer_type.eq_ignore_ascii_case("Group") && has_child_layers {
        for child in &node.children {
            if child.tag == "layer" {
                import_ozml_layer(
                    project,
                    comp,
                    child,
                    folder,
                    res,
                    ozml_to_layer,
                    eg_next_id,
                );
            }
        }
        return;
    }

    let cw = comp.width;
    let ch = comp.height;
    let mut is_drop_zone = layer_type.eq_ignore_ascii_case("DropZone");

    let mut layer = if node.child("text").is_some() || layer_type.eq_ignore_ascii_case("Text") {
        let mut l = build_layer(project, comp, name, LayerSource::Text, (cw, ch), None);
        if let Some(t) = node.child("text") {
            let text_str = t.child_text("string").unwrap_or("");
            let font = t.child_text("font").unwrap_or("Inter");
            let size = t
                .child_text("size")
                .and_then(|s| s.trim().parse::<f64>().ok())
                .unwrap_or(48.0)
                .clamp(1.0, 4096.0);
            let align = match t.child_text("alignment").unwrap_or("left").trim() {
                "center" => Justify::Center,
                "right" => Justify::Right,
                _ => Justify::Left,
            };
            let tracking = t
                .child_text("tracking")
                .and_then(|s| s.trim().parse::<f64>().ok())
                .unwrap_or(0.0);
            let leading = t
                .child_text("leading")
                .and_then(|s| s.trim().parse::<f64>().ok());
            let fill = if let Some(col) = t.child("color") {
                [
                    col.attr_f32("r").unwrap_or(1.0),
                    col.attr_f32("g").unwrap_or(1.0),
                    col.attr_f32("b").unwrap_or(1.0),
                    col.attr_f32("a").unwrap_or(1.0),
                ]
            } else {
                [1.0, 1.0, 1.0, 1.0]
            };
            let mut doc = TextDoc::plain(text_str);
            doc.font = font.to_string();
            doc.size = size;
            doc.justify = align;
            doc.tracking = tracking;
            doc.leading = leading;
            doc.fill = fill;
            if let Some(st) = l.props.prop_mut("text/sourceText") {
                st.value = Value::Text(Box::new(doc));
            }
        }
        l
    } else if node.child("shape").is_some() || layer_type.eq_ignore_ascii_case("Shape") {
        let mut l = build_layer(project, comp, name, LayerSource::Shape, (cw, ch), None);
        // Shape vertices in `<ozml>` are in canvas/artboard coordinates, so place the shape layer
        // origin at (0, 0) unless overridden by relative transform.
        if let Some(pos) = l.transform_mut().and_then(|t| t.get_mut("position")) {
            pos.value = Value::Vec3([0.0, 0.0, 0.0]);
        }
        if let Some(sh) = node.child("shape") {
            let mut ids = Ids(&mut project.next_id);
            let mut items = Vec::new();
            for p_node in &sh.children {
                if p_node.tag == "path" {
                    let sp = parse_ozml_path(p_node);
                    if !sp.is_empty() {
                        items.push(shape_path(&mut ids, sp));
                    }
                }
            }
            if let Some(st) = sh.child("stroke")
                && st.attr("enabled").unwrap_or("1") != "0"
            {
                let width = st.attr_f64("width").unwrap_or(1.0).max(0.0);
                let r = st.attr_f64("r").unwrap_or(1.0);
                let g = st.attr_f64("g").unwrap_or(1.0);
                let b = st.attr_f64("b").unwrap_or(1.0);
                let a = st.attr_f64("a").unwrap_or(1.0);
                let opacity = st.attr_f64("opacity").unwrap_or(1.0) * 100.0;
                let mut sg = shape_stroke(&mut ids, [r, g, b, a], width);
                if let Some(op_prop) = sg.get_mut("opacity") {
                    op_prop.value = Value::Scalar(opacity.clamp(0.0, 100.0));
                }
                items.push(sg);
            }
            if let Some(fl) = sh.child("fill")
                && fl.attr("enabled").unwrap_or("1") != "0"
            {
                let r = fl.attr_f64("r").unwrap_or(1.0);
                let g = fl.attr_f64("g").unwrap_or(1.0);
                let b = fl.attr_f64("b").unwrap_or(1.0);
                let a = fl.attr_f64("a").unwrap_or(1.0);
                let opacity = fl.attr_f64("opacity").unwrap_or(1.0) * 100.0;
                let mut fg = shape_fill(&mut ids, [r, g, b, a]);
                if let Some(op_prop) = fg.get_mut("opacity") {
                    op_prop.value = Value::Scalar(opacity.clamp(0.0, 100.0));
                }
                if sh
                    .attr("fillRule")
                    .is_some_and(|rule| rule.eq_ignore_ascii_case("EvenOdd"))
                    && let Some(rule_prop) = fg.get_mut("rule")
                {
                    rule_prop.value = Value::Enum(1);
                }
                items.push(fg);
            }
            let grp = shape_group(&mut ids, "Group 1", items);
            if let Some(contents) = l.props.group_mut("contents") {
                contents.children.push(grp.into());
            }
        }
        l
    } else if let Some(sol) = node.child("solid") {
        let sw = sol.attr_u32("width").unwrap_or(cw).max(1);
        let sh = sol.attr_u32("height").unwrap_or(ch).max(1);
        let r = sol.attr_f32("r").unwrap_or(0.0);
        let g = sol.attr_f32("g").unwrap_or(0.0);
        let b = sol.attr_f32("b").unwrap_or(0.0);
        let solid_id = project.add_item(
            name,
            Label::Red,
            Some(folder),
            ItemKind::Solid(Solid {
                color: [r, g, b],
                width: sw,
                height: sh,
                pixel_aspect: 1.0,
            }),
        );
        res.items.push(solid_id);
        build_layer(
            project,
            comp,
            name,
            LayerSource::Solid { item: solid_id },
            (sw, sh),
            None,
        )
    } else if let Some(ft) = node.child("footage") {
        let fw = ft.attr_u32("width").unwrap_or(cw).max(1);
        let fh = ft.attr_u32("height").unwrap_or(ch).max(1);
        let src_path = ft.attr("source").unwrap_or(name).to_string();
        if ft.attr("dropZone") == Some("1") {
            is_drop_zone = true;
        }
        let footage = Footage {
            path: src_path,
            kind: FootageKind::Video,
            width: fw,
            height: fh,
            pixel_aspect: 1.0,
            frame_rate: comp.frame_rate,
            duration: comp.duration,
            has_video: true,
            has_audio: false,
            alpha: AlphaMode::Straight,
            missing: true,
            ..Default::default()
        };
        let ft_id = project.add_item(name, Label::Aqua, Some(folder), ItemKind::Footage(footage));
        res.items.push(ft_id);
        build_layer(
            project,
            comp,
            name,
            LayerSource::Footage { item: ft_id },
            (fw, fh),
            None,
        )
    } else {
        build_layer(project, comp, name, LayerSource::Null, (100, 100), None)
    };

    if node.child_text("enabled") == Some("0") {
        layer.switches.video = false;
    }
    if node.child_text("locked") == Some("1") {
        layer.switches.locked = true;
    }
    if let Some(bm_str) = node.child_text("blendMode") {
        layer.blend_mode = parse_blend_mode(bm_str);
    }
    if let Some(op_str) = node.child_text("opacity")
        && let Ok(op) = op_str.trim().parse::<f64>()
        && let Some(op_prop) = layer.transform_mut().and_then(|t| t.get_mut("opacity"))
    {
        op_prop.value = Value::Scalar((op * 100.0).clamp(0.0, 100.0));
    }

    if let Some(tr) = node.child("transform") {
        if !matches!(layer.source, LayerSource::Shape)
            && let Some(pos_node) = tr.child("position")
        {
            let x = pos_node.attr_f64("x").unwrap_or(cw as f64 * 0.5);
            let y = pos_node.attr_f64("y").unwrap_or(ch as f64 * 0.5);
            let z = pos_node.attr_f64("z").unwrap_or(0.0);
            if let Some(pos_prop) = layer.transform_mut().and_then(|t| t.get_mut("position")) {
                pos_prop.value = Value::Vec3([x, y, z]);
            }
        }
        if let Some(sc_node) = tr.child("scale") {
            let sx = sc_node.attr_f64("x").unwrap_or(100.0);
            let sy = sc_node.attr_f64("y").unwrap_or(100.0);
            let sz = sc_node.attr_f64("z").unwrap_or(100.0);
            if let Some(sc_prop) = layer.transform_mut().and_then(|t| t.get_mut("scale")) {
                sc_prop.value = Value::Vec3([sx, sy, sz]);
            }
        }
        if let Some(rot_node) = tr.child("rotation") {
            let rz = rot_node.attr_f64("z").unwrap_or(0.0);
            if let Some(rot_prop) = layer.transform_mut().and_then(|t| t.get_mut("rotation")) {
                rot_prop.value = Value::Scalar(rz);
            }
        }
    }

    // Vector masks (`<mask closed="...">`)
    let mut mask_idx = 0_usize;
    for child in &node.children {
        if child.tag == "mask" {
            let sp = parse_ozml_path(child);
            if !sp.is_empty() {
                let mut ids = Ids(&mut project.next_id);
                let color = effectcraft_project::build::MASK_COLORS
                    [mask_idx % effectcraft_project::build::MASK_COLORS.len()];
                mask_idx += 1;
                let mg = build_mask(
                    &mut ids,
                    &format!("Mask {mask_idx}"),
                    sp,
                    MaskMode::Add,
                    color,
                );
                if let Some(masks) = layer.props.group_mut("masks") {
                    masks.children.push(mg.into());
                }
            }
        } else if child.tag == "behavior" {
            apply_ozml_behavior(&mut layer, comp, child);
        }
    }

    if is_drop_zone {
        let id = *eg_next_id;
        *eg_next_id += 1;
        comp.essential
            .get_or_insert_with(Default::default)
            .controls
            .push(EgControl {
                id,
                name: format!("{name} Drop Zone"),
                kind: EgKind::Media { layer: layer.id },
                as_type: None,
            });
    }

    if ozml_id > 0 {
        ozml_to_layer.insert(ozml_id, layer.id);
    }
    comp.layers.push(layer);
}

fn parse_ozml_path(path_node: &XmlNode) -> ShapePath {
    let closed = path_node.attr("closed") != Some("0");
    let mut vertices = Vec::new();
    let mut in_tangents = Vec::new();
    let mut out_tangents = Vec::new();
    for knot in &path_node.children {
        if knot.tag != "knot" {
            continue;
        }
        let x = knot.attr_f64("x").unwrap_or(0.0);
        let y = knot.attr_f64("y").unwrap_or(0.0);
        let in_x = knot.attr_f64("inX").unwrap_or(x);
        let in_y = knot.attr_f64("inY").unwrap_or(y);
        let out_x = knot.attr_f64("outX").unwrap_or(x);
        let out_y = knot.attr_f64("outY").unwrap_or(y);
        vertices.push([x, y]);
        in_tangents.push([in_x - x, in_y - y]);
        out_tangents.push([out_x - x, out_y - y]);
    }
    ShapePath {
        vertices,
        in_tangents,
        out_tangents,
        closed,
        feather: Vec::new(),
    }
}

fn apply_ozml_behavior(layer: &mut Layer, comp: &Comp, beh: &XmlNode) {
    let kind = beh.attr("type").unwrap_or("");
    match kind {
        "Oscillate" => {
            let amp = beh.attr_f64("amplitude").unwrap_or(25.0);
            let freq = beh.attr_f64("frequency").unwrap_or(1.0);
            let phase = beh.attr_f64("phase").unwrap_or(0.0);
            let channel = beh.attr("channel").unwrap_or("rotation");
            if channel.contains("position") {
                if let Some(pos) = layer.transform_mut().and_then(|t| t.get_mut("position")) {
                    pos.expr = Some(Expression {
                        text: format!(
                            "value + [0, {amp:.4} * Math.sin(2 * Math.PI * {freq:.4} * time + {phase:.4}), 0]"
                        ),
                        enabled: true,
                    });
                }
            } else if let Some(rot) = layer.transform_mut().and_then(|t| t.get_mut("rotation")) {
                rot.expr = Some(Expression {
                    text: format!(
                        "value + {amp:.4} * Math.sin(2 * Math.PI * {freq:.4} * time + {phase:.4})"
                    ),
                    enabled: true,
                });
            }
        }
        "Spin" => {
            let rate_deg = beh.attr_f64("rate").unwrap_or(45.0);
            if let Some(rot) = layer.transform_mut().and_then(|t| t.get_mut("rotation")) {
                rot.expr = Some(Expression {
                    text: format!("value + {rate_deg:.4} * time"),
                    enabled: true,
                });
            }
        }
        "Throw" => {
            let vx = beh.attr_f64("vx").unwrap_or(100.0);
            let vy = beh.attr_f64("vy").unwrap_or(0.0);
            if let Some(pos) = layer.transform_mut().and_then(|t| t.get_mut("position")) {
                pos.expr = Some(Expression {
                    text: format!("value + [{vx:.4} * time, {vy:.4} * time, 0]"),
                    enabled: true,
                });
            }
        }
        "FadeInFadeOut" => {
            let fade_in_frames = beh.attr_u32("fadeIn").unwrap_or(15) as i64;
            let fade_out_frames = beh.attr_u32("fadeOut").unwrap_or(15) as i64;
            let fd = comp.frame_duration().0;
            let t0 = layer.in_point;
            let t1 = Tick((t0.0 + fade_in_frames * fd).min(layer.out_point.0));
            let t2 = Tick((layer.out_point.0 - fade_out_frames * fd).max(t1.0));
            let t3 = layer.out_point;
            if let Some(op) = layer.transform_mut().and_then(|t| t.get_mut("opacity")) {
                op.keys = vec![
                    Keyframe::new(t0, Value::Scalar(0.0)),
                    Keyframe::new(t1, Value::Scalar(100.0)),
                    Keyframe::new(t2, Value::Scalar(100.0)),
                    Keyframe::new(t3, Value::Scalar(0.0)),
                ];
            }
        }
        _ => {}
    }
}

fn resolve_channel_prop_uid(layer: &Layer, channel: &str) -> Option<Uid> {
    if channel.contains("text/string") || channel.contains("sourceText") {
        return layer.props.prop("text/sourceText").map(|p| p.uid);
    }
    if channel.contains("fill/color")
        && let Some(contents) = layer.props.group("contents")
    {
        return find_first_prop_uid(contents, "color");
    }
    if channel.contains("opacity") {
        return layer.props.prop("transform/opacity").map(|p| p.uid);
    }
    if channel.contains("position") {
        return layer.props.prop("transform/position").map(|p| p.uid);
    }
    if channel.contains("scale") {
        return layer.props.prop("transform/scale").map(|p| p.uid);
    }
    if channel.contains("rotation") {
        return layer.props.prop("transform/rotation").map(|p| p.uid);
    }
    None
}

fn find_first_prop_uid(group: &PropGroup, match_id: &str) -> Option<Uid> {
    for child in &group.children {
        match child {
            Node::Prop(p) if p.match_id == match_id => return Some(p.uid),
            Node::Group(g) => {
                if let Some(uid) = find_first_prop_uid(g, match_id) {
                    return Some(uid);
                }
            }
            Node::Prop(_) => {}
        }
    }
    None
}

/// Export an EffectCraft composition (`comp_id`) to an Apple Motion 5 `<ozml version="5.10">` XML string.
pub fn export_motion_ozml(project: &Project, comp_id: ItemId) -> Result<String> {
    let comp = project.comp(comp_id).ok_or(Error::NoComp(comp_id.0))?;
    let comp_name = project
        .item(comp_id)
        .map_or("EffectCraft Composition", |i| i.name.as_str());
    let fps = comp.frame_rate.as_f64();
    let dur_frames = (comp.duration.0 / comp.frame_duration().0.max(1)).max(1);
    let [bg_r, bg_g, bg_b] = comp.background;

    let mut out = String::with_capacity(8192);
    let _ = writeln!(out, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    let _ = writeln!(out, "<!DOCTYPE ozxmlscene>");
    let _ = writeln!(out, "<ozml version=\"5.10\">");
    let _ = writeln!(out, "  <displayversion>5.8</displayversion>");
    let _ = writeln!(out, "  <factory id=\"1\" uuid=\"effectcraft-ozml-factory\">");
    let _ = writeln!(out, "    <description>ArtCraft EffectCraft Motion Export</description>");
    let _ = writeln!(out, "  </factory>");
    let _ = writeln!(out, "  <scene>");
    let _ = writeln!(out, "    <sceneSettings>");
    let _ = writeln!(out, "      <width>{}</width>", comp.width);
    let _ = writeln!(out, "      <height>{}</height>", comp.height);
    let _ = writeln!(out, "      <duration>{dur_frames}</duration>");
    let _ = writeln!(out, "      <frameRate>{fps:.4}</frameRate>");
    let _ = writeln!(out, "      <NTSC>0</NTSC>");
    let _ = writeln!(out, "      <channels>4</channels>");
    let _ = writeln!(
        out,
        "      <backgroundColor red=\"{bg_r:.4}\" green=\"{bg_g:.4}\" blue=\"{bg_b:.4}\" alpha=\"1.0000\" />"
    );
    let _ = writeln!(out, "    </sceneSettings>");
    let _ = writeln!(
        out,
        "    <layer name=\"{}\" id=\"100\" type=\"Group\">",
        xml_escape(comp_name)
    );

    // Export bottom-to-top so `<ozml>` layer order matches import order.
    for layer in comp.layers.iter().rev() {
        let is_drop_zone = comp
            .essential
            .as_ref()
            .is_some_and(|eg| eg.controls.iter().any(|c| matches!(c.kind, EgKind::Media { layer: lid } if lid == layer.id)));
        write_comp_layer(&mut out, project, comp, layer, is_drop_zone, 3);
    }

    let _ = writeln!(out, "    </layer>");

    if let Some(eg) = comp.essential.as_ref()
        && !eg.controls.is_empty()
    {
        let _ = writeln!(out, "    <publishedParameters>");
        let mut idx = 0_usize;
        for ctrl in &eg.controls {
            if let EgKind::Property { layer, prop, .. } = &ctrl.kind
                && let Some(l) = comp.layer(*layer)
                && let Some(p) = l.props.find(*prop)
            {
                let (channel, kind, def_val) = match &p.value {
                    Value::Text(doc) => ("./text/string", "string", doc.text.clone()),
                    Value::Color([r, g, b, a]) => (
                        "./shape/fill/color",
                        "color",
                        format!("{r:.4},{g:.4},{b:.4},{a:.4}"),
                    ),
                    Value::Scalar(v) => ("./transform/opacity", "float", format!("{v:.4}")),
                    _ => ("./transform/position", "point", String::new()),
                };
                let _ = writeln!(
                    out,
                    "      <parameter index=\"{idx}\" name=\"{}\" targetLayer=\"{}\" channel=\"{channel}\" type=\"{kind}\" default=\"{}\" />",
                    xml_escape(&ctrl.name),
                    layer.0,
                    xml_escape(&def_val),
                );
                idx += 1;
            }
        }
        let _ = writeln!(out, "    </publishedParameters>");
    }

    let _ = writeln!(out, "  </scene>");
    let _ = writeln!(out, "</ozml>");
    Ok(out)
}

fn write_comp_layer(
    out: &mut String,
    project: &Project,
    comp: &Comp,
    layer: &Layer,
    is_drop_zone: bool,
    indent: usize,
) {
    let pad = "  ".repeat(indent);
    let layer_type = match &layer.source {
        LayerSource::Text => "Text",
        LayerSource::Shape => "Shape",
        LayerSource::Solid { .. } => "Solid",
        LayerSource::Footage { .. } => {
            if is_drop_zone {
                "DropZone"
            } else {
                "Footage"
            }
        }
        _ => "Group",
    };
    let enabled = u8::from(layer.switches.video);
    let locked = u8::from(layer.switches.locked);
    let blend = blend_mode_name(layer.blend_mode);
    let opacity = layer
        .props
        .prop("transform/opacity")
        .and_then(|p| match p.value {
            Value::Scalar(v) => Some((v / 100.0).clamp(0.0, 1.0)),
            _ => None,
        })
        .unwrap_or(1.0);
    let [pos_x, pos_y, pos_z] = layer
        .props
        .prop("transform/position")
        .and_then(|p| match p.value {
            Value::Vec3(v) => Some(v),
            _ => None,
        })
        .unwrap_or([comp.width as f64 * 0.5, comp.height as f64 * 0.5, 0.0]);
    let [sc_x, sc_y, sc_z] = layer
        .props
        .prop("transform/scale")
        .and_then(|p| match p.value {
            Value::Vec3(v) => Some(v),
            _ => None,
        })
        .unwrap_or([100.0, 100.0, 100.0]);
    let rot_z = layer
        .props
        .prop("transform/rotation")
        .and_then(|p| match p.value {
            Value::Scalar(v) => Some(v),
            _ => None,
        })
        .unwrap_or(0.0);

    let _ = writeln!(
        out,
        "{pad}<layer name=\"{}\" id=\"{}\" type=\"{layer_type}\">",
        xml_escape(&layer.name),
        layer.id.0
    );
    let _ = writeln!(out, "{pad}  <enabled>{enabled}</enabled>");
    let _ = writeln!(out, "{pad}  <locked>{locked}</locked>");
    let _ = writeln!(out, "{pad}  <blendMode>{blend}</blendMode>");
    let _ = writeln!(out, "{pad}  <opacity>{opacity:.4}</opacity>");
    let _ = writeln!(out, "{pad}  <transform>");
    let _ = writeln!(
        out,
        "{pad}    <position x=\"{pos_x:.3}\" y=\"{pos_y:.3}\" z=\"{pos_z:.3}\" />"
    );
    let _ = writeln!(
        out,
        "{pad}    <scale x=\"{sc_x:.4}\" y=\"{sc_y:.4}\" z=\"{sc_z:.4}\" />"
    );
    let _ = writeln!(out, "{pad}    <rotation z=\"{rot_z:.4}\" />");
    let _ = writeln!(out, "{pad}  </transform>");

    match &layer.source {
        LayerSource::Text => {
            if let Some(p) = layer.props.prop("text/sourceText")
                && let Value::Text(doc) = &p.value
            {
                let align = match doc.justify {
                    Justify::Center | Justify::JustifyLastCenter => "center",
                    Justify::Right | Justify::JustifyLastRight | Justify::JustifyAll => "right",
                    Justify::Left | Justify::JustifyLastLeft => "left",
                };
                let [r, g, b, a] = doc.fill;
                let leading = doc.leading.unwrap_or(doc.size * 1.2);
                let _ = writeln!(out, "{pad}  <text>");
                let _ = writeln!(out, "{pad}    <string>{}</string>", xml_escape(&doc.text));
                let _ = writeln!(out, "{pad}    <font>{}</font>", xml_escape(&doc.font));
                let _ = writeln!(out, "{pad}    <size>{:.3}</size>", doc.size);
                let _ = writeln!(out, "{pad}    <alignment>{align}</alignment>");
                let _ = writeln!(
                    out,
                    "{pad}    <color r=\"{r:.4}\" g=\"{g:.4}\" b=\"{b:.4}\" a=\"{a:.4}\" />"
                );
                let _ = writeln!(out, "{pad}    <tracking>{:.3}</tracking>", doc.tracking);
                let _ = writeln!(out, "{pad}    <leading>{leading:.3}</leading>");
                let _ = writeln!(out, "{pad}  </text>");
            }
        }
        LayerSource::Shape => {
            if let Some(contents) = layer.props.group("contents") {
                write_comp_shape_contents(out, contents, indent + 1);
            }
        }
        LayerSource::Solid { item } => {
            if let Some(ItemKind::Solid(sol)) = project.item(*item).map(|i| &i.kind) {
                let [r, g, b] = sol.color;
                let _ = writeln!(
                    out,
                    "{pad}  <solid width=\"{}\" height=\"{}\" r=\"{r:.4}\" g=\"{g:.4}\" b=\"{b:.4}\" a=\"1.0000\" />",
                    sol.width, sol.height
                );
            }
        }
        LayerSource::Footage { item } => {
            if let Some(ItemKind::Footage(ft)) = project.item(*item).map(|i| &i.kind) {
                let _ = writeln!(
                    out,
                    "{pad}  <footage width=\"{}\" height=\"{}\" source=\"{}\" dropZone=\"{}\" />",
                    ft.width,
                    ft.height,
                    xml_escape(&ft.path),
                    u8::from(is_drop_zone),
                );
            }
        }
        _ => {}
    }

    if let Some(masks) = layer.props.group("masks") {
        for child in &masks.children {
            if let Node::Group(mg) = child
                && matches!(mg.kind, GroupKind::Mask { .. })
                && let Some(p) = mg.prop("path")
                && let Value::Path(sp) = &p.value
            {
                write_shape_path_ozml(out, sp, "mask", indent + 1);
            }
        }
    }

    let _ = writeln!(out, "{pad}</layer>");
}

fn write_comp_shape_contents(out: &mut String, contents: &PropGroup, indent: usize) {
    let pad = "  ".repeat(indent);
    let mut paths = Vec::new();
    let mut fill_rgba = None;
    let mut stroke_info = None;
    collect_shape_items(contents, &mut paths, &mut fill_rgba, &mut stroke_info);

    let _ = writeln!(out, "{pad}<shape fillRule=\"NonZero\">");
    if let Some(([r, g, b, a], op)) = fill_rgba {
        let _ = writeln!(
            out,
            "{pad}  <fill enabled=\"1\" r=\"{r:.4}\" g=\"{g:.4}\" b=\"{b:.4}\" a=\"{a:.4}\" opacity=\"{op:.4}\" />"
        );
    } else {
        let _ = writeln!(
            out,
            "{pad}  <fill enabled=\"0\" r=\"0.0\" g=\"0.0\" b=\"0.0\" a=\"0.0\" opacity=\"0.0\" />"
        );
    }
    if let Some(([r, g, b, a], width, op)) = stroke_info {
        let _ = writeln!(
            out,
            "{pad}  <stroke enabled=\"1\" width=\"{width:.3}\" r=\"{r:.4}\" g=\"{g:.4}\" b=\"{b:.4}\" a=\"{a:.4}\" opacity=\"{op:.4}\" cap=\"Butt\" join=\"Miter\" />"
        );
    }
    for sp in paths {
        write_shape_path_ozml(out, sp, "path", indent + 1);
    }
    let _ = writeln!(out, "{pad}</shape>");
}

#[allow(clippy::type_complexity)]
fn collect_shape_items<'a>(
    group: &'a PropGroup,
    paths: &mut Vec<&'a ShapePath>,
    fill_rgba: &mut Option<([f64; 4], f64)>,
    stroke_info: &mut Option<([f64; 4], f64, f64)>,
) {
    for child in &group.children {
        if let Node::Group(g) = child {
            match g.match_id.as_str() {
                "path" => {
                    if let Some(p) = g.prop("path")
                        && let Value::Path(sp) = &p.value
                    {
                        paths.push(sp);
                    }
                }
                "fill" => {
                    if let Some(p) = g.prop("color")
                        && let Value::Color(c) = p.value
                    {
                        let op = g
                            .prop("opacity")
                            .and_then(|op| match op.value {
                                Value::Scalar(v) => Some((v / 100.0).clamp(0.0, 1.0)),
                                _ => None,
                            })
                            .unwrap_or(1.0);
                        *fill_rgba = Some((c, op));
                    }
                }
                "stroke" => {
                    if let Some(p) = g.prop("color")
                        && let Value::Color(c) = p.value
                    {
                        let w = g
                            .prop("width")
                            .and_then(|w| match w.value {
                                Value::Scalar(v) => Some(v),
                                _ => None,
                            })
                            .unwrap_or(1.0);
                        let op = g
                            .prop("opacity")
                            .and_then(|op| match op.value {
                                Value::Scalar(v) => Some((v / 100.0).clamp(0.0, 1.0)),
                                _ => None,
                            })
                            .unwrap_or(1.0);
                        *stroke_info = Some((c, w, op));
                    }
                }
                _ => collect_shape_items(g, paths, fill_rgba, stroke_info),
            }
        }
    }
}

fn write_shape_path_ozml(out: &mut String, sp: &ShapePath, tag: &str, indent: usize) {
    let pad = "  ".repeat(indent);
    if sp.vertices.is_empty() {
        return;
    }
    let _ = writeln!(out, "{pad}<{tag} closed=\"{}\">", u8::from(sp.closed));
    for (idx, &[x, y]) in sp.vertices.iter().enumerate() {
        let [ix, iy] = sp.in_tangents.get(idx).copied().unwrap_or([0.0, 0.0]);
        let [ox, oy] = sp.out_tangents.get(idx).copied().unwrap_or([0.0, 0.0]);
        let smooth = u8::from(ix.hypot(iy) > 1e-4 || ox.hypot(oy) > 1e-4);
        let _ = writeln!(
            out,
            "{pad}  <knot x=\"{x:.3}\" y=\"{y:.3}\" inX=\"{:.3}\" inY=\"{:.3}\" outX=\"{:.3}\" outY=\"{:.3}\" smooth=\"{smooth}\" />",
            x + ix,
            y + iy,
            x + ox,
            y + oy,
        );
    }
    let _ = writeln!(out, "{pad}</{tag}>");
}

fn parse_blend_mode(s: &str) -> BlendMode {
    match s.trim() {
        "Darken" => BlendMode::Darken,
        "Multiply" => BlendMode::Multiply,
        "ColorBurn" => BlendMode::ColorBurn,
        "LinearBurn" => BlendMode::LinearBurn,
        "Lighten" => BlendMode::Lighten,
        "Screen" => BlendMode::Screen,
        "ColorDodge" => BlendMode::ColorDodge,
        "Add" | "LinearDodge" => BlendMode::Add,
        "Overlay" => BlendMode::Overlay,
        "SoftLight" => BlendMode::SoftLight,
        "HardLight" => BlendMode::HardLight,
        "VividLight" => BlendMode::VividLight,
        "LinearLight" => BlendMode::LinearLight,
        "PinLight" => BlendMode::PinLight,
        "HardMix" => BlendMode::HardMix,
        "Difference" => BlendMode::Difference,
        "Exclusion" => BlendMode::Exclusion,
        "Subtract" => BlendMode::Subtract,
        "Divide" => BlendMode::Divide,
        "Hue" => BlendMode::Hue,
        "Saturation" => BlendMode::Saturation,
        "Color" => BlendMode::Color,
        "Luminosity" => BlendMode::Luminosity,
        _ => BlendMode::Normal,
    }
}

fn blend_mode_name(b: BlendMode) -> &'static str {
    match b {
        BlendMode::Normal | BlendMode::Dissolve | BlendMode::DancingDissolve => "Normal",
        BlendMode::Darken | BlendMode::DarkerColor => "Darken",
        BlendMode::Multiply => "Multiply",
        BlendMode::ColorBurn | BlendMode::ClassicColorBurn => "ColorBurn",
        BlendMode::LinearBurn => "LinearBurn",
        BlendMode::Lighten | BlendMode::LighterColor => "Lighten",
        BlendMode::Screen => "Screen",
        BlendMode::ColorDodge | BlendMode::ClassicColorDodge => "ColorDodge",
        BlendMode::Add | BlendMode::LinearDodge => "Add",
        BlendMode::Overlay => "Overlay",
        BlendMode::SoftLight => "SoftLight",
        BlendMode::HardLight => "HardLight",
        BlendMode::VividLight => "VividLight",
        BlendMode::LinearLight => "LinearLight",
        BlendMode::PinLight => "PinLight",
        BlendMode::HardMix => "HardMix",
        BlendMode::Difference | BlendMode::ClassicDifference => "Difference",
        BlendMode::Exclusion => "Exclusion",
        BlendMode::Subtract => "Subtract",
        BlendMode::Divide => "Divide",
        BlendMode::Hue => "Hue",
        BlendMode::Saturation => "Saturation",
        BlendMode::Color => "Color",
        BlendMode::Luminosity => "Luminosity",
        BlendMode::StencilAlpha
        | BlendMode::StencilLuma
        | BlendMode::SilhouetteAlpha
        | BlendMode::SilhouetteLuma
        | BlendMode::AlphaAdd
        | BlendMode::LuminescentPremul => "Normal",
    }
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

fn xml_unescape(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

// ---------------------------------------------------------------- lightweight XML DOM

#[derive(Clone, Debug, Default)]
struct XmlNode {
    tag: String,
    attrs: BTreeMap<String, String>,
    text: String,
    children: Vec<XmlNode>,
}

impl XmlNode {
    fn child(&self, tag: &str) -> Option<&XmlNode> {
        self.children.iter().find(|c| c.tag == tag)
    }

    fn child_text(&self, tag: &str) -> Option<&str> {
        self.child(tag).map(|c| c.text.as_str())
    }

    fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.get(key).map(String::as_str)
    }

    fn attr_f64(&self, key: &str) -> Option<f64> {
        self.attr(key)?.trim().parse::<f64>().ok()
    }

    fn attr_f32(&self, key: &str) -> Option<f32> {
        self.attr(key)?.trim().parse::<f32>().ok()
    }

    fn attr_u64(&self, key: &str) -> Option<u64> {
        self.attr(key)?.trim().parse::<u64>().ok()
    }

    fn attr_u32(&self, key: &str) -> Option<u32> {
        self.attr(key)?.trim().parse::<u32>().ok()
    }
}

fn parse_xml(input: &str) -> Option<XmlNode> {
    let mut root = XmlNode {
        tag: "root".to_string(),
        ..Default::default()
    };
    let mut stack: Vec<XmlNode> = Vec::new();
    let mut i = 0_usize;
    let bytes = input.as_bytes();

    while i < bytes.len() {
        let Some(lt_rel) = input[i..].find('<') else {
            let tail = xml_unescape(input[i..].trim());
            if !tail.is_empty()
                && let Some(top) = stack.last_mut()
            {
                top.text.push_str(&tail);
            }
            break;
        };
        let lt = i + lt_rel;
        if lt > i {
            let txt = xml_unescape(input[i..lt].trim());
            if !txt.is_empty()
                && let Some(top) = stack.last_mut()
            {
                if !top.text.is_empty() {
                    top.text.push(' ');
                }
                top.text.push_str(&txt);
            }
        }
        let rest = &input[lt..];
        if rest.starts_with("<!--") {
            let end = rest.find("-->").map_or(input.len(), |p| lt + p + 3);
            i = end;
            continue;
        }
        if rest.starts_with("<?") {
            let end = rest.find("?>").map_or(input.len(), |p| lt + p + 2);
            i = end;
            continue;
        }
        if rest.starts_with("<!") {
            let end = rest.find('>').map_or(input.len(), |p| lt + p + 1);
            i = end;
            continue;
        }
        let gt_rel = find_tag_end(rest)?;
        let tag_body = rest[1..gt_rel].trim();
        i = lt + gt_rel + 1;

        if let Some(close_name) = tag_body.strip_prefix('/') {
            let close_name = close_name.trim();
            if let Some(done) = stack.pop() {
                let _ = close_name;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(done);
                } else {
                    root.children.push(done);
                }
            }
        } else {
            let self_closing = tag_body.ends_with('/');
            let clean = if self_closing {
                tag_body.strip_suffix('/').unwrap_or(tag_body).trim()
            } else {
                tag_body
            };
            let (tag_name, attrs) = parse_tag_and_attrs(clean);
            if tag_name.is_empty() {
                continue;
            }
            let node = XmlNode {
                tag: tag_name,
                attrs,
                text: String::new(),
                children: Vec::new(),
            };
            if self_closing {
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else {
                    root.children.push(node);
                }
            } else {
                stack.push(node);
            }
        }
    }

    while let Some(done) = stack.pop() {
        if let Some(parent) = stack.last_mut() {
            parent.children.push(done);
        } else {
            root.children.push(done);
        }
    }

    if root.children.len() == 1 {
        root.children.pop()
    } else if !root.children.is_empty() {
        Some(root)
    } else {
        None
    }
}

fn find_tag_end(s: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    for (idx, ch) in s.char_indices().skip(1) {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => {}
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None if ch == '>' => return Some(idx),
            None => {}
        }
    }
    None
}

fn parse_tag_and_attrs(s: &str) -> (String, BTreeMap<String, String>) {
    let mut attrs = BTreeMap::new();
    let s = s.trim();
    let name_end = s.find(char::is_whitespace).unwrap_or(s.len());
    let tag = s[..name_end].to_string();
    let mut rest = s[name_end..].trim();

    while !rest.is_empty() {
        let Some(eq_idx) = rest.find('=') else {
            break;
        };
        let key = rest[..eq_idx].trim().to_string();
        rest = rest[eq_idx + 1..].trim_start();
        if let Some(q) = rest.chars().next().filter(|&c| c == '"' || c == '\'') {
            rest = &rest[q.len_utf8()..];
            if let Some(end_q) = rest.find(q) {
                let val = xml_unescape(&rest[..end_q]);
                if !key.is_empty() {
                    attrs.insert(key, val);
                }
                rest = rest[end_q + q.len_utf8()..].trim_start();
            } else {
                break;
            }
        } else {
            let val_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            let val = xml_unescape(&rest[..val_end]);
            if !key.is_empty() {
                attrs.insert(key, val);
            }
            rest = rest[val_end..].trim_start();
        }
    }
    (tag, attrs)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn imports_motion_ozml_with_shapes_text_dropzones_behaviors_and_published_params() {
        let sample_ozml = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE ozxmlscene>
<ozml version="5.10">
  <displayversion>5.8</displayversion>
  <scene>
    <sceneSettings>
      <width>1920</width>
      <height>1080</height>
      <duration>300</duration>
      <frameRate>60.0000</frameRate>
      <backgroundColor red="0.0500" green="0.0600" blue="0.0800" alpha="1.0000" />
    </sceneSettings>
    <layer name="LowerThird" id="100" type="Group">
      <layer name="Avatar [DropZone]" id="101" type="DropZone">
        <enabled>1</enabled>
        <locked>0</locked>
        <blendMode>Normal</blendMode>
        <opacity>1.0000</opacity>
        <footage width="400" height="400" source="avatar.png" dropZone="1" />
      </layer>
      <layer name="BannerShape" id="102" type="Shape">
        <enabled>1</enabled>
        <locked>0</locked>
        <blendMode>Normal</blendMode>
        <opacity>0.9000</opacity>
        <shape fillRule="NonZero">
          <fill enabled="1" r="0.1000" g="0.5000" b="0.9000" a="1.0000" opacity="1.0000" />
          <stroke enabled="1" width="4.000" r="1.0000" g="1.0000" b="1.0000" a="1.0000" opacity="1.0000" cap="Round" join="Round" />
          <path closed="1">
            <knot x="100.000" y="800.000" inX="100.000" inY="800.000" outX="100.000" outY="800.000" smooth="0" />
            <knot x="700.000" y="800.000" inX="700.000" inY="800.000" outX="700.000" outY="800.000" smooth="0" />
            <knot x="700.000" y="920.000" inX="700.000" inY="920.000" outX="700.000" outY="920.000" smooth="0" />
            <knot x="100.000" y="920.000" inX="100.000" inY="920.000" outX="100.000" outY="920.000" smooth="0" />
          </path>
        </shape>
        <behavior type="Oscillate" channel="rotation" amplitude="15.0" frequency="2.0" phase="0.0" />
      </layer>
      <layer name="HeadlineText" id="103" type="Text">
        <enabled>1</enabled>
        <locked>0</locked>
        <blendMode>Normal</blendMode>
        <opacity>1.0000</opacity>
        <transform>
          <position x="320.000" y="860.000" z="0.0" />
          <scale x="100.0000" y="100.0000" z="100.0" />
          <rotation z="0.0000" />
        </transform>
        <text>
          <string>ARTCRAFT STUDIO</string>
          <font>Inter</font>
          <size>54.000</size>
          <alignment>center</alignment>
          <color r="1.0000" g="0.9500" b="0.2000" a="1.0000" />
          <tracking>25.000</tracking>
          <leading>64.000</leading>
        </text>
        <behavior type="FadeInFadeOut" fadeIn="12" fadeOut="12" />
      </layer>
    </layer>
    <publishedParameters>
      <parameter index="0" name="Banner Fill Color" targetLayer="102" channel="./shape/fill/color" type="color" default="0.1,0.5,0.9,1.0" />
      <parameter index="1" name="Headline Text" targetLayer="103" channel="./text/string" type="string" default="ARTCRAFT STUDIO" />
    </publishedParameters>
  </scene>
</ozml>"#;

        assert!(is_motion_ozml(sample_ozml.as_bytes()));
        let mut project = Project::default();
        let res = import_motion_ozml(
            &mut project,
            sample_ozml.as_bytes(),
            &ImportOptions {
                name: "LowerThird.motn".to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(res.comps.len(), 1);
        let comp_id = res.comps[0];
        let comp = project.comp(comp_id).unwrap();
        assert_eq!(comp.width, 1920);
        assert_eq!(comp.height, 1080);
        assert_eq!(comp.layers.len(), 3);

        // Top layer is HeadlineText (reversed from bottom-up OZML stack).
        let text_layer = &comp.layers[0];
        assert_eq!(text_layer.name, "HeadlineText");
        assert!(matches!(text_layer.source, LayerSource::Text));
        let op_prop = text_layer.props.prop("transform/opacity").unwrap();
        assert_eq!(op_prop.keys.len(), 4, "FadeInFadeOut behavior creates 4 opacity keys");

        // Middle layer is BannerShape with Oscillate behavior expression on rotation.
        let shape_layer = &comp.layers[1];
        assert_eq!(shape_layer.name, "BannerShape");
        assert!(matches!(shape_layer.source, LayerSource::Shape));
        let rot_prop = shape_layer.props.prop("transform/rotation").unwrap();
        assert!(rot_prop.has_expression());

        // Essential Graphics has 1 Media DropZone + 2 Published Parameters = 3 controls.
        let eg = comp.essential.as_ref().unwrap();
        assert_eq!(eg.controls.len(), 3);

        // Round-trip back to Apple Motion `<ozml>`.
        let exported = export_motion_ozml(&project, comp_id).unwrap();
        assert!(exported.contains("<ozml version=\"5.10\">"));
        assert!(exported.contains("<string>ARTCRAFT STUDIO</string>"));
        assert!(exported.contains("type=\"DropZone\""));
        assert!(exported.contains("<publishedParameters>"));
    }
}
