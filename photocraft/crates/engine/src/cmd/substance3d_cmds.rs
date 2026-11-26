//! Substance 3D engine commands (`Sampler`, `Stager`, `Modeler`, `Designer`, `Painter`):
//! - `filter.material.samplePbr`: Generates a 6-layer `"PBR Material Maps"` group (`BaseColor`,
//!   `Normal`, `Roughness`, `Metallic`, `Height`, `AO`) at the document's native bit depth
//!   (`8`/`16`/`32f`), or replaces the active layer with a single PBR channel (`"createGroup": false`).
//! - `filter.material.makeSeamless`: Converts the active layer into a seamlessly tileable texture.
//! - `filter.material.proceduralPbr`: Generates Cook-Torrance GGX shaded procedural PBR materials.
//! - `layer.new3dStageLayer`: Raymarches a 3D Stage / SDF Clay object with background-matched
//!   Equirectangular IBL, optional Active Layer Decal UV wrapping, weathering, and ground-plane
//!   shadow catcher into a new `"3D Stage"` raster layer.

use photocraft_algo::substance_pbr::{
    MaterialSamplerParams, ObjMesh, PbrChannel, ProceduralPbrParams, ProceduralPbrPreset, SdfBooleanOp,
    Stage3dInputs, Stage3dParams, StageMaterialPreset, StageShape, make_seamless_surface, parse_obj_mesh,
    render_procedural_pbr_surface, render_stage_3d_surface, sample_pbr_channel_surface,
    sample_pbr_maps_rgba,
};
use photocraft_color::{BlendMode, ColorMode};
use photocraft_doc::{Document, Group, Layer, LayerContent, LayerId, SmartFilter};
use photocraft_geom::Rect;
use photocraft_raster::{Surface, from_rgba_into, to_rgba};
use serde_json::{Value, json};

use crate::commands::{CommandSpec, layer_param};
use crate::filters::has_filterable_layer;
use crate::jobs::JobCtx;
use crate::{EngineError, Result, Session};

const MAX_STAGE_PIXELS: u64 = 64_000_000;

fn bad(cmd: &str, msg: impl Into<String>) -> EngineError {
    EngineError::BadParams { cmd: cmd.into(), msg: msg.into() }
}

fn f32_or(p: &Value, k: &str, d: f32) -> f32 {
    p.get(k)
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite())
        .map_or(d, |v| v as f32)
}

fn u32_or(p: &Value, k: &str, d: u32) -> u32 {
    p.get(k)
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(d)
}

fn bool_or(p: &Value, k: &str, d: bool) -> bool {
    p.get(k).and_then(Value::as_bool).unwrap_or(d)
}

fn vec3_or(p: &Value, k: &str, d: [f32; 3], lo: f32, hi: f32) -> [f32; 3] {
    let Some(arr) = p.get(k).and_then(Value::as_array) else {
        return d;
    };
    if arr.len() < 3 {
        return d;
    }
    let elem = |i: usize, def: f32| {
        arr.get(i)
            .and_then(Value::as_f64)
            .filter(|v| v.is_finite())
            .map_or(def, |v| (v as f32).clamp(lo, hi))
    };
    [elem(0, d[0]), elem(1, d[1]), elem(2, d[2])]
}

fn rgb_or(p: &Value, k: &str, d: [f32; 3]) -> [f32; 3] {
    if let Some(hex) = p.get(k).and_then(Value::as_str) {
        let h = hex.trim().trim_start_matches('#');
        let byte = |i: usize| {
            h.get(i..i + 2)
                .and_then(|s| u8::from_str_radix(s, 16).ok())
                .map(|v| f32::from(v) / 255.0)
        };
        if let (Some(r), Some(g), Some(b)) = (byte(0), byte(2), byte(4)) {
            return [r, g, b];
        }
        return d;
    }
    vec3_or(p, k, d, 0.0, 1.0)
}

fn has_supported_doc(s: &Session) -> std::result::Result<(), String> {
    let st = s.active().ok_or("no document open")?;
    if matches!(st.doc.mode, ColorMode::Bitmap | ColorMode::Indexed) {
        Err(format!("not available in {:?} mode", st.doc.mode))
    } else {
        Ok(())
    }
}

fn has_supported_filter_layer(s: &Session) -> std::result::Result<(), String> {
    has_supported_doc(s)?;
    has_filterable_layer(s)
}

// ------------------------------------------------------------------ Parameter Parsers

pub fn parse_sampler_params(p: &Value) -> MaterialSamplerParams {
    let def = MaterialSamplerParams::default();
    MaterialSamplerParams {
        delight_strength: f32_or(p, "delightStrength", def.delight_strength),
        make_tileable: bool_or(p, "makeTileable", def.make_tileable),
        tile_feather: f32_or(p, "tileFeather", def.tile_feather),
        normal_strength: f32_or(p, "normalStrength", def.normal_strength),
        roughness_base: f32_or(p, "roughnessBase", def.roughness_base),
        roughness_contrast: f32_or(p, "roughnessContrast", def.roughness_contrast),
        metallic_level: f32_or(p, "metallicLevel", def.metallic_level),
        ao_strength: f32_or(p, "aoStrength", def.ao_strength),
    }
    .sanitized()
}

fn parse_optional_channel(cmd: &str, p: &Value) -> Result<Option<PbrChannel>> {
    let Some(val) = p.get("channel") else {
        return Ok(None);
    };
    if val.is_null() {
        return Ok(None);
    }
    let Some(s) = val.as_str() else {
        return Err(bad(cmd, "`channel` must be a PBR channel name string"));
    };
    if s.trim().is_empty() || s == "group" || s == "all" {
        return Ok(None);
    }
    PbrChannel::from_id(s)
        .map(Some)
        .ok_or_else(|| bad(cmd, format!("unknown PBR channel `{s}` (baseColor|normal|roughness|metallic|height|ambientOcclusion)")))
}

pub fn parse_procedural_params(cmd: &str, p: &Value) -> Result<ProceduralPbrParams> {
    let def = ProceduralPbrParams::default();
    let preset = match p.get("preset") {
        None | Some(Value::Null) => def.preset,
        Some(v) => {
            let s = v.as_str().ok_or_else(|| bad(cmd, "`preset` must be a string"))?;
            ProceduralPbrPreset::from_id(s).ok_or_else(|| {
                bad(
                    cmd,
                    format!("unknown procedural PBR preset `{s}` (brushedSteel|carbonHexWeave|terrazzoMarble|ceramicTiles|weatheredCopper|wovenFabric)"),
                )
            })?
        }
    };
    Ok(ProceduralPbrParams {
        preset,
        scale: f32_or(p, "scale", def.scale),
        roughness: f32_or(p, "roughness", def.roughness),
        normal_strength: f32_or(p, "normalStrength", def.normal_strength),
        wear: f32_or(p, "wear", def.wear),
        light_angle_deg: f32_or(p, "lightAngle", f32_or(p, "lightAngleDeg", def.light_angle_deg)),
        light_elevation_deg: f32_or(p, "lightElevation", f32_or(p, "lightElevationDeg", def.light_elevation_deg)),
        seed: u32_or(p, "seed", def.seed),
    }
    .sanitized())
}

pub fn parse_stage3d_params(cmd: &str, p: &Value) -> Result<(Stage3dParams, Option<ObjMesh>)> {
    let def = Stage3dParams::default();
    let shape = match p.get("shape") {
        None | Some(Value::Null) => def.shape,
        Some(v) => {
            let s = v.as_str().ok_or_else(|| bad(cmd, "`shape` must be a string"))?;
            StageShape::from_id(s).ok_or_else(|| bad(cmd, format!("unknown 3D shape `{s}`")))?
        }
    };
    let sdf_op = match p.get("sdfOp") {
        None | Some(Value::Null) => def.sdf_op,
        Some(v) => {
            let s = v.as_str().ok_or_else(|| bad(cmd, "`sdfOp` must be a string"))?;
            SdfBooleanOp::from_id(s).ok_or_else(|| bad(cmd, format!("unknown `sdfOp` `{s}`")))?
        }
    };
    let secondary_shape = match p.get("secondaryShape") {
        None | Some(Value::Null) => def.secondary_shape,
        Some(v) => {
            let s = v.as_str().ok_or_else(|| bad(cmd, "`secondaryShape` must be a string"))?;
            StageShape::from_id(s).ok_or_else(|| bad(cmd, format!("unknown `secondaryShape` `{s}`")))?
        }
    };
    let material = match p.get("material") {
        None | Some(Value::Null) => def.material,
        Some(v) => {
            let s = v.as_str().ok_or_else(|| bad(cmd, "`material` must be a string"))?;
            StageMaterialPreset::from_id(s).ok_or_else(|| bad(cmd, format!("unknown `material` `{s}`")))?
        }
    };

    let mut rot_deg = vec3_or(p, "rotDeg", def.rot_deg, -3600.0, 3600.0);
    if p.get("rotX").is_some() {
        rot_deg[0] = f32_or(p, "rotX", rot_deg[0]);
    }
    if p.get("rotY").is_some() {
        rot_deg[1] = f32_or(p, "rotY", rot_deg[1]);
    }
    if p.get("rotZ").is_some() {
        rot_deg[2] = f32_or(p, "rotZ", rot_deg[2]);
    }

    let center = match p.get("center").and_then(Value::as_array) {
        Some(arr) if arr.len() >= 2 => [
            arr.first().and_then(Value::as_f64).filter(|v| v.is_finite()).map_or(def.center[0], |v| v as f32),
            arr.get(1).and_then(Value::as_f64).filter(|v| v.is_finite()).map_or(def.center[1], |v| v as f32),
        ],
        _ => def.center,
    };

    let custom_mesh = match p.get("objData") {
        None | Some(Value::Null) => None,
        Some(v) => {
            let text = v.as_str().ok_or_else(|| bad(cmd, "`objData` must be a Wavefront .obj string"))?;
            if text.trim().is_empty() {
                None
            } else {
                Some(parse_obj_mesh(text).map_err(|e| bad(cmd, e))?)
            }
        }
    };

    let params = Stage3dParams {
        shape,
        sdf_op,
        secondary_shape,
        smooth_k: f32_or(p, "smoothK", def.smooth_k),
        secondary_offset: vec3_or(p, "secondaryOffset", def.secondary_offset, -2.5, 2.5),
        material,
        base_color: rgb_or(p, "baseColor", def.base_color),
        metallic: f32_or(p, "metallic", def.metallic),
        roughness: f32_or(p, "roughness", def.roughness),
        weathering: f32_or(p, "weathering", def.weathering),
        rot_deg,
        scale: f32_or(p, "scale", def.scale),
        center,
        fov_deg: f32_or(p, "fovDeg", def.fov_deg),
        match_background_ibl: bool_or(p, "matchBackgroundIbl", def.match_background_ibl),
        light_dir: vec3_or(p, "lightDir", def.light_dir, -10.0, 10.0),
        light_intensity: f32_or(p, "lightIntensity", def.light_intensity),
        shadow_catcher: f32_or(p, "shadowCatcher", def.shadow_catcher),
        ground_y: f32_or(p, "groundY", def.ground_y),
    }
    .sanitized();

    Ok((params, custom_mesh))
}

// ------------------------------------------------------------------ Smart Filter Replay & Surface Commit

/// Smart Filter surface replay hook called from `crate::filters::apply_filter_to_surface_in`.
pub fn apply_to_surface(id: &str, params: &Value, surf: &Surface, canvas: Rect) -> Option<Surface> {
    let area = if canvas.is_empty() { surf.content_bounds() } else { canvas };
    if area.is_empty() {
        return Some(surf.clone());
    }
    match id {
        "filter.material.samplePbr" => {
            let ch = params
                .get("channel")
                .and_then(Value::as_str)
                .and_then(PbrChannel::from_id)
                .unwrap_or(PbrChannel::BaseColor);
            Some(sample_pbr_channel_surface(surf, area, ch, &parse_sampler_params(params)))
        }
        "filter.material.makeSeamless" => {
            let feather = f32_or(params, "feather", f32_or(params, "tileFeather", 0.22)).clamp(0.05, 0.5);
            Some(make_seamless_surface(surf, area, feather))
        }
        "filter.material.proceduralPbr" => {
            let pp = parse_procedural_params(id, params).unwrap_or_default();
            Some(render_procedural_pbr_surface(surf, area, &pp))
        }
        _ => None,
    }
}

fn commit_filtered_surface(
    base: &mut Surface,
    filtered: &Surface,
    area: Rect,
    selection: Option<&Surface>,
    lock_alpha: bool,
) {
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

fn run_destructive_or_smart_filter(
    s: &mut Session,
    cmd_id: &str,
    label: &str,
    p: &Value,
    render: impl FnOnce(&Surface, Rect) -> Surface,
) -> Result<Value> {
    let layer_id = layer_param(s, p)?;
    let mut stored_params = p.clone();
    if let Value::Object(m) = &mut stored_params {
        m.remove("layer");
        m.remove("target");
        m.remove("coalesce");
    }
    s.edit(label, |doc, _| {
        let locks = doc.effective_locks(layer_id);
        if locks.pixels || locks.all {
            let name = doc.layer(layer_id).map_or("Layer", |l| l.name.as_str());
            return Err(EngineError::Other(format!(
                "Could not complete your request because the layer \"{name}\" is locked"
            )));
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
                let sf = SmartFilter {
                    command: cmd_id.to_string(),
                    params: stored_params,
                    blend: BlendMode::Normal,
                    opacity: 1.0,
                    visible: true,
                };
                crate::smart_cmds::add_smart_filter_in(doc, layer_id, sf, selection.as_ref(), &JobCtx::new())?;
                Ok(json!({ "layer": layer_id.0, "smartFilter": true }))
            }
            other => Err(EngineError::Other(format!(
                "filters need a pixel layer (active layer is {} {} layer)",
                other.article(),
                other.kind_name()
            ))),
        }
    })
}

// ------------------------------------------------------------------ Command Implementations

fn extract_layer_surface(doc: &Document, layer_id: LayerId) -> Option<Surface> {
    let l = doc.layer(layer_id)?;
    match &l.content {
        LayerContent::Raster(s) => Some(s.clone()),
        LayerContent::Smart(sm) => sm.cache.clone(),
        _ => None,
    }
}

fn unique_exact_or_numbered(doc: &Document, base: &str) -> String {
    if doc.walk().iter().any(|(_, _, l)| l.name == base) {
        doc.next_layer_name(base)
    } else {
        base.to_string()
    }
}

fn run_sample_pbr(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "filter.material.samplePbr";
    let explicit_channel = parse_optional_channel(CMD, p)?;
    let create_group = p
        .get("createGroup")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| explicit_channel.is_none());
    let sp = parse_sampler_params(p);

    if !create_group {
        let ch = explicit_channel.unwrap_or(PbrChannel::BaseColor);
        let mut single_params = p.clone();
        if let Value::Object(m) = &mut single_params {
            m.insert("createGroup".into(), json!(false));
            m.insert("channel".into(), json!(ch.id()));
        }
        return run_destructive_or_smart_filter(s, CMD, "Sample PBR Channel", &single_params, |surf, canvas| {
            sample_pbr_channel_surface(surf, canvas, ch, &sp)
        });
    }

    let src_id = layer_param(s, p)?;
    let (group_id, created_ids) = s.edit("Sample PBR Material", |doc, active| {
        let canvas = doc.bounds();
        if u64::from(canvas.width()) * u64::from(canvas.height()) > MAX_STAGE_PIXELS {
            return Err(bad(CMD, "document dimensions exceed PBR Material Sampler 64 MP working limit"));
        }
        let src_surf = extract_layer_surface(doc, src_id)
            .ok_or_else(|| EngineError::Other("PBR Material Sampler requires a pixel or Smart Object layer".into()))?;
        let fmt = doc.pixel_format();
        let src_fmt = src_surf.format();
        let src_ch = src_fmt.channels();
        let w = canvas.width() as usize;
        let h = canvas.height() as usize;
        let raw = src_surf.read_region(canvas);
        let rgba_in: Vec<[f32; 4]> = raw.chunks_exact(src_ch).map(|px| to_rgba(&src_fmt, px)).collect();

        let maps = sample_pbr_maps_rgba(&rgba_in, w, h, &sp);
        let dst_ch = fmt.channels();
        let mut children = Vec::with_capacity(6);
        let mut ids = Vec::with_capacity(6);

        for pbr_ch in PbrChannel::ALL {
            let map_px = maps.channel(pbr_ch);
            let mut buf = vec![0.0f32; w * h * dst_ch];
            for (q, dst) in map_px.iter().zip(buf.chunks_exact_mut(dst_ch)) {
                from_rgba_into(&fmt, *q, dst);
            }
            let mut ch_surf = Surface::new(fmt);
            ch_surf.write_region(canvas, &buf);
            let layer = Layer::new(pbr_ch.layer_name(), LayerContent::Raster(ch_surf));
            ids.push(layer.id);
            children.push(layer);
        }

        let grp_name = unique_exact_or_numbered(doc, "PBR Material Maps");
        let grp = Layer::new(
            grp_name,
            LayerContent::Group(Group {
                children,
                expanded: true,
                artboard: None,
                frame_stroke: None,
            }),
        );
        let gid = crate::layer_place::insert_new(doc, Some(src_id), grp, crate::layer_place::Place::Auto);
        *active = Some(gid);
        Ok((gid, ids))
    })?;

    Ok(json!({
        "group": group_id.0,
        "layers": created_ids.iter().map(|id| id.0).collect::<Vec<_>>(),
    }))
}

fn run_make_seamless(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "filter.material.makeSeamless";
    if let Some(v) = p.get("feather")
        && !v.is_null()
        && !v.as_f64().is_some_and(f64::is_finite)
    {
        return Err(bad(CMD, "`feather` must be a finite number in 0.05..0.5"));
    }
    let feather = f32_or(p, "feather", f32_or(p, "tileFeather", 0.22)).clamp(0.05, 0.5);
    run_destructive_or_smart_filter(s, CMD, "Make Seamless", p, |surf, canvas| {
        make_seamless_surface(surf, canvas, feather)
    })
}

fn run_procedural_pbr(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "filter.material.proceduralPbr";
    let pp = parse_procedural_params(CMD, p)?;
    run_destructive_or_smart_filter(s, CMD, "Procedural PBR Material", p, |surf, canvas| {
        render_procedural_pbr_surface(surf, canvas, &pp)
    })
}

fn extract_decal_rgba(doc: &Document, layer_id: LayerId, canvas: Rect) -> Option<(Vec<[f32; 4]>, usize, usize)> {
    let surf = extract_layer_surface(doc, layer_id)?;
    let cb = surf.content_bounds().intersect(&canvas);
    let region = if cb.is_empty() { canvas } else { cb };
    if region.is_empty() {
        return None;
    }
    let fmt = surf.format();
    let ch = fmt.channels();
    let w = region.width() as usize;
    let h = region.height() as usize;
    let raw = surf.read_region(region);
    let px: Vec<[f32; 4]> = raw.chunks_exact(ch).map(|p| to_rgba(&fmt, p)).collect();
    Some((px, w, h))
}

fn run_new_3d_stage_layer(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "layer.new3dStageLayer";
    let (params, custom_mesh) = parse_stage3d_params(CMD, p)?;
    let custom_name = p
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string);

    let new_id = s.edit("New 3D Stage Layer", |doc, active| {
        let canvas = doc.bounds();
        let w = canvas.width() as usize;
        let h = canvas.height() as usize;
        if (w as u64).saturating_mul(h as u64) > MAX_STAGE_PIXELS {
            return Err(bad(CMD, "document dimensions exceed 3D Stage 64 MP working limit"));
        }

        let composite = photocraft_compose::render(doc, canvas);
        let decal_buf = if params.material == StageMaterialPreset::ActiveLayerDecal {
            active.and_then(|aid| extract_decal_rgba(doc, aid, canvas))
        } else {
            None
        };

        let inputs = Stage3dInputs {
            backdrop_rgba: Some((&composite.px, w, h)),
            decal_rgba: decal_buf.as_ref().map(|(px, dw, dh)| (px.as_slice(), *dw, *dh)),
            custom_mesh: custom_mesh.as_ref(),
        };

        let fmt = doc.pixel_format();
        let stage_surf = render_stage_3d_surface(fmt, canvas, &params, &inputs);
        let name = custom_name.unwrap_or_else(|| unique_exact_or_numbered(doc, "3D Stage"));
        let layer = Layer::new(name, LayerContent::Raster(stage_surf));
        let bg = crate::background_cmds::background_id(doc);
        let id = crate::layer_place::insert_new(doc, *active, layer, crate::layer_place::Place::Auto);
        crate::background_cmds::keep_background_at_bottom(bg, doc)?;
        *active = Some(id);
        Ok(id)
    })?;

    Ok(json!({
        "layer": new_id.0,
        "shape": params.shape.id(),
        "material": params.material.id(),
    }))
}

// ------------------------------------------------------------------ Command Registry Specs

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec {
            id: "filter.material.samplePbr",
            label: "Material Sampler (Photo to PBR)…",
            menu: &["Filter", "Substance 3D"],
            shortcut: None,
            params: r#"{"createGroup":bool=true,"channel":"baseColor|normal|roughness|metallic|height|ambientOcclusion"?,"delightStrength":0..1=0.75,"makeTileable":bool=false,"tileFeather":0.05..0.5=0.22,"normalStrength":0.1..8=2,"roughnessBase":0.04..1=0.45,"roughnessContrast":0..2=1,"metallicLevel":0..1=0,"aoStrength":0..2=0.85,"layer":id?}"#,
            enabled: has_supported_filter_layer,
            run: run_sample_pbr,
            journal: true,
        },
        CommandSpec {
            id: "filter.material.makeSeamless",
            label: "Make Seamless (Tileable)",
            menu: &["Filter", "Substance 3D"],
            shortcut: None,
            params: r#"{"feather":0.05..0.5=0.22,"layer":id?}"#,
            enabled: has_supported_filter_layer,
            run: run_make_seamless,
            journal: true,
        },
        CommandSpec {
            id: "filter.material.proceduralPbr",
            label: "Procedural PBR Material…",
            menu: &["Filter", "Substance 3D"],
            shortcut: None,
            params: r#"{"preset":"brushedSteel|carbonHexWeave|terrazzoMarble|ceramicTiles|weatheredCopper|wovenFabric"="brushedSteel","scale":0.25..16=4,"roughness":0.04..1=0.32,"normalStrength":0.1..6=1.6,"wear":0..1=0.25,"lightAngle":-180..180=42,"lightElevation":10..85=54,"seed":u32=1337,"layer":id?}"#,
            enabled: has_supported_filter_layer,
            run: run_procedural_pbr,
            journal: true,
        },
        CommandSpec {
            id: "layer.new3dStageLayer",
            label: "New 3D Stage Layer…",
            menu: &["Layer", "New"],
            shortcut: None,
            params: r##"{"shape":"sphere|roundedBox|bottle|can|cylinder|torus|displayCard|customObj"="bottle","sdfOp":"none|smoothUnion|smoothSubtract|smoothIntersect"="none","secondaryShape":"sphere|roundedBox|bottle|can|cylinder|torus|displayCard|customObj"="torus","smoothK":0.02..0.8=0.16,"secondaryOffset":[x,y,z],"material":"mattePlastic|brushedMetal|polishedGold|frostedGlass|ceramic|marble|weatheredCopper|activeLayerDecal"="ceramic","baseColor":[r,g,b]|"#rrggbb","metallic":0..1=0.08,"roughness":0.04..1=0.24,"weathering":0..1=0.15,"rotDeg":[rx,ry,rz],"scale":0.2..2.5=1,"center":[cx,cy],"fovDeg":20..90=42,"matchBackgroundIbl":bool=true,"lightDir":[lx,ly,lz],"lightIntensity":0..10=2.2,"shadowCatcher":0..1=0.65,"groundY":-1.2..1.2=-0.64,"objData":str?,"name":str?}"##,
            enabled: has_supported_doc,
            run: run_new_3d_stage_layer,
            journal: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use photocraft_doc::{LayerContent, LayerId};
    use photocraft_geom::Rect;
    use serde_json::json;

    use crate::Session;

    #[test]
    fn sample_pbr_creates_six_layer_group_across_depths_and_supports_undo_redo() {
        for depth in [8, 16, 32] {
            let mut s = Session::new();
            s.execute("file.new", json!({"width": 32, "height": 32, "depth": depth, "background": "white"})).unwrap();
            s.execute("edit.fill", json!({"contents": "color", "color": "#886644"})).unwrap();

            let res = s
                .execute(
                    "filter.material.samplePbr",
                    json!({"createGroup": true, "delightStrength": 0.85, "makeTileable": true, "normalStrength": 2.5}),
                )
                .unwrap();
            let group_id = LayerId(res["group"].as_u64().unwrap());
            let layer_ids = res["layers"].as_array().unwrap();
            assert_eq!(layer_ids.len(), 6);

            let doc = &s.active().unwrap().doc;
            let grp_layer = doc.layer(group_id).unwrap();
            assert_eq!(grp_layer.name, "PBR Material Maps");
            let LayerContent::Group(grp) = &grp_layer.content else {
                panic!("expected LayerContent::Group");
            };
            let names: Vec<&str> = grp.children.iter().map(|c| c.name.as_str()).collect();
            assert_eq!(names, ["BaseColor", "Normal", "Roughness", "Metallic", "Height", "AO"]);
            for child in &grp.children {
                let surf = child.surface().unwrap();
                assert_eq!(surf.format(), doc.pixel_format());
                assert!(!surf.tile_bounds().is_empty());
            }

            // Undo removes the group, Redo restores it!
            s.execute("edit.undo", json!({})).unwrap();
            assert!(s.active().unwrap().doc.layer(group_id).is_none());
            s.execute("edit.redo", json!({})).unwrap();
            assert!(s.active().unwrap().doc.layer(group_id).is_some());
        }
    }

    #[test]
    fn sample_pbr_single_channel_make_seamless_and_procedural_pbr_respect_selection_and_undo() {
        let mut s = Session::new();
        s.execute("file.new", json!({"width": 32, "height": 32, "depth": 16, "background": "white"})).unwrap();
        s.execute("layer.new.layer", json!({"name": "Tex"})).unwrap();
        s.execute("edit.fill", json!({"contents": "color", "color": "#4477aa"})).unwrap();

        // Restrict edit to an inner 16x16 selection
        s.execute("select.rect", json!({"x": 8, "y": 8, "width": 16, "height": 16})).unwrap();
        s.execute("filter.material.proceduralPbr", json!({"preset": "ceramicTiles", "scale": 3.0})).unwrap();

        let tex_id = s.active().unwrap().active_layer.unwrap();
        let outside_px = s
            .active()
            .unwrap()
            .doc
            .layer(tex_id)
            .unwrap()
            .surface()
            .unwrap()
            .read_region(Rect::from_xywh(0, 0, 1, 1));
        let inside_px = s
            .active()
            .unwrap()
            .doc
            .layer(tex_id)
            .unwrap()
            .surface()
            .unwrap()
            .read_region(Rect::from_xywh(12, 12, 1, 1));
        assert_ne!(outside_px, inside_px, "selection must confine procedural PBR filter");

        s.execute("select.deselect", json!({})).unwrap();
        s.execute("filter.material.makeSeamless", json!({"feather": 0.25})).unwrap();
        s.execute(
            "filter.material.samplePbr",
            json!({"createGroup": false, "channel": "normal", "normalStrength": 3.0}),
        )
        .unwrap();

        // Graceful error on invalid channel / preset / feather (Rule 9)
        assert!(s.execute("filter.material.samplePbr", json!({"channel": 999})).is_err());
        assert!(s.execute("filter.material.samplePbr", json!({"channel": "notAChannel"})).is_err());
        assert!(s.execute("filter.material.proceduralPbr", json!({"preset": "invalidPreset"})).is_err());
        assert!(s.execute("filter.material.makeSeamless", json!({"feather": "bad"})).is_err());
    }

    #[test]
    fn new_3d_stage_layer_wraps_active_decal_and_samples_backdrop_ibl() {
        let mut s = Session::new();
        s.execute("file.new", json!({"width": 48, "height": 48, "background": "white"})).unwrap();
        s.execute("edit.fill", json!({"contents": "color", "color": "#ffcc88"})).unwrap();

        // Create an active decal layer with vibrant red graphic
        let dec = s.execute("layer.new.layer", json!({"name": "Brand Label"})).unwrap();
        let dec_id = LayerId(dec["layer"].as_u64().unwrap());
        s.edit("Paint Decal", |doc, _| {
            doc.layer_mut(dec_id)
                .unwrap()
                .surface_mut()
                .unwrap()
                .fill_rect(Rect::from_xywh(8, 8, 32, 32), &[0.92, 0.12, 0.15, 1.0]);
            Ok(())
        })
        .unwrap();

        let stage = s
            .execute(
                "layer.new3dStageLayer",
                json!({
                    "shape": "bottle",
                    "sdfOp": "smoothUnion",
                    "secondaryShape": "torus",
                    "smoothK": 0.14,
                    "material": "activeLayerDecal",
                    "matchBackgroundIbl": true,
                    "shadowCatcher": 0.75,
                    "weathering": 0.25,
                }),
            )
            .unwrap();
        let stage_id = LayerId(stage["layer"].as_u64().unwrap());
        let doc = &s.active().unwrap().doc;
        let stage_layer = doc.layer(stage_id).unwrap();
        assert_eq!(stage_layer.name, "3D Stage");
        let raw = stage_layer.surface().unwrap().read_region(doc.bounds());
        let px_chunks = raw.as_chunks::<4>().0;
        assert!(px_chunks.iter().any(|px| px[3] > 0.9), "3D Stage layer must contain opaque shaded geometry");
        assert!(px_chunks.iter().any(|px| px[3] > 0.05 && px[3] < 0.85), "3D Stage layer must include soft shadow/AA alpha");

        // Graceful failure on malformed OBJ or unknown shape
        assert!(s.execute("layer.new3dStageLayer", json!({"shape": "notAShape"})).is_err());
        assert!(s.execute("layer.new3dStageLayer", json!({"shape": "customObj", "objData": "v 0 0 0\nf 99 98 97"})).is_err());
    }
}
