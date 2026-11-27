//! Photoshop 2027 (`28.0.0.20`) engine commands:
//! - NextGen Filters & Live Adjustments: `Fireflies` (`반딧불이`), `Water Ripple` (`물결`)
//! - 3D Relight / Lightshop Layer & Filter (`새 재조명 레이어…` / `재조명`)
//! - Oil Brush Tool (`유화 브러시 도구`: `Blocky`, `Glaze`, `Round`, `Paint Mix`)
//! - Remove Ambient Effects (`주변 효과 제거…`: `Shadows`, `Reflections`, `Lens flares`, `Atmospheric`)
//! - Type 2027: `Convert to Text Layers` (`텍스트 레이어로 변환`), `Dynamic Fit` & `Release Dynamic Fit` (`다이나믹 맞춤 풀기`), `Copy/Paste Text Attributes` (`텍스트 속성 붙여넣기`)
//! - Generative Recompose (`생성형 재구성…`)
//! - Generative Restyle Layer, Filter & `Capture as Layer` (`생성형 스타일 변경` / `레이어로 캡처`)
//! - Mockup Group, `Environmental Lighting` & `Release Mockup` (`목업` / `환경 조명` / `목업 풀기`)
//! - Agent Skills 8 One-Click Workflows (`에이전트 스킬`)

use photocraft_algo::ambient_remove::{AmbientMode, AmbientRemoveParams, remove_ambient_effects_surface};
use photocraft_algo::nge_fx::{FirefliesParams, WaterRippleParams, apply_fireflies_surface, apply_water_ripple_surface};
use photocraft_algo::raster_to_text::detect_and_clean_raster_text;
use photocraft_algo::recompose::{RecomposeParams, generative_recompose_surface};
use photocraft_algo::relight_3d::{Lamp3d, Relight3dParams, apply_relight_3d_surface};
use photocraft_algo::resample::{Resample, resize_surface};
use photocraft_algo::restyle::{RestyleParams, RestyleStyle, apply_restyle_surface};
use photocraft_algo::{FilterParams, matting, segment::subject};
use photocraft_color::{BlendMode, Color, ColorMode, PixelFormat};
use photocraft_doc::adjust::{Adjustment, CurvePoint, ToneSpace};
use photocraft_doc::effects::{Contour, Effect, FxCommon, GradientStyle, Shadow};
use photocraft_doc::ps2027::{LiveBackdropFx, RelightLamp, RestylePreset, TextAttributeClipboard};
use photocraft_doc::text::{CharStyle, ParagraphRun, TextRun, TextShape};
use photocraft_doc::{Document, Fill, Group, Layer, LayerContent, LayerId, LayerMask, Size, SmartFilter, TextLayer};
use photocraft_geom::{Affine, Rect};
use photocraft_paint::oil_brush::{OilBrushPreset, paint_oil_stroke};
use photocraft_raster::{Surface, from_rgba_into};
use serde_json::{Value, json};

use crate::commands::{CommandSpec, has_paintable, layer_param, mask_new_layer};
use crate::filters::has_filterable_layer;
use crate::jobs::JobCtx;
use crate::{EngineError, Result, Session};

fn bad(cmd: &str, msg: impl Into<String>) -> EngineError {
    EngineError::BadParams { cmd: cmd.into(), msg: msg.into() }
}

fn f32_or(p: &Value, k: &str, d: f32) -> f32 {
    p.get(k).and_then(Value::as_f64).filter(|v| v.is_finite()).map_or(d, |v| v as f32)
}

fn u32_or(p: &Value, k: &str, d: u32) -> u32 {
    p.get(k).and_then(Value::as_u64).and_then(|v| u32::try_from(v).ok()).unwrap_or(d)
}

fn bool_or(p: &Value, k: &str, d: bool) -> bool {
    p.get(k).and_then(Value::as_bool).unwrap_or(d)
}

fn rgb_or(p: &Value, k: &str, d: [f32; 3]) -> [f32; 3] {
    match p.get(k) {
        Some(Value::Array(arr)) if arr.len() >= 3 => {
            let g = |i: usize, def: f32| arr.get(i).and_then(Value::as_f64).filter(|v| v.is_finite()).map_or(def, |v| (v as f32).clamp(0.0, 1.0));
            [g(0, d[0]), g(1, d[1]), g(2, d[2])]
        }
        Some(Value::String(hex)) => {
            let h = hex.trim().trim_start_matches('#');
            let byte = |i: usize| h.get(i..i + 2).and_then(|s| u8::from_str_radix(s, 16).ok()).map(|v| f32::from(v) / 255.0);
            match (byte(0), byte(2), byte(4)) {
                (Some(r), Some(g), Some(b)) => [r, g, b],
                _ => d,
            }
        }
        _ => d,
    }
}

fn rgba_or(p: &Value, k: &str, d: [f32; 4]) -> [f32; 4] {
    let rgb = rgb_or(p, k, [d[0], d[1], d[2]]);
    let a = p
        .get(k)
        .and_then(Value::as_array)
        .and_then(|arr| arr.get(3))
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite())
        .map_or(d[3], |v| (v as f32).clamp(0.0, 1.0));
    [rgb[0], rgb[1], rgb[2], a]
}

fn has_doc(s: &Session) -> std::result::Result<(), String> {
    s.active().map(|_| ()).ok_or_else(|| "no document open".into())
}

fn has_rgb_or_gray_doc(s: &Session) -> std::result::Result<(), String> {
    let st = s.active().ok_or("no document open")?;
    if matches!(st.doc.mode, ColorMode::Bitmap | ColorMode::Indexed) { Err(format!("not available in {:?} mode", st.doc.mode)) } else { Ok(()) }
}

// ------------------------------------------------------------------ Parsers & Smart Filter Replay

pub fn parse_fireflies_params(p: &Value) -> FirefliesParams {
    let def = FirefliesParams::default();
    FirefliesParams {
        quantity: u32_or(p, "quantity", def.quantity).clamp(1, 300),
        depth_variation: f32_or(p, "depthVariation", def.depth_variation).clamp(0.0, 100.0),
        blurriness: f32_or(p, "blurriness", def.blurriness).clamp(0.0, 100.0),
        color: rgb_or(p, "color", def.color),
        progress: f32_or(p, "progress", def.progress).clamp(0.0, 100.0),
        seed: u32_or(p, "seed", def.seed),
    }
}

pub fn parse_water_ripple_params(p: &Value) -> WaterRippleParams {
    let def = WaterRippleParams::default();
    WaterRippleParams {
        horizon: f32_or(p, "horizon", def.horizon).clamp(0.05, 0.95),
        amplitude: f32_or(p, "amplitude", def.amplitude).clamp(0.0, 100.0),
        frequency: f32_or(p, "frequency", def.frequency).clamp(1.0, 100.0),
        progress: f32_or(p, "progress", def.progress).clamp(0.0, 100.0),
    }
}

pub fn parse_relight_lamps(p: &Value, default_lamps: [RelightLamp; 3]) -> [RelightLamp; 3] {
    let mut lamps = default_lamps;
    let arr_opt = p.get("lamps").or_else(|| p.get("lights")).and_then(Value::as_array);
    if let Some(arr) = arr_opt {
        for (idx, item) in arr.iter().take(3).enumerate() {
            let base = lamps[idx];
            lamps[idx] = RelightLamp {
                enabled: bool_or(item, "enabled", base.enabled),
                x: f32_or(item, "x", base.x),
                y: f32_or(item, "y", base.y),
                z: f32_or(item, "z", base.z),
                intensity: f32_or(item, "intensity", base.intensity),
                color: rgb_or(item, "color", base.color),
                spread: f32_or(item, "spread", base.spread),
            }
            .sanitized();
        }
    }
    lamps
}

fn lamps_to_algo(lamps: [RelightLamp; 3]) -> [Lamp3d; 3] {
    lamps.map(|l| Lamp3d { enabled: l.enabled, x: l.x, y: l.y, z: l.z, intensity: l.intensity, color: l.color, spread: l.spread })
}

pub fn parse_relight_algo_params(p: &Value) -> Relight3dParams {
    let lamps = parse_relight_lamps(p, RelightLamp::default_rig());
    Relight3dParams {
        ambient: f32_or(p, "ambient", 0.55).clamp(0.0, 1.5),
        surface_relief: f32_or(p, "surfaceRelief", 1.0).clamp(0.0, 2.0),
        lamps: lamps_to_algo(lamps),
    }
}

pub fn parse_restyle_preset(p: &Value, fallback: RestylePreset) -> RestylePreset {
    p.get("preset").or_else(|| p.get("style")).and_then(Value::as_str).and_then(RestylePreset::from_id).unwrap_or(fallback)
}

fn preset_to_algo_style(preset: RestylePreset) -> RestyleStyle {
    match preset {
        RestylePreset::OilPainting => RestyleStyle::OilPainting,
        RestylePreset::Watercolor => RestyleStyle::Watercolor,
        RestylePreset::CyberpunkNeon => RestyleStyle::CyberpunkNeon,
        RestylePreset::VintageFilm => RestyleStyle::VintageFilm,
        RestylePreset::PencilSketch => RestyleStyle::PencilSketch,
        RestylePreset::PopArt => RestyleStyle::PopArt,
        RestylePreset::AnimeCel => RestyleStyle::AnimeCel,
        RestylePreset::NoirInk => RestyleStyle::NoirInk,
    }
}

pub fn parse_restyle_algo_params(p: &Value) -> RestyleParams {
    let preset = parse_restyle_preset(p, RestylePreset::OilPainting);
    let prompt = p.get("prompt").and_then(Value::as_str).unwrap_or("").to_string();
    let intensity = f32_or(p, "intensity", 80.0).clamp(0.0, 100.0);
    RestyleParams { style: preset_to_algo_style(preset), prompt, intensity }
}

/// Smart Filter surface replay hook called from `crate::filters::apply_filter_to_surface_in`.
pub fn apply_to_surface(id: &str, params: &Value, surf: &Surface, canvas: Rect) -> Option<Surface> {
    let area = if canvas.is_empty() { surf.content_bounds() } else { canvas };
    if area.is_empty() {
        return Some(surf.clone());
    }
    match id {
        "filter.nge.fireflies" => Some(apply_fireflies_surface(surf, area, &parse_fireflies_params(params))),
        "filter.nge.waterRipple" => Some(apply_water_ripple_surface(surf, area, &parse_water_ripple_params(params))),
        "filter.render.relight3d" => Some(apply_relight_3d_surface(surf, area, &parse_relight_algo_params(params))),
        "filter.stylize.restyle" => Some(apply_restyle_surface(surf, area, &parse_restyle_algo_params(params))),
        _ => None,
    }
}

/// Blends `filtered` onto `base` inside `area` respecting optional `selection` mask and `lock_alpha`.
fn commit_filtered_surface(base: &mut Surface, filtered: &Surface, area: Rect, selection: Option<&Surface>, lock_alpha: bool) {
    if area.is_empty() {
        return;
    }
    let fmt = base.format();
    let ch = fmt.channels();
    let w = area.width() as usize;
    let b_raw = base.read_region(area);
    let f_raw = filtered.read_region(area);
    let mut out = b_raw.clone();
    for y in area.y0..area.y1 {
        let ry = (y - area.y0) as usize;
        for x in area.x0..area.x1 {
            let rx = (x - area.x0) as usize;
            let idx = (ry * w + rx) * ch;
            let sel_w = selection.map_or(1.0, |m| m.sample_channel(x, y, 0).clamp(0.0, 1.0));
            if sel_w <= 0.0 {
                continue;
            }
            for c in 0..ch {
                out[idx + c] = b_raw[idx + c] * (1.0 - sel_w) + f_raw[idx + c] * sel_w;
            }
            if lock_alpha && fmt.alpha && ch > 0 {
                out[idx + ch - 1] = b_raw[idx + ch - 1];
            }
        }
    }
    base.write_region(area, &out);
    base.prune();
}

/// Shared execution for destructive pixel layer OR non-destructive Smart Object filter.
fn run_surface_or_smart_filter(s: &mut Session, cmd_id: &str, label: &str, p: &Value, render: impl FnOnce(&Surface, Rect) -> Surface) -> Result<Value> {
    let layer_id = layer_param(s, p)?;
    let stored_params = {
        let mut q = p.clone();
        if let Value::Object(m) = &mut q {
            m.remove("layer");
            m.remove("target");
            m.remove("coalesce");
        }
        q
    };
    s.edit(label, |doc, _| {
        let locks = doc.effective_locks(layer_id);
        if locks.pixels || locks.all {
            let name = doc.layer(layer_id).map_or("Layer", |l| l.name.as_str());
            return Err(EngineError::Other(format!("Could not complete your request because the layer \"{name}\" is locked")));
        }
        let selection = doc.selection.clone();
        let canvas = doc.bounds();
        let l = doc.layer_mut(layer_id).ok_or(EngineError::NoLayer(layer_id))?;
        match &mut l.content {
            LayerContent::Raster(surf) => {
                let filtered = render(surf, canvas);
                commit_filtered_surface(surf, &filtered, canvas, selection.as_ref(), locks.transparency);
                Ok(json!({ "layer": layer_id.0 }))
            }
            LayerContent::Smart(_) => {
                let sf = SmartFilter { command: cmd_id.to_string(), params: stored_params, blend: BlendMode::Normal, opacity: 1.0, visible: true };
                crate::smart_cmds::add_smart_filter_in(doc, layer_id, sf, selection.as_ref(), &JobCtx::new())?;
                Ok(json!({ "layer": layer_id.0, "smartFilter": true }))
            }
            other => Err(EngineError::Other(format!("filters need a pixel layer (active layer is {} {} layer)", other.article(), other.kind_name()))),
        }
    })
}

// ------------------------------------------------------------------ Live Backdrop FX Layers

fn create_live_fx_layer(s: &mut Session, cmd: &str, label: &str, default_base: &str, fx: LiveBackdropFx, p: &Value) -> Result<Value> {
    let opacity = f32_or(p, "opacity", 1.0).clamp(0.0, 1.0);
    let custom_name = p.get("name").and_then(Value::as_str).map(ToString::to_string);
    let clip = bool_or(p, "clipToPrevious", false);
    let id = s.edit(label, |doc, active| {
        let name = custom_name.unwrap_or_else(|| doc.next_layer_name(default_base));
        let mut l = Layer::new(name, LayerContent::Raster(Surface::new(doc.pixel_format())));
        l.opacity = opacity;
        l.live_fx = Some(fx);
        mask_new_layer(doc, &mut l, p, cmd, true)?;
        let id = crate::layer_place::insert_new(doc, *active, l, crate::layer_place::Place::Auto);
        crate::clip_group::settle(doc, &[id], &Default::default(), clip);
        *active = Some(id);
        Ok(id)
    })?;
    Ok(json!({ "layer": id.0 }))
}

fn update_live_fx(fx: &mut LiveBackdropFx, p: &Value) {
    match fx {
        LiveBackdropFx::Fireflies { quantity, depth_variation, blurriness, color, progress, seed } => {
            if p.get("quantity").is_some() {
                *quantity = u32_or(p, "quantity", *quantity).clamp(1, 300);
            }
            if p.get("depthVariation").is_some() {
                *depth_variation = f32_or(p, "depthVariation", *depth_variation).clamp(0.0, 100.0);
            }
            if p.get("blurriness").is_some() {
                *blurriness = f32_or(p, "blurriness", *blurriness).clamp(0.0, 100.0);
            }
            if p.get("color").is_some() {
                *color = rgb_or(p, "color", *color);
            }
            if p.get("progress").is_some() {
                *progress = f32_or(p, "progress", *progress).clamp(0.0, 100.0);
            }
            if p.get("seed").is_some() {
                *seed = u32_or(p, "seed", *seed);
            }
        }
        LiveBackdropFx::WaterRipple { horizon, amplitude, frequency, progress } => {
            if p.get("horizon").is_some() {
                *horizon = f32_or(p, "horizon", *horizon).clamp(0.05, 0.95);
            }
            if p.get("amplitude").is_some() {
                *amplitude = f32_or(p, "amplitude", *amplitude).clamp(0.0, 100.0);
            }
            if p.get("frequency").is_some() {
                *frequency = f32_or(p, "frequency", *frequency).clamp(1.0, 100.0);
            }
            if p.get("progress").is_some() {
                *progress = f32_or(p, "progress", *progress).clamp(0.0, 100.0);
            }
        }
        LiveBackdropFx::Relight3d { ambient, surface_relief, lamps } => {
            if p.get("ambient").is_some() {
                *ambient = f32_or(p, "ambient", *ambient).clamp(0.0, 1.5);
            }
            if p.get("surfaceRelief").is_some() {
                *surface_relief = f32_or(p, "surfaceRelief", *surface_relief).clamp(0.0, 2.0);
            }
            if p.get("lamps").is_some() || p.get("lights").is_some() {
                *lamps = parse_relight_lamps(p, *lamps);
            }
        }
        LiveBackdropFx::Restyle { preset, prompt, intensity } => {
            if p.get("preset").is_some() || p.get("style").is_some() {
                *preset = parse_restyle_preset(p, *preset);
            }
            if let Some(pr) = p.get("prompt").and_then(Value::as_str) {
                *prompt = pr.to_string();
            }
            if p.get("intensity").is_some() {
                *intensity = f32_or(p, "intensity", *intensity).clamp(0.0, 100.0);
            }
        }
    }
}

fn has_live_fx_layer(s: &Session) -> std::result::Result<(), String> {
    let l = crate::active_layer_of(s)?;
    if l.live_fx.is_some() { Ok(()) } else { Err("active layer is not a Live Effect (Fireflies / Water Ripple / Relight / Restyle) layer".into()) }
}

// ------------------------------------------------------------------ Oil Brush Tool

fn run_oil_brush(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "paint.oilBrush";
    let pts = crate::brush_cmds::parse_points(p, CMD)?;
    let layer_id = if p.get("layer").is_some() { Some(layer_param(s, p)?) } else { s.active().and_then(|st| st.active_layer) };
    let mut params = s.tools.oil_brush.clone();
    if let Some(pr) = p.get("preset").and_then(Value::as_str) {
        params.preset = OilBrushPreset::from_id(pr).ok_or_else(|| bad(CMD, format!("unknown oil brush preset `{pr}` (blocky|glaze|round)")))?;
    }
    if p.get("size").is_some() {
        params.size = f32_or(p, "size", params.size).clamp(1.0, 500.0);
    }
    if p.get("paintMix").is_some() {
        let raw_mix = f32_or(p, "paintMix", params.paint_mix);
        params.paint_mix = if raw_mix > 1.0 { (raw_mix / 100.0).clamp(0.0, 1.0) } else { raw_mix.clamp(0.0, 1.0) };
    }
    if p.get("opacity").is_some() {
        let raw_op = f32_or(p, "opacity", params.opacity);
        params.opacity = if raw_op > 1.0 { (raw_op / 100.0).clamp(0.0, 1.0) } else { raw_op.clamp(0.0, 1.0) };
    }
    params.color = rgba_or(p, "color", s.tools.foreground);

    let damaged = s.edit("Oil Brush", |doc, _| {
        let bounds = doc.bounds();
        let sel = doc.selection.clone();
        let (surf, lock_alpha) = crate::channel_cmds::target_surface(doc, layer_id, p)?;
        let clip = bounds.union(&surf.tile_bounds());
        let rect = paint_oil_stroke(surf, &pts, &params, clip, sel.as_ref(), lock_alpha);
        surf.prune();
        Ok(rect)
    })?;
    if !damaged.is_empty() {
        s.hint_damage(damaged);
    }
    Ok(json!({
        "preset": params.preset.id(),
        "bounds": [damaged.x0, damaged.y0, damaged.width(), damaged.height()],
    }))
}

fn set_oil_brush(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "tools.setOilBrush";
    if let Some(pr) = p.get("preset").and_then(Value::as_str) {
        s.tools.oil_brush.preset = OilBrushPreset::from_id(pr).ok_or_else(|| bad(CMD, format!("unknown oil brush preset `{pr}`")))?;
    }
    if p.get("size").is_some() {
        s.tools.oil_brush.size = f32_or(p, "size", s.tools.oil_brush.size).clamp(1.0, 500.0);
    }
    if p.get("paintMix").is_some() {
        let v = f32_or(p, "paintMix", s.tools.oil_brush.paint_mix);
        s.tools.oil_brush.paint_mix = if v > 1.0 { (v / 100.0).clamp(0.0, 1.0) } else { v.clamp(0.0, 1.0) };
    }
    if p.get("opacity").is_some() {
        let v = f32_or(p, "opacity", s.tools.oil_brush.opacity);
        s.tools.oil_brush.opacity = if v > 1.0 { (v / 100.0).clamp(0.0, 1.0) } else { v.clamp(0.0, 1.0) };
    }
    Ok(json!({
        "preset": s.tools.oil_brush.preset.id(),
        "size": s.tools.oil_brush.size,
        "paintMix": s.tools.oil_brush.paint_mix,
        "opacity": s.tools.oil_brush.opacity,
    }))
}

// ------------------------------------------------------------------ Remove Ambient Effects

fn run_remove_ambient(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "edit.removeAmbientEffects";
    let mode_str = p.get("mode").and_then(Value::as_str).unwrap_or("shadows");
    let mode =
        AmbientMode::from_id(mode_str).ok_or_else(|| bad(CMD, format!("unknown ambient mode `{mode_str}` (shadows|reflections|lensFlares|atmospheric)")))?;
    let params = AmbientRemoveParams {
        mode,
        remove_related_elements: bool_or(p, "removeRelatedElements", bool_or(p, "removeRelated", true)),
        enhance: bool_or(p, "enhance", true),
        strength: f32_or(p, "strength", 80.0).clamp(0.0, 100.0),
    };
    let output_new_layer = bool_or(p, "outputToNewLayer", false);
    if output_new_layer {
        let src_id = layer_param(s, p)?;
        let new_id = s.edit("Remove Ambient Effects", |doc, active| {
            let canvas = doc.bounds();
            let selection = doc.selection.clone();
            let src_surf = doc.layer(src_id).and_then(Layer::surface).ok_or_else(|| EngineError::Other("active layer has no raster pixels".into()))?.clone();
            let filtered = remove_ambient_effects_surface(&src_surf, canvas, &params);
            let mut out_surf = src_surf;
            commit_filtered_surface(&mut out_surf, &filtered, canvas, selection.as_ref(), false);
            let base_name = format!("{} (Clean)", mode.label());
            let l = Layer::new(doc.next_layer_name(&base_name), LayerContent::Raster(out_surf));
            let id = crate::layer_place::insert_new(doc, *active, l, crate::layer_place::Place::Auto);
            *active = Some(id);
            Ok(id)
        })?;
        Ok(json!({ "layer": new_id.0, "mode": mode.id() }))
    } else {
        run_surface_or_smart_filter(s, CMD, "Remove Ambient Effects", p, |surf, canvas| remove_ambient_effects_surface(surf, canvas, &params))
    }
}

// ------------------------------------------------------------------ Type 2027 Commands

fn has_raster_layer(s: &Session) -> std::result::Result<(), String> {
    let l = crate::active_layer_of(s)?;
    if matches!(l.content, LayerContent::Raster(_)) { Ok(()) } else { Err("Convert to Text Layers requires a raster pixel layer".into()) }
}

fn has_type_layer(s: &Session) -> std::result::Result<(), String> {
    let l = crate::active_layer_of(s)?;
    if matches!(l.content, LayerContent::Text(_)) { Ok(()) } else { Err("active layer is not a type layer".into()) }
}

fn has_dynamic_fit_type_layer(s: &Session) -> std::result::Result<(), String> {
    let l = crate::active_layer_of(s)?;
    match &l.content {
        LayerContent::Text(t) if t.dynamic_fit => Ok(()),
        LayerContent::Text(_) => Err("active type layer does not have Dynamic Fit enabled".into()),
        _ => Err("active layer is not a type layer".into()),
    }
}

fn run_raster_to_text(s: &mut Session, p: &Value) -> Result<Value> {
    let src_id = layer_param(s, p)?;
    let keep_original = bool_or(p, "keepOriginal", false);
    let created = s.edit("Convert to Text Layers", |doc, active| {
        crate::commands::check_pixels_unlocked(doc, src_id)?;
        let (dpi, fmt) = (doc.resolution_dpi, doc.pixel_format());
        let search_rect = doc.selection.as_ref().map(Surface::content_bounds).filter(|r| !r.is_empty()).unwrap_or_else(|| doc.bounds());
        let src_surf = doc
            .layer(src_id)
            .and_then(|l| match &l.content {
                LayerContent::Raster(s) => Some(s.clone()),
                _ => None,
            })
            .ok_or_else(|| EngineError::Other("Convert to Text Layers requires a raster layer".into()))?;

        let mut res = detect_and_clean_raster_text(&src_surf, search_rect, dpi);
        if res.lines.is_empty() {
            // Fallback when layer has low-contrast artistic lettering: create one editable box over non-empty content.
            let cb = src_surf.content_bounds().intersect(&search_rect);
            if cb.is_empty() {
                return Err(EngineError::Other("no text or visible glyphs detected on the active raster layer".into()));
            }
            res.lines.push(photocraft_algo::raster_to_text::DetectedTextLine {
                bounds: cb,
                text: "Text".into(),
                size_pt: ((cb.height() as f32 * 0.72) * 72.0 / dpi.max(1.0)).clamp(12.0, 144.0),
                color: [0.1, 0.1, 0.1, 1.0],
                bold: false,
            });
        }

        if !keep_original
            && let Some(l) = doc.layer_mut(src_id)
            && let LayerContent::Raster(surf) = &mut l.content
        {
            *surf = res.cleaned_surface;
        }

        let mut ids = Vec::with_capacity(res.lines.len());
        let mut cursor = Some(src_id);
        for line in &res.lines {
            let mut style = CharStyle {
                font_family: "Inter".into(),
                font_style: if line.bold { "Bold".into() } else { "Regular".into() },
                size_pt: line.size_pt,
                color: Color::rgba(line.color[0], line.color[1], line.color[2], line.color[3]),
                faux_bold: line.bold,
                ..CharStyle::default()
            };
            style.size_pt = style.size_pt.clamp(6.0, 512.0);
            let mut tl = TextLayer {
                text: line.text.clone(),
                font_family: style.font_family.clone(),
                size_pt: style.size_pt,
                color: style.color,
                transform: Affine::translate(f64::from(line.bounds.x0), f64::from(line.bounds.y0) + f64::from(line.bounds.height()) * 0.82),
                runs: vec![TextRun { len: line.text.len(), style }],
                ..TextLayer::default()
            };
            crate::type_cmds::refresh_at(dpi, fmt, &mut tl);
            let layer_name = tl.text.lines().next().unwrap_or("Text").chars().take(32).collect::<String>();
            let text_layer = Layer::new(if layer_name.is_empty() { "Text".into() } else { layer_name }, LayerContent::Text(tl));
            let new_id = crate::layer_place::insert_new(doc, cursor, text_layer, crate::layer_place::Place::Auto);
            cursor = Some(new_id);
            ids.push(new_id);
        }
        if let Some(&last) = ids.last() {
            *active = Some(last);
        }
        Ok(ids)
    })?;
    Ok(json!({
        "layers": created.iter().map(|id| id.0).collect::<Vec<_>>(),
        "count": created.len(),
    }))
}

fn run_set_dynamic_fit(s: &mut Session, p: &Value) -> Result<Value> {
    let id = layer_param(s, p)?;
    let enabled_opt = p.get("enabled").and_then(Value::as_bool);
    let enabled = s.edit("Dynamic Text Fit", |doc, _| {
        crate::type_cmds::check_text_unlocked(doc, id)?;
        let (dpi, fmt) = (doc.resolution_dpi, doc.pixel_format());
        let doc_w = doc.size.width as f32;
        let doc_h = doc.size.height as f32;
        let l = doc.layer_mut(id).ok_or(EngineError::NoLayer(id))?;
        let LayerContent::Text(t) = &mut l.content else {
            return Err(EngineError::Other("active layer is not a type layer".into()));
        };
        let next = enabled_opt.unwrap_or(!t.dynamic_fit);
        if next && matches!(t.shape, TextShape::Point) {
            // Promote point text to a paragraph box so Dynamic Fit has an active frame!
            let bx = t.transform.m[4].clamp(0.0, f64::from((doc_w - 16.0).max(0.0)));
            let by = (t.transform.m[5] - f64::from(t.size_pt)).clamp(0.0, f64::from((doc_h - 16.0).max(0.0)));
            let bw = f64::from((doc_w * 0.65).max(80.0));
            let bh = f64::from((t.size_pt * 2.4).max(40.0));
            crate::type_cmds::apply_box(t, [bx, by, bw, bh]);
        }
        t.dynamic_fit = next;
        crate::type_cmds::refresh_at(dpi, fmt, t);
        Ok(next)
    })?;
    Ok(json!({ "layer": id.0, "dynamicFit": enabled }))
}

fn run_release_dynamic_fit(s: &mut Session, p: &Value) -> Result<Value> {
    let id = layer_param(s, p)?;
    let new_size = s.edit("Release Dynamic Fit", |doc, _| {
        crate::type_cmds::check_text_unlocked(doc, id)?;
        let (dpi, fmt) = (doc.resolution_dpi, doc.pixel_format());
        let l = doc.layer_mut(id).ok_or(EngineError::NoLayer(id))?;
        let LayerContent::Text(t) = &mut l.content else {
            return Err(EngineError::Other("active layer is not a type layer".into()));
        };
        if !t.dynamic_fit {
            return Err(EngineError::Other("active type layer does not have Dynamic Fit enabled".into()));
        }
        {
            let mut eng = photocraft_text::shared().lock().unwrap_or_else(|e| e.into_inner());
            eng.release_dynamic_fit(t, dpi);
        }
        crate::type_cmds::refresh_at(dpi, fmt, t);
        Ok(t.size_pt)
    })?;
    Ok(json!({ "layer": id.0, "dynamicFit": false, "sizePt": new_size }))
}

fn run_copy_text_attributes(s: &mut Session, p: &Value) -> Result<Value> {
    let id = layer_param(s, p)?;
    let st = s.active().ok_or(EngineError::NoDocument)?;
    let l = st.doc.layer(id).ok_or(EngineError::NoLayer(id))?;
    let LayerContent::Text(t) = &l.content else {
        return Err(EngineError::Other("active layer is not a type layer".into()));
    };
    let char_style = t.char_runs().into_iter().next().map(|r| r.style).unwrap_or_default();
    let paragraph_style = t.paragraphs.first().map(|p| p.style.clone()).unwrap_or_default();
    let clip = TextAttributeClipboard { char_style: char_style.clone(), paragraph_style, antialias: t.antialias, orientation: t.orientation };
    s.text_attr_clipboard = Some(clip);
    Ok(json!({
        "layer": id.0,
        "fontFamily": char_style.font_family,
        "sizePt": char_style.size_pt,
    }))
}

fn can_paste_text_attributes(s: &Session) -> std::result::Result<(), String> {
    if s.text_attr_clipboard.is_none() {
        return Err("no text attributes copied".into());
    }
    let st = s.active().ok_or("no document open")?;
    let has_text = st.selected_layers().into_iter().any(|id| st.doc.layer(id).is_some_and(|l| matches!(l.content, LayerContent::Text(_))));
    if has_text { Ok(()) } else { Err("no type layer selected".into()) }
}

fn run_paste_text_attributes(s: &mut Session, p: &Value) -> Result<Value> {
    let clip = s.text_attr_clipboard.clone().ok_or_else(|| EngineError::Other("no text attributes copied".into()))?;
    let targets: Vec<LayerId> = if p.get("layer").is_some() {
        vec![layer_param(s, p)?]
    } else {
        let st = s.active().ok_or(EngineError::NoDocument)?;
        st.selected_layers().into_iter().filter(|&id| st.doc.layer(id).is_some_and(|l| matches!(l.content, LayerContent::Text(_)))).collect()
    };
    if targets.is_empty() {
        return Err(EngineError::Other("no type layer selected".into()));
    }
    s.edit("Paste Text Attributes", |doc, _| {
        let (dpi, fmt) = (doc.resolution_dpi, doc.pixel_format());
        for &id in &targets {
            crate::type_cmds::check_text_unlocked(doc, id)?;
            if let Some(l) = doc.layer_mut(id)
                && let LayerContent::Text(t) = &mut l.content
            {
                let len = t.text.len();
                t.runs = vec![TextRun { len, style: clip.char_style.clone() }];
                t.paragraphs = vec![ParagraphRun { len, style: clip.paragraph_style.clone() }];
                t.antialias = clip.antialias;
                t.orientation = clip.orientation;
                t.sync_summary();
                crate::type_cmds::refresh_at(dpi, fmt, t);
            }
        }
        Ok(())
    })?;
    Ok(json!({ "layers": targets.iter().map(|id| id.0).collect::<Vec<_>>() }))
}

// ------------------------------------------------------------------ Generative Recompose

fn resolve_recompose_dimensions(cur: Size, p: &Value) -> (u32, u32, String) {
    let aspect = p.get("aspectRatio").and_then(Value::as_str).unwrap_or("custom").trim().to_string();
    let explicit_w = p.get("width").or_else(|| p.get("targetWidth")).and_then(Value::as_u64).map(|v| v.clamp(16, 16384) as u32);
    let explicit_h = p.get("height").or_else(|| p.get("targetHeight")).and_then(Value::as_u64).map(|v| v.clamp(16, 16384) as u32);
    if let (Some(w), Some(h)) = (explicit_w, explicit_h) {
        return (w, h, aspect);
    }
    let ratio_pair: Option<(f64, f64)> = match aspect.as_str() {
        "1:1" => Some((1.0, 1.0)),
        "16:9" => Some((16.0, 9.0)),
        "9:16" => Some((9.0, 16.0)),
        "4:5" => Some((4.0, 5.0)),
        "5:4" => Some((5.0, 4.0)),
        "4:3" => Some((4.0, 3.0)),
        "3:4" => Some((3.0, 4.0)),
        "3:2" => Some((3.0, 2.0)),
        "2:3" => Some((2.0, 3.0)),
        "21:9" => Some((21.0, 9.0)),
        _ => None,
    };
    if let Some((rw, rh)) = ratio_pair {
        let base_long = cur.width.max(cur.height).max(64) as f64;
        let (tw, th) = if rw >= rh {
            (base_long.round() as u32, (base_long * rh / rw).round().max(16.0) as u32)
        } else {
            ((base_long * rw / rh).round().max(16.0) as u32, base_long.round() as u32)
        };
        return (explicit_w.unwrap_or(tw), explicit_h.unwrap_or(th), aspect);
    }
    (explicit_w.unwrap_or(cur.width.max(16)), explicit_h.unwrap_or(cur.height.max(16)), aspect)
}

fn recompose_layers_recursive(layers: &mut [Layer], old_size: Size, params: &RecomposeParams, dpi: f32, fmt: PixelFormat) {
    let sx = params.target_width as f64 / old_size.width.max(1) as f64;
    let sy = params.target_height as f64 / old_size.height.max(1) as f64;
    for l in layers {
        match &mut l.content {
            LayerContent::Raster(surf) if l.live_fx.is_none() && !surf.tile_bounds().is_empty() => {
                let (new_surf, _) = generative_recompose_surface(surf, old_size, params);
                *surf = new_surf;
            }
            LayerContent::Text(t) => {
                t.transform.m[4] *= sx;
                t.transform.m[5] *= sy;
                crate::type_cmds::refresh_at(dpi, fmt, t);
            }
            LayerContent::Group(g) => {
                if let Some(ab) = &mut g.artboard {
                    ab.rect = Rect::new(
                        (ab.rect.x0 as f64 * sx).round() as i32,
                        (ab.rect.y0 as f64 * sy).round() as i32,
                        (ab.rect.x1 as f64 * sx).round() as i32,
                        (ab.rect.y1 as f64 * sy).round() as i32,
                    );
                }
                recompose_layers_recursive(&mut g.children, old_size, params, dpi, fmt);
            }
            _ => {}
        }
        if let Some(m) = &mut l.mask
            && !m.surface.tile_bounds().is_empty()
        {
            m.surface = resize_surface(&m.surface, sx, sy, Resample::Bilinear);
        }
    }
}

fn run_generative_recompose(s: &mut Session, p: &Value) -> Result<Value> {
    let prompt = p.get("prompt").and_then(Value::as_str).unwrap_or("").to_string();
    let (tw, th, aspect) = {
        let st = s.active().ok_or(EngineError::NoDocument)?;
        resolve_recompose_dimensions(st.doc.size, p)
    };
    // Guard memory for reasonable canvas sizes in seam carving + outpaint.
    if u64::from(tw) * u64::from(th) > 64_000_000 {
        return Err(bad("edit.generativeRecompose", "target dimensions exceed Generative Recompose 64 MP working limit"));
    }
    let params = RecomposeParams { target_width: tw, target_height: th, aspect_ratio: aspect.clone(), prompt };
    s.edit("Generative Recompose", |doc, _| {
        let old = doc.size;
        let (dpi, fmt) = (doc.resolution_dpi, doc.pixel_format());
        doc.size = Size::new(tw, th);
        doc.selection = None;
        recompose_layers_recursive(&mut doc.layers, old, &params, dpi, fmt);
        Ok(())
    })?;
    Ok(json!({ "width": tw, "height": th, "aspectRatio": aspect }))
}

// ------------------------------------------------------------------ Capture Live FX as Layer

fn run_capture_as_layer(s: &mut Session, p: &Value) -> Result<Value> {
    let target_id = layer_param(s, p)?;
    let new_id = s.edit("Capture as Layer", |doc, active| {
        let fmt = doc.pixel_format();
        let canvas = doc.bounds();
        // Composite a temporary clone where layers strictly above `target_id` are hidden so we capture
        // the exact styled appearance up to and including `target_id`!
        let mut temp = doc.clone();
        hide_layers_above(&mut temp.layers, target_id, &mut false);
        let buf = photocraft_compose::render(&temp, canvas);
        let ch = fmt.channels();
        let mut raw = vec![0.0f32; (canvas.width() as usize) * (canvas.height() as usize) * ch];
        for (px, dst) in buf.px.iter().zip(raw.chunks_exact_mut(ch)) {
            from_rgba_into(&fmt, *px, dst);
        }
        let mut surf = Surface::new(fmt);
        surf.write_region(canvas, &raw);
        let base_name = doc.layer(target_id).map_or("Captured Style".to_string(), |l| format!("{} Capture", l.name));
        let layer = Layer::new(doc.next_layer_name(&base_name), LayerContent::Raster(surf));
        let id = crate::layer_place::insert_new(doc, Some(target_id), layer, crate::layer_place::Place::Auto);
        *active = Some(id);
        Ok(id)
    })?;
    Ok(json!({ "layer": new_id.0 }))
}

fn hide_layers_above(layers: &mut [Layer], target: LayerId, seen: &mut bool) {
    for l in layers {
        if *seen {
            l.visible = false;
        }
        if l.id == target {
            *seen = true;
            continue;
        }
        if let LayerContent::Group(g) = &mut l.content {
            hide_layers_above(&mut g.children, target, seen);
        }
    }
}

// ------------------------------------------------------------------ Mockup Group & Environmental Lighting

fn find_mockup_target(doc: &Document, active: LayerId) -> Option<LayerId> {
    if doc.layer(active).is_some_and(|l| l.mockup_lighting.is_some()) {
        return Some(active);
    }
    let path = doc.path_of(active)?;
    if path.len() >= 2 { doc.layer_at(&path[..path.len() - 1]).filter(|l| l.mockup_lighting.is_some()).map(|l| l.id) } else { None }
}

fn has_mockup_target(s: &Session) -> std::result::Result<(), String> {
    let st = s.active().ok_or("no document open")?;
    let active = st.active_layer.ok_or("no active layer")?;
    if find_mockup_target(&st.doc, active).is_some() { Ok(()) } else { Err("active layer is not a Mockup".into()) }
}

fn run_create_mockup(s: &mut Session, p: &Value) -> Result<Value> {
    let raw_lighting = f32_or(p, "lighting", f32_or(p, "environmentalLighting", 0.80));
    let lighting = if raw_lighting > 2.0 { (raw_lighting / 100.0).clamp(0.0, 2.0) } else { raw_lighting.clamp(0.0, 2.0) };
    let selected = {
        let st = s.active().ok_or(EngineError::NoDocument)?;
        st.selected_layers()
    };
    if selected.is_empty() {
        return Err(EngineError::Other("no layer selected for Mockup".into()));
    }
    let group_id = s.edit("Create Mockup", |doc, active| {
        // Extract selected decal layer(s) in stack order and wrap them in a Mockup Group!
        let mut children = Vec::new();
        let mut insert_anchor = None;
        for &id in &selected {
            if let Some(prev) = neighbor_below(doc, id, &selected) {
                insert_anchor = Some(prev);
            }
            if let Some(mut extracted) = remove_layer_by_id(&mut doc.layers, id) {
                extracted.locks.transparency = false;
                extracted.locks.position = false;
                children.push(extracted);
            }
        }
        if children.is_empty() {
            return Err(EngineError::Other("could not group selected layers into Mockup".into()));
        }
        let mut grp_layer =
            Layer::new(doc.next_layer_name("Mockup"), LayerContent::Group(Group { children, expanded: true, artboard: None, frame_stroke: None }));
        grp_layer.blend = BlendMode::Normal;
        grp_layer.mockup_lighting = Some(lighting);
        let gid = if let Some(anchor) = insert_anchor {
            crate::layer_place::insert_new(doc, Some(anchor), grp_layer, crate::layer_place::Place::Auto)
        } else {
            let id = grp_layer.id;
            doc.layers.push(grp_layer);
            id
        };
        *active = Some(gid);
        Ok(gid)
    })?;
    Ok(json!({ "layer": group_id.0, "lighting": lighting }))
}

fn neighbor_below(doc: &Document, target: LayerId, ignore: &[LayerId]) -> Option<LayerId> {
    let flat: Vec<LayerId> = doc.walk().into_iter().map(|(_, _, l)| l.id).collect();
    let pos = flat.iter().position(|&id| id == target)?;
    flat[..pos].iter().rev().copied().find(|id| !ignore.contains(id))
}

fn remove_layer_by_id(layers: &mut Vec<Layer>, target: LayerId) -> Option<Layer> {
    if let Some(pos) = layers.iter().position(|l| l.id == target) {
        return Some(layers.remove(pos));
    }
    for l in layers {
        if let LayerContent::Group(g) = &mut l.content
            && let Some(found) = remove_layer_by_id(&mut g.children, target)
        {
            return Some(found);
        }
    }
    None
}

fn run_set_mockup_lighting(s: &mut Session, p: &Value) -> Result<Value> {
    let raw_lighting = f32_or(p, "lighting", f32_or(p, "environmentalLighting", 0.80));
    let lighting = if raw_lighting > 2.0 { (raw_lighting / 100.0).clamp(0.0, 2.0) } else { raw_lighting.clamp(0.0, 2.0) };
    let target = {
        let st = s.active().ok_or(EngineError::NoDocument)?;
        let req = if p.get("layer").is_some() { layer_param(s, p)? } else { st.active_layer.ok_or(EngineError::Other("no active layer".into()))? };
        find_mockup_target(&st.doc, req).ok_or_else(|| EngineError::Other("target layer is not a Mockup".into()))?
    };
    s.edit("Mockup Environmental Lighting", |doc, _| {
        let l = doc.layer_mut(target).ok_or(EngineError::NoLayer(target))?;
        l.mockup_lighting = Some(lighting);
        Ok(())
    })?;
    Ok(json!({ "layer": target.0, "lighting": lighting }))
}

fn run_release_mockup(s: &mut Session, p: &Value) -> Result<Value> {
    let target = {
        let st = s.active().ok_or(EngineError::NoDocument)?;
        let req = if p.get("layer").is_some() { layer_param(s, p)? } else { st.active_layer.ok_or(EngineError::Other("no active layer".into()))? };
        find_mockup_target(&st.doc, req).ok_or_else(|| EngineError::Other("target layer is not a Mockup".into()))?
    };
    let released = s.edit("Release Mockup", |doc, active| {
        let out = ungroup_mockup_in(&mut doc.layers, target);
        if let Some(&top_id) = out.last() {
            *active = Some(top_id);
        }
        Ok(out)
    })?;
    Ok(json!({ "released": released.iter().map(|id| id.0).collect::<Vec<_>>() }))
}

fn ungroup_mockup_in(layers: &mut Vec<Layer>, target: LayerId) -> Vec<LayerId> {
    if let Some(pos) = layers.iter().position(|l| l.id == target) {
        layers[pos].mockup_lighting = None;
        if matches!(layers[pos].content, LayerContent::Group(_)) {
            let grp = layers.remove(pos);
            if let LayerContent::Group(g) = grp.content {
                let ids: Vec<LayerId> = g.children.iter().map(|c| c.id).collect();
                for (i, child) in g.children.into_iter().enumerate() {
                    layers.insert(pos + i, child);
                }
                return ids;
            }
        }
        return vec![target];
    }
    for l in layers {
        if let LayerContent::Group(g) = &mut l.content {
            let res = ungroup_mockup_in(&mut g.children, target);
            if !res.is_empty() {
                return res;
            }
        }
    }
    Vec::new()
}

// ------------------------------------------------------------------ Agent Skills (8 One-Click Workflows)

/// Computes Photoshop's Select Subject mask over `doc`, falling back to a soft center-subject mask
/// if the canvas has no distinct foreground boundary (e.g. a blank flat test document).
fn compute_subject_mask_surface(doc: &Document) -> Surface {
    let bounds = doc.bounds();
    if let Some(region) = crate::smartselect_cmds::with_doc_sampler(doc, None, true, |smp, _| subject::select_subject(smp, bounds)) {
        return matting::region_surface(&region);
    }
    let w = bounds.width().max(1);
    let h = bounds.height().max(1);
    let cx = w as f32 * 0.5;
    let cy = h as f32 * 0.5;
    let rx = (w as f32 * 0.34).max(1.0);
    let ry = (h as f32 * 0.36).max(1.0);
    let mut vals = Vec::with_capacity((w as usize) * (h as usize));
    for y in 0..h {
        for x in 0..w {
            let dx = (x as f32 + 0.5 - cx) / rx;
            let dy = (y as f32 + 0.5 - cy) / ry;
            let d = (dx * dx + dy * dy).sqrt();
            let v = ((1.15 - d) * 3.0).clamp(0.0, 1.0);
            vals.push(v);
        }
    }
    let mut mask_surf = Surface::new(PixelFormat::GRAY8);
    mask_surf.write_region(bounds, &vals);
    mask_surf
}

fn stamp_visible_surface(doc: &Document) -> Surface {
    let fmt = doc.pixel_format();
    let bounds = doc.bounds();
    let buf = photocraft_compose::render(doc, bounds);
    let ch = fmt.channels();
    let mut raw = vec![0.0f32; buf.px.len() * ch];
    for (p, dst) in buf.px.iter().zip(raw.chunks_exact_mut(ch)) {
        from_rgba_into(&fmt, *p, dst);
    }
    let mut surf = Surface::new(fmt);
    surf.write_region(bounds, &raw);
    surf
}

fn pt(input: f32, output: f32) -> CurvePoint {
    CurvePoint { input, output }
}

fn run_skill_cinematic_grade(s: &mut Session, _: &Value) -> Result<Value> {
    let id = s.edit("Agent Skill: Cinematic Grade", |doc, active| {
        let cb = Layer::new(
            "Teal & Orange Balance",
            LayerContent::Adjustment(Adjustment::ColorBalance {
                shadows: [-14.0, -3.0, 12.0],
                midtones: [8.0, 1.0, -8.0],
                highlights: [14.0, 4.0, -12.0],
                preserve_luminosity: true,
            }),
        );
        let curves = Layer::new(
            "Cinematic Contrast Curve",
            LayerContent::Adjustment(Adjustment::Curves {
                master: vec![pt(0.0, 0.03), pt(0.24, 0.19), pt(0.76, 0.83), pt(1.0, 0.98)],
                per_channel: [vec![pt(0.0, 0.0), pt(1.0, 1.0)], vec![pt(0.0, 0.0), pt(1.0, 1.0)], vec![pt(0.0, 0.04), pt(0.5, 0.5), pt(1.0, 0.95)]],
                space: ToneSpace::Rgb,
                black: Vec::new(),
                pencil: false,
            }),
        );
        let vib = Layer::new("Film Vibrance", LayerContent::Adjustment(Adjustment::Vibrance { vibrance: 18.0, saturation: -6.0 }));
        let grp = Layer::new(
            doc.next_layer_name("Cinematic Grade"),
            LayerContent::Group(Group { children: vec![cb, curves, vib], expanded: true, artboard: None, frame_stroke: None }),
        );
        let gid = crate::layer_place::insert_new(doc, *active, grp, crate::layer_place::Place::Auto);
        *active = Some(gid);
        Ok(gid)
    })?;
    Ok(json!({ "skill": "cinematicGrade", "layer": id.0 }))
}

fn run_skill_enhance_and_resize(s: &mut Session, p: &Value) -> Result<Value> {
    let scale = f32_or(p, "scale", 2.0).clamp(1.25, 4.0);
    let (nw, nh) = s.edit("Agent Skill: Enhance and Resize", |doc, active| {
        let old = doc.size;
        let nw = ((old.width as f32 * scale).round() as u32).clamp(16, 8192);
        let nh = ((old.height as f32 * scale).round() as u32).clamp(16, 8192);
        let (dpi, fmt) = (doc.resolution_dpi, doc.pixel_format());
        doc.size = Size::new(nw, nh);
        doc.selection = None;
        let new_bounds = Rect::from_size(doc.size);
        let sharpen_fp = FilterParams::SmartSharpen { amount: 85.0, radius: 1.1, reduce_noise: 18.0 };
        upscale_and_sharpen_layers(&mut doc.layers, old, &sharpen_fp, new_bounds, dpi, fmt);
        let vib = Layer::new(doc.next_layer_name("Detail Vibrance"), LayerContent::Adjustment(Adjustment::Vibrance { vibrance: 12.0, saturation: 3.0 }));
        let vid = crate::layer_place::insert_new(doc, *active, vib, crate::layer_place::Place::Auto);
        *active = Some(vid);
        Ok((nw, nh))
    })?;
    Ok(json!({ "skill": "enhanceAndResize", "width": nw, "height": nh }))
}

fn upscale_and_sharpen_layers(layers: &mut [Layer], old: Size, fp: &FilterParams, new_bounds: Rect, dpi: f32, fmt: PixelFormat) {
    let sx = f64::from(new_bounds.width()) / f64::from(old.width.max(1));
    let sy = f64::from(new_bounds.height()) / f64::from(old.height.max(1));
    let scale_aff = Affine { m: [sx, 0.0, 0.0, sy, 0.0, 0.0] };
    for l in layers {
        match &mut l.content {
            LayerContent::Raster(surf) if l.live_fx.is_none() && !surf.tile_bounds().is_empty() => {
                let up = resize_surface(surf, sx, sy, Resample::Lanczos);
                *surf = photocraft_algo::apply_in(&up, fp, new_bounds, new_bounds, None, new_bounds);
            }
            LayerContent::Text(t) => {
                t.transform = t.transform.then(&scale_aff);
                crate::type_cmds::refresh_at(dpi, fmt, t);
            }
            LayerContent::Group(g) => {
                upscale_and_sharpen_layers(&mut g.children, old, fp, new_bounds, dpi, fmt);
            }
            _ => {}
        }
        if let Some(m) = &mut l.mask
            && !m.surface.tile_bounds().is_empty()
        {
            m.surface = resize_surface(&m.surface, sx, sy, Resample::Bilinear);
        }
    }
}

fn run_skill_perfect_portrait(s: &mut Session, _: &Value) -> Result<Value> {
    let id = s.edit("Agent Skill: Perfect Portrait", |doc, active| {
        let bounds = doc.bounds();
        let stamped = stamp_visible_surface(doc);
        let smoothed = photocraft_algo::apply_in(&stamped, &FilterParams::SurfaceBlur { radius: 4.0, threshold: 16.0 }, bounds, bounds, None, bounds);
        let mut skin = Layer::new("Soft Skin Polish", LayerContent::Raster(smoothed));
        skin.opacity = 0.42;

        let mut relight = Layer::new("Portrait Key Light", LayerContent::Raster(Surface::new(doc.pixel_format())));
        relight.opacity = 0.55;
        let mut lamps = RelightLamp::default_rig();
        lamps[0].x = 0.38;
        lamps[0].y = 0.28;
        lamps[0].intensity = 1.25;
        lamps[0].color = [1.0, 0.93, 0.85];
        relight.live_fx = Some(LiveBackdropFx::Relight3d { ambient: 0.68, surface_relief: 0.85, lamps });

        let warm = Layer::new("Portrait Tone", LayerContent::Adjustment(Adjustment::Vibrance { vibrance: 14.0, saturation: 2.0 }));
        let grp = Layer::new(
            doc.next_layer_name("Perfect Portrait"),
            LayerContent::Group(Group { children: vec![skin, relight, warm], expanded: true, artboard: None, frame_stroke: None }),
        );
        let gid = crate::layer_place::insert_new(doc, *active, grp, crate::layer_place::Place::Auto);
        *active = Some(gid);
        Ok(gid)
    })?;
    Ok(json!({ "skill": "perfectPortrait", "layer": id.0 }))
}

fn run_skill_product_studio_shot(s: &mut Session, _: &Value) -> Result<Value> {
    let id = s.edit("Agent Skill: Product Studio Shot", |doc, active| {
        let mask_surf = compute_subject_mask_surface(doc);
        let subject_px = stamp_visible_surface(doc);
        let backdrop = Layer::new(
            "Studio Sweep Backdrop",
            LayerContent::Fill(Fill::Gradient {
                stops: vec![(0.0, Color::rgb(0.98, 0.98, 0.99)), (1.0, Color::rgb(0.84, 0.86, 0.90))],
                angle: 90.0,
                scale: 1.25,
                style: GradientStyle::Radial,
                reverse: false,
                opacity_stops: Vec::new(),
                midpoints: Vec::new(),
                offset: (0.0, -0.08),
                dither: true,
                align: true,
            }),
        );
        let mut isolated = Layer::new("Product Cutout", LayerContent::Raster(subject_px));
        isolated.mask = Some(LayerMask { surface: mask_surf, ..LayerMask::reveal_all() });
        isolated.effects.enabled = true;
        isolated.effects.items.push(Effect::DropShadow(Shadow {
            common: FxCommon::new(BlendMode::Multiply, 0.34),
            color: Color::rgb(0.08, 0.10, 0.14),
            angle: 90.0,
            use_global_light: false,
            distance: (doc.size.height as f32 * 0.035).clamp(4.0, 36.0),
            spread: 0.06,
            size: (doc.size.height as f32 * 0.06).clamp(6.0, 56.0),
            contour: Contour::Linear,
            anti_alias: true,
            noise: 0.0,
            knocks_out: true,
        }));
        let grp = Layer::new(
            doc.next_layer_name("Product Studio Shot"),
            LayerContent::Group(Group { children: vec![backdrop, isolated], expanded: true, artboard: None, frame_stroke: None }),
        );
        let gid = crate::layer_place::insert_new(doc, *active, grp, crate::layer_place::Place::Auto);
        *active = Some(gid);
        Ok(gid)
    })?;
    Ok(json!({ "skill": "productStudioShot", "layer": id.0 }))
}

fn run_skill_remove_background(s: &mut Session, p: &Value) -> Result<Value> {
    let target_id = layer_param(s, p)?;
    s.edit("Agent Skill: Remove Background", |doc, _| {
        let mask_surf = compute_subject_mask_surface(doc);
        crate::extra_cmds::background_to_layer_for_mask(doc, target_id);
        let l = doc.layer_mut(target_id).ok_or(EngineError::NoLayer(target_id))?;
        l.mask = Some(LayerMask { surface: mask_surf, ..LayerMask::reveal_all() });
        Ok(())
    })?;
    Ok(json!({ "skill": "removeBackground", "layer": target_id.0 }))
}

fn run_skill_retouch_portrait(s: &mut Session, _: &Value) -> Result<Value> {
    let id = s.edit("Agent Skill: Retouch Portrait", |doc, active| {
        let bounds = doc.bounds();
        let stamped = stamp_visible_surface(doc);
        let de_blemish = photocraft_algo::apply_in(&stamped, &FilterParams::SurfaceBlur { radius: 5.0, threshold: 18.0 }, bounds, bounds, None, bounds);
        let crisp =
            photocraft_algo::apply_in(&de_blemish, &FilterParams::UnsharpMask { amount: 48.0, radius: 0.9, threshold: 4.0 }, bounds, bounds, None, bounds);
        let mut retouch = Layer::new(doc.next_layer_name("Portrait Retouch"), LayerContent::Raster(crisp));
        retouch.opacity = 0.65;
        let rid = crate::layer_place::insert_new(doc, *active, retouch, crate::layer_place::Place::Auto);
        *active = Some(rid);
        Ok(rid)
    })?;
    Ok(json!({ "skill": "retouchPortrait", "layer": id.0 }))
}

fn run_skill_social_media_hero(s: &mut Session, p: &Value) -> Result<Value> {
    let aspect = p.get("aspectRatio").and_then(Value::as_str).unwrap_or("16:9").to_string();
    let (tw, th, _) = {
        let st = s.active().ok_or(EngineError::NoDocument)?;
        resolve_recompose_dimensions(st.doc.size, &json!({ "aspectRatio": aspect }))
    };
    let params = RecomposeParams { target_width: tw, target_height: th, aspect_ratio: aspect.clone(), prompt: "social hero banner".into() };
    s.edit("Agent Skill: Social Media Hero", |doc, active| {
        let old = doc.size;
        let (dpi, fmt) = (doc.resolution_dpi, doc.pixel_format());
        doc.size = Size::new(tw, th);
        doc.selection = None;
        recompose_layers_recursive(&mut doc.layers, old, &params, dpi, fmt);
        let polish = Layer::new(
            doc.next_layer_name("Hero Pop"),
            LayerContent::Adjustment(Adjustment::BrightnessContrast { brightness: 4.0, contrast: 16.0, legacy: false }),
        );
        let pid = crate::layer_place::insert_new(doc, *active, polish, crate::layer_place::Place::Auto);
        *active = Some(pid);
        Ok(())
    })?;
    Ok(json!({ "skill": "socialMediaHero", "width": tw, "height": th, "aspectRatio": aspect }))
}

fn run_skill_subject_spotlight(s: &mut Session, _: &Value) -> Result<Value> {
    let id = s.edit("Agent Skill: Subject Spotlight", |doc, active| {
        let bounds = doc.bounds();
        let subj_mask = compute_subject_mask_surface(doc);
        // Invert subject mask so Exposure darkens the ambient surroundings!
        let mut inv_vals = subj_mask.read_region(bounds);
        for v in &mut inv_vals {
            *v = (1.0 - *v).clamp(0.0, 1.0);
        }
        let mut bg_mask = Surface::new(PixelFormat::GRAY8);
        bg_mask.write_region(bounds, &inv_vals);

        let mut dim_bg = Layer::new("Dim Surroundings", LayerContent::Adjustment(Adjustment::Exposure { exposure: -1.15, offset: 0.0, gamma: 0.92 }));
        dim_bg.mask = Some(LayerMask { surface: bg_mask, ..LayerMask::reveal_all() });

        let mut spot = Layer::new("Subject Key Spotlight", LayerContent::Raster(Surface::new(doc.pixel_format())));
        let mut lamps = RelightLamp::default_rig();
        lamps[0].enabled = true;
        lamps[0].x = 0.50;
        lamps[0].y = 0.38;
        lamps[0].z = 0.48;
        lamps[0].intensity = 1.45;
        lamps[0].spread = 0.55;
        lamps[1].enabled = false;
        lamps[2].enabled = false;
        spot.live_fx = Some(LiveBackdropFx::Relight3d { ambient: 0.45, surface_relief: 1.05, lamps });
        spot.mask = Some(LayerMask { surface: subj_mask, ..LayerMask::reveal_all() });

        let grp = Layer::new(
            doc.next_layer_name("Subject Spotlight"),
            LayerContent::Group(Group { children: vec![dim_bg, spot], expanded: true, artboard: None, frame_stroke: None }),
        );
        let gid = crate::layer_place::insert_new(doc, *active, grp, crate::layer_place::Place::Auto);
        *active = Some(gid);
        Ok(gid)
    })?;
    Ok(json!({ "skill": "subjectSpotlight", "layer": id.0 }))
}

// ------------------------------------------------------------------ Command Registry Specs

pub fn specs() -> Vec<CommandSpec> {
    vec![
        // 1. NextGen Filters & Live Adjustments (`Fireflies` & `Water Ripple`)
        CommandSpec {
            id: "filter.nge.fireflies",
            label: "Fireflies…",
            menu: &["Filter", "NextGen"],
            shortcut: None,
            params: r##"{"layer":id?,"quantity":1..300=48,"depthVariation":0..100=65,"blurriness":0..100=45,"color":[r,g,b]|"#rrggbb","progress":0..100=25,"seed":u32=2027}"##,
            enabled: has_filterable_layer,
            run: |s, p| {
                let fp = parse_fireflies_params(p);
                run_surface_or_smart_filter(s, "filter.nge.fireflies", "Fireflies", p, |surf, canvas| apply_fireflies_surface(surf, canvas, &fp))
            },
            journal: true,
        },
        CommandSpec {
            id: "filter.nge.waterRipple",
            label: "Water Ripple…",
            menu: &["Filter", "NextGen"],
            shortcut: None,
            params: r#"{"layer":id?,"horizon":0.05..0.95=0.58,"amplitude":0..100=42,"frequency":1..100=36,"progress":0..100=20}"#,
            enabled: has_filterable_layer,
            run: |s, p| {
                let wp = parse_water_ripple_params(p);
                run_surface_or_smart_filter(s, "filter.nge.waterRipple", "Water Ripple", p, |surf, canvas| apply_water_ripple_surface(surf, canvas, &wp))
            },
            journal: true,
        },
        CommandSpec {
            id: "layer.newAdjustmentLayer.fireflies",
            label: "Fireflies…",
            menu: &["Layer", "New Adjustment Layer"],
            shortcut: None,
            params: r##"{"quantity":1..300=48,"depthVariation":0..100=65,"blurriness":0..100=45,"color":[r,g,b]|"#rrggbb","progress":0..100=25,"seed":u32=2027,"name":str?,"opacity":0..1=1,"clipToPrevious":bool=false}"##,
            enabled: has_rgb_or_gray_doc,
            run: |s, p| {
                let fp = parse_fireflies_params(p);
                let fx = LiveBackdropFx::Fireflies {
                    quantity: fp.quantity,
                    depth_variation: fp.depth_variation,
                    blurriness: fp.blurriness,
                    color: fp.color,
                    progress: fp.progress,
                    seed: fp.seed,
                };
                create_live_fx_layer(s, "layer.newAdjustmentLayer.fireflies", "New Fireflies Layer", "Fireflies", fx, p)
            },
            journal: true,
        },
        CommandSpec {
            id: "layer.newAdjustmentLayer.waterRipple",
            label: "Water Ripple…",
            menu: &["Layer", "New Adjustment Layer"],
            shortcut: None,
            params: r#"{"horizon":0.05..0.95=0.58,"amplitude":0..100=42,"frequency":1..100=36,"progress":0..100=20,"name":str?,"opacity":0..1=1,"clipToPrevious":bool=false}"#,
            enabled: has_rgb_or_gray_doc,
            run: |s, p| {
                let wp = parse_water_ripple_params(p);
                let fx = LiveBackdropFx::WaterRipple { horizon: wp.horizon, amplitude: wp.amplitude, frequency: wp.frequency, progress: wp.progress };
                create_live_fx_layer(s, "layer.newAdjustmentLayer.waterRipple", "New Water Ripple Layer", "Water Ripple", fx, p)
            },
            journal: true,
        },
        CommandSpec {
            id: "layer.liveFx.setParams",
            label: "Edit Live Effect",
            menu: &[],
            shortcut: None,
            params: r#"{"layer":id?, …parameters of the active Fireflies / WaterRipple / Relight3d / Restyle layer}"#,
            enabled: has_live_fx_layer,
            run: |s, p| {
                let id = layer_param(s, p)?;
                s.edit("Edit Live Effect", |doc, _| {
                    let l = doc.layer_mut(id).ok_or(EngineError::NoLayer(id))?;
                    let fx = l.live_fx.as_mut().ok_or_else(|| EngineError::Other("layer has no live_fx".into()))?;
                    update_live_fx(fx, p);
                    Ok(())
                })?;
                Ok(json!({ "layer": id.0 }))
            },
            journal: true,
        },
        CommandSpec {
            id: "layer.liveFx.clear",
            label: "Clear Live Effect",
            menu: &[],
            shortcut: None,
            params: r#"{"layer":id?}"#,
            enabled: has_live_fx_layer,
            run: |s, p| {
                let id = layer_param(s, p)?;
                s.edit("Clear Live Effect", |doc, _| {
                    let l = doc.layer_mut(id).ok_or(EngineError::NoLayer(id))?;
                    l.live_fx = None;
                    Ok(())
                })?;
                Ok(json!({ "layer": id.0 }))
            },
            journal: true,
        },
        // 2. Relight / Lightshop Layer & Filter (`새 재조명 레이어…` / `재조명`)
        CommandSpec {
            id: "layer.newRelightLayer",
            label: "New Relight Layer…",
            menu: &["Layer", "New"],
            shortcut: None,
            params: r#"{"ambient":0..1.5=0.55,"surfaceRelief":0..2=1,"lamps":[{"enabled":bool,"x":0..1,"y":0..1,"z":0.05..2,"intensity":0..4,"color":[r,g,b],"spread":0.05..3}],"name":str?,"opacity":0..1=1,"clipToPrevious":bool=false}"#,
            enabled: has_rgb_or_gray_doc,
            run: |s, p| {
                let lamps = parse_relight_lamps(p, RelightLamp::default_rig());
                let fx = LiveBackdropFx::Relight3d {
                    ambient: f32_or(p, "ambient", 0.55).clamp(0.0, 1.5),
                    surface_relief: f32_or(p, "surfaceRelief", 1.0).clamp(0.0, 2.0),
                    lamps,
                };
                create_live_fx_layer(s, "layer.newRelightLayer", "New Relight Layer", "Relight", fx, p)
            },
            journal: true,
        },
        CommandSpec {
            id: "layer.editRelightLayer",
            label: "Edit Relight Layer",
            menu: &[],
            shortcut: None,
            params: r#"{"layer":id?,"ambient":0..1.5?,"surfaceRelief":0..2?,"lamps":[…]?}"#,
            enabled: |s| {
                let l = crate::active_layer_of(s)?;
                if matches!(l.live_fx, Some(LiveBackdropFx::Relight3d { .. })) { Ok(()) } else { Err("active layer is not a Relight layer".into()) }
            },
            run: |s, p| {
                let id = layer_param(s, p)?;
                s.edit("Edit Relight Layer", |doc, _| {
                    let l = doc.layer_mut(id).ok_or(EngineError::NoLayer(id))?;
                    let Some(fx @ LiveBackdropFx::Relight3d { .. }) = l.live_fx.as_mut() else {
                        return Err(EngineError::Other("layer is not a Relight layer".into()));
                    };
                    update_live_fx(fx, p);
                    Ok(())
                })?;
                Ok(json!({ "layer": id.0 }))
            },
            journal: true,
        },
        CommandSpec {
            id: "filter.render.relight3d",
            label: "Relight (3D)…",
            menu: &["Filter", "Render"],
            shortcut: None,
            params: r#"{"layer":id?,"ambient":0..1.5=0.55,"surfaceRelief":0..2=1,"lamps":[…]}"#,
            enabled: has_filterable_layer,
            run: |s, p| {
                let rp = parse_relight_algo_params(p);
                run_surface_or_smart_filter(s, "filter.render.relight3d", "Relight", p, |surf, canvas| apply_relight_3d_surface(surf, canvas, &rp))
            },
            journal: true,
        },
        // 3. Oil Brush Tool (`유화 브러시 도구`)
        CommandSpec {
            id: "paint.oilBrush",
            label: "Oil Brush",
            menu: &[],
            shortcut: None,
            params: r##"{"points":[[x,y,pressure?],…],"preset":"blocky|glaze|round"?,"size":1..500?,"paintMix":0..1?,"opacity":0..1?,"color":[r,g,b,a?]|"#rrggbb"?,"layer":id?}"##,
            enabled: has_paintable,
            run: run_oil_brush,
            journal: true,
        },
        CommandSpec {
            id: "tools.setOilBrush",
            label: "Set Oil Brush",
            menu: &[],
            shortcut: None,
            params: r#"{"preset":"blocky|glaze|round"?,"size":1..500?,"paintMix":0..1?,"opacity":0..1?}"#,
            enabled: |_| Ok(()),
            run: set_oil_brush,
            journal: true,
        },
        // 4. Remove Ambient Effects (`주변 효과 제거…`)
        CommandSpec {
            id: "edit.removeAmbientEffects",
            label: "Remove Ambient Effects…",
            menu: &["Edit"],
            shortcut: None,
            params: r#"{"mode":"shadows|reflections|lensFlares|atmospheric"="shadows","removeRelatedElements":bool=true,"enhance":bool=true,"strength":0..100=80,"outputToNewLayer":bool=false,"layer":id?}"#,
            enabled: |s| {
                has_rgb_or_gray_doc(s)?;
                has_filterable_layer(s)
            },
            run: run_remove_ambient,
            journal: true,
        },
        // 5. Type 2027 (`Convert to Text Layers`, `Dynamic Text Fit`, `Release Dynamic Fit`, `Copy/Paste Text Attributes`)
        CommandSpec {
            id: "type.rasterToText",
            label: "Convert to Text Layers",
            menu: &["Type"],
            shortcut: None,
            params: r#"{"layer":id?,"keepOriginal":bool=false}"#,
            enabled: has_raster_layer,
            run: run_raster_to_text,
            journal: true,
        },
        CommandSpec {
            id: "type.setDynamicFit",
            label: "Dynamic Text Fit",
            menu: &["Type"],
            shortcut: None,
            params: r#"{"layer":id?,"enabled":bool?}"#,
            enabled: has_type_layer,
            run: run_set_dynamic_fit,
            journal: true,
        },
        CommandSpec {
            id: "type.releaseDynamicFit",
            label: "Release Dynamic Fit",
            menu: &["Type"],
            shortcut: None,
            params: r#"{"layer":id?}"#,
            enabled: has_dynamic_fit_type_layer,
            run: run_release_dynamic_fit,
            journal: true,
        },
        CommandSpec {
            id: "edit.copyTextAttributes",
            label: "Copy Text Attributes",
            menu: &["Edit"],
            shortcut: None,
            params: r#"{"layer":id?}"#,
            enabled: has_type_layer,
            run: run_copy_text_attributes,
            journal: true,
        },
        CommandSpec {
            id: "edit.pasteTextAttributes",
            label: "Paste Text Attributes",
            menu: &["Edit"],
            shortcut: None,
            params: r#"{"layer":id?}"#,
            enabled: can_paste_text_attributes,
            run: run_paste_text_attributes,
            journal: true,
        },
        // 6. Generative Recompose (`생성형 재구성…`)
        CommandSpec {
            id: "edit.generativeRecompose",
            label: "Generative Recompose…",
            menu: &["Edit"],
            shortcut: None,
            params: r#"{"width":u32?,"height":u32?,"aspectRatio":"1:1|16:9|9:16|4:5|4:3|3:2|21:9|custom"?,"prompt":str?}"#,
            enabled: has_rgb_or_gray_doc,
            run: run_generative_recompose,
            journal: true,
        },
        // 7. Restyle Layer, Filter & `Capture as Layer` (`생성형 스타일 변경` / `레이어로 캡처`)
        CommandSpec {
            id: "layer.newRestyleLayer",
            label: "New Restyle Layer…",
            menu: &["Layer", "New"],
            shortcut: None,
            params: r#"{"preset":"oilPainting|watercolor|cyberpunkNeon|vintageFilm|pencilSketch|popArt|animeCel|noirInk"="oilPainting","prompt":str="","intensity":0..100=80,"name":str?,"opacity":0..1=1,"clipToPrevious":bool=false}"#,
            enabled: has_rgb_or_gray_doc,
            run: |s, p| {
                let fx = LiveBackdropFx::Restyle {
                    preset: parse_restyle_preset(p, RestylePreset::OilPainting),
                    prompt: p.get("prompt").and_then(Value::as_str).unwrap_or("").to_string(),
                    intensity: f32_or(p, "intensity", 80.0).clamp(0.0, 100.0),
                };
                create_live_fx_layer(s, "layer.newRestyleLayer", "New Restyle Layer", "Restyle", fx, p)
            },
            journal: true,
        },
        CommandSpec {
            id: "layer.restyle.setParams",
            label: "Edit Restyle Layer",
            menu: &[],
            shortcut: None,
            params: r#"{"layer":id?,"preset":str?,"prompt":str?,"intensity":0..100?}"#,
            enabled: |s| {
                let l = crate::active_layer_of(s)?;
                if matches!(l.live_fx, Some(LiveBackdropFx::Restyle { .. })) { Ok(()) } else { Err("active layer is not a Restyle layer".into()) }
            },
            run: |s, p| {
                let id = layer_param(s, p)?;
                s.edit("Edit Restyle Layer", |doc, _| {
                    let l = doc.layer_mut(id).ok_or(EngineError::NoLayer(id))?;
                    let Some(fx @ LiveBackdropFx::Restyle { .. }) = l.live_fx.as_mut() else {
                        return Err(EngineError::Other("layer is not a Restyle layer".into()));
                    };
                    update_live_fx(fx, p);
                    Ok(())
                })?;
                Ok(json!({ "layer": id.0 }))
            },
            journal: true,
        },
        CommandSpec {
            id: "layer.restyle.captureAsLayer",
            label: "Capture as Layer",
            menu: &["Layer"],
            shortcut: None,
            params: r#"{"layer":id?}"#,
            enabled: has_live_fx_layer,
            run: run_capture_as_layer,
            journal: true,
        },
        CommandSpec {
            id: "filter.stylize.restyle",
            label: "Generative Restyle…",
            menu: &["Filter", "Stylize"],
            shortcut: None,
            params: r#"{"layer":id?,"preset":"oilPainting|watercolor|cyberpunkNeon|vintageFilm|pencilSketch|popArt|animeCel|noirInk"="oilPainting","prompt":str="","intensity":0..100=80}"#,
            enabled: has_filterable_layer,
            run: |s, p| {
                let rp = parse_restyle_algo_params(p);
                run_surface_or_smart_filter(s, "filter.stylize.restyle", "Generative Restyle", p, |surf, canvas| apply_restyle_surface(surf, canvas, &rp))
            },
            journal: true,
        },
        // 8. Mockup Group & Environmental Lighting (`목업` / `환경 조명` / `목업 풀기`)
        CommandSpec {
            id: "layer.mockup.create",
            label: "Create Mockup",
            menu: &["Layer"],
            shortcut: None,
            params: r#"{"lighting":0..2=0.8}"#,
            enabled: |s| {
                has_doc(s)?;
                crate::active_layer_of(s).map(|_| ())
            },
            run: run_create_mockup,
            journal: true,
        },
        CommandSpec {
            id: "layer.mockup.setLighting",
            label: "Mockup Environmental Lighting",
            menu: &[],
            shortcut: None,
            params: r#"{"layer":id?,"lighting":0..2}"#,
            enabled: has_mockup_target,
            run: run_set_mockup_lighting,
            journal: true,
        },
        CommandSpec {
            id: "layer.mockup.release",
            label: "Release Mockup",
            menu: &["Layer"],
            shortcut: None,
            params: r#"{"layer":id?}"#,
            enabled: has_mockup_target,
            run: run_release_mockup,
            journal: true,
        },
        // 9. Agent Skills 8 One-Click Workflows (`에이전트 스킬`)
        CommandSpec {
            id: "agent.skill.cinematicGrade",
            label: "Cinematic Grade",
            menu: &["Filter", "Agent Skills"],
            shortcut: None,
            params: "{}",
            enabled: has_rgb_or_gray_doc,
            run: run_skill_cinematic_grade,
            journal: true,
        },
        CommandSpec {
            id: "agent.skill.enhanceAndResize",
            label: "Enhance and Resize",
            menu: &["Filter", "Agent Skills"],
            shortcut: None,
            params: r#"{"scale":1.25..4=2}"#,
            enabled: has_rgb_or_gray_doc,
            run: run_skill_enhance_and_resize,
            journal: true,
        },
        CommandSpec {
            id: "agent.skill.perfectPortrait",
            label: "Perfect Portrait",
            menu: &["Filter", "Agent Skills"],
            shortcut: None,
            params: "{}",
            enabled: has_rgb_or_gray_doc,
            run: run_skill_perfect_portrait,
            journal: true,
        },
        CommandSpec {
            id: "agent.skill.productStudioShot",
            label: "Product Studio Shot",
            menu: &["Filter", "Agent Skills"],
            shortcut: None,
            params: "{}",
            enabled: has_rgb_or_gray_doc,
            run: run_skill_product_studio_shot,
            journal: true,
        },
        CommandSpec {
            id: "agent.skill.removeBackground",
            label: "Remove Background",
            menu: &["Filter", "Agent Skills"],
            shortcut: None,
            params: r#"{"layer":id?}"#,
            enabled: has_filterable_layer,
            run: run_skill_remove_background,
            journal: true,
        },
        CommandSpec {
            id: "agent.skill.retouchPortrait",
            label: "Retouch Portrait",
            menu: &["Filter", "Agent Skills"],
            shortcut: None,
            params: "{}",
            enabled: has_rgb_or_gray_doc,
            run: run_skill_retouch_portrait,
            journal: true,
        },
        CommandSpec {
            id: "agent.skill.socialMediaHero",
            label: "Social Media Hero",
            menu: &["Filter", "Agent Skills"],
            shortcut: None,
            params: r#"{"aspectRatio":str="16:9"}"#,
            enabled: has_rgb_or_gray_doc,
            run: run_skill_social_media_hero,
            journal: true,
        },
        CommandSpec {
            id: "agent.skill.subjectSpotlight",
            label: "Subject Spotlight",
            menu: &["Filter", "Agent Skills"],
            shortcut: None,
            params: "{}",
            enabled: has_rgb_or_gray_doc,
            run: run_skill_subject_spotlight,
            journal: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    #[test]
    fn nge_fireflies_water_ripple_relight_and_restyle_capture_work_end_to_end() {
        let mut s = Session::new();
        s.execute("file.new", json!({"width": 64, "height": 64, "background": "white"})).unwrap();
        s.execute("edit.fill", json!({"contents": "color", "color": "#223355"})).unwrap();

        // Non-destructive Fireflies, Water Ripple, Relight, Restyle + Capture as Layer!
        let ff = s.execute("layer.newAdjustmentLayer.fireflies", json!({"quantity": 36, "blurriness": 50})).unwrap();
        s.execute("layer.liveFx.setParams", json!({"layer": ff["layer"], "quantity": 64})).unwrap();

        let _wr = s.execute("layer.newAdjustmentLayer.waterRipple", json!({"horizon": 0.6, "amplitude": 45})).unwrap();
        let rl = s.execute("layer.newRelightLayer", json!({"ambient": 0.45})).unwrap();
        s.execute(
            "layer.editRelightLayer",
            json!({"layer": rl["layer"], "lamps": [{"enabled": true, "x": 0.2, "y": 0.2, "z": 0.5, "intensity": 1.6, "color": [1.0, 0.8, 0.6], "spread": 0.7}]}),
        )
        .unwrap();

        let rs = s.execute("layer.newRestyleLayer", json!({"preset": "cyberpunkNeon", "intensity": 85})).unwrap();
        s.execute("layer.restyle.setParams", json!({"layer": rs["layer"], "intensity": 92, "prompt": "vivid neon"})).unwrap();
        let cap = s.execute("layer.restyle.captureAsLayer", json!({})).unwrap();
        let cap_id = photocraft_doc::LayerId(cap["layer"].as_u64().unwrap());
        let st = s.active().unwrap();
        assert!(st.doc.layer(cap_id).unwrap().surface().is_some());
    }

    #[test]
    fn oil_brush_ambient_remove_recompose_mockup_type_and_agent_skills_work() {
        let mut s = Session::new();
        s.execute("file.new", json!({"width": 64, "height": 48, "background": "white"})).unwrap();

        // Oil Brush Tool stroke
        s.execute("tools.setOilBrush", json!({"preset": "blocky", "size": 18, "paintMix": 0.55})).unwrap();
        let ob = s.execute("paint.oilBrush", json!({"points": [[12, 12, 0.9], [44, 32, 0.85]], "color": "#cc3311"})).unwrap();
        assert_eq!(ob["preset"], "blocky");

        // Remove Ambient Effects (all 4 modes)
        for mode in ["shadows", "reflections", "lensFlares", "atmospheric"] {
            s.execute("edit.removeAmbientEffects", json!({"mode": mode, "strength": 60, "enhance": true})).unwrap();
        }

        // Create decal layer -> Create Mockup -> Adjust Environmental Lighting -> Release Mockup
        s.execute("layer.new.layer", json!({"name": "Logo"})).unwrap();
        s.execute("paint.oilBrush", json!({"points": [[20, 20], [36, 28]], "preset": "glaze", "color": "#1188ff"})).unwrap();
        let mk = s.execute("layer.mockup.create", json!({"lighting": 0.9})).unwrap();
        s.execute("layer.mockup.setLighting", json!({"layer": mk["layer"], "lighting": 1.15})).unwrap();
        let rel = s.execute("layer.mockup.release", json!({})).unwrap();
        assert_eq!(rel["released"].as_array().unwrap().len(), 1);

        // Type Dynamic Fit + Release Dynamic Fit + Copy/Paste Text Attributes
        let t1 = s.execute("type.create", json!({"text": "HELLO 2027", "x": 8, "y": 24, "size": 18, "color": "#ff4400"})).unwrap();
        s.execute("type.setDynamicFit", json!({"layer": t1["layer"], "enabled": true})).unwrap();
        let rel_fit = s.execute("type.releaseDynamicFit", json!({"layer": t1["layer"]})).unwrap();
        assert_eq!(rel_fit["dynamicFit"], false);

        s.execute("edit.copyTextAttributes", json!({"layer": t1["layer"]})).unwrap();
        let t2 = s.execute("type.create", json!({"text": "WORLD", "x": 8, "y": 40, "size": 10, "color": "#000000"})).unwrap();
        s.execute("edit.pasteTextAttributes", json!({"layer": t2["layer"]})).unwrap();
        let t2_id = photocraft_doc::LayerId(t2["layer"].as_u64().unwrap());
        if let photocraft_doc::LayerContent::Text(tl2) = &s.active().unwrap().doc.layer(t2_id).unwrap().content {
            assert!((tl2.size_pt - rel_fit["sizePt"].as_f64().unwrap() as f32).abs() < 0.01);
        } else {
            panic!("expected Text layer");
        }

        // Generative Recompose
        let rec = s.execute("edit.generativeRecompose", json!({"width": 80, "height": 48, "aspectRatio": "custom"})).unwrap();
        assert_eq!(rec["width"], 80);
        assert_eq!(s.active().unwrap().doc.size.width, 80);

        // All 8 Agent Skills
        for skill in [
            "agent.skill.cinematicGrade",
            "agent.skill.perfectPortrait",
            "agent.skill.productStudioShot",
            "agent.skill.retouchPortrait",
            "agent.skill.subjectSpotlight",
            "agent.skill.socialMediaHero",
            "agent.skill.enhanceAndResize",
        ] {
            s.execute(skill, json!({"scale": 1.25})).unwrap();
        }
        // Remove background on a raster layer
        s.execute("layer.new.layer", json!({"name": "Subject Test"})).unwrap();
        s.execute("edit.fill", json!({"contents": "color", "color": "#dd6622"})).unwrap();
        s.execute("agent.skill.removeBackground", json!({})).unwrap();
    }
}
