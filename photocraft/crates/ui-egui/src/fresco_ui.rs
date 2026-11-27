//! Adobe Fresco (`FRSC v5.7.1`) Live Watercolor interactive dialog and Properties panel controls:
//! - `fresco.applyLiveWatercolor`: **Live Watercolor…** capillary diffusion dialog
//! - `fresco.dryLayer`: **Dry Watercolor Layer** one-click action & wetness status badge
//! - Strictly dispatches through `crate::menus::invoke` / `app.run` (`panel-commands` compliant).

use egui::RichText;
use photocraft_doc::LayerId;
use photocraft_engine::fresco_cmds::layer_wetness_status;
use photocraft_paint::fresco_watercolor::FrescoWatercolorPreset;
use serde_json::{Map, Value, json};

use crate::props_layout::section;
use crate::state::DialogKind;
use crate::theme::{ROW_GAP, Tokens, mono};
use crate::{PhotocraftApp, widgets};

pub const FRESCO_WATERCOLOR_MARKER: &str = "__frescoWatercolor";

pub fn owns_watercolor(fields: &Map<String, Value>) -> bool {
    fields.contains_key(FRESCO_WATERCOLOR_MARKER)
}

/// Intercepts parameterless invocations of `fresco.applyLiveWatercolor` to open the dialog.
pub fn invoke(app: &mut PhotocraftApp, id: &str, params: &Value) -> Option<Result<Value, String>> {
    let empty = params.as_object().is_none_or(|o| o.is_empty());
    if !empty || id != "fresco.applyLiveWatercolor" {
        return None;
    }
    Some(open_watercolor_dialog(app))
}

pub fn open_watercolor_dialog(app: &mut PhotocraftApp) -> Result<Value, String> {
    if let Some(why) = app.session.disabled_reason_with("fresco.applyLiveWatercolor", &json!({})) {
        return Err(why);
    }
    let mut fields = Map::new();
    fields.insert(FRESCO_WATERCOLOR_MARKER.into(), Value::Bool(true));
    fields.insert("__command".into(), json!("fresco.applyLiveWatercolor"));
    fields.insert("__label".into(), json!("Live Watercolor…"));
    fields.insert("preset".into(), json!("washSoft"));
    fields.insert("waterFlow".into(), json!(0.65));
    fields.insert("diffusionSteps".into(), json!(8));
    fields.insert("advectionStrength".into(), json!(0.85));
    fields.insert("cauliflowerEdge".into(), json!(0.55));
    fields.insert("paperGrain".into(), json!(0.35));
    fields.insert("evaporation".into(), json!(0.12));
    let dialog = app.ui.open_dialog(DialogKind::Command, fields);
    Ok(json!({ "dialog": dialog }))
}

fn field_f32(fields: &Map<String, Value>, k: &str, d: f32) -> f32 {
    fields.get(k).and_then(Value::as_f64).filter(|v| v.is_finite()).map_or(d, |v| v as f32)
}

fn field_str(fields: &Map<String, Value>, k: &str, d: &str) -> String {
    fields.get(k).and_then(Value::as_str).unwrap_or(d).to_string()
}

fn pct_row(ui: &mut egui::Ui, label: &str, fields: &mut Map<String, Value>, key: &str, def: f32, max_pct: f32) {
    let t = Tokens::get(ui.ctx());
    ui.label(RichText::new(label).color(t.text_dim));
    let mut pct = field_f32(fields, key, def) * 100.0;
    if widgets::value_field(ui, &mut pct, 0.0..=max_pct, "%", 82.0).changed() {
        fields.insert(key.into(), json!((pct / 100.0).clamp(0.0, max_pct / 100.0)));
    }
    ui.end_row();
}

pub fn watercolor_body(ui: &mut egui::Ui, fields: &mut Map<String, Value>) {
    let t = Tokens::get(ui.ctx());
    let mut preset = field_str(fields, "preset", "washSoft");
    let preset_opts: Vec<(String, &str)> = FrescoWatercolorPreset::ALL.iter().map(|p| (p.id().into(), tl!(p.label()))).collect();

    egui::Grid::new("fresco-watercolor-dialog-grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        ui.label(RichText::new(tl!("Watercolor Preset")).color(t.text_dim));
        if widgets::dropdown(ui, "fwc-preset", tl!("Watercolor Preset"), &mut preset, &preset_opts, 185.0) {
            fields.insert("preset".into(), json!(preset));
        }
        ui.end_row();

        pct_row(ui, tl!("Water Flow"), fields, "waterFlow", 0.65, 100.0);

        ui.label(RichText::new(tl!("Diffusion Steps")).color(t.text_dim));
        let mut steps = field_f32(fields, "diffusionSteps", 8.0);
        if widgets::value_field(ui, &mut steps, 1.0..=24.0, "", 82.0).changed() {
            fields.insert("diffusionSteps".into(), json!(steps.round().clamp(1.0, 24.0) as u32));
        }
        ui.end_row();

        pct_row(ui, tl!("Pigment Advection"), fields, "advectionStrength", 0.85, 200.0);
        pct_row(ui, tl!("Cauliflower Edge"), fields, "cauliflowerEdge", 0.55, 150.0);
        pct_row(ui, tl!("Paper Grain"), fields, "paperGrain", 0.35, 100.0);
        pct_row(ui, tl!("Evaporation"), fields, "evaporation", 0.12, 100.0);
    });
}

pub fn confirm_watercolor(app: &mut PhotocraftApp, fields: &Map<String, Value>) -> Result<Value, String> {
    app.run(
        "fresco.applyLiveWatercolor",
        json!({
            "preset": field_str(fields, "preset", "washSoft"),
            "waterFlow": field_f32(fields, "waterFlow", 0.65).clamp(0.0, 1.0),
            "diffusionSteps": field_f32(fields, "diffusionSteps", 8.0).round().clamp(1.0, 24.0) as u32,
            "advectionStrength": field_f32(fields, "advectionStrength", 0.85).clamp(0.0, 2.0),
            "cauliflowerEdge": field_f32(fields, "cauliflowerEdge", 0.55).clamp(0.0, 1.5),
            "paperGrain": field_f32(fields, "paperGrain", 0.35).clamp(0.0, 1.0),
            "evaporation": field_f32(fields, "evaporation", 0.12).clamp(0.0, 1.0),
        }),
    )
}

/// Properties panel section for Fresco Live Watercolor & Dry Layer controls.
pub fn properties_section(app: &mut PhotocraftApp, ui: &mut egui::Ui, layer_id: Option<LayerId>) {
    if !section(ui, "fresco-live-watercolor", tl!("Fresco Live Watercolor")) {
        return;
    }
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let target_id = layer_id.or_else(|| app.session.active().and_then(|st| st.active_layer));
    let (wet_frac, max_wet) = target_id.map_or((0.0, 0.0), |id| layer_wetness_status(&app.session, id));

    ui.horizontal(|ui| {
        ui.label(RichText::new(tl!("Canvas Wetness")).color(t.text_dim));
        let status_txt = if max_wet > 0.005 { format!("{:.0}% ({:.0}% {})", max_wet * 100.0, wet_frac * 100.0, tl!("Wet")) } else { tl!("Dry").to_string() };
        ui.label(RichText::new(status_txt).font(mono(11.5)).color(t.text));
    });
    ui.add_space(4.0);

    let mut action: Option<(&'static str, Value)> = None;
    ui.horizontal_wrapped(|ui| {
        let wc_enabled = app.session.is_enabled("fresco.applyLiveWatercolor");
        if ui.add_enabled_ui(wc_enabled, |ui| widgets::secondary_button(ui, tl!("Live Watercolor…"), 0.0).clicked()).inner {
            action = Some(("fresco.applyLiveWatercolor", json!({})));
        }
        let dry_enabled = app.session.is_enabled("fresco.dryLayer") && max_wet > 0.005;
        if ui.add_enabled_ui(dry_enabled, |ui| widgets::secondary_button(ui, tl!("Dry Watercolor Layer"), 0.0).clicked()).inner {
            action = Some(("fresco.dryLayer", json!({})));
        }
    });
    ui.add_space(ROW_GAP);

    if let Some((cmd, params)) = action
        && let Err(e) = crate::menus::invoke(app, &ctx, cmd, params)
    {
        app.ui.status = e;
        app.ui.status_error = true;
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{PhotocraftApp, Services};

    #[test]
    fn parameterless_fresco_watercolor_opens_dialog_and_confirm_runs_diffusion_and_dry() {
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Services::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 32, "height": 32, "background": "white"})).unwrap();
        app.run("layer.new.layer", json!({"name": "Fresco Wash"})).unwrap();
        app.run("edit.fill", json!({"contents": "color", "color": "#2277cc"})).unwrap();
        app.sync_views();

        let r = crate::menus::invoke(&mut app, &ctx, "fresco.applyLiveWatercolor", json!({})).unwrap();
        let dlg_id = r["dialog"].as_u64().unwrap();
        assert_eq!(app.ui.dialogs.len(), 1);
        assert!(super::owns_watercolor(&app.ui.dialogs[0].fields));
        if let Some(d) = app.ui.dialog_mut(dlg_id) {
            d.fields.insert("preset".into(), json!("wetSpatter"));
            d.fields.insert("waterFlow".into(), json!(0.85));
        }
        crate::dialogs::confirm(&mut app, dlg_id).unwrap();
        assert!(app.ui.dialogs.is_empty());

        let lid = app.session.active().unwrap().active_layer.unwrap();
        assert!(photocraft_engine::fresco_cmds::layer_wetness_status(&app.session, lid).1 > 0.1);

        crate::menus::invoke(&mut app, &ctx, "fresco.dryLayer", json!({})).unwrap();
        assert_eq!(photocraft_engine::fresco_cmds::layer_wetness_status(&app.session, lid), (0.0, 0.0));
    }

    #[test]
    fn fresco_korean_localization_preserves_ellipsis_shape() {
        let ko = crate::i18n::ko_ps2027_lang();
        for (id, en_label) in [("fresco.applyLiveWatercolor", "Live Watercolor…"), ("fresco.dryLayer", "Dry Watercolor Layer")] {
            let tr = crate::i18n::tr_id(ko, id, en_label);
            assert_ne!(tr, en_label, "expected Korean translation for {id}");
            assert_eq!(tr.ends_with('…'), en_label.ends_with('…'), "ellipsis mismatch for {id}: {tr}");
        }
        for plain_en in [
            "Fresco Live Watercolor",
            "Watercolor Preset",
            "Water Flow",
            "Diffusion Steps",
            "Pigment Advection",
            "Cauliflower Edge",
            "Paper Grain",
            "Evaporation",
            "Wash Soft",
            "Wet Spatter",
            "Detail Ink",
            "Granulating",
        ] {
            assert_ne!(crate::i18n::tr(ko, plain_en), plain_en, "missing Korean string for {plain_en}");
        }
    }
}
