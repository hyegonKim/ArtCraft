//! AGC style conversion: solid/gradient/image/video fills, strokes, blend modes, transforms and
//! shadow/blur filters.

use figcraft_doc::color::{
    BlendMode, Color, ColorStop, Gradient, GradientKind, ImageFilters, ImagePaint, ImageScaleMode, Paint, PaintKind, VideoPaint,
};
use figcraft_doc::geom::Affine;
use figcraft_doc::{Blur, Effect, EffectKind, Shadow, StrokeAlign, StrokeCap, StrokeJoin, StrokeProps, VideoData};
use serde_json::Value;

use super::tree::ImportCtx;

const MAX_COORD: f64 = 1_000_000.0;
const MAX_STROKE: f64 = 10_000.0;
const MAX_STOPS: usize = 128;

pub(super) fn f64_at(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64).filter(|n| n.is_finite())
}

pub(super) fn str_at<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

pub(super) fn bool_at(v: &Value, key: &str) -> Option<bool> {
    v.get(key).and_then(Value::as_bool)
}

/// Parse an AGC transform object (`{a, b, c, d, tx, ty}` or `{a, b, c, d, e, f}`).
pub(super) fn parse_transform(v: Option<&Value>) -> Affine {
    let Some(t) = v else { return Affine::IDENTITY };
    let a = f64_at(t, "a").unwrap_or(1.0).clamp(-1e4, 1e4);
    let b = f64_at(t, "b").unwrap_or(0.0).clamp(-1e4, 1e4);
    let c = f64_at(t, "c").unwrap_or(0.0).clamp(-1e4, 1e4);
    let d = f64_at(t, "d").unwrap_or(1.0).clamp(-1e4, 1e4);
    let tx = f64_at(t, "tx").or_else(|| f64_at(t, "e")).unwrap_or(0.0).clamp(-MAX_COORD, MAX_COORD);
    let ty = f64_at(t, "ty").or_else(|| f64_at(t, "f")).unwrap_or(0.0).clamp(-MAX_COORD, MAX_COORD);
    let xf = Affine::new([a, b, c, d, tx, ty]);
    if xf.determinant().abs() < 1e-9 {
        Affine::translate((tx, ty))
    } else {
        xf
    }
}

/// Parse an AGC color representation:
/// - `{"mode": "RGB", "value": {"r": 255, "g": 128, "b": 0}, "alpha": 0.9}`
/// - `{"value": 16711680, "alpha": 1.0}` (`0xRRGGBB` integer)
/// - `"#RRGGBB"` or `{"value": "#RRGGBB"}`
pub(super) fn parse_color(v: &Value) -> Option<Color> {
    if let Some(hex) = v.as_str() {
        return Color::from_hex(hex);
    }
    let alpha = f64_at(v, "alpha").or_else(|| f64_at(v, "a")).unwrap_or(1.0).clamp(0.0, 1.0) as f32;
    let mode_rgb = str_at(v, "mode").is_some_and(|m| m.eq_ignore_ascii_case("rgb"));
    if let Some(val) = v.get("value") {
        if let Some(n) = val.as_u64() {
            let r = ((n >> 16) & 0xFF) as u8;
            let g = ((n >> 8) & 0xFF) as u8;
            let b = (n & 0xFF) as u8;
            return Some(Color::from_rgb8_const(r, g, b).with_alpha(alpha));
        }
        if let Some(hex) = val.as_str() {
            return Color::from_hex(hex).map(|c| c.with_alpha(c.a * alpha));
        }
        if val.is_object() {
            let (r, g, b) = rgb_channels(val, mode_rgb || val.get("r").is_some_and(Value::is_i64))?;
            return Some(Color::new(r, g, b, alpha));
        }
    }
    if v.get("r").is_some() {
        let is_255 = mode_rgb || v.get("r").is_some_and(Value::is_i64);
        let (r, g, b) = rgb_channels(v, is_255)?;
        return Some(Color::new(r, g, b, alpha));
    }
    None
}

fn rgb_channels(v: &Value, scale_255: bool) -> Option<(f32, f32, f32)> {
    let r = f64_at(v, "r")?;
    let g = f64_at(v, "g")?;
    let b = f64_at(v, "b")?;
    let denom = if scale_255 || r > 1.0 || g > 1.0 || b > 1.0 { 255.0 } else { 1.0 };
    Some((
        (r / denom).clamp(0.0, 1.0) as f32,
        (g / denom).clamp(0.0, 1.0) as f32,
        (b / denom).clamp(0.0, 1.0) as f32,
    ))
}

pub(super) fn parse_blend_mode(s: Option<&str>) -> BlendMode {
    match s.unwrap_or("normal").trim().to_ascii_lowercase().as_str() {
        "pass-through" | "passthrough" => BlendMode::PassThrough,
        "darken" => BlendMode::Darken,
        "multiply" => BlendMode::Multiply,
        "color-burn" | "colorburn" => BlendMode::ColorBurn,
        "lighten" => BlendMode::Lighten,
        "screen" => BlendMode::Screen,
        "color-dodge" | "colordodge" => BlendMode::ColorDodge,
        "overlay" => BlendMode::Overlay,
        "soft-light" | "softlight" => BlendMode::SoftLight,
        "hard-light" | "hardlight" => BlendMode::HardLight,
        "difference" => BlendMode::Difference,
        "exclusion" => BlendMode::Exclusion,
        "hue" => BlendMode::Hue,
        "saturation" => BlendMode::Saturation,
        "color" => BlendMode::Color,
        "luminosity" => BlendMode::Luminosity,
        _ => BlendMode::Normal,
    }
}

pub(super) fn parse_fills(style: Option<&Value>, box_w: f64, box_h: f64, ctx: &mut ImportCtx<'_>) -> Vec<Paint> {
    let Some(st) = style else { return Vec::new() };
    let Some(fill) = st.get("fill") else { return Vec::new() };
    let mut out = Vec::new();
    if let Some(arr) = fill.as_array() {
        for item in arr.iter().take(16) {
            if let Some(p) = parse_one_fill(item, box_w, box_h, ctx) {
                out.push(p);
            }
        }
    } else if let Some(p) = parse_one_fill(fill, box_w, box_h, ctx) {
        out.push(p);
    }
    out
}

pub(super) fn parse_one_fill(fill: &Value, box_w: f64, box_h: f64, ctx: &mut ImportCtx<'_>) -> Option<Paint> {
    let ty = str_at(fill, "type").unwrap_or("solid");
    let visible = bool_at(fill, "visible").unwrap_or(true);
    let fill_opacity = f64_at(fill, "alpha").or_else(|| f64_at(fill, "opacity")).unwrap_or(1.0).clamp(0.0, 1.0) as f32;
    let mut paint = match ty {
        "none" => return None,
        "solid" => {
            let c = fill.get("color").and_then(parse_color).or_else(|| parse_color(fill))?;
            let mut p = Paint::solid(c);
            p.opacity *= fill_opacity;
            p
        }
        "gradient" => {
            let g = parse_gradient(fill, box_w, box_h, ctx)?;
            let mut p = Paint::gradient(g);
            p.opacity = fill_opacity;
            p
        }
        "pattern" => {
            let mut p = parse_pattern_fill(fill, ctx)?;
            p.opacity = fill_opacity;
            p
        }
        other => {
            ctx.warn(format!("Unsupported Adobe XD fill type `{other}` was skipped"));
            return None;
        }
    };
    paint.visible = visible;
    Some(paint)
}

fn parse_gradient(fill: &Value, box_w: f64, box_h: f64, ctx: &mut ImportCtx<'_>) -> Option<Gradient> {
    let grad = fill.get("gradient").unwrap_or(fill);
    let ux_grad = grad
        .get("meta")
        .and_then(|m| m.get("ux"))
        .and_then(|u| u.get("gradientResources"))
        .or_else(|| fill.get("meta").and_then(|m| m.get("ux")).and_then(|u| u.get("gradientResources")));

    let ref_id = str_at(grad, "ref").or_else(|| str_at(fill, "gradientRef"));
    let resolved_def = ref_id.and_then(|id| ctx.gradients.get(id)).cloned();

    let kind_str = str_at(grad, "type")
        .or_else(|| ux_grad.and_then(|u| str_at(u, "type")))
        .or_else(|| resolved_def.as_ref().and_then(|d| str_at(d, "type")))
        .unwrap_or("linear");
    let kind = match kind_str.to_ascii_lowercase().as_str() {
        "radial" => GradientKind::Radial,
        "angular" | "conic" | "sweep" => GradientKind::Angular,
        _ => GradientKind::Linear,
    };

    let stops_val = grad
        .get("stops")
        .or_else(|| ux_grad.and_then(|u| u.get("stops")))
        .or_else(|| resolved_def.as_ref().and_then(|d| d.get("stops")));
    let mut stops = Vec::new();
    if let Some(arr) = stops_val.and_then(Value::as_array) {
        for s in arr.iter().take(MAX_STOPS) {
            let pos = f64_at(s, "offset").or_else(|| f64_at(s, "position")).unwrap_or(0.0).clamp(0.0, 1.0) as f32;
            if let Some(c) = s.get("color").and_then(parse_color).or_else(|| parse_color(s)) {
                stops.push(ColorStop { position: pos, color: c });
            }
        }
    }
    if stops.is_empty() {
        return None;
    }
    if stops.len() == 1
        && let Some(only) = stops.first().cloned()
    {
        stops.push(ColorStop { position: 1.0, color: only.color });
    }

    let units_user = str_at(grad, "units")
        .or_else(|| resolved_def.as_ref().and_then(|d| str_at(d, "units")))
        .is_some_and(|u| u == "userSpaceOnUse");
    let norm_x = |v: f64| if units_user && box_w > 1e-6 { (v / box_w).clamp(-10.0, 10.0) } else { v.clamp(-10.0, 10.0) };
    let norm_y = |v: f64| if units_user && box_h > 1e-6 { (v / box_h).clamp(-10.0, 10.0) } else { v.clamp(-10.0, 10.0) };

    let mut g = Gradient::default_for(kind, Color::BLACK);
    g.stops = stops;

    match kind {
        GradientKind::Linear => {
            let x1 = f64_at(grad, "x1").or_else(|| ux_grad.and_then(|u| f64_at(u, "x1")));
            let y1 = f64_at(grad, "y1").or_else(|| ux_grad.and_then(|u| f64_at(u, "y1")));
            let x2 = f64_at(grad, "x2").or_else(|| ux_grad.and_then(|u| f64_at(u, "x2")));
            let y2 = f64_at(grad, "y2").or_else(|| ux_grad.and_then(|u| f64_at(u, "y2")));
            if let (Some(x1), Some(y1), Some(x2), Some(y2)) = (x1, y1, x2, y2) {
                let from = (norm_x(x1), norm_y(y1));
                let to = (norm_x(x2), norm_y(y2));
                g.from = from;
                g.to = to;
                g.width = (from.0 - (to.1 - from.1), from.1 + (to.0 - from.0));
            }
        }
        GradientKind::Radial | GradientKind::Angular | GradientKind::Diamond => {
            let cx = f64_at(grad, "cx").or_else(|| ux_grad.and_then(|u| f64_at(u, "cx"))).map(norm_x).unwrap_or(0.5);
            let cy = f64_at(grad, "cy").or_else(|| ux_grad.and_then(|u| f64_at(u, "cy"))).map(norm_y).unwrap_or(0.5);
            let r = f64_at(grad, "r").or_else(|| ux_grad.and_then(|u| f64_at(u, "r"))).map(norm_x).unwrap_or(0.5).max(0.01);
            g.from = (cx, cy);
            g.to = (cx + r, cy);
            g.width = (cx, cy + r);
        }
    }
    Some(g)
}

fn parse_pattern_fill(fill: &Value, ctx: &mut ImportCtx<'_>) -> Option<Paint> {
    let pat = fill.get("pattern").unwrap_or(fill);
    let ux = pat
        .get("meta")
        .and_then(|m| m.get("ux"))
        .or_else(|| fill.get("meta").and_then(|m| m.get("ux")));

    let uid = ux
        .and_then(|u| str_at(u, "uid").or_else(|| str_at(u, "videoUid")))
        .or_else(|| str_at(pat, "uid"))
        .or_else(|| {
            str_at(pat, "href").map(|h| h.trim_start_matches("resources/").trim_start_matches("./"))
        })?;

    let scale_mode = match ux
        .and_then(|u| str_at(u, "scaleBehavior"))
        .or_else(|| str_at(pat, "scaleBehavior"))
        .unwrap_or("fill")
        .to_ascii_lowercase()
        .as_str()
    {
        "fit" | "contain" => ImageScaleMode::Fit,
        "tile" | "repeat" => ImageScaleMode::Tile,
        "stretch" | "none" | "crop" => ImageScaleMode::Crop,
        _ => ImageScaleMode::Fill,
    };
    let tile_scale = ux.and_then(|u| f64_at(u, "scale")).unwrap_or(1.0).clamp(0.01, 100.0);

    let poster_uid = ux.and_then(|u| str_at(u, "posterUid").or_else(|| str_at(u, "thumbnailUid")));
    let is_video_hint = ux.and_then(|u| bool_at(u, "isVideo")).unwrap_or(false)
        || str_at(pat, "mediaType").is_some_and(|m| m.starts_with("video"))
        || ctx.asset_mime(uid).is_some_and(|m| m.starts_with("video/"));

    if is_video_hint {
        if let Some((vhash, phash)) = ctx.load_video_asset(uid, poster_uid) {
            let mut vp = VideoPaint::new(vhash, Some(phash), scale_mode);
            vp.scale = tile_scale;
            if let Some(u) = ux {
                vp.autoplay = bool_at(u, "autoplay").or_else(|| bool_at(u, "autoPlay")).unwrap_or(true);
                vp.loop_playback = bool_at(u, "loop").or_else(|| bool_at(u, "loopPlayback")).unwrap_or(true);
                vp.muted = bool_at(u, "muted").unwrap_or(false);
            }
            return Some(Paint {
                kind: PaintKind::Video(vp),
                opacity: 1.0,
                visible: true,
                blend: BlendMode::Normal,
                color_alpha: 1.0,
            });
        }
    }

    // Check raw bytes: an asset without a manifest MIME may still be an MP4/MOV/WebM video.
    if let Some(raw) = ctx.read_asset_bytes(uid) {
        if VideoData::is_video_bytes(&raw) {
            if let Some((vhash, phash)) = ctx.register_video_bytes(uid, raw, poster_uid) {
                let mut vp = VideoPaint::new(vhash, Some(phash), scale_mode);
                vp.scale = tile_scale;
                return Some(Paint {
                    kind: PaintKind::Video(vp),
                    opacity: 1.0,
                    visible: true,
                    blend: BlendMode::Normal,
                    color_alpha: 1.0,
                });
            }
        } else if let Some(ihash) = ctx.register_image_bytes(uid, raw) {
            return Some(Paint {
                kind: PaintKind::Image(ImagePaint {
                    image: ihash,
                    scale_mode,
                    scale: tile_scale,
                    rotation: 0,
                    crop: None,
                    filters: ImageFilters::default(),
                }),
                opacity: 1.0,
                visible: true,
                blend: BlendMode::Normal,
                color_alpha: 1.0,
            });
        }
    }

    ctx.warn(format!("Pattern asset `{uid}` is missing or unreadable in `.xd` archive"));
    None
}

pub(super) fn parse_stroke(style: Option<&Value>, ctx: &mut ImportCtx<'_>) -> (Vec<Paint>, StrokeProps) {
    let mut props = StrokeProps::default();
    let Some(st) = style else { return (Vec::new(), props) };
    let Some(stroke) = st.get("stroke") else { return (Vec::new(), props) };
    let ty = str_at(stroke, "type").unwrap_or("solid");
    if ty == "none" {
        return (Vec::new(), props);
    }
    let visible = bool_at(stroke, "visible").unwrap_or(true);
    props.weight = f64_at(stroke, "width").unwrap_or(1.0).clamp(0.0, MAX_STROKE);
    props.align = match str_at(stroke, "align").unwrap_or("inside").to_ascii_lowercase().as_str() {
        "outside" | "outer" => StrokeAlign::Outside,
        "center" | "middle" => StrokeAlign::Center,
        _ => StrokeAlign::Inside,
    };
    let cap = match str_at(stroke, "cap").or_else(|| str_at(stroke, "lineCap")).unwrap_or("none").to_ascii_lowercase().as_str() {
        "round" => StrokeCap::Round,
        "square" | "projecting" => StrokeCap::Square,
        _ => StrokeCap::None,
    };
    props.cap_start = cap;
    props.cap_end = cap;
    props.join = match str_at(stroke, "join").or_else(|| str_at(stroke, "lineJoin")).unwrap_or("miter").to_ascii_lowercase().as_str() {
        "round" => StrokeJoin::Round,
        "bevel" => StrokeJoin::Bevel,
        _ => StrokeJoin::Miter,
    };
    if let Some(ml) = f64_at(stroke, "miterLimit").filter(|m| *m >= 1.0) {
        props.miter_limit = ml.clamp(1.0, 100.0);
    }
    if let Some(dashes) = stroke.get("dash").and_then(Value::as_array) {
        props.dashes = dashes
            .iter()
            .take(32)
            .filter_map(Value::as_f64)
            .filter(|d| d.is_finite() && *d >= 0.0)
            .map(|d| d.min(MAX_STROKE))
            .collect();
    }

    let mut paints = Vec::new();
    if ty == "gradient" {
        if let Some(mut p) = parse_one_fill(stroke, 100.0, 100.0, ctx) {
            p.visible = visible;
            paints.push(p);
        }
    } else if let Some(c) = stroke.get("color").and_then(parse_color).or_else(|| parse_color(stroke)) {
        let mut p = Paint::solid(c);
        p.visible = visible;
        paints.push(p);
    }
    (paints, props)
}

pub(super) fn parse_effects(style: Option<&Value>, ctx: &mut ImportCtx<'_>) -> Vec<Effect> {
    let Some(st) = style else { return Vec::new() };
    let Some(filters) = st.get("filters").and_then(Value::as_array) else { return Vec::new() };
    let mut out = Vec::new();
    for f in filters.iter().take(32) {
        let ty = str_at(f, "type").unwrap_or("");
        let visible = bool_at(f, "visible").unwrap_or(true);
        let params = f.get("params").unwrap_or(f);
        match ty {
            "uxdesign#dropShadow" | "dropShadow" => {
                if let Some(list) = params.get("dropShadows").and_then(Value::as_array) {
                    for item in list.iter().take(8) {
                        out.push(Effect { kind: EffectKind::DropShadow(parse_shadow(item)), visible });
                    }
                } else {
                    out.push(Effect { kind: EffectKind::DropShadow(parse_shadow(params)), visible });
                }
            }
            "uxdesign#innerShadow" | "innerShadow" => {
                if let Some(list) = params.get("innerShadows").and_then(Value::as_array) {
                    for item in list.iter().take(8) {
                        out.push(Effect { kind: EffectKind::InnerShadow(parse_shadow(item)), visible });
                    }
                } else {
                    out.push(Effect { kind: EffectKind::InnerShadow(parse_shadow(params)), visible });
                }
            }
            "uxdesign#backgroundBlur" | "backgroundBlur" => {
                let r = f64_at(params, "blurAmount")
                    .or_else(|| f64_at(params, "radius"))
                    .unwrap_or(10.0)
                    .clamp(0.0, 1000.0);
                out.push(Effect { kind: EffectKind::BackgroundBlur(Blur::uniform(r)), visible });
            }
            "uxdesign#objectBlur" | "objectBlur" | "uxdesign#blur" | "blur" => {
                let r = f64_at(params, "blurAmount")
                    .or_else(|| f64_at(params, "radius"))
                    .unwrap_or(10.0)
                    .clamp(0.0, 1000.0);
                let is_bg = bool_at(params, "backgroundEffect").unwrap_or(false);
                let kind = if is_bg {
                    EffectKind::BackgroundBlur(Blur::uniform(r))
                } else {
                    EffectKind::LayerBlur(Blur::uniform(r))
                };
                out.push(Effect { kind, visible });
            }
            other if !other.is_empty() => {
                ctx.warn(format!("Unsupported Adobe XD filter `{other}` was skipped"));
            }
            _ => {}
        }
    }
    out
}

fn parse_shadow(v: &Value) -> Shadow {
    let dx = f64_at(v, "dx").or_else(|| f64_at(v, "x")).unwrap_or(0.0).clamp(-MAX_STROKE, MAX_STROKE);
    let dy = f64_at(v, "dy").or_else(|| f64_at(v, "y")).unwrap_or(3.0).clamp(-MAX_STROKE, MAX_STROKE);
    let radius = f64_at(v, "r")
        .or_else(|| f64_at(v, "blur"))
        .or_else(|| f64_at(v, "radius"))
        .unwrap_or(6.0)
        .clamp(0.0, MAX_STROKE);
    let spread = f64_at(v, "spread").or_else(|| f64_at(v, "choke")).unwrap_or(0.0).clamp(-MAX_STROKE, MAX_STROKE);
    let color = v
        .get("color")
        .and_then(parse_color)
        .unwrap_or_else(|| Color::new(0.0, 0.0, 0.0, 0.16));
    Shadow {
        color,
        offset: (dx, dy),
        radius,
        spread,
        blend: BlendMode::Normal,
        show_behind: false,
    }
}
