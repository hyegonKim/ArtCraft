//! Apple Motion (`.motn` / `.moti` `<ozml version="5.10">`) native project exporter.
//!
//! Mirrors Pixelmator Pro's "Export to Motion Project (`.motn`)" workflow so layered designs,
//! lower-thirds, thumbnails, vector shapes, text layers, layer styles (Drop Shadow, Outer Glow),
//! and video layers created in PhotoCraft can be opened directly in EffectCraft, Apple Motion, or
//! Final Cut Pro with their full layer hierarchy, blend modes, transforms, and editable vector/text
//! data intact.

use std::fmt::Write;

use photocraft_color::{BlendMode, Color};

use crate::{
    Document, Effect, Fill, Layer, LayerContent, LineCap, LineJoin, Path, ShapeLayer, TextAlign,
    TextLayer, VideoSource,
};

/// Options controlling how a [`Document`] is exported to an Apple Motion `.motn` XML project.
#[derive(Clone, Debug, PartialEq)]
pub struct MotionExportOptions {
    /// Nominal timeline frame rate when the document has no [`crate::Timeline`] (default `30.0`).
    pub fps: f32,
    /// Nominal timeline duration in frames when the document has no [`crate::Timeline`] (default `300`).
    pub duration_frames: usize,
    /// Relative directory prefix for rasterized layer assets (default `"Media"`).
    pub asset_dir: String,
    /// Publish top-level text and shape fill controls to `<publishedParameters>` so the project can
    /// act immediately as a Final Cut Pro Title (`.moti`) or EffectCraft Essential Graphics template.
    pub publish_template_controls: bool,
}

impl Default for MotionExportOptions {
    fn default() -> Self {
        Self {
            fps: 30.0,
            duration_frames: 300,
            asset_dir: "Media".into(),
            publish_template_controls: true,
        }
    }
}

/// Export `doc` to an Apple Motion `<ozml version="5.10">` XML string (`.motn` / `.moti`).
pub fn export_motion_ozml(doc: &Document, opts: &MotionExportOptions) -> String {
    let fps = doc
        .timeline
        .as_ref()
        .map_or(opts.fps, |t| t.fps)
        .clamp(1.0, 240.0);
    let duration_frames = doc
        .timeline
        .as_ref()
        .map_or(opts.duration_frames, |t| t.duration)
        .max(1);
    let w = doc.size.width.max(1);
    let h = doc.size.height.max(1);

    let mut out = String::with_capacity(4096);
    let _ = writeln!(out, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    let _ = writeln!(out, "<!DOCTYPE ozml SYSTEM \"ozml.dtd\">");
    let _ = writeln!(out, "<ozml version=\"5.10\" generator=\"PhotoCraft\">");
    let _ = writeln!(out, "  <displayversion>5.8</displayversion>");
    let _ = writeln!(out, "  <scene name=\"{}\">", xml_escape(&doc.name));
    let _ = writeln!(out, "    <sceneSettings>");
    let _ = writeln!(out, "      <width>{w}</width>");
    let _ = writeln!(out, "      <height>{h}</height>");
    let _ = writeln!(out, "      <frameRate>{fps:.4}</frameRate>");
    let _ = writeln!(out, "      <duration>{duration_frames}</duration>");
    let _ = writeln!(out, "      <colorSpace>Rec.709</colorSpace>");
    let _ = writeln!(out, "    </sceneSettings>");

    if opts.publish_template_controls {
        write_published_parameters(&mut out, &doc.layers);
    }

    // Document::layers is bottom-to-top; emit in top-to-bottom composition order for Motion/EffectCraft.
    for layer in doc.layers.iter().rev() {
        write_layer(&mut out, layer, doc, opts, 2);
    }

    let _ = writeln!(out, "  </scene>");
    let _ = writeln!(out, "</ozml>");
    out
}

fn write_published_parameters(out: &mut String, layers: &[Layer]) {
    let mut params: Vec<(u64, String, &'static str, u64, &'static str)> = Vec::new();
    let mut next_param_id = 1u64;
    for l in layers.iter().rev() {
        match &l.content {
            LayerContent::Text(_) => {
                params.push((
                    next_param_id,
                    format!("{} Text", l.name),
                    "text",
                    l.id.0,
                    "text/sourceText",
                ));
                next_param_id += 1;
            }
            LayerContent::Shape(s) if s.fill.is_some() => {
                params.push((
                    next_param_id,
                    format!("{} Color", l.name),
                    "color",
                    l.id.0,
                    "shape/fillColor",
                ));
                next_param_id += 1;
            }
            LayerContent::Smart(_) => {
                params.push((
                    next_param_id,
                    format!("{} Drop Zone", l.name),
                    "media",
                    l.id.0,
                    "media/source",
                ));
                next_param_id += 1;
            }
            _ => {}
        }
    }
    if params.is_empty() {
        return;
    }
    let _ = writeln!(out, "    <publishedParameters>");
    for (id, name, kind, layer_id, prop) in params {
        let _ = writeln!(
            out,
            "      <parameter id=\"{id}\" name=\"{}\" type=\"{kind}\" targetLayer=\"{layer_id}\" targetProperty=\"{prop}\" />",
            xml_escape(&name)
        );
    }
    let _ = writeln!(out, "    </publishedParameters>");
}

fn write_layer(
    out: &mut String,
    layer: &Layer,
    doc: &Document,
    opts: &MotionExportOptions,
    indent: usize,
) {
    let pad = "  ".repeat(indent);
    let active = u8::from(layer.visible);
    let locked = u8::from(layer.locks.any());
    let blend = blend_mode_name(layer.blend);
    let eff_opacity = (layer.opacity * layer.fill_opacity).clamp(0.0, 1.0);
    let label = layer.label.label();

    let (pos_x, pos_y, scale_x, scale_y, rot_deg) = layer_transform(layer, doc);
    let cx = doc.size.width as f64 * 0.5;
    let cy = doc.size.height as f64 * 0.5;
    // Motion scene coordinates have origin (0, 0) at canvas centre and +Y up; we also write
    // canvasX/canvasY (top-left pixel coordinates) for lossless EffectCraft / PhotoCraft import.
    let motion_x = pos_x - cx;
    let motion_y = cy - pos_y;

    let _ = writeln!(
        out,
        "{pad}<layer id=\"{}\" name=\"{}\" active=\"{active}\" locked=\"{locked}\" blendMode=\"{blend}\" opacity=\"{eff_opacity:.4}\" label=\"{label}\" clipped=\"{}\">",
        layer.id.0,
        xml_escape(&layer.name),
        u8::from(layer.clipped),
    );
    let _ = writeln!(out, "{pad}  <properties>");
    let _ = writeln!(out, "{pad}    <transform>");
    let _ = writeln!(
        out,
        "{pad}      <position x=\"{motion_x:.3}\" y=\"{motion_y:.3}\" z=\"0.0\" canvasX=\"{pos_x:.3}\" canvasY=\"{pos_y:.3}\" />"
    );
    let _ = writeln!(
        out,
        "{pad}      <scale x=\"{scale_x:.3}\" y=\"{scale_y:.3}\" z=\"100.0\" />"
    );
    let _ = writeln!(out, "{pad}      <rotation z=\"{rot_deg:.3}\" />");
    let _ = writeln!(out, "{pad}      <anchorPoint x=\"0.0\" y=\"0.0\" />");
    let _ = writeln!(out, "{pad}    </transform>");
    let _ = writeln!(out, "{pad}    <opacity>{eff_opacity:.4}</opacity>");
    let _ = writeln!(out, "{pad}    <blendMode>{blend}</blendMode>");
    let _ = writeln!(out, "{pad}  </properties>");

    if let Some(vm) = &layer.vector_mask
        && vm.enabled
        && !vm.path.is_empty()
    {
        write_path(out, &vm.path, "vectorMask", indent + 1);
    }

    if layer.effects.enabled {
        for fx in &layer.effects.items {
            if fx.enabled() {
                write_effect(out, fx, indent + 1);
            }
        }
    }

    match &layer.content {
        LayerContent::Group(g) => {
            for child in g.children.iter().rev() {
                write_layer(out, child, doc, opts, indent + 1);
            }
        }
        LayerContent::Text(t) => write_text_content(out, t, doc.resolution_dpi, indent + 1),
        LayerContent::Shape(s) => write_shape_content(out, s, indent + 1),
        LayerContent::Fill(Fill::Solid(c)) => {
            let [r, g, b] = c.to_rgb();
            let _ = writeln!(
                out,
                "{pad}  <solid width=\"{}\" height=\"{}\" r=\"{r:.4}\" g=\"{g:.4}\" b=\"{b:.4}\" a=\"{:.4}\" />",
                doc.size.width.max(1),
                doc.size.height.max(1),
                c.alpha.clamp(0.0, 1.0),
            );
        }
        LayerContent::Fill(_) | LayerContent::Raster(_) => {
            let src_path = if let Some(v) = &layer.video
                && let VideoSource::File { path } = &v.source
            {
                path.clone()
            } else {
                format!("{}/layer_{}.png", opts.asset_dir.trim_end_matches('/'), layer.id.0)
            };
            let (fps_attr, dur_attr) = match &layer.video {
                Some(v) => (v.fps.max(1.0), v.len().max(1)),
                None => (opts.fps.max(1.0), 1),
            };
            let _ = writeln!(
                out,
                "{pad}  <footage src=\"{}\" width=\"{}\" height=\"{}\" dropZone=\"0\" fps=\"{fps_attr:.4}\" durationFrames=\"{dur_attr}\" />",
                xml_escape(&src_path),
                doc.size.width.max(1),
                doc.size.height.max(1),
            );
        }
        LayerContent::Smart(sm) => {
            let src_path = match &sm.source {
                crate::SmartSource::Embedded { file_name, .. } => {
                    format!("{}/{}", opts.asset_dir.trim_end_matches('/'), file_name)
                }
                crate::SmartSource::Linked { path } => path.clone(),
            };
            let _ = writeln!(
                out,
                "{pad}  <footage src=\"{}\" width=\"{}\" height=\"{}\" dropZone=\"1\" fps=\"{:.4}\" durationFrames=\"{}\" />",
                xml_escape(&src_path),
                doc.size.width.max(1),
                doc.size.height.max(1),
                opts.fps.max(1.0),
                opts.duration_frames.max(1),
            );
        }
        LayerContent::Adjustment(adj) => {
            let _ = writeln!(
                out,
                "{pad}  <adjustment kind=\"{}\" />",
                xml_escape(adjustment_kind_name(adj))
            );
        }
    }

    let _ = writeln!(out, "{pad}</layer>");
}

fn layer_transform(layer: &Layer, doc: &Document) -> (f64, f64, f64, f64, f64) {
    let cx = doc.size.width as f64 * 0.5;
    let cy = doc.size.height as f64 * 0.5;
    let aff = match &layer.content {
        LayerContent::Text(t) => Some(t.transform),
        LayerContent::Smart(s) => Some(s.transform),
        _ => None,
    };
    if let Some(a) = aff {
        let [m0, m1, m2, m3, tx, ty] = a.m;
        let sx = (m0 * m0 + m1 * m1).sqrt();
        let sy = (m2 * m2 + m3 * m3).sqrt();
        let rot_rad = m1.atan2(m0);
        let pos_x = if tx == 0.0 && ty == 0.0 { cx } else { tx };
        let pos_y = if tx == 0.0 && ty == 0.0 { cy } else { ty };
        (
            pos_x,
            pos_y,
            (sx * 100.0).clamp(0.01, 10000.0),
            (sy * 100.0).clamp(0.01, 10000.0),
            rot_rad.to_degrees(),
        )
    } else {
        (cx, cy, 100.0, 100.0, 0.0)
    }
}

fn write_text_content(out: &mut String, t: &TextLayer, dpi: f32, indent: usize) {
    let pad = "  ".repeat(indent);
    let runs = t.char_runs();
    let style = runs.first().map(|r| &r.style);
    let fallback_font = if t.font_family.is_empty() {
        "Inter"
    } else {
        t.font_family.as_str()
    };
    let font = style
        .and_then(|s| s.postscript_name.as_deref().or(Some(s.font_family.as_str())))
        .filter(|f| !f.is_empty())
        .unwrap_or(fallback_font);
    let pt_to_px = (dpi.max(1.0) / 72.0).max(0.1);
    let size_px = (t.size_pt.max(1.0) * pt_to_px).clamp(1.0, 4096.0);
    let [r, g, b] = t.color.to_rgb();
    let align = t
        .paragraphs
        .first()
        .map_or("left", |p| match p.style.align {
            TextAlign::Left | TextAlign::JustifyLeft => "left",
            TextAlign::Center | TextAlign::JustifyCenter => "center",
            TextAlign::Right | TextAlign::JustifyRight | TextAlign::JustifyAll => "right",
        });
    let tracking = style.map_or(0.0, |s| s.tracking);
    let leading = style
        .and_then(|s| s.leading_pt)
        .map_or(size_px * 1.2, |l| l * pt_to_px);

    let _ = writeln!(out, "{pad}<text>");
    let _ = writeln!(out, "{pad}  <string>{}</string>", xml_escape(&t.text));
    let _ = writeln!(out, "{pad}  <font>{}</font>", xml_escape(font));
    let _ = writeln!(out, "{pad}  <size>{size_px:.3}</size>");
    let _ = writeln!(out, "{pad}  <alignment>{align}</alignment>");
    let _ = writeln!(
        out,
        "{pad}  <color r=\"{r:.4}\" g=\"{g:.4}\" b=\"{b:.4}\" a=\"{:.4}\" />",
        t.color.alpha.clamp(0.0, 1.0)
    );
    let _ = writeln!(out, "{pad}  <tracking>{tracking:.3}</tracking>");
    let _ = writeln!(out, "{pad}  <leading>{leading:.3}</leading>");
    let _ = writeln!(out, "{pad}</text>");
}

fn write_shape_content(out: &mut String, s: &ShapeLayer, indent: usize) {
    let pad = "  ".repeat(indent);
    let rule = match s.path.fill_rule {
        crate::FillRule::NonZero => "NonZero",
        crate::FillRule::EvenOdd => "EvenOdd",
    };
    let _ = writeln!(out, "{pad}<shape fillRule=\"{rule}\">");
    if let Some(fill) = &s.fill {
        let c = fill_representative_color(fill);
        let [r, g, b] = c.to_rgb();
        let _ = writeln!(
            out,
            "{pad}  <fill enabled=\"1\" r=\"{r:.4}\" g=\"{g:.4}\" b=\"{b:.4}\" a=\"{:.4}\" opacity=\"{:.4}\" />",
            c.alpha.clamp(0.0, 1.0),
            s.density.clamp(0.0, 1.0),
        );
    } else {
        let _ = writeln!(
            out,
            "{pad}  <fill enabled=\"0\" r=\"0.0\" g=\"0.0\" b=\"0.0\" a=\"0.0\" opacity=\"0.0\" />"
        );
    }
    if let Some(st) = &s.stroke
        && st.width > 0.0
    {
        let c = fill_representative_color(&st.paint);
        let [r, g, b] = c.to_rgb();
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
            "{pad}  <stroke enabled=\"1\" width=\"{:.3}\" r=\"{r:.4}\" g=\"{g:.4}\" b=\"{b:.4}\" a=\"{:.4}\" opacity=\"{:.4}\" cap=\"{cap}\" join=\"{join}\" />",
            st.width,
            c.alpha.clamp(0.0, 1.0),
            st.opacity.clamp(0.0, 1.0),
        );
    }
    write_path(out, &s.path, "path", indent + 1);
    let _ = writeln!(out, "{pad}</shape>");
}

fn write_path(out: &mut String, path: &Path, tag: &str, indent: usize) {
    let pad = "  ".repeat(indent);
    for sp in &path.subpaths {
        if sp.knots.is_empty() {
            continue;
        }
        let _ = writeln!(out, "{pad}<{tag} closed=\"{}\">", u8::from(sp.closed));
        for k in &sp.knots {
            let _ = writeln!(
                out,
                "{pad}  <knot x=\"{:.3}\" y=\"{:.3}\" inX=\"{:.3}\" inY=\"{:.3}\" outX=\"{:.3}\" outY=\"{:.3}\" smooth=\"{}\" />",
                k.anchor.x,
                k.anchor.y,
                k.in_ctrl.x,
                k.in_ctrl.y,
                k.out_ctrl.x,
                k.out_ctrl.y,
                u8::from(k.smooth),
            );
        }
        let _ = writeln!(out, "{pad}</{tag}>");
    }
}

fn write_effect(out: &mut String, fx: &Effect, indent: usize) {
    let pad = "  ".repeat(indent);
    match fx {
        Effect::DropShadow(sh) => {
            let [r, g, b] = sh.color.to_rgb();
            let _ = writeln!(
                out,
                "{pad}<filter type=\"DropShadow\" enabled=\"1\" opacity=\"{:.4}\" angle=\"{:.3}\" distance=\"{:.3}\" blur=\"{:.3}\" r=\"{r:.4}\" g=\"{g:.4}\" b=\"{b:.4}\" a=\"{:.4}\" />",
                sh.common.opacity.clamp(0.0, 1.0),
                sh.angle,
                sh.distance,
                sh.size,
                sh.color.alpha.clamp(0.0, 1.0),
            );
        }
        Effect::OuterGlow(gl) => {
            let c = match &gl.paint {
                crate::FxPaint::Color(c) => *c,
                crate::FxPaint::Gradient(gr) => {
                    gr.stops.first().map_or(Color::WHITE, |(_, col)| *col)
                }
                crate::FxPaint::Pattern { .. } => Color::WHITE,
            };
            let [r, g, b] = c.to_rgb();
            let _ = writeln!(
                out,
                "{pad}<filter type=\"OuterGlow\" enabled=\"1\" opacity=\"{:.4}\" blur=\"{:.3}\" r=\"{r:.4}\" g=\"{g:.4}\" b=\"{b:.4}\" a=\"{:.4}\" />",
                gl.common.opacity.clamp(0.0, 1.0),
                gl.size,
                c.alpha.clamp(0.0, 1.0),
            );
        }
        _ => {}
    }
}

fn fill_representative_color(fill: &Fill) -> Color {
    match fill {
        Fill::Solid(c) => *c,
        Fill::Gradient { stops, .. } => stops.first().map_or(Color::WHITE, |(_, c)| *c),
        Fill::Pattern { .. } => Color::WHITE,
    }
}

fn adjustment_kind_name(adj: &crate::Adjustment) -> &'static str {
    use crate::Adjustment::*;
    match adj {
        BrightnessContrast { .. } => "BrightnessContrast",
        Levels { .. } => "Levels",
        Curves { .. } => "Curves",
        Exposure { .. } => "Exposure",
        Vibrance { .. } => "Vibrance",
        HueSaturation { .. } => "HueSaturation",
        ColorBalance { .. } => "ColorBalance",
        BlackWhite { .. } => "BlackAndWhite",
        PhotoFilter { .. } => "PhotoFilter",
        ChannelMixer { .. } => "ChannelMixer",
        ColorLookup { .. } => "ColorLookup",
        Invert => "Invert",
        Posterize { .. } => "Posterize",
        Threshold { .. } => "Threshold",
        GradientMap { .. } => "GradientMap",
        SelectiveColor { .. } => "SelectiveColor",
        Unsupported { .. } => "Adjustment",
    }
}

fn blend_mode_name(b: BlendMode) -> &'static str {
    match b {
        BlendMode::PassThrough => "PassThrough",
        BlendMode::Normal | BlendMode::Dissolve => "Normal",
        BlendMode::Darken | BlendMode::DarkerColor => "Darken",
        BlendMode::Multiply => "Multiply",
        BlendMode::ColorBurn | BlendMode::PaintNetColorBurn => "ColorBurn",
        BlendMode::LinearBurn => "LinearBurn",
        BlendMode::Lighten | BlendMode::LighterColor => "Lighten",
        BlendMode::Screen | BlendMode::Glow => "Screen",
        BlendMode::ColorDodge | BlendMode::Reflect | BlendMode::PaintNetColorDodge => "ColorDodge",
        BlendMode::LinearDodge => "Add",
        BlendMode::Overlay => "Overlay",
        BlendMode::SoftLight => "SoftLight",
        BlendMode::HardLight => "HardLight",
        BlendMode::VividLight => "VividLight",
        BlendMode::LinearLight => "LinearLight",
        BlendMode::PinLight => "PinLight",
        BlendMode::HardMix => "HardMix",
        BlendMode::Difference | BlendMode::Negation => "Difference",
        BlendMode::Exclusion | BlendMode::Xor => "Exclusion",
        BlendMode::Subtract => "Subtract",
        BlendMode::Divide => "Divide",
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
    use crate::{
        ColorMode, Effects, FxCommon, LabelColor, SampleType, Shadow, ShapeStroke, Size, Subpath,
    };

    #[test]
    fn exports_layered_document_to_apple_motion_ozml() {
        let mut doc = Document::new(
            "LowerThirdCard",
            Size::new(1920, 1080),
            ColorMode::Rgb,
            SampleType::U8,
        );

        let bar_path = Path::new(vec![Subpath::polygon(&[
            (120.0, 820.0),
            (720.0, 820.0),
            (720.0, 940.0),
            (120.0, 940.0),
        ])]);
        let mut shape_layer = Layer::new(
            "Accent Bar",
            LayerContent::Shape(ShapeLayer {
                path: bar_path,
                fill: Some(Fill::Solid(Color::rgb(0.95, 0.25, 0.15))),
                stroke: Some(ShapeStroke {
                    width: 4.0,
                    paint: Fill::Solid(Color::WHITE),
                    ..Default::default()
                }),
                ..Default::default()
            }),
        );
        shape_layer.label = LabelColor::Red;
        shape_layer.blend = BlendMode::Screen;
        shape_layer.effects = Effects {
            enabled: true,
            items: vec![Effect::DropShadow(Shadow {
                common: FxCommon::new(BlendMode::Multiply, 0.75),
                color: Color::BLACK,
                angle: 135.0,
                use_global_light: true,
                distance: 10.0,
                spread: 0.0,
                size: 16.0,
                contour: crate::Contour::Linear,
                anti_alias: true,
                noise: 0.0,
                knocks_out: true,
            })],
            psd_raw: None,
            reference: None,
        };

        let mut text_layer = Layer::new(
            "Headline",
            LayerContent::Text(TextLayer {
                text: "ArtCraft Studio".into(),
                font_family: "Inter".into(),
                size_pt: 48.0,
                color: Color::WHITE,
                transform: crate::Affine::translate(200.0, 880.0),
                ..Default::default()
            }),
        );
        text_layer.opacity = 0.9;

        let group = Layer::group("Lower Third Group", vec![shape_layer, text_layer]);
        doc.layers.push(group);

        let ozml = export_motion_ozml(&doc, &MotionExportOptions::default());
        assert!(ozml.contains("<ozml version=\"5.10\" generator=\"PhotoCraft\">"));
        assert!(ozml.contains("<scene name=\"LowerThirdCard\">"));
        assert!(ozml.contains("<width>1920</width>"));
        assert!(ozml.contains("<height>1080</height>"));
        assert!(ozml.contains("<string>ArtCraft Studio</string>"));
        assert!(ozml.contains("<shape fillRule=\"NonZero\">"));
        assert!(ozml.contains("<knot x=\"120.000\" y=\"820.000\""));
        assert!(ozml.contains("<filter type=\"DropShadow\""));
    }
}
