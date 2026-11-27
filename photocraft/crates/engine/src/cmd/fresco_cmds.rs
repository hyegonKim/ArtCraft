//! Adobe Fresco (`FRSC v5.7.1`) Live Watercolor engine commands:
//! - `fresco.applyLiveWatercolor`: Simulates per-pixel wet-canvas capillary diffusion, pigment
//!   advection along wetness gradients, and organic cauliflower/coffee-ring edge deposition on the
//!   active raster layer (or as a non-destructive Smart Filter on Smart Objects), updating the
//!   document's per-layer [`WetCanvasMap`].
//! - `fresco.dryLayer`: Rapidly evaporates all remaining moisture on the target layer, precipitating
//!   mobile suspended pigment with final contact-line edge darkening and clearing its wetness map.

use photocraft_color::{BlendMode, ColorMode};
use photocraft_doc::{LayerContent, LayerId, SmartFilter};
use photocraft_geom::Rect;
use photocraft_paint::StrokePoint;
use photocraft_paint::fresco_watercolor::{FrescoWatercolorParams, FrescoWatercolorPreset, MAX_WATERCOLOR_PIXELS, WetCanvasMap, apply_live_watercolor_surface};
use photocraft_raster::Surface;
use serde_json::{Value, json};

use crate::commands::{CommandSpec, layer_param, parse_hex};
use crate::filters::has_filterable_layer;
use crate::jobs::JobCtx;
use crate::{EngineError, Result, Session};

fn bad(cmd: &str, msg: impl Into<String>) -> EngineError {
    EngineError::BadParams { cmd: cmd.into(), msg: msg.into() }
}

fn checked_f32(cmd: &str, p: &Value, key: &str, def: f32, lo: f32, hi: f32) -> Result<f32> {
    let Some(v) = p.get(key) else { return Ok(def) };
    if v.is_null() {
        return Ok(def);
    }
    let Some(f) = v.as_f64().filter(|x| x.is_finite()) else {
        return Err(bad(cmd, format!("`{key}` must be a finite number in {lo}..={hi}")));
    };
    Ok((f as f32).clamp(lo, hi))
}

fn checked_u32(cmd: &str, p: &Value, key: &str, def: u32, lo: u32, hi: u32) -> Result<u32> {
    let Some(v) = p.get(key) else { return Ok(def) };
    if v.is_null() {
        return Ok(def);
    }
    let Some(n) = v.as_u64().or_else(|| v.as_f64().filter(|f| f.is_finite() && *f >= 0.0).map(|f| f.round() as u64)) else {
        return Err(bad(cmd, format!("`{key}` must be an integer in {lo}..={hi}")));
    };
    Ok((n.min(u64::from(u32::MAX)) as u32).clamp(lo, hi))
}

fn parse_rgba(cmd: &str, p: &Value, key: &str, def: [f32; 4]) -> Result<[f32; 4]> {
    let Some(v) = p.get(key) else { return Ok(def) };
    if v.is_null() {
        return Ok(def);
    }
    if let Some(hex) = v.as_str() {
        return parse_hex(hex).ok_or_else(|| bad(cmd, format!("invalid hex color `{hex}` for `{key}`")));
    }
    let Some(arr) = v.as_array() else {
        return Err(bad(cmd, format!("`{key}` must be a `#rrggbb` hex string or `[r,g,b,a?]` array")));
    };
    if arr.len() < 3 {
        return Err(bad(cmd, format!("`{key}` array needs at least 3 components")));
    }
    let comp = |i: usize, d: f32| -> Result<f32> {
        let Some(item) = arr.get(i) else { return Ok(d) };
        let Some(f) = item.as_f64().filter(|x| x.is_finite()) else {
            return Err(bad(cmd, format!("`{key}[{i}]` must be a finite number")));
        };
        Ok((f as f32).clamp(0.0, 1.0))
    };
    Ok([comp(0, def[0])?, comp(1, def[1])?, comp(2, def[2])?, comp(3, def[3])?])
}

fn parse_stroke_points(cmd: &str, p: &Value) -> Result<Option<Vec<StrokePoint>>> {
    let Some(v) = p.get("points") else { return Ok(None) };
    if v.is_null() {
        return Ok(None);
    }
    let Some(arr) = v.as_array() else {
        return Err(bad(cmd, "`points` must be an array of `[x, y, pressure?]` or `{x, y, pressure?}`"));
    };
    if arr.len() > 50_000 {
        return Err(bad(cmd, "`points` exceeds maximum 50,000 samples per stroke"));
    }
    let mut pts = Vec::with_capacity(arr.len());
    for item in arr {
        if let Some(pair) = item.as_array() {
            let x = pair.first().and_then(Value::as_f64).filter(|x| x.is_finite()).ok_or_else(|| bad(cmd, "stroke point missing finite `x`"))?;
            let y = pair.get(1).and_then(Value::as_f64).filter(|y| y.is_finite()).ok_or_else(|| bad(cmd, "stroke point missing finite `y`"))?;
            let pr = pair.get(2).and_then(Value::as_f64).filter(|v| v.is_finite()).map_or(1.0, |v| (v as f32).clamp(0.05, 1.0));
            pts.push(StrokePoint::new(x, y, pr));
        } else if item.is_object() {
            let x = item.get("x").and_then(Value::as_f64).filter(|x| x.is_finite()).ok_or_else(|| bad(cmd, "stroke point object missing finite `x`"))?;
            let y = item.get("y").and_then(Value::as_f64).filter(|y| y.is_finite()).ok_or_else(|| bad(cmd, "stroke point object missing finite `y`"))?;
            let pr = item.get("pressure").and_then(Value::as_f64).filter(|v| v.is_finite()).map_or(1.0, |v| (v as f32).clamp(0.05, 1.0));
            pts.push(StrokePoint::new(x, y, pr));
        } else {
            return Err(bad(cmd, "each stroke point must be `[x, y, pressure?]` or `{x, y}`"));
        }
    }
    Ok(Some(pts))
}

pub fn parse_watercolor_params(cmd: &str, p: &Value) -> Result<FrescoWatercolorParams> {
    let def = FrescoWatercolorParams::default();
    let preset = match p.get("preset") {
        None | Some(Value::Null) => def.preset,
        Some(v) => {
            let s = v.as_str().ok_or_else(|| bad(cmd, "`preset` must be a string"))?;
            FrescoWatercolorPreset::from_id(s)
                .ok_or_else(|| bad(cmd, format!("unknown watercolor preset `{s}` (washSoft|wetSpatter|detailInk|granulating)")))?
        }
    };
    Ok(FrescoWatercolorParams {
        preset,
        water_flow: checked_f32(cmd, p, "waterFlow", def.water_flow, 0.0, 1.0)?,
        diffusion_steps: checked_u32(cmd, p, "diffusionSteps", def.diffusion_steps, 1, 24)?,
        advection_strength: checked_f32(cmd, p, "advectionStrength", def.advection_strength, 0.0, 2.0)?,
        cauliflower_edge: checked_f32(cmd, p, "cauliflowerEdge", def.cauliflower_edge, 0.0, 1.5)?,
        paper_grain: checked_f32(cmd, p, "paperGrain", def.paper_grain, 0.0, 1.0)?,
        evaporation: checked_f32(cmd, p, "evaporation", def.evaporation, 0.0, 1.0)?,
        seed: checked_u32(cmd, p, "seed", def.seed, 0, u32::MAX)?,
    }
    .sanitized())
}

fn has_supported_watercolor_layer(s: &Session) -> std::result::Result<(), String> {
    let st = s.active().ok_or("no document open")?;
    if matches!(st.doc.mode, ColorMode::Bitmap | ColorMode::Indexed) {
        return Err(format!("not available in {:?} mode", st.doc.mode));
    }
    has_filterable_layer(s)
}

/// Smart Filter surface replay hook called from `crate::filters::apply_filter_to_surface_in`.
pub fn apply_to_surface(id: &str, params: &Value, surf: &Surface, canvas: Rect) -> Option<Surface> {
    if id != "fresco.applyLiveWatercolor" {
        return None;
    }
    let area = if canvas.is_empty() { surf.content_bounds() } else { canvas };
    if area.is_empty() {
        return Some(surf.clone());
    }
    let wp = parse_watercolor_params(id, params).unwrap_or_default();
    let mut wet_map = WetCanvasMap::new(area)?;
    let pts = parse_stroke_points(id, params).ok().flatten();
    let size = checked_f32(id, params, "size", 24.0, 1.0, 400.0).unwrap_or(24.0);
    let color = parse_rgba(id, params, "color", [0.15, 0.35, 0.85, 1.0]).unwrap_or([0.15, 0.35, 0.85, 1.0]);
    Some(apply_live_watercolor_surface(surf, area, &mut wet_map, &wp, pts.as_deref(), size, color))
}

fn blend_filtered_region(base: &mut Surface, filtered: &Surface, area: Rect, selection: Option<&Surface>, lock_alpha: bool) {
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
                if let (Some(&bv), Some(&fv), Some(dst)) = (b_raw.get(idx + c), f_raw.get(idx + c), out.get_mut(idx + c)) {
                    *dst = bv * (1.0 - sel_w) + fv * sel_w;
                }
            }
            if lock_alpha
                && fmt.alpha
                && ch > 0
                && let (Some(&ba), Some(dst_a)) = (b_raw.get(idx + ch - 1), out.get_mut(idx + ch - 1))
            {
                *dst_a = ba;
            }
        }
    }
    base.write_region(area, &out);
    base.prune();
}

fn run_apply_live_watercolor(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "fresco.applyLiveWatercolor";
    let wp = parse_watercolor_params(CMD, p)?;
    let pts = parse_stroke_points(CMD, p)?;
    let size = checked_f32(CMD, p, "size", s.tools.brush.size.clamp(1.0, 400.0), 1.0, 400.0)?;
    let color = parse_rgba(CMD, p, "color", s.tools.foreground)?;
    let layer_id = layer_param(s, p)?;

    let canvas = s.active().ok_or(EngineError::NoDocument)?.doc.bounds();
    let total_px = (canvas.width() as usize).saturating_mul(canvas.height() as usize);
    if total_px > MAX_WATERCOLOR_PIXELS {
        return Err(bad(CMD, "document dimensions exceed Live Watercolor 64 MP working limit"));
    }

    let mut existing_wet = s
        .active()
        .and_then(|st| st.fresco_wetness.get(&layer_id).cloned())
        .or_else(|| WetCanvasMap::new(canvas))
        .ok_or_else(|| bad(CMD, "unable to initialize wet-canvas map for empty canvas"))?;

    let mut stored_params = p.clone();
    if let Value::Object(m) = &mut stored_params {
        m.remove("layer");
        m.remove("target");
        m.remove("coalesce");
    }

    let mut smart_applied = false;
    s.edit("Live Watercolor", |doc, _| {
        let locks = doc.effective_locks(layer_id);
        if locks.pixels || locks.all {
            let name = doc.layer(layer_id).map_or("Layer", |l| l.name.as_str());
            return Err(EngineError::Other(format!("Could not complete your request because the layer \"{name}\" is locked")));
        }
        let selection = doc.selection.clone();
        let bounds = doc.bounds();
        let l = doc.layer_mut(layer_id).ok_or(EngineError::NoLayer(layer_id))?;
        match &mut l.content {
            LayerContent::Raster(surf) => {
                let filtered = apply_live_watercolor_surface(surf, bounds, &mut existing_wet, &wp, pts.as_deref(), size, color);
                blend_filtered_region(surf, &filtered, bounds, selection.as_ref(), locks.transparency);
                Ok(())
            }
            LayerContent::Smart(_) => {
                let sf = SmartFilter { command: CMD.to_string(), params: stored_params, blend: BlendMode::Normal, opacity: 1.0, visible: true };
                crate::smart_cmds::add_smart_filter_in(doc, layer_id, sf, selection.as_ref(), &JobCtx::new())?;
                smart_applied = true;
                Ok(())
            }
            other => Err(EngineError::Other(format!("Live Watercolor needs a pixel layer (active layer is {} {} layer)", other.article(), other.kind_name()))),
        }
    })?;

    let wet_fraction = existing_wet.wet_fraction();
    let max_wetness = existing_wet.max_wetness();
    if !smart_applied && let Some(st) = s.active_mut() {
        st.fresco_wetness.insert(layer_id, existing_wet);
    }

    Ok(json!({
        "layer": layer_id.0,
        "preset": wp.preset.id(),
        "wetFraction": wet_fraction,
        "maxWetness": max_wetness,
        "smartFilter": smart_applied,
    }))
}

fn run_dry_layer(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "fresco.dryLayer";
    let boost = checked_f32(CMD, p, "cauliflowerBoost", 0.35, 0.0, 1.5)?;
    let layer_id = layer_param(s, p)?;

    let maybe_wet = s.active().and_then(|st| st.fresco_wetness.get(&layer_id).cloned());
    let Some(mut wet_map) = maybe_wet.filter(|m| m.max_wetness() > 1e-5) else {
        if let Some(st) = s.active_mut() {
            st.fresco_wetness.remove(&layer_id);
        }
        return Ok(json!({
            "layer": layer_id.0,
            "dried": false,
            "peakWetness": 0.0,
        }));
    };

    let peak = s.edit("Dry Watercolor Layer", |doc, _| {
        let locks = doc.effective_locks(layer_id);
        if locks.pixels || locks.all {
            let name = doc.layer(layer_id).map_or("Layer", |l| l.name.as_str());
            return Err(EngineError::Other(format!("Could not complete your request because the layer \"{name}\" is locked")));
        }
        let selection = doc.selection.clone();
        let bounds = doc.bounds();
        let l = doc.layer_mut(layer_id).ok_or(EngineError::NoLayer(layer_id))?;
        match &mut l.content {
            LayerContent::Raster(surf) => Ok(wet_map.dry_all(surf, bounds, boost, selection.as_ref(), locks.transparency)),
            other => {
                Err(EngineError::Other(format!("Dry Watercolor Layer needs a pixel layer (active layer is {} {} layer)", other.article(), other.kind_name())))
            }
        }
    })?;

    if let Some(st) = s.active_mut() {
        st.fresco_wetness.remove(&layer_id);
    }

    Ok(json!({
        "layer": layer_id.0,
        "dried": peak > 1e-5,
        "peakWetness": peak,
    }))
}

/// Wetness metrics `(wet_fraction, max_wetness)` for `layer_id` in the active document, for UI badges.
pub fn layer_wetness_status(s: &Session, layer_id: LayerId) -> (f32, f32) {
    s.active().and_then(|st| st.fresco_wetness.get(&layer_id)).map_or((0.0, 0.0), |m| (m.wet_fraction(), m.max_wetness()))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec {
            id: "fresco.applyLiveWatercolor",
            label: "Live Watercolor…",
            menu: &["Filter", "Fresco"],
            shortcut: None,
            params: r##"{"preset":"washSoft|wetSpatter|detailInk|granulating"="washSoft","waterFlow":0..1=0.65,"diffusionSteps":1..24=8,"advectionStrength":0..2=0.85,"cauliflowerEdge":0..1.5=0.55,"paperGrain":0..1=0.35,"evaporation":0..1=0.12,"seed":u32=42,"points":[[x,y,pressure?],…]?,"size":1..400=24,"color":"#rrggbb"|[r,g,b,a]?,"layer":id?}"##,
            enabled: has_supported_watercolor_layer,
            run: run_apply_live_watercolor,
            journal: true,
        },
        CommandSpec {
            id: "fresco.dryLayer",
            label: "Dry Watercolor Layer",
            menu: &["Filter", "Fresco"],
            shortcut: None,
            params: r#"{"cauliflowerBoost":0..1.5=0.35,"layer":id?}"#,
            enabled: has_supported_watercolor_layer,
            run: run_dry_layer,
            journal: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use photocraft_doc::LayerId;
    use photocraft_geom::Rect;
    use serde_json::json;

    use super::layer_wetness_status;
    use crate::Session;

    #[test]
    fn live_watercolor_diffuses_across_depths_tracks_wetness_and_dry_layer_clears_it() {
        for depth in [8, 16, 32] {
            let mut s = Session::new();
            s.execute("file.new", json!({"width": 40, "height": 40, "depth": depth, "background": "white"})).unwrap();
            let l_res = s.execute("layer.new.layer", json!({"name": "Watercolor Wash"})).unwrap();
            let lid = LayerId(l_res["layer"].as_u64().unwrap());

            let wc = s
                .execute(
                    "fresco.applyLiveWatercolor",
                    json!({
                        "preset": "washSoft",
                        "waterFlow": 0.80,
                        "diffusionSteps": 10,
                        "advectionStrength": 1.0,
                        "cauliflowerEdge": 0.75,
                        "paperGrain": 0.40,
                        "points": [[12.0, 20.0, 0.9], [28.0, 20.0, 1.0]],
                        "size": 14.0,
                        "color": "#2266dd"
                    }),
                )
                .unwrap();
            assert!(wc["wetFraction"].as_f64().unwrap() > 0.05);
            assert!(wc["maxWetness"].as_f64().unwrap() > 0.2);
            assert!(layer_wetness_status(&s, lid).1 > 0.2);

            let surf_wet = s.active().unwrap().doc.layer(lid).unwrap().surface().unwrap().rgba(20, 20);
            assert!(surf_wet[3] > 0.2 && surf_wet[2] > surf_wet[0], "depth={depth}: expected blue pigment, got {surf_wet:?}");

            // Dry layer commits remaining pigment and resets wetness to 0!
            let dry = s.execute("fresco.dryLayer", json!({"cauliflowerBoost": 0.55})).unwrap();
            assert_eq!(dry["dried"], true);
            assert_eq!(layer_wetness_status(&s, lid), (0.0, 0.0));

            // Calling dryLayer again on an already-dry layer is a safe no-op.
            let dry_again = s.execute("fresco.dryLayer", json!({})).unwrap();
            assert_eq!(dry_again["dried"], false);

            // Undo & Redo round-trip cleanly.
            s.execute("edit.undo", json!({})).unwrap();
            s.execute("edit.redo", json!({})).unwrap();
        }
    }

    #[test]
    fn live_watercolor_respects_selection_and_rejects_hostile_params_without_panic() {
        let mut s = Session::new();
        s.execute("file.new", json!({"width": 32, "height": 32, "background": "white"})).unwrap();
        let l = s.execute("layer.new.layer", json!({"name": "Selected Wash"})).unwrap();
        let lid = LayerId(l["layer"].as_u64().unwrap());

        s.execute("select.rect", json!({"x": 8, "y": 8, "width": 16, "height": 16})).unwrap();
        s.execute(
            "fresco.applyLiveWatercolor",
            json!({
                "preset": "granulating",
                "points": [[4.0, 16.0, 1.0], [28.0, 16.0, 1.0]],
                "size": 12.0,
                "color": "#dd3344"
            }),
        )
        .unwrap();

        let outside = s.active().unwrap().doc.layer(lid).unwrap().surface().unwrap().read_region(Rect::from_xywh(2, 16, 1, 1));
        let inside = s.active().unwrap().doc.layer(lid).unwrap().surface().unwrap().read_region(Rect::from_xywh(16, 16, 1, 1));
        assert_eq!(outside[3], 0.0, "outside selection must stay untouched");
        assert!(inside[3] > 0.2, "inside selection must receive wet watercolor stroke");

        // Hostile parameter checks (Rule 9)
        assert!(s.execute("fresco.applyLiveWatercolor", json!({"preset": "notAPreset"})).is_err());
        assert!(s.execute("fresco.applyLiveWatercolor", json!({"preset": 123})).is_err());
        assert!(s.execute("fresco.applyLiveWatercolor", json!({"waterFlow": "bad"})).is_err());
        assert!(s.execute("fresco.applyLiveWatercolor", json!({"points": "notAnArray"})).is_err());
        assert!(s.execute("fresco.dryLayer", json!({"cauliflowerBoost": "bad"})).is_err());
    }
}
