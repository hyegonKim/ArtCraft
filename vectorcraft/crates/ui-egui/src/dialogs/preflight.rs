//! File → Export → Review Before Export (`내보내기 전 프리플라이트` — Illustrator 2027 `31.0`):
//! pre-export analyzer checking out-of-gamut colors, missing fonts, stray points, low-PPI raster
//! links, bleed and artboard bounds, recommending optimal export format/settings, with one-click
//! Text White-on-Black PNG Mask export and handoff to Export As.

use serde_json::{Value, json};
use vectorcraft_color::Color;

use super::{DialogSpec, form};
use crate::panels::c32;
use crate::state::Dialog;
use crate::theme::Tokens;
use crate::{FilePick, VectorcraftApp, picks};

pub const KIND: &str = "preflight";

const LABEL_W: f32 = 96.0;

pub(super) const SPEC: DialogSpec = DialogSpec {
    heading: |_| tl!("Review Before Export").into(),
    body,
    confirm,
    ok: Some("Export…"),
    min_width: 420.0,
    ..DialogSpec::FORM
};

pub fn fields() -> Value {
    json!({
        "target": "web",
        "minPpi": 150
    })
}

fn body(app: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) -> bool {
    let t = Tokens::get(ui.ctx());
    let targets = [
        ("web", tl!("Web")),
        ("print", tl!("Print")),
        ("pdf", tl!("PDF")),
        ("svg", tl!("SVG")),
        ("png", tl!("PNG")),
    ];
    form::choice(ui, d, "target", tl!("Target"), (LABEL_W, 148.0), &targets);
    form::slider_w(ui, d, ("minPpi", tl!("Minimum PPI"), LABEL_W), 72.0..=600.0, " ppi", &|x| c32(&Color::gray(0.25 + 0.55 * x)));

    let query = json!({
        "target": d.str("target"),
        "minPpi": d.f64("minPpi", 150.0),
    });
    let report = app.session.execute("document.preflight", &query).ok();

    ui.add_space(6.0);
    if let Some(rep) = &report {
        draw_report(ui, rep, &t);
    }

    ui.add_space(6.0);
    let can_save = picks::can(app, &picks::PickRequest::Save(mask_pick()));
    if ui.add_enabled(can_save, egui::Button::new(tl!("Export Text Mask (PNG)…"))).clicked() {
        let _ = picks::in_dialog(app, d, |app, _d| {
            if let Some(path) = picks::save(app, &mask_pick()) {
                app.run("document.exportTextWhiteOnBlackPng", json!({"path": path, "ppi": 144}))?;
            }
            Ok(())
        });
    }
    false
}

fn mask_pick() -> FilePick {
    FilePick {
        name: "text-mask.png".into(),
        folder: None,
        filters: vec![FilePick::filter("PNG", &["png"])],
    }
}

fn draw_report(ui: &mut egui::Ui, rep: &Value, t: &Tokens) {
    let errors = rep["errorCount"].as_u64().unwrap_or(0);
    let warnings = rep["warningCount"].as_u64().unwrap_or(0);
    let badge_col = if errors > 0 {
        egui::Color32::from_rgb(210, 68, 68)
    } else if warnings > 0 {
        egui::Color32::from_rgb(218, 154, 42)
    } else {
        egui::Color32::from_rgb(56, 168, 98)
    };
    let status_text = if errors == 0 && warnings == 0 {
        tl!("Ready to export — no issues detected").to_string()
    } else {
        crate::i18n::fmt(
            tl!("Errors: {errors}, Warnings: {warnings}"),
            &[("errors", &errors.to_string()), ("warnings", &warnings.to_string())],
        )
    };
    ui.label(egui::RichText::new(status_text).strong().color(badge_col));

    let rec_fmt = rep["recommendedFormat"].as_str().unwrap_or("svg").to_ascii_uppercase();
    ui.label(
        egui::RichText::new(crate::i18n::fmt(tl!("Recommended format: {format}"), &[("format", &rec_fmt)]))
            .size(11.5)
            .color(t.text_dim),
    );

    if let Some(issues) = rep["issues"].as_array()
        && !issues.is_empty()
    {
        ui.add_space(4.0);
        egui::Frame::NONE
            .fill(t.input)
            .stroke(egui::Stroke::new(1.0, t.input_border))
            .corner_radius(egui::CornerRadius::same(4))
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                    for item in issues {
                        let sev = item["severity"].as_str().unwrap_or("info");
                        let msg = item["message"].as_str().unwrap_or("");
                        let col = match sev {
                            "error" => egui::Color32::from_rgb(215, 76, 76),
                            "warning" => egui::Color32::from_rgb(218, 154, 42),
                            _ => t.text_dim,
                        };
                        ui.label(egui::RichText::new(format!("• [{sev}] {msg}")).size(11.5).color(col));
                    }
                });
            });
    }
}

fn confirm(app: &mut VectorcraftApp, _d: &Dialog) -> Result<Value, String> {
    app.ui.dialog = None;
    crate::dialogs::open_export_as(app, None, &Value::Null);
    Ok(Value::Null)
}
