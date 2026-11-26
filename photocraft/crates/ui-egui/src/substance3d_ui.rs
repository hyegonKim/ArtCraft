//! Substance 3D Studio (`Sampler`, `Stager`, `Modeler`, `Designer`, `Painter`) interactive
//! dialogs and Properties panel quick-launch controls:
//! - `layer.new3dStageLayer`: **3D Stage & SDF Clay Mockup** dialog
//! - `filter.material.samplePbr`: **Material Sampler (Photo → PBR)** dialog
//! - `filter.material.proceduralPbr`: **Procedural PBR Material** dialog
//! - Properties panel `Substance 3D` quick-launch buttons (routing strictly through `menus::invoke`).

use egui::RichText;
use photocraft_algo::substance_pbr::{
    PbrChannel, ProceduralPbrPreset, SdfBooleanOp, StageMaterialPreset, StageShape,
};
use serde_json::{Map, Value, json};

use crate::props_layout::section;
use crate::state::DialogKind;
use crate::theme::{ROW_GAP, Tokens};
use crate::{PhotocraftApp, widgets};

pub const STAGE3D_MARKER: &str = "__stage3d";
pub const SAMPLE_PBR_MARKER: &str = "__samplePbr";
pub const PROCEDURAL_PBR_MARKER: &str = "__proceduralPbr";

pub fn owns_stage3d(fields: &Map<String, Value>) -> bool {
    fields.contains_key(STAGE3D_MARKER)
}

pub fn owns_sample_pbr(fields: &Map<String, Value>) -> bool {
    fields.contains_key(SAMPLE_PBR_MARKER)
}

pub fn owns_procedural_pbr(fields: &Map<String, Value>) -> bool {
    fields.contains_key(PROCEDURAL_PBR_MARKER)
}

/// Intercepts parameterless invocations of Substance 3D dialog commands.
pub fn invoke(app: &mut PhotocraftApp, id: &str, params: &Value) -> Option<Result<Value, String>> {
    let empty = params.as_object().is_none_or(|o| o.is_empty());
    if !empty {
        return None;
    }
    match id {
        "layer.new3dStageLayer" => Some(open_stage3d_dialog(app)),
        "filter.material.samplePbr" => Some(open_sample_pbr_dialog(app)),
        "filter.material.proceduralPbr" => Some(open_procedural_pbr_dialog(app)),
        _ => None,
    }
}

pub fn open_stage3d_dialog(app: &mut PhotocraftApp) -> Result<Value, String> {
    if let Some(why) = app.session.disabled_reason_with("layer.new3dStageLayer", &json!({})) {
        return Err(why);
    }
    let mut fields = Map::new();
    fields.insert(STAGE3D_MARKER.into(), Value::Bool(true));
    fields.insert("__command".into(), json!("layer.new3dStageLayer"));
    fields.insert("__label".into(), json!("New 3D Stage Layer…"));
    fields.insert("shape".into(), json!("bottle"));
    fields.insert("sdfOp".into(), json!("none"));
    fields.insert("secondaryShape".into(), json!("torus"));
    fields.insert("smoothK".into(), json!(0.16));
    fields.insert("material".into(), json!("ceramic"));
    fields.insert("baseColor".into(), json!([0.84, 0.86, 0.90]));
    fields.insert("metallic".into(), json!(0.08));
    fields.insert("roughness".into(), json!(0.24));
    fields.insert("weathering".into(), json!(0.15));
    fields.insert("rotX".into(), json!(12.0));
    fields.insert("rotY".into(), json!(-24.0));
    fields.insert("rotZ".into(), json!(0.0));
    fields.insert("scale".into(), json!(1.0));
    fields.insert("matchBackgroundIbl".into(), json!(true));
    fields.insert("shadowCatcher".into(), json!(0.65));
    let dialog = app.ui.open_dialog(DialogKind::Command, fields);
    Ok(json!({ "dialog": dialog }))
}

pub fn open_sample_pbr_dialog(app: &mut PhotocraftApp) -> Result<Value, String> {
    if let Some(why) = app.session.disabled_reason_with("filter.material.samplePbr", &json!({})) {
        return Err(why);
    }
    let mut fields = Map::new();
    fields.insert(SAMPLE_PBR_MARKER.into(), Value::Bool(true));
    fields.insert("__command".into(), json!("filter.material.samplePbr"));
    fields.insert("__label".into(), json!("Material Sampler (Photo to PBR)…"));
    fields.insert("outputMode".into(), json!("group"));
    fields.insert("delightStrength".into(), json!(0.75));
    fields.insert("makeTileable".into(), json!(false));
    fields.insert("tileFeather".into(), json!(0.22));
    fields.insert("normalStrength".into(), json!(2.0));
    fields.insert("roughnessBase".into(), json!(0.45));
    fields.insert("roughnessContrast".into(), json!(1.0));
    fields.insert("metallicLevel".into(), json!(0.0));
    fields.insert("aoStrength".into(), json!(0.85));
    let dialog = app.ui.open_dialog(DialogKind::Command, fields);
    Ok(json!({ "dialog": dialog }))
}

pub fn open_procedural_pbr_dialog(app: &mut PhotocraftApp) -> Result<Value, String> {
    if let Some(why) = app.session.disabled_reason_with("filter.material.proceduralPbr", &json!({})) {
        return Err(why);
    }
    let mut fields = Map::new();
    fields.insert(PROCEDURAL_PBR_MARKER.into(), Value::Bool(true));
    fields.insert("__command".into(), json!("filter.material.proceduralPbr"));
    fields.insert("__label".into(), json!("Procedural PBR Material…"));
    fields.insert("preset".into(), json!("brushedSteel"));
    fields.insert("scale".into(), json!(4.0));
    fields.insert("roughness".into(), json!(0.32));
    fields.insert("normalStrength".into(), json!(1.6));
    fields.insert("wear".into(), json!(0.25));
    fields.insert("lightAngle".into(), json!(42.0));
    fields.insert("lightElevation".into(), json!(54.0));
    let dialog = app.ui.open_dialog(DialogKind::Command, fields);
    Ok(json!({ "dialog": dialog }))
}

fn field_f32(fields: &Map<String, Value>, k: &str, d: f32) -> f32 {
    fields.get(k).and_then(Value::as_f64).filter(|v| v.is_finite()).map_or(d, |v| v as f32)
}

fn field_str(fields: &Map<String, Value>, k: &str, d: &str) -> String {
    fields.get(k).and_then(Value::as_str).unwrap_or(d).to_string()
}

fn field_rgb(fields: &Map<String, Value>, k: &str, d: [f32; 3]) -> [f32; 3] {
    let Some(arr) = fields.get(k).and_then(Value::as_array) else {
        return d;
    };
    let get_c = |i: usize, def: f32| {
        arr.get(i)
            .and_then(Value::as_f64)
            .filter(|v| v.is_finite())
            .map_or(def, |v| (v as f32).clamp(0.0, 1.0))
    };
    [get_c(0, d[0]), get_c(1, d[1]), get_c(2, d[2])]
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

// ------------------------------------------------------------------ 3D Stage & SDF Clay Dialog

pub fn stage3d_body(ui: &mut egui::Ui, fields: &mut Map<String, Value>) {
    let t = Tokens::get(ui.ctx());
    let mut shape = field_str(fields, "shape", "bottle");
    let mut sdf_op = field_str(fields, "sdfOp", "none");
    let mut sec_shape = field_str(fields, "secondaryShape", "torus");
    let mut mat = field_str(fields, "material", "ceramic");
    let mut color = field_rgb(fields, "baseColor", [0.84, 0.86, 0.90]);
    let mut match_ibl = fields.get("matchBackgroundIbl").and_then(Value::as_bool).unwrap_or(true);

    let shape_opts: Vec<(String, &str)> = StageShape::ALL.iter().map(|s| (s.id().into(), tl!(s.label()))).collect();
    let op_opts: Vec<(String, &str)> = SdfBooleanOp::ALL.iter().map(|o| (o.id().into(), tl!(o.label()))).collect();
    let mat_opts: Vec<(String, &str)> = StageMaterialPreset::ALL.iter().map(|m| (m.id().into(), tl!(m.label()))).collect();

    egui::Grid::new("stage3d-dialog-grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        ui.label(RichText::new(tl!("3D Shape")).color(t.text_dim));
        if widgets::dropdown(ui, "s3d-shape", tl!("3D Shape"), &mut shape, &shape_opts, 185.0) {
            fields.insert("shape".into(), json!(shape));
        }
        ui.end_row();

        ui.label(RichText::new(tl!("SDF Clay Boolean")).color(t.text_dim));
        if widgets::dropdown(ui, "s3d-op", tl!("SDF Clay Boolean"), &mut sdf_op, &op_opts, 185.0) {
            fields.insert("sdfOp".into(), json!(sdf_op));
        }
        ui.end_row();

        if sdf_op != "none" {
            ui.label(RichText::new(tl!("Secondary Shape")).color(t.text_dim));
            if widgets::dropdown(ui, "s3d-sec-shape", tl!("Secondary Shape"), &mut sec_shape, &shape_opts, 185.0) {
                fields.insert("secondaryShape".into(), json!(sec_shape));
            }
            ui.end_row();

            ui.label(RichText::new(tl!("Smooth Blend")).color(t.text_dim));
            let mut sk = field_f32(fields, "smoothK", 0.16);
            if widgets::value_field(ui, &mut sk, 0.02..=0.80, "", 82.0).changed() {
                fields.insert("smoothK".into(), json!(sk));
            }
            ui.end_row();
        }

        ui.label(RichText::new(tl!("Material Preset")).color(t.text_dim));
        if widgets::dropdown(ui, "s3d-mat", tl!("Material Preset"), &mut mat, &mat_opts, 185.0) {
            fields.insert("material".into(), json!(mat));
        }
        ui.end_row();

        ui.label(RichText::new(tl!("Base Color")).color(t.text_dim));
        if ui.color_edit_button_rgb(&mut color).changed() {
            fields.insert("baseColor".into(), json!(color));
        }
        ui.end_row();

        pct_row(ui, tl!("Metallic"), fields, "metallic", 0.08, 100.0);
        pct_row(ui, tl!("Roughness"), fields, "roughness", 0.24, 100.0);
        pct_row(ui, tl!("Edge Wear"), fields, "weathering", 0.15, 100.0);

        for (lbl, key, def) in [
            (tl!("Rotate X"), "rotX", 12.0f32),
            (tl!("Rotate Y"), "rotY", -24.0),
            (tl!("Rotate Z"), "rotZ", 0.0),
        ] {
            ui.label(RichText::new(lbl).color(t.text_dim));
            let mut deg = field_f32(fields, key, def);
            if widgets::value_field(ui, &mut deg, -180.0..=180.0, "°", 82.0).changed() {
                fields.insert(key.into(), json!(deg));
            }
            ui.end_row();
        }

        pct_row(ui, tl!("Object Scale"), fields, "scale", 1.0, 250.0);
        pct_row(ui, tl!("Shadow Catcher"), fields, "shadowCatcher", 0.65, 100.0);
    });

    ui.add_space(4.0);
    if widgets::checkbox(ui, &mut match_ibl, tl!("Match Background IBL")).changed() {
        fields.insert("matchBackgroundIbl".into(), json!(match_ibl));
    }
}

pub fn confirm_stage3d(app: &mut PhotocraftApp, fields: &Map<String, Value>) -> Result<Value, String> {
    let rx = field_f32(fields, "rotX", 12.0);
    let ry = field_f32(fields, "rotY", -24.0);
    let rz = field_f32(fields, "rotZ", 0.0);
    app.run(
        "layer.new3dStageLayer",
        json!({
            "shape": field_str(fields, "shape", "bottle"),
            "sdfOp": field_str(fields, "sdfOp", "none"),
            "secondaryShape": field_str(fields, "secondaryShape", "torus"),
            "smoothK": field_f32(fields, "smoothK", 0.16),
            "material": field_str(fields, "material", "ceramic"),
            "baseColor": field_rgb(fields, "baseColor", [0.84, 0.86, 0.90]),
            "metallic": field_f32(fields, "metallic", 0.08),
            "roughness": field_f32(fields, "roughness", 0.24).clamp(0.04, 1.0),
            "weathering": field_f32(fields, "weathering", 0.15),
            "rotDeg": [rx, ry, rz],
            "scale": field_f32(fields, "scale", 1.0).clamp(0.2, 2.5),
            "matchBackgroundIbl": fields.get("matchBackgroundIbl").and_then(Value::as_bool).unwrap_or(true),
            "shadowCatcher": field_f32(fields, "shadowCatcher", 0.65),
        }),
    )
}

// ------------------------------------------------------------------ Material Sampler (Photo -> PBR) Dialog

pub fn sample_pbr_body(ui: &mut egui::Ui, fields: &mut Map<String, Value>) {
    let t = Tokens::get(ui.ctx());
    let mut out_mode = field_str(fields, "outputMode", "group");
    let mut tileable = fields.get("makeTileable").and_then(Value::as_bool).unwrap_or(false);

    let mut mode_opts: Vec<(String, &str)> = vec![("group".into(), tl!("All 6 Maps as Layer Group"))];
    for ch in PbrChannel::ALL {
        mode_opts.push((ch.id().into(), tl!(ch.label())));
    }

    egui::Grid::new("sample-pbr-dialog-grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        ui.label(RichText::new(tl!("Output")).color(t.text_dim));
        if widgets::dropdown(ui, "spbr-output", tl!("Output"), &mut out_mode, &mode_opts, 195.0) {
            fields.insert("outputMode".into(), json!(out_mode));
        }
        ui.end_row();

        pct_row(ui, tl!("Delight Strength"), fields, "delightStrength", 0.75, 100.0);

        ui.label(RichText::new(tl!("Normal Strength")).color(t.text_dim));
        let mut ns = field_f32(fields, "normalStrength", 2.0);
        if widgets::value_field(ui, &mut ns, 0.1..=8.0, "", 82.0).changed() {
            fields.insert("normalStrength".into(), json!(ns));
        }
        ui.end_row();

        pct_row(ui, tl!("Roughness Base"), fields, "roughnessBase", 0.45, 100.0);
        pct_row(ui, tl!("Roughness Contrast"), fields, "roughnessContrast", 1.0, 200.0);
        pct_row(ui, tl!("Metallic Level"), fields, "metallicLevel", 0.0, 100.0);
        pct_row(ui, tl!("AO Strength"), fields, "aoStrength", 0.85, 200.0);

        if tileable {
            pct_row(ui, tl!("Tile Feather"), fields, "tileFeather", 0.22, 50.0);
        }
    });

    ui.add_space(4.0);
    if widgets::checkbox(ui, &mut tileable, tl!("Make Seamless (Tileable)")).changed() {
        fields.insert("makeTileable".into(), json!(tileable));
    }
}

pub fn confirm_sample_pbr(app: &mut PhotocraftApp, fields: &Map<String, Value>) -> Result<Value, String> {
    let mode = field_str(fields, "outputMode", "group");
    let create_group = mode == "group";
    let mut p = json!({
        "createGroup": create_group,
        "delightStrength": field_f32(fields, "delightStrength", 0.75),
        "makeTileable": fields.get("makeTileable").and_then(Value::as_bool).unwrap_or(false),
        "tileFeather": field_f32(fields, "tileFeather", 0.22).clamp(0.05, 0.5),
        "normalStrength": field_f32(fields, "normalStrength", 2.0).clamp(0.1, 8.0),
        "roughnessBase": field_f32(fields, "roughnessBase", 0.45).clamp(0.04, 1.0),
        "roughnessContrast": field_f32(fields, "roughnessContrast", 1.0).clamp(0.0, 2.0),
        "metallicLevel": field_f32(fields, "metallicLevel", 0.0).clamp(0.0, 1.0),
        "aoStrength": field_f32(fields, "aoStrength", 0.85).clamp(0.0, 2.0),
    });
    if !create_group {
        p["channel"] = json!(mode);
    }
    app.run("filter.material.samplePbr", p)
}

// ------------------------------------------------------------------ Procedural PBR Material Dialog

pub fn procedural_pbr_body(ui: &mut egui::Ui, fields: &mut Map<String, Value>) {
    let t = Tokens::get(ui.ctx());
    let mut preset = field_str(fields, "preset", "brushedSteel");
    let preset_opts: Vec<(String, &str)> = ProceduralPbrPreset::ALL
        .iter()
        .map(|p| (p.id().into(), tl!(p.label())))
        .collect();

    egui::Grid::new("proc-pbr-dialog-grid").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        ui.label(RichText::new(tl!("Material Graph")).color(t.text_dim));
        if widgets::dropdown(ui, "ppbr-preset", tl!("Material Graph"), &mut preset, &preset_opts, 185.0) {
            fields.insert("preset".into(), json!(preset));
        }
        ui.end_row();

        ui.label(RichText::new(tl!("Pattern Scale")).color(t.text_dim));
        let mut sc = field_f32(fields, "scale", 4.0);
        if widgets::value_field(ui, &mut sc, 0.5..=16.0, "×", 82.0).changed() {
            fields.insert("scale".into(), json!(sc));
        }
        ui.end_row();

        pct_row(ui, tl!("Roughness"), fields, "roughness", 0.32, 100.0);

        ui.label(RichText::new(tl!("Normal Strength")).color(t.text_dim));
        let mut ns = field_f32(fields, "normalStrength", 1.6);
        if widgets::value_field(ui, &mut ns, 0.1..=6.0, "", 82.0).changed() {
            fields.insert("normalStrength".into(), json!(ns));
        }
        ui.end_row();

        pct_row(ui, tl!("Surface Wear"), fields, "wear", 0.25, 100.0);

        ui.label(RichText::new(tl!("Light Angle")).color(t.text_dim));
        let mut ang = field_f32(fields, "lightAngle", 42.0);
        if widgets::value_field(ui, &mut ang, -180.0..=180.0, "°", 82.0).changed() {
            fields.insert("lightAngle".into(), json!(ang));
        }
        ui.end_row();

        ui.label(RichText::new(tl!("Light Elevation")).color(t.text_dim));
        let mut el = field_f32(fields, "lightElevation", 54.0);
        if widgets::value_field(ui, &mut el, 10.0..=85.0, "°", 82.0).changed() {
            fields.insert("lightElevation".into(), json!(el));
        }
        ui.end_row();
    });
}

pub fn confirm_procedural_pbr(app: &mut PhotocraftApp, fields: &Map<String, Value>) -> Result<Value, String> {
    app.run(
        "filter.material.proceduralPbr",
        json!({
            "preset": field_str(fields, "preset", "brushedSteel"),
            "scale": field_f32(fields, "scale", 4.0).clamp(0.25, 16.0),
            "roughness": field_f32(fields, "roughness", 0.32).clamp(0.04, 1.0),
            "normalStrength": field_f32(fields, "normalStrength", 1.6).clamp(0.1, 6.0),
            "wear": field_f32(fields, "wear", 0.25).clamp(0.0, 1.0),
            "lightAngle": field_f32(fields, "lightAngle", 42.0),
            "lightElevation": field_f32(fields, "lightElevation", 54.0).clamp(10.0, 85.0),
        }),
    )
}

// ------------------------------------------------------------------ Properties Quick-Launch Section

const SUBSTANCE_QUICK_ACTIONS: &[(&str, &str)] = &[
    ("layer.new3dStageLayer", "New 3D Stage Layer…"),
    ("filter.material.samplePbr", "Photo to PBR…"),
    ("filter.material.proceduralPbr", "Procedural PBR…"),
    ("filter.material.makeSeamless", "Make Seamless"),
];

pub fn properties_section(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    if !section(ui, "substance-3d-studio", tl!("Substance 3D")) {
        return;
    }
    let ctx = ui.ctx().clone();
    let mut picked: Option<&'static str> = None;
    ui.horizontal_wrapped(|ui| {
        for &(cmd, label) in SUBSTANCE_QUICK_ACTIONS {
            let enabled = app.session.is_enabled(cmd);
            if ui
                .add_enabled_ui(enabled, |ui| widgets::secondary_button(ui, tl!(label), 0.0).clicked())
                .inner
            {
                picked = Some(cmd);
            }
        }
    });
    ui.add_space(ROW_GAP);
    if let Some(cmd) = picked
        && let Err(e) = crate::menus::invoke(app, &ctx, cmd, json!({}))
    {
        app.ui.status = e;
        app.ui.status_error = true;
    }
}

#[cfg(test)]
mod tests {
    use photocraft_doc::LayerContent;
    use serde_json::json;

    use crate::{PhotocraftApp, Services};

    #[test]
    fn parameterless_stage3d_and_sample_pbr_open_dialogs_and_confirm_executes_commands() {
        let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), Services::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 32, "height": 32, "background": "white"})).unwrap();
        app.sync_views();

        // 1. Parameterless `layer.new3dStageLayer` opens interactive 3D Stage dialog
        let r_stage = crate::menus::invoke(&mut app, &ctx, "layer.new3dStageLayer", json!({})).unwrap();
        let dlg_id = r_stage["dialog"].as_u64().unwrap();
        assert_eq!(app.ui.dialogs.len(), 1);
        assert!(super::owns_stage3d(&app.ui.dialogs[0].fields));
        if let Some(d) = app.ui.dialog_mut(dlg_id) {
            d.fields.insert("shape".into(), json!("sphere"));
            d.fields.insert("sdfOp".into(), json!("smoothUnion"));
            d.fields.insert("material".into(), json!("polishedGold"));
        }
        crate::dialogs::confirm(&mut app, dlg_id).unwrap();
        assert!(app.ui.dialogs.is_empty());
        assert!(
            app.session
                .active()
                .unwrap()
                .doc
                .layers
                .iter()
                .any(|l| l.name == "3D Stage"),
            "confirming 3D Stage dialog must create 3D Stage layer"
        );

        // 2. Parameterless `filter.material.samplePbr` opens Material Sampler dialog -> creates 6-layer group
        let r_pbr = crate::menus::invoke(&mut app, &ctx, "filter.material.samplePbr", json!({})).unwrap();
        let pbr_dlg = r_pbr["dialog"].as_u64().unwrap();
        assert_eq!(app.ui.dialogs.len(), 1);
        assert!(super::owns_sample_pbr(&app.ui.dialogs[0].fields));
        crate::dialogs::confirm(&mut app, pbr_dlg).unwrap();
        assert!(app.ui.dialogs.is_empty());
        let has_pbr_group = app.session.active().unwrap().doc.layers.iter().any(|l| {
            l.name == "PBR Material Maps"
                && matches!(&l.content, LayerContent::Group(g) if g.children.len() == 6)
        });
        assert!(has_pbr_group, "confirming Material Sampler dialog must create 6-map PBR group");

        // 3. Parameterless `filter.material.proceduralPbr` opens Procedural PBR dialog -> applies to active raster layer
        app.run("layer.new.layer", json!({"name": "Proc Layer"})).unwrap();
        let r_proc = crate::menus::invoke(&mut app, &ctx, "filter.material.proceduralPbr", json!({})).unwrap();
        let proc_dlg = r_proc["dialog"].as_u64().unwrap();
        assert!(super::owns_procedural_pbr(&app.ui.dialogs[0].fields));
        crate::dialogs::confirm(&mut app, proc_dlg).unwrap();
        assert!(app.ui.dialogs.is_empty());
    }

    #[test]
    fn substance3d_korean_localization_preserves_ellipsis_shape() {
        let ko = crate::i18n::ko_ps2027_lang();
        for (id, en_label) in [
            ("layer.new3dStageLayer", "New 3D Stage Layer…"),
            ("filter.material.samplePbr", "Material Sampler (Photo to PBR)…"),
            ("filter.material.proceduralPbr", "Procedural PBR Material…"),
            ("filter.material.makeSeamless", "Make Seamless (Tileable)"),
        ] {
            let tr = crate::i18n::tr_id(ko, id, en_label);
            assert_ne!(tr, en_label, "expected Korean translation for {id}");
            assert_eq!(tr.ends_with('…'), en_label.ends_with('…'), "ellipsis mismatch for {id}: {tr}");
        }
    }
}
