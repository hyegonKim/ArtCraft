//! Photoshop 2027 (`28.0.0.20`) interactive dialogs and Properties panel controls:
//! - Interactive 3-lamp 3D Relight / Lightshop dialog (`layer.newRelightLayer`, `filter.render.relight3d`)
//! - `Generative Recompose` aspect-ratio & output-dimension dialog (`edit.generativeRecompose`)
//! - Properties panel sections for:
//!   * `LiveBackdropFx` (`Fireflies`, `WaterRipple`, `Relight3d`, `Restyle` + `Capture as Layer`)
//!   * `Mockup` (`Create Mockup`, `Environmental Lighting`, `Release Mockup`)
//!   * Type 2027 (`Convert to Text Layers`, `Dynamic Text Fit`, `Release Dynamic Fit`, `Copy/Paste Text Attributes`)
//!   * `Agent Skills` (`에이전트 스킬` — 8 one-click workflows)
//!
//! Every document/session mutation goes strictly through `app.run(...)` or `crate::menus::invoke(...)`.

use egui::{Color32, Pos2, RichText, Sense, Stroke, pos2, vec2};
use photocraft_doc::ps2027::{LiveBackdropFx, RelightLamp, RestylePreset};
use photocraft_doc::{LayerContent, LayerId};
use serde_json::{Map, Value, json};

use crate::props_layout::section;
use crate::state::DialogKind;
use crate::theme::{ROW_GAP, Tokens, mono};
use crate::{PhotocraftApp, widgets};

pub const RELIGHT_MARKER: &str = "__relight3d";
pub const RECOMPOSE_MARKER: &str = "__recompose";

pub fn owns_relight(fields: &Map<String, Value>) -> bool {
    fields.contains_key(RELIGHT_MARKER)
}

pub fn owns_recompose(fields: &Map<String, Value>) -> bool {
    fields.contains_key(RECOMPOSE_MARKER)
}

/// Intercepts parameterless invocations of `layer.newRelightLayer`, `filter.render.relight3d`, and
/// `edit.generativeRecompose` to open their dedicated interactive dialogs.
pub fn invoke(app: &mut PhotocraftApp, id: &str, params: &Value) -> Option<Result<Value, String>> {
    let empty = params.as_object().is_none_or(|o| o.is_empty());
    if !empty {
        return None;
    }
    match id {
        "layer.newRelightLayer" | "filter.render.relight3d" => Some(open_relight_dialog(app, id)),
        "edit.generativeRecompose" => Some(open_recompose_dialog(app)),
        _ => crate::substance3d_ui::invoke(app, id, params).or_else(|| crate::fresco_ui::invoke(app, id, params)),
    }
}

fn default_lamps_json() -> Value {
    let l1 = RelightLamp::default();
    let l2 = RelightLamp { enabled: false, x: 0.72, y: 0.35, z: 0.30, intensity: 0.85, color: [0.55, 0.78, 1.0], spread: 0.50 };
    let l3 = RelightLamp { enabled: false, x: 0.50, y: 0.78, z: 0.28, intensity: 0.75, color: [1.0, 0.55, 0.75], spread: 0.45 };
    json!([l1, l2, l3])
}

pub fn open_relight_dialog(app: &mut PhotocraftApp, cmd: &str) -> Result<Value, String> {
    if app.session.active().is_none() {
        return Err("no document open".into());
    }
    let mut fields = Map::new();
    fields.insert(RELIGHT_MARKER.into(), Value::Bool(true));
    fields.insert("__command".into(), json!(cmd));
    let label = if cmd == "layer.newRelightLayer" { "New Relight Layer…" } else { "Relight…" };
    fields.insert("__label".into(), json!(label));
    fields.insert("activeLamp".into(), json!(0));
    fields.insert("ambient".into(), json!(0.35));
    fields.insert("surfaceRelief".into(), json!(1.0));
    fields.insert("lamps".into(), default_lamps_json());
    let dialog = app.ui.open_dialog(DialogKind::Command, fields);
    Ok(json!({ "dialog": dialog }))
}

pub fn open_recompose_dialog(app: &mut PhotocraftApp) -> Result<Value, String> {
    let (w, h) = app.session.active().map(|st| (st.doc.size.width, st.doc.size.height)).ok_or_else(|| "no document open".to_string())?;
    let mut fields = Map::new();
    fields.insert(RECOMPOSE_MARKER.into(), Value::Bool(true));
    fields.insert("__command".into(), json!("edit.generativeRecompose"));
    fields.insert("__label".into(), json!("Generative Recompose…"));
    fields.insert("aspectRatio".into(), json!("16:9"));
    let target_h = ((u64::from(w) * 9) / 16).max(1) as u32;
    fields.insert("width".into(), json!(w));
    fields.insert("height".into(), json!(target_h.max(h / 2).max(1)));
    fields.insert("prompt".into(), json!(""));
    let dialog = app.ui.open_dialog(DialogKind::Command, fields);
    Ok(json!({ "dialog": dialog }))
}

fn lamps_from_fields(fields: &Map<String, Value>) -> [RelightLamp; 3] {
    let mut out =
        [RelightLamp::default(), RelightLamp { enabled: false, x: 0.72, ..Default::default() }, RelightLamp { enabled: false, y: 0.78, ..Default::default() }];
    if let Some(arr) = fields.get("lamps").and_then(Value::as_array) {
        for (i, v) in arr.iter().take(3).enumerate() {
            if let Ok(parsed) = serde_json::from_value::<RelightLamp>(v.clone()) {
                out[i] = parsed;
            }
        }
    }
    out
}

/// Interactive 3-lamp 2D X/Y pad + per-lamp Z/Intensity/Spread/Color controls.
pub fn relight_body(ui: &mut egui::Ui, fields: &mut Map<String, Value>) {
    let t = Tokens::get(ui.ctx());
    let mut lamps = lamps_from_fields(fields);
    let mut sel = fields.get("activeLamp").and_then(Value::as_u64).unwrap_or(0).min(2) as usize;
    let mut ambient = fields.get("ambient").and_then(Value::as_f64).unwrap_or(0.35) as f32;
    let mut relief = fields.get("surfaceRelief").and_then(Value::as_f64).unwrap_or(1.0) as f32;
    let mut changed = false;

    // Interactive 2D X/Y light stage pad
    let pad_size = vec2(260.0, 156.0);
    let (pad_rect, pad_resp) = ui.allocate_exact_size(pad_size, Sense::click_and_drag());
    ui.painter().rect_filled(pad_rect, 6.0, Color32::from_gray(28));
    ui.painter().rect_stroke(pad_rect, 6.0, Stroke::new(1.0, t.card_border), egui::StrokeKind::Inside);
    // Crosshairs
    ui.painter().line_segment(
        [pos2(pad_rect.center().x, pad_rect.top() + 4.0), pos2(pad_rect.center().x, pad_rect.bottom() - 4.0)],
        Stroke::new(1.0, Color32::from_white_alpha(26)),
    );
    ui.painter().line_segment(
        [pos2(pad_rect.left() + 4.0, pad_rect.center().y), pos2(pad_rect.right() - 4.0, pad_rect.center().y)],
        Stroke::new(1.0, Color32::from_white_alpha(26)),
    );

    if (pad_resp.clicked() || pad_resp.dragged())
        && let Some(pos) = pad_resp.interact_pointer_pos()
    {
        if pad_resp.clicked() {
            // Select nearest enabled/visible lamp if clicked near its handle
            let mut best = (sel, f32::MAX);
            for (i, l) in lamps.iter().enumerate() {
                let lp = pos2(pad_rect.left() + l.x.clamp(0.0, 1.0) * pad_rect.width(), pad_rect.top() + l.y.clamp(0.0, 1.0) * pad_rect.height());
                let dist = lp.distance(pos);
                if dist < 16.0 && dist < best.1 {
                    best = (i, dist);
                }
            }
            sel = best.0;
            fields.insert("activeLamp".into(), json!(sel));
        }
        lamps[sel].x = ((pos.x - pad_rect.left()) / pad_rect.width()).clamp(0.0, 1.0);
        lamps[sel].y = ((pos.y - pad_rect.top()) / pad_rect.height()).clamp(0.0, 1.0);
        lamps[sel].enabled = true;
        changed = true;
    }

    for (i, l) in lamps.iter().enumerate() {
        let lp: Pos2 = pos2(pad_rect.left() + l.x.clamp(0.0, 1.0) * pad_rect.width(), pad_rect.top() + l.y.clamp(0.0, 1.0) * pad_rect.height());
        let c = Color32::from_rgb(
            (l.color[0].clamp(0.0, 1.0) * 255.0) as u8,
            (l.color[1].clamp(0.0, 1.0) * 255.0) as u8,
            (l.color[2].clamp(0.0, 1.0) * 255.0) as u8,
        );
        let radius = if i == sel { 8.0 } else { 6.0 };
        let fill = if l.enabled { c } else { Color32::from_gray(90) };
        ui.painter().circle_filled(lp, radius, fill);
        ui.painter().circle_stroke(
            lp,
            radius + 1.5,
            Stroke::new(if i == sel { 2.0 } else { 1.0 }, if i == sel { Color32::WHITE } else { Color32::from_black_alpha(180) }),
        );
    }

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        for (i, name) in [(0, tl!("Light 1")), (1, tl!("Light 2")), (2, tl!("Light 3"))] {
            if ui.selectable_label(sel == i, name).clicked() {
                sel = i;
                fields.insert("activeLamp".into(), json!(sel));
            }
        }
    });

    ui.add_space(4.0);
    let lamp = &mut lamps[sel];
    if widgets::checkbox(ui, &mut lamp.enabled, tl!("Enabled")).changed() {
        changed = true;
    }

    egui::Grid::new("relight-lamp-grid").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
        ui.label(RichText::new("X").color(t.text_dim));
        let mut x_pct = lamp.x * 100.0;
        if widgets::value_field(ui, &mut x_pct, 0.0..=100.0, "%", 72.0).changed() {
            lamp.x = (x_pct / 100.0).clamp(0.0, 1.0);
            changed = true;
        }
        ui.end_row();

        ui.label(RichText::new("Y").color(t.text_dim));
        let mut y_pct = lamp.y * 100.0;
        if widgets::value_field(ui, &mut y_pct, 0.0..=100.0, "%", 72.0).changed() {
            lamp.y = (y_pct / 100.0).clamp(0.0, 1.0);
            changed = true;
        }
        ui.end_row();

        ui.label(RichText::new(tl!("Z (Depth)")).color(t.text_dim));
        let mut z_pct = lamp.z * 100.0;
        if widgets::value_field(ui, &mut z_pct, 2.0..=100.0, "%", 72.0).changed() {
            lamp.z = (z_pct / 100.0).clamp(0.02, 1.0);
            changed = true;
        }
        ui.end_row();

        ui.label(RichText::new(tl!("Intensity")).color(t.text_dim));
        let mut int_pct = lamp.intensity * 100.0;
        if widgets::value_field(ui, &mut int_pct, 0.0..=300.0, "%", 72.0).changed() {
            lamp.intensity = (int_pct / 100.0).clamp(0.0, 3.0);
            changed = true;
        }
        ui.end_row();

        ui.label(RichText::new(tl!("Spread")).color(t.text_dim));
        let mut spr_pct = lamp.spread * 100.0;
        if widgets::value_field(ui, &mut spr_pct, 5.0..=100.0, "%", 72.0).changed() {
            lamp.spread = (spr_pct / 100.0).clamp(0.05, 1.0);
            changed = true;
        }
        ui.end_row();

        ui.label(RichText::new(tl!("Color")).color(t.text_dim));
        if ui.color_edit_button_rgb(&mut lamp.color).changed() {
            changed = true;
        }
        ui.end_row();

        ui.label(RichText::new(tl!("Ambient")).color(t.text_dim));
        let mut amb_pct = ambient * 100.0;
        if widgets::value_field(ui, &mut amb_pct, 0.0..=100.0, "%", 72.0).changed() {
            ambient = (amb_pct / 100.0).clamp(0.0, 1.0);
            fields.insert("ambient".into(), json!(ambient));
        }
        ui.end_row();

        ui.label(RichText::new(tl!("Surface Relief")).color(t.text_dim));
        let mut rel_pct = relief * 100.0;
        if widgets::value_field(ui, &mut rel_pct, 0.0..=300.0, "%", 72.0).changed() {
            relief = (rel_pct / 100.0).clamp(0.0, 3.0);
            fields.insert("surfaceRelief".into(), json!(relief));
        }
        ui.end_row();
    });

    if changed {
        fields.insert("lamps".into(), json!(lamps));
    }
}

pub fn confirm_relight(app: &mut PhotocraftApp, fields: &Map<String, Value>) -> Result<Value, String> {
    let cmd = fields.get("__command").and_then(Value::as_str).unwrap_or("layer.newRelightLayer").to_string();
    let lamps = lamps_from_fields(fields);
    let ambient = fields.get("ambient").and_then(Value::as_f64).unwrap_or(0.35);
    let relief = fields.get("surfaceRelief").and_then(Value::as_f64).unwrap_or(1.0);
    app.run(
        &cmd,
        json!({
            "lamps": lamps,
            "ambient": ambient,
            "surfaceRelief": relief,
        }),
    )
}

/// Renders the `Edit › Generative Recompose…` dialog body.
pub fn recompose_body(app: &mut PhotocraftApp, ui: &mut egui::Ui, fields: &mut Map<String, Value>) {
    let t = Tokens::get(ui.ctx());
    let (doc_w, doc_h) = app.session.active().map_or((1920, 1080), |st| (st.doc.size.width, st.doc.size.height));
    let mut ratio = fields.get("aspectRatio").and_then(Value::as_str).unwrap_or("16:9").to_string();
    let mut width = fields.get("width").and_then(Value::as_f64).unwrap_or(f64::from(doc_w)).clamp(1.0, 16384.0) as f32;
    let mut height = fields.get("height").and_then(Value::as_f64).unwrap_or(f64::from(doc_h)).clamp(1.0, 16384.0) as f32;
    let mut prompt = fields.get("prompt").and_then(Value::as_str).unwrap_or("").to_string();

    ui.horizontal(|ui| {
        ui.label(RichText::new(tl!("Current")).color(t.text_dim));
        ui.label(RichText::new(format!("{doc_w} × {doc_h} px")).font(mono(12.0)).color(t.text));
    });
    ui.add_space(6.0);

    let ratios = [
        ("16:9".to_string(), "16:9 (Widescreen)"),
        ("9:16".to_string(), "9:16 (Vertical / Reels)"),
        ("1:1".to_string(), "1:1 (Square)"),
        ("4:5".to_string(), "4:5 (Portrait)"),
        ("4:3".to_string(), "4:3 (Standard)"),
        ("3:2".to_string(), "3:2 (Photo)"),
        ("21:9".to_string(), "21:9 (Cinematic)"),
        ("custom".to_string(), "Custom"),
    ];

    egui::Grid::new("recompose-dialog-grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        ui.label(RichText::new(tl!("Aspect Ratio")).color(t.text_dim));
        if widgets::dropdown(ui, "recompose-ratio", tl!("Aspect Ratio"), &mut ratio, &ratios, 175.0) {
            if let Some((num, den)) = ratio.split_once(':').and_then(|(a, b)| Some((a.parse::<f32>().ok()?, b.parse::<f32>().ok()?)))
                && den > 0.0
            {
                height = (width * den / num).round().clamp(1.0, 16384.0);
                fields.insert("height".into(), json!(height as u32));
            }
            fields.insert("aspectRatio".into(), json!(ratio));
        }
        ui.end_row();

        ui.label(RichText::new(tl!("Output Width")).color(t.text_dim));
        if widgets::value_field(ui, &mut width, 1.0..=16384.0, "px", 96.0).changed() {
            fields.insert("width".into(), json!(width.round() as u32));
            fields.insert("aspectRatio".into(), json!("custom"));
        }
        ui.end_row();

        ui.label(RichText::new(tl!("Output Height")).color(t.text_dim));
        if widgets::value_field(ui, &mut height, 1.0..=16384.0, "px", 96.0).changed() {
            fields.insert("height".into(), json!(height.round() as u32));
            fields.insert("aspectRatio".into(), json!("custom"));
        }
        ui.end_row();

        ui.label(RichText::new(tl!("Prompt")).color(t.text_dim));
        if ui.text_edit_singleline(&mut prompt).changed() {
            fields.insert("prompt".into(), json!(prompt));
        }
        ui.end_row();
    });
}

pub fn confirm_recompose(app: &mut PhotocraftApp, fields: &Map<String, Value>) -> Result<Value, String> {
    let width = fields.get("width").and_then(Value::as_u64).unwrap_or(1920).max(1) as u32;
    let height = fields.get("height").and_then(Value::as_u64).unwrap_or(1080).max(1) as u32;
    let ratio = fields.get("aspectRatio").and_then(Value::as_str).unwrap_or("custom");
    let prompt = fields.get("prompt").and_then(Value::as_str).unwrap_or("");
    let mut p = json!({
        "width": width,
        "height": height,
        "prompt": prompt,
    });
    if ratio != "custom" {
        p["aspectRatio"] = json!(ratio);
    }
    app.run("edit.generativeRecompose", p)
}

// ------------------------------------------------------------------ Properties Panel Sections

const AGENT_SKILLS: &[(&str, &str)] = &[
    ("agent.skill.cinematicGrade", "Cinematic Grade"),
    ("agent.skill.enhanceAndResize", "Enhance and Resize"),
    ("agent.skill.perfectPortrait", "Perfect Portrait"),
    ("agent.skill.productStudioShot", "Product Studio Shot"),
    ("agent.skill.removeBackground", "Remove Background"),
    ("agent.skill.retouchPortrait", "Retouch Portrait"),
    ("agent.skill.socialMediaHero", "Social Media Hero"),
    ("agent.skill.subjectSpotlight", "Subject Spotlight"),
];

/// Document-level Photoshop 2027 Properties sections (shown when the Background or document root is selected).
pub fn document_properties(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    if let Some(bg_id) = app.session.active().and_then(|st| st.active_layer) {
        live_fx_section(app, ui, bg_id);
    }
    crate::substance3d_ui::properties_section(app, ui);
    crate::fresco_ui::properties_section(app, ui, None);
    agent_skills_section(app, ui);
}

/// Layer-level Photoshop 2027 Properties sections for active layer `layer_id`.
pub fn layer_properties(app: &mut PhotocraftApp, ui: &mut egui::Ui, layer_id: LayerId) {
    live_fx_section(app, ui, layer_id);
    mockup_section(app, ui, layer_id);
    type_2027_section(app, ui, layer_id);
    let is_raster = app.session.active().and_then(|st| st.doc.layer(layer_id)).is_some_and(|l| matches!(l.content, LayerContent::Raster(_)));
    if is_raster {
        crate::substance3d_ui::properties_section(app, ui);
        crate::fresco_ui::properties_section(app, ui, Some(layer_id));
    }
}

fn live_fx_section(app: &mut PhotocraftApp, ui: &mut egui::Ui, layer_id: LayerId) {
    let Some(fx) = app.session.active().and_then(|st| st.doc.layer(layer_id)).and_then(|l| l.live_fx.clone()) else {
        return;
    };
    let t = Tokens::get(ui.ctx());
    if !section(ui, "ps2027-live-fx", tl!("Live Effect")) {
        return;
    }
    let mut next_params: Option<Value> = None;
    let mut capture_restyle = false;
    let mut clear_fx = false;

    match fx {
        LiveBackdropFx::Fireflies { mut quantity, mut depth_variation, mut blurriness, mut color, mut progress, seed } => {
            egui::Grid::new(("fx-fireflies", layer_id.0)).num_columns(2).spacing([8.0, 5.0]).show(ui, |ui| {
                ui.label(RichText::new(tl!("Quantity")).color(t.text_dim));
                let mut q = quantity as f32;
                if widgets::value_field(ui, &mut q, 1.0..=500.0, "", 68.0).changed() {
                    quantity = q.round() as u32;
                    next_params = Some(json!({ "layer": layer_id.0, "kind": "fireflies", "quantity": quantity, "depthVariation": depth_variation, "blurriness": blurriness, "color": color, "progress": progress, "seed": seed }));
                }
                ui.end_row();
                ui.label(RichText::new(tl!("Depth Variation")).color(t.text_dim));
                if widgets::value_field(ui, &mut depth_variation, 0.0..=100.0, "%", 68.0).changed() {
                    next_params = Some(json!({ "layer": layer_id.0, "kind": "fireflies", "quantity": quantity, "depthVariation": depth_variation, "blurriness": blurriness, "color": color, "progress": progress, "seed": seed }));
                }
                ui.end_row();
                ui.label(RichText::new(tl!("Blurriness")).color(t.text_dim));
                if widgets::value_field(ui, &mut blurriness, 0.0..=100.0, "%", 68.0).changed() {
                    next_params = Some(json!({ "layer": layer_id.0, "kind": "fireflies", "quantity": quantity, "depthVariation": depth_variation, "blurriness": blurriness, "color": color, "progress": progress, "seed": seed }));
                }
                ui.end_row();
                ui.label(RichText::new(tl!("Progress")).color(t.text_dim));
                if widgets::value_field(ui, &mut progress, 0.0..=100.0, "%", 68.0).changed() {
                    next_params = Some(json!({ "layer": layer_id.0, "kind": "fireflies", "quantity": quantity, "depthVariation": depth_variation, "blurriness": blurriness, "color": color, "progress": progress, "seed": seed }));
                }
                ui.end_row();
                ui.label(RichText::new(tl!("Color")).color(t.text_dim));
                if ui.color_edit_button_rgb(&mut color).changed() {
                    next_params = Some(json!({ "layer": layer_id.0, "kind": "fireflies", "quantity": quantity, "depthVariation": depth_variation, "blurriness": blurriness, "color": color, "progress": progress, "seed": seed }));
                }
                ui.end_row();
            });
        }
        LiveBackdropFx::WaterRipple { mut horizon, mut amplitude, mut frequency, mut progress } => {
            egui::Grid::new(("fx-water-ripple", layer_id.0)).num_columns(2).spacing([8.0, 5.0]).show(ui, |ui| {
                for (label, val_kind) in [(tl!("Horizon"), 0u8), (tl!("Amplitude"), 1), (tl!("Frequency"), 2), (tl!("Progress"), 3)] {
                    ui.label(RichText::new(label).color(t.text_dim));
                    let changed = match val_kind {
                        0 => widgets::value_field(ui, &mut horizon, 5.0..=95.0, "%", 68.0).changed(),
                        1 => widgets::value_field(ui, &mut amplitude, 0.0..=100.0, "%", 68.0).changed(),
                        2 => widgets::value_field(ui, &mut frequency, 1.0..=100.0, "", 68.0).changed(),
                        _ => widgets::value_field(ui, &mut progress, 0.0..=100.0, "%", 68.0).changed(),
                    };
                    if changed {
                        next_params = Some(json!({
                            "layer": layer_id.0,
                            "kind": "waterRipple",
                            "horizon": horizon,
                            "amplitude": amplitude,
                            "frequency": frequency,
                            "progress": progress,
                        }));
                    }
                    ui.end_row();
                }
            });
        }
        LiveBackdropFx::Relight3d { mut lamps, mut ambient, mut surface_relief } => {
            let sel_key = egui::Id::unique(("props-relight-sel", layer_id.0));
            let mut sel = ui.data(|d| d.get_temp::<usize>(sel_key)).unwrap_or(0).min(2);
            ui.horizontal(|ui| {
                for (i, lbl) in [(0, tl!("Light 1")), (1, tl!("Light 2")), (2, tl!("Light 3"))] {
                    if ui.selectable_label(sel == i, lbl).clicked() {
                        sel = i;
                        ui.data_mut(|d| d.insert_temp(sel_key, sel));
                    }
                }
            });
            let mut changed = widgets::checkbox(ui, &mut lamps[sel].enabled, tl!("Enabled")).changed();
            egui::Grid::new(("fx-relight3d", layer_id.0)).num_columns(2).spacing([8.0, 5.0]).show(ui, |ui| {
                let mut x_pct = lamps[sel].x * 100.0;
                ui.label(RichText::new("X").color(t.text_dim));
                if widgets::value_field(ui, &mut x_pct, 0.0..=100.0, "%", 68.0).changed() {
                    lamps[sel].x = (x_pct / 100.0).clamp(0.0, 1.0);
                    changed = true;
                }
                ui.end_row();
                let mut y_pct = lamps[sel].y * 100.0;
                ui.label(RichText::new("Y").color(t.text_dim));
                if widgets::value_field(ui, &mut y_pct, 0.0..=100.0, "%", 68.0).changed() {
                    lamps[sel].y = (y_pct / 100.0).clamp(0.0, 1.0);
                    changed = true;
                }
                ui.end_row();
                let mut int_pct = lamps[sel].intensity * 100.0;
                ui.label(RichText::new(tl!("Intensity")).color(t.text_dim));
                if widgets::value_field(ui, &mut int_pct, 0.0..=300.0, "%", 68.0).changed() {
                    lamps[sel].intensity = (int_pct / 100.0).clamp(0.0, 3.0);
                    changed = true;
                }
                ui.end_row();
                ui.label(RichText::new(tl!("Color")).color(t.text_dim));
                if ui.color_edit_button_rgb(&mut lamps[sel].color).changed() {
                    changed = true;
                }
                ui.end_row();
                let mut amb_pct = ambient * 100.0;
                ui.label(RichText::new(tl!("Ambient")).color(t.text_dim));
                if widgets::value_field(ui, &mut amb_pct, 0.0..=100.0, "%", 68.0).changed() {
                    ambient = (amb_pct / 100.0).clamp(0.0, 1.0);
                    changed = true;
                }
                ui.end_row();
                let mut rel_pct = surface_relief * 100.0;
                ui.label(RichText::new(tl!("Surface Relief")).color(t.text_dim));
                if widgets::value_field(ui, &mut rel_pct, 0.0..=300.0, "%", 68.0).changed() {
                    surface_relief = (rel_pct / 100.0).clamp(0.0, 3.0);
                    changed = true;
                }
                ui.end_row();
            });
            if changed {
                next_params = Some(json!({
                    "layer": layer_id.0,
                    "kind": "relight3d",
                    "lamps": lamps,
                    "ambient": ambient,
                    "surfaceRelief": surface_relief,
                }));
            }
        }
        LiveBackdropFx::Restyle { preset, mut intensity, prompt } => {
            let mut preset_id = preset.id().to_string();
            let presets: Vec<(String, &str)> = RestylePreset::ALL.iter().map(|p| (p.id().to_string(), tl!(p.label()))).collect();
            egui::Grid::new(("fx-restyle", layer_id.0)).num_columns(2).spacing([8.0, 5.0]).show(ui, |ui| {
                ui.label(RichText::new(tl!("Style")).color(t.text_dim));
                let c_p = widgets::dropdown(ui, "props-restyle-preset", tl!("Style"), &mut preset_id, &presets, 130.0);
                ui.end_row();
                ui.label(RichText::new(tl!("Intensity")).color(t.text_dim));
                let c_i = widgets::value_field(ui, &mut intensity, 0.0..=100.0, "%", 68.0).changed();
                ui.end_row();
                if c_p || c_i {
                    next_params = Some(json!({
                        "layer": layer_id.0,
                        "kind": "restyle",
                        "style": preset_id,
                        "intensity": intensity,
                        "prompt": prompt,
                    }));
                }
            });
            ui.add_space(4.0);
            if widgets::secondary_button(ui, tl!("Capture as Layer"), ui.available_width()).clicked() {
                capture_restyle = true;
            }
        }
    }

    ui.add_space(4.0);
    if widgets::secondary_button(ui, tl!("Clear Live Effect"), ui.available_width()).clicked() {
        clear_fx = true;
    }
    ui.add_space(ROW_GAP);

    if let Some(p) = next_params {
        let _ = app.run("layer.liveFx.setParams", p);
    }
    if capture_restyle {
        let _ = app.run("layer.restyle.captureAsLayer", json!({ "layer": layer_id.0 }));
    }
    if clear_fx {
        let _ = app.run("layer.liveFx.clear", json!({ "layer": layer_id.0 }));
    }
}

fn mockup_section(app: &mut PhotocraftApp, ui: &mut egui::Ui, layer_id: LayerId) {
    let mockup_info = app.session.active().and_then(|st| {
        if let Some(l) = st.doc.layer(layer_id)
            && let Some(lighting) = l.mockup_lighting
        {
            return Some((layer_id, lighting));
        }
        // Also find parent group if active layer is inside a Mockup group
        for g in &st.doc.layers {
            if let Some(lighting) = g.mockup_lighting
                && let LayerContent::Group(grp) = &g.content
                && grp.children.iter().any(|c| c.id == layer_id)
            {
                return Some((g.id, lighting));
            }
        }
        None
    });
    let Some((group_id, lighting)) = mockup_info else {
        return;
    };
    let t = Tokens::get(ui.ctx());
    if !section(ui, "ps2027-mockup", tl!("Mockup")) {
        return;
    }
    let mut pct = (lighting * 100.0).clamp(0.0, 100.0);
    let mut updated = None;
    ui.horizontal(|ui| {
        ui.label(RichText::new(tl!("Environmental Lighting")).color(t.text_dim));
        if widgets::value_field(ui, &mut pct, 0.0..=100.0, "%", 68.0).changed() {
            updated = Some(pct / 100.0);
        }
    });
    ui.add_space(4.0);
    let release = widgets::secondary_button(ui, tl!("Release Mockup"), ui.available_width()).clicked();
    ui.add_space(ROW_GAP);

    if let Some(val) = updated {
        let _ = app.run("layer.mockup.setLighting", json!({ "layer": group_id.0, "lighting": val }));
    }
    if release {
        let _ = app.run("layer.mockup.release", json!({ "layer": group_id.0 }));
    }
}

fn type_2027_section(app: &mut PhotocraftApp, ui: &mut egui::Ui, layer_id: LayerId) {
    let Some((dynamic_fit, is_box, has_clip)) = app.session.active().and_then(|st| {
        let l = st.doc.layer(layer_id)?;
        let LayerContent::Text(tl) = &l.content else { return None };
        let is_box = matches!(tl.shape, photocraft_doc::text::TextShape::Box { .. });
        Some((tl.dynamic_fit, is_box, app.session.text_attr_clipboard.is_some()))
    }) else {
        return;
    };
    if !is_box || !dynamic_fit {
        return;
    }
    if !section(ui, "ps2027-type", tl!("Dynamic Text Fit")) {
        return;
    }
    let mut fit = dynamic_fit;
    let mut cmd_to_run: Option<(&'static str, Value)> = None;
    if widgets::checkbox(ui, &mut fit, tl!("Dynamic Text Fit")).changed() {
        cmd_to_run = Some(("type.setDynamicFit", json!({ "layer": layer_id.0, "enabled": fit })));
    }
    if fit && widgets::secondary_button(ui, tl!("Release Dynamic Fit"), ui.available_width()).clicked() {
        cmd_to_run = Some(("type.releaseDynamicFit", json!({ "layer": layer_id.0 })));
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if widgets::secondary_button(ui, tl!("Copy Text Attributes"), 0.0).clicked() {
            cmd_to_run = Some(("edit.copyTextAttributes", json!({ "layer": layer_id.0 })));
        }
        if ui.add_enabled_ui(has_clip, |ui| widgets::secondary_button(ui, tl!("Paste Text Attributes"), 0.0).clicked()).inner {
            cmd_to_run = Some(("edit.pasteTextAttributes", json!({ "layer": layer_id.0 })));
        }
    });
    ui.add_space(ROW_GAP);

    if let Some((cmd, p)) = cmd_to_run
        && let Err(e) = app.run(cmd, p)
    {
        app.ui.status = e;
        app.ui.status_error = true;
    }
}

fn agent_skills_section(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    if !section(ui, "ps2027-agent-skills", tl!("Agent Skills")) {
        return;
    }
    let mut picked: Option<&'static str> = None;
    ui.horizontal_wrapped(|ui| {
        for &(cmd, label) in AGENT_SKILLS {
            let enabled = app.session.is_enabled(cmd);
            if ui.add_enabled_ui(enabled, |ui| widgets::secondary_button(ui, tl!(label), 0.0).clicked()).inner {
                picked = Some(cmd);
            }
        }
    });
    ui.add_space(ROW_GAP);
    if let Some(cmd) = picked
        && let Err(e) = app.run(cmd, json!({}))
    {
        app.ui.status = e;
        app.ui.status_error = true;
    }
}
