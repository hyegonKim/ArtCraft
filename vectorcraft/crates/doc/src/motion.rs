//! Apple Motion (`.motn` / `.moti` / `.motr` / `.moef`) `<ozml>` project export for VectorCraft.
//!
//! Converts a [`Document`]'s artboard and vector hierarchy (layers, groups, live shapes,
//! cubic Bézier paths, compound paths, rich text, placed images, and symbol instances)
//! into an Apple Motion 5 `<ozml version="5.10">` XML document so vector illustrations,
//! logos, HUD elements, and lower-thirds remain 100% resolution-independent and editable
//! inside Apple Motion, Final Cut Pro, and EffectCraft.

use std::borrow::Cow;
use std::fmt::Write as _;

use vectorcraft_color::{BlendMode, Color, Paint};
use vectorcraft_geom::{Affine, AnchorKind, FillRule, PathData, Point, Rect};

use crate::appearance::{Appearance, LineCap, LineJoin};
use crate::node::{ImageObject, Node, NodeKind};
use crate::text::{Justify, TextObject};
use crate::Document;

/// Options controlling how a [`Document`] artboard is exported as an Apple Motion `<ozml>` project.
#[derive(Clone, Debug, PartialEq)]
pub struct MotionExportOptions {
    /// Index of the artboard in [`Document::artboards`] to export (defaults to `0`).
    pub artboard_index: usize,
    /// Timeline frame rate (frames per second). Defaults to `60.0`.
    pub frame_rate: f64,
    /// Project duration in frames. Defaults to `300` (5 seconds at 60 fps).
    pub duration_frames: u32,
    /// Whether hidden nodes (`!node.visible`) shouldstill be emitted with `<enabled>0</enabled>`.
    pub include_hidden: bool,
    /// Whether template layers (`NodeKind::Layer { template: true, .. }`) should be exported.
    pub include_template_layers: bool,
    /// Whether to auto-publish editable text strings and shape fill colors as
    /// `<publishedParameters>` for Final Cut Pro / EffectCraft rig templates.
    pub publish_parameters: bool,
}

impl Default for MotionExportOptions {
    fn default() -> Self {
        Self {
            artboard_index: 0,
            frame_rate: 60.0,
            duration_frames: 300,
            include_hidden: true,
            include_template_layers: false,
            publish_parameters: true,
        }
    }
}

#[derive(Clone, Debug)]
struct PublishedParam {
    name: String,
    target_id: u64,
    channel: &'static str,
    kind: &'static str,
    default_value: String,
}

/// Export a VectorCraft [`Document`] artboard to an Apple Motion 5 `<ozml version="5.10">` XML string.
#[must_use]
pub fn export_motion_ozml(doc: &Document, opts: &MotionExportOptions) -> String {
    let fps = if opts.frame_rate.is_finite() && opts.frame_rate > 0.0 {
        opts.frame_rate
    } else {
        60.0
    };
    let dur = opts.duration_frames.max(1);
    let artboard = doc
        .artboards
        .get(opts.artboard_index)
        .or_else(|| doc.artboards.first());

    let ab_rect = artboard.map_or_else(
        || Rect::new(0.0, 0.0, 1920.0, 1080.0),
        |ab| ab.rect,
    );
    let width = ab_rect.width().round().max(1.0) as u32;
    let height = ab_rect.height().round().max(1.0) as u32;
    let origin = Point::new(ab_rect.x0, ab_rect.y0);
    let [bg_r, bg_g, bg_b] = artboard
        .and_then(|ab| ab.color)
        .map_or([0.0, 0.0, 0.0], |c| c.to_rgb());
    let bg_a = if artboard.and_then(|ab| ab.color).is_some() {
        1.0
    } else {
        0.0
    };
    let root_name = if !doc.title.is_empty() {
        doc.title.as_str()
    } else if let Some(ab) = artboard {
        ab.name.as_str()
    } else {
        "VectorCraft Artboard"
    };

    let mut out = String::with_capacity(8192);
    let _ = writeln!(out, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    let _ = writeln!(out, "<!DOCTYPE ozxmlscene>");
    let _ = writeln!(out, "<ozml version=\"5.10\">");
    let _ = writeln!(out, "  <displayversion>5.8</displayversion>");
    let _ = writeln!(out, "  <factory id=\"1\" uuid=\"vectorcraft-ozml-factory\">");
    let _ = writeln!(out, "    <description>ArtCraft VectorCraft Motion Export</description>");
    let _ = writeln!(out, "  </factory>");
    let _ = writeln!(out, "  <scene>");
    let _ = writeln!(out, "    <sceneSettings>");
    let _ = writeln!(out, "      <width>{width}</width>");
    let _ = writeln!(out, "      <height>{height}</height>");
    let _ = writeln!(out, "      <duration>{dur}</duration>");
    let _ = writeln!(out, "      <frameRate>{fps:.4}</frameRate>");
    let _ = writeln!(out, "      <NTSC>0</NTSC>");
    let _ = writeln!(out, "      <channels>4</channels>");
    let _ = writeln!(
        out,
        "      <backgroundColor red=\"{bg_r:.4}\" green=\"{bg_g:.4}\" blue=\"{bg_b:.4}\" alpha=\"{bg_a:.4}\" />"
    );
    let _ = writeln!(out, "    </sceneSettings>");

    let mut published = Vec::new();
    let _ = writeln!(
        out,
        "    <layer name=\"{}\" id=\"100\" type=\"Group\">",
        xml_escape(root_name)
    );
    let mut next_id = 1000_u64;
    for layer in &doc.layers {
        write_node(
            &mut out,
            layer,
            doc,
            origin,
            ab_rect,
            opts,
            &mut next_id,
            &mut published,
            3,
        );
    }
    let _ = writeln!(out, "    </layer>");

    if opts.publish_parameters && !published.is_empty() {
        let _ = writeln!(out, "    <publishedParameters>");
        for (idx, p) in published.iter().enumerate() {
            let _ = writeln!(
                out,
                "      <parameter index=\"{idx}\" name=\"{}\" targetLayer=\"{}\" channel=\"{}\" type=\"{}\" default=\"{}\" />",
                xml_escape(&p.name),
                p.target_id,
                p.channel,
                p.kind,
                xml_escape(&p.default_value),
            );
        }
        let _ = writeln!(out, "    </publishedParameters>");
    }

    let _ = writeln!(out, "  </scene>");
    let _ = writeln!(out, "</ozml>");
    out
}

#[allow(clippy::too_many_arguments)]
fn write_node(
    out: &mut String,
    node: &Node,
    doc: &Document,
    origin: Point,
    ab_rect: Rect,
    opts: &MotionExportOptions,
    next_id: &mut u64,
    published: &mut Vec<PublishedParam>,
    indent: usize,
) {
    if !node.visible && !opts.include_hidden {
        return;
    }
    if let NodeKind::Layer { template: true, .. } = &node.kind
        && !opts.include_template_layers
    {
        return;
    }
    if let NodeKind::Path { guide: true, .. } = &node.kind {
        return;
    }

    let id = if node.id.0 > 0 {
        node.id.0 + 100
    } else {
        *next_id += 1;
        *next_id
    };
    *next_id = (*next_id).max(id + 1);

    let pad = "  ".repeat(indent);
    let enabled = u8::from(node.visible);
    let locked = u8::from(node.locked);
    let opacity = node.opacity.clamp(0.0, 1.0);
    let blend = blend_mode_name(node.blend);
    let name = node_display_name(node);
    let is_drop_zone = name.contains("[DropZone]") || name.contains("{{DropZone}}");

    let layer_type = match &node.kind {
        NodeKind::Layer { .. }
        | NodeKind::Group { .. }
        | NodeKind::Blend { .. }
        | NodeKind::Envelope { .. }
        | NodeKind::Intertwine { .. } => "Group",
        NodeKind::Path { .. } | NodeKind::Compound { .. } | NodeKind::Mesh(_) => "Shape",
        NodeKind::Text(_) => "Text",
        NodeKind::Image(_) | NodeKind::PlacedDocument(_) => {
            if is_drop_zone {
                "DropZone"
            } else {
                "Footage"
            }
        }
        NodeKind::SymbolInstance { .. } | NodeKind::Repeat(_) => "Group",
    };

    let _ = writeln!(
        out,
        "{pad}<layer name=\"{}\" id=\"{id}\" type=\"{layer_type}\">",
        xml_escape(&name)
    );
    let _ = writeln!(out, "{pad}  <enabled>{enabled}</enabled>");
    let _ = writeln!(out, "{pad}  <locked>{locked}</locked>");
    let _ = writeln!(out, "{pad}  <blendMode>{blend}</blendMode>");
    let _ = writeln!(out, "{pad}  <opacity>{opacity:.4}</opacity>");

    let (pos_x, pos_y, scale_x, scale_y, rot_deg) = node_transform(node, origin, ab_rect);
    let _ = writeln!(out, "{pad}  <transform>");
    let _ = writeln!(
        out,
        "{pad}    <position x=\"{pos_x:.3}\" y=\"{pos_y:.3}\" z=\"0.0\" />"
    );
    let _ = writeln!(
        out,
        "{pad}    <scale x=\"{scale_x:.4}\" y=\"{scale_y:.4}\" z=\"100.0\" />"
    );
    let _ = writeln!(out, "{pad}    <rotation z=\"{rot_deg:.4}\" />");
    let _ = writeln!(out, "{pad}  </transform>");

    match &node.kind {
        NodeKind::Layer { children, .. }
        | NodeKind::Group { children, .. }
        | NodeKind::Blend { children, .. }
        | NodeKind::Intertwine { children } => {
            for child in children {
                write_node(
                    out,
                    child,
                    doc,
                    origin,
                    ab_rect,
                    opts,
                    next_id,
                    published,
                    indent + 1,
                );
            }
        }
        NodeKind::Envelope { content, .. } => {
            for child in content {
                write_node(
                    out,
                    child,
                    doc,
                    origin,
                    ab_rect,
                    opts,
                    next_id,
                    published,
                    indent + 1,
                );
            }
        }
        NodeKind::Path {
            path, rule, live, ..
        } => {
            let resolved: Cow<'_, PathData> = match live.as_deref() {
                Some(ls) => Cow::Owned(ls.to_path()),
                None => Cow::Borrowed(path),
            };
            write_shape_content(
                out,
                std::slice::from_ref(resolved.as_ref()),
                *rule,
                &node.appearance,
                origin,
                indent + 1,
            );
            maybe_publish_fill(&name, id, &node.appearance, opts, published);
        }
        NodeKind::Compound { children, rule } => {
            let mut paths = Vec::new();
            collect_compound_paths(children, &mut paths);
            write_shape_content(out, &paths, *rule, &node.appearance, origin, indent + 1);
            maybe_publish_fill(&name, id, &node.appearance, opts, published);
        }
        NodeKind::Text(t) => {
            write_text_content(out, t, &node.appearance, indent + 1);
            if opts.publish_parameters {
                let text_str: String = t.runs.iter().map(|r| r.text.as_str()).collect();
                published.push(PublishedParam {
                    name: format!("{name} Text"),
                    target_id: id,
                    channel: "./text/string",
                    kind: "string",
                    default_value: text_str,
                });
            }
        }
        NodeKind::Image(img) => {
            write_image_content(out, img, is_drop_zone, indent + 1);
        }
        NodeKind::SymbolInstance { symbol, xf } => {
            if let Some(sym) = doc.symbols.iter().find(|s| &s.name == symbol) {
                let mut expanded = (*sym.art).clone();
                let _ = xf;
                write_node(
                    out,
                    &expanded,
                    doc,
                    origin,
                    ab_rect,
                    opts,
                    next_id,
                    published,
                    indent + 1,
                );
                let _ = &mut expanded;
            }
        }
        NodeKind::Repeat(rep) => {
            for child in &rep.source {
                write_node(
                    out,
                    child,
                    doc,
                    origin,
                    ab_rect,
                    opts,
                    next_id,
                    published,
                    indent + 1,
                );
            }
        }
        NodeKind::Mesh(_) | NodeKind::PlacedDocument(_) => {}
    }

    let _ = writeln!(out, "{pad}</layer>");
}

fn collect_compound_paths(children: &[std::sync::Arc<Node>], out: &mut Vec<PathData>) {
    for ch in children {
        match &ch.kind {
            NodeKind::Path { path, live, .. } => {
                if let Some(ls) = live.as_deref() {
                    out.push(ls.to_path());
                } else {
                    out.push(path.clone());
                }
            }
            NodeKind::Compound { children: sub, .. } | NodeKind::Group { children: sub, .. } => {
                collect_compound_paths(sub, out);
            }
            _ => {}
        }
    }
}

fn maybe_publish_fill(
    name: &str,
    id: u64,
    appearance: &Appearance,
    opts: &MotionExportOptions,
    published: &mut Vec<PublishedParam>,
) {
    if !opts.publish_parameters {
        return;
    }
    if let Some(fill) = appearance.fill()
        && fill.visible
        && let Some(c) = paint_representative_color(&fill.paint)
    {
        let [r, g, b] = c.to_rgb();
        published.push(PublishedParam {
            name: format!("{name} Fill Color"),
            target_id: id,
            channel: "./shape/fill/color",
            kind: "color",
            default_value: format!("{r:.4},{g:.4},{b:.4},{:.4}", fill.opacity.clamp(0.0, 1.0)),
        });
    }
}

fn node_display_name(node: &Node) -> String {
    if let Some(n) = node.name.as_deref().filter(|s| !s.is_empty()) {
        return n.to_string();
    }
    match &node.kind {
        NodeKind::Layer { .. } => format!("Layer {}", node.id.0),
        NodeKind::Group { .. } => format!("Group {}", node.id.0),
        NodeKind::Path { live: Some(ls), .. } => ls.label().to_string(),
        NodeKind::Path { .. } => format!("Path {}", node.id.0),
        NodeKind::Compound { .. } => format!("Compound Path {}", node.id.0),
        NodeKind::Text(t) => {
            let plain: String = t.runs.iter().map(|r| r.text.as_str()).collect();
            let first = plain.lines().next().unwrap_or("Text").trim();
            if first.is_empty() {
                format!("Text {}", node.id.0)
            } else {
                first.chars().take(32).collect()
            }
        }
        NodeKind::Image(img) => img.key.clone(),
        NodeKind::SymbolInstance { symbol, .. } => symbol.clone(),
        NodeKind::Blend { .. } => "Blend".to_string(),
        NodeKind::Envelope { .. } => "Envelope".to_string(),
        NodeKind::Mesh(_) => "Mesh".to_string(),
        NodeKind::Repeat(_) => "Repeat".to_string(),
        NodeKind::PlacedDocument(_) => "Placed Document".to_string(),
        NodeKind::Intertwine { .. } => "Intertwine".to_string(),
    }
}

fn node_transform(node: &Node, origin: Point, ab_rect: Rect) -> (f64, f64, f64, f64, f64) {
    let cx = ab_rect.width() * 0.5;
    let cy = ab_rect.height() * 0.5;
    let xf: Option<Affine> = match &node.kind {
        NodeKind::Text(t) => Some(t.xf),
        NodeKind::Image(img) => Some(img.xf),
        NodeKind::SymbolInstance { xf, .. } => Some(*xf),
        _ => None,
    };
    if let Some(a) = xf {
        let [m0, m1, m2, m3, tx, ty] = a.as_coeffs();
        let sx = m0.hypot(m1);
        let sy = m2.hypot(m3);
        let rot_rad = m1.atan2(m0);
        let px = tx - origin.x;
        let py = ty - origin.y;
        (
            px,
            py,
            (sx * 100.0).clamp(0.01, 10000.0),
            (sy * 100.0).clamp(0.01, 10000.0),
            rot_rad.to_degrees(),
        )
    } else {
        (cx, cy, 100.0, 100.0, node.bbox_angle)
    }
}

fn write_shape_content(
    out: &mut String,
    paths: &[PathData],
    rule: FillRule,
    appearance: &Appearance,
    origin: Point,
    indent: usize,
) {
    let pad = "  ".repeat(indent);
    let rule_str = match rule {
        FillRule::NonZero => "NonZero",
        FillRule::EvenOdd => "EvenOdd",
    };
    let _ = writeln!(out, "{pad}<shape fillRule=\"{rule_str}\">");

    if let Some(fill) = appearance.fill()
        && fill.visible
        && let Some(c) = paint_representative_color(&fill.paint)
    {
        let [r, g, b] = c.to_rgb();
        let alpha = fill.opacity.clamp(0.0, 1.0);
        let _ = writeln!(
            out,
            "{pad}  <fill enabled=\"1\" r=\"{r:.4}\" g=\"{g:.4}\" b=\"{b:.4}\" a=\"{alpha:.4}\" opacity=\"{alpha:.4}\" />"
        );
    } else {
        let _ = writeln!(
            out,
            "{pad}  <fill enabled=\"0\" r=\"0.0\" g=\"0.0\" b=\"0.0\" a=\"0.0\" opacity=\"0.0\" />"
        );
    }

    if let Some(st) = appearance.stroke()
        && st.visible
        && st.width > 0.0
        && let Some(c) = paint_representative_color(&st.paint)
    {
        let [r, g, b] = c.to_rgb();
        let alpha = st.opacity.clamp(0.0, 1.0);
        let cap = match st.cap {
            LineCap::Butt => "Butt",
            LineCap::Round => "Round",
            LineCap::Square => "Square",
        };
        let join = match st.join {
            LineJoin::Miter => "Miter",
            LineJoin::Round => "Round",
            LineJoin::Bevel => "Bevel",
        };
        let _ = writeln!(
            out,
            "{pad}  <stroke enabled=\"1\" width=\"{:.3}\" r=\"{r:.4}\" g=\"{g:.4}\" b=\"{b:.4}\" a=\"{alpha:.4}\" opacity=\"{alpha:.4}\" cap=\"{cap}\" join=\"{join}\" />",
            st.width,
        );
    }

    for path in paths {
        write_path_data(out, path, origin, indent + 1);
    }

    let _ = writeln!(out, "{pad}</shape>");
}

fn write_path_data(out: &mut String, path: &PathData, origin: Point, indent: usize) {
    let pad = "  ".repeat(indent);
    for sp in &path.subpaths {
        if sp.anchors.is_empty() {
            continue;
        }
        let _ = writeln!(out, "{pad}<path closed=\"{}\">", u8::from(sp.closed));
        for a in &sp.anchors {
            let x = a.p.x - origin.x;
            let y = a.p.y - origin.y;
            let in_x = a.h_in.x - origin.x;
            let in_y = a.h_in.y - origin.y;
            let out_x = a.h_out.x - origin.x;
            let out_y = a.h_out.y - origin.y;
            let smooth = u8::from(a.kind == AnchorKind::Smooth);
            let _ = writeln!(
                out,
                "{pad}  <knot x=\"{x:.3}\" y=\"{y:.3}\" inX=\"{in_x:.3}\" inY=\"{in_y:.3}\" outX=\"{out_x:.3}\" outY=\"{out_y:.3}\" smooth=\"{smooth}\" />"
            );
        }
        let _ = writeln!(out, "{pad}</path>");
    }
}

fn write_text_content(
    out: &mut String,
    t: &TextObject,
    appearance: &Appearance,
    indent: usize,
) {
    let pad = "  ".repeat(indent);
    let plain: String = t.runs.iter().map(|r| r.text.as_str()).collect();
    let first_style = t.runs.first().map(|r| &r.style);
    let font = first_style
        .map(|s| s.font_family.as_str())
        .filter(|f| !f.is_empty())
        .unwrap_or("Inter");
    let size = first_style.map_or(24.0, |s| s.size.max(1.0));
    let leading = first_style
        .and_then(|s| s.leading)
        .unwrap_or(size * 1.2);
    let tracking = first_style.map_or(0.0, |s| s.tracking);
    let color = first_style
        .and_then(|s| paint_representative_color(&s.fill))
        .or_else(|| appearance.fill().and_then(|f| paint_representative_color(&f.paint)))
        .unwrap_or(Color::WHITE);
    let [r, g, b] = color.to_rgb();
    let align = match t.para.justify {
        Justify::Auto | Justify::Left | Justify::JustifyLeft => "left",
        Justify::Center | Justify::JustifyCenter => "center",
        Justify::Right | Justify::JustifyRight | Justify::JustifyAll => "right",
    };

    let _ = writeln!(out, "{pad}<text>");
    let _ = writeln!(out, "{pad}  <string>{}</string>", xml_escape(&plain));
    let _ = writeln!(out, "{pad}  <font>{}</font>", xml_escape(font));
    let _ = writeln!(out, "{pad}  <size>{size:.3}</size>");
    let _ = writeln!(out, "{pad}  <alignment>{align}</alignment>");
    let _ = writeln!(
        out,
        "{pad}  <color r=\"{r:.4}\" g=\"{g:.4}\" b=\"{b:.4}\" a=\"1.0000\" />"
    );
    let _ = writeln!(out, "{pad}  <tracking>{tracking:.3}</tracking>");
    let _ = writeln!(out, "{pad}  <leading>{leading:.3}</leading>");
    let _ = writeln!(out, "{pad}</text>");
}

fn write_image_content(out: &mut String, img: &ImageObject, is_drop_zone: bool, indent: usize) {
    let pad = "  ".repeat(indent);
    let source = img
        .link
        .as_ref()
        .map_or(img.key.as_str(), |l| l.path.as_str());
    let _ = writeln!(
        out,
        "{pad}<footage width=\"{}\" height=\"{}\" source=\"{}\" dropZone=\"{}\" />",
        img.width,
        img.height,
        xml_escape(source),
        u8::from(is_drop_zone),
    );
}

fn paint_representative_color(paint: &Paint) -> Option<Color> {
    match paint {
        Paint::None => None,
        Paint::Solid { color, .. } => Some(*color),
        Paint::Gradient(g) => g.gradient.stops.first().map(|s| s.color),
        Paint::Pattern { .. } => Some(Color::WHITE),
    }
}

fn blend_mode_name(b: BlendMode) -> &'static str {
    match b {
        BlendMode::Normal => "Normal",
        BlendMode::Darken => "Darken",
        BlendMode::Multiply => "Multiply",
        BlendMode::ColorBurn => "ColorBurn",
        BlendMode::Lighten => "Lighten",
        BlendMode::Screen => "Screen",
        BlendMode::ColorDodge => "ColorDodge",
        BlendMode::Overlay => "Overlay",
        BlendMode::SoftLight => "SoftLight",
        BlendMode::HardLight => "HardLight",
        BlendMode::Difference => "Difference",
        BlendMode::Exclusion => "Exclusion",
        BlendMode::Hue => "Hue",
        BlendMode::Saturation => "Saturation",
        BlendMode::Color => "Color",
        BlendMode::Luminosity => "Luminosity",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::NodeId;
    use std::sync::Arc;
    use vectorcraft_geom::{Anchor, SubPath};

    #[test]
    fn exports_vector_artboard_to_apple_motion_ozml() {
        let mut doc = Document::new(1920.0, 1080.0);
        doc.title = "BroadcastBadge".to_string();

        let badge_path = PathData {
            subpaths: vec![SubPath::new(
                vec![
                    Anchor::corner(Point::new(100.0, 100.0)),
                    Anchor::corner(Point::new(500.0, 100.0)),
                    Anchor::corner(Point::new(500.0, 240.0)),
                    Anchor::corner(Point::new(100.0, 240.0)),
                ],
                true,
            )],
        };
        let mut shape_node = Node::path(
            NodeId(10),
            badge_path,
            Appearance::basic(
                Paint::solid(Color::rgb(0.1, 0.5, 0.9)),
                Paint::solid(Color::WHITE),
                4.0,
            ),
        );
        shape_node.name = Some("BadgeFrame".to_string());

        let style = crate::text::CharStyle {
            font_family: "Inter".to_string(),
            size: 48.0,
            fill: Paint::solid(Color::WHITE),
            ..Default::default()
        };
        let text_obj = TextObject::point(
            Point::new(140.0, 185.0),
            "BREAKING NEWS",
            style,
        );
        let mut text_node = Node::new(NodeId(11), NodeKind::Text(Box::new(text_obj)));
        text_node.name = Some("Headline".to_string());

        let mut layer = Node::layer(
            NodeId(1),
            "Overlay Layer",
            crate::node::LayerColor::Preset(0),
        );
        if let NodeKind::Layer { children, .. } = &mut layer.kind {
            children.push(Arc::new(shape_node));
            children.push(Arc::new(text_node));
        }
        doc.layers = vec![Arc::new(layer)];

        let ozml = export_motion_ozml(&doc, &MotionExportOptions::default());
        assert!(ozml.contains("<ozml version=\"5.10\">"));
        assert!(ozml.contains("<width>1920</width>"));
        assert!(ozml.contains("<height>1080</height>"));
        assert!(ozml.contains("name=\"BadgeFrame\""));
        assert!(ozml.contains("<shape fillRule=\"NonZero\">"));
        assert!(ozml.contains("<stroke enabled=\"1\" width=\"4.000\""));
        assert!(ozml.contains("<knot x=\"100.000\" y=\"100.000\""));
        assert!(ozml.contains("<string>BREAKING NEWS</string>"));
        assert!(ozml.contains("<publishedParameters>"));
        assert!(ozml.contains("name=\"BadgeFrame Fill Color\""));
        assert!(ozml.contains("name=\"Headline Text\""));
    }
}
