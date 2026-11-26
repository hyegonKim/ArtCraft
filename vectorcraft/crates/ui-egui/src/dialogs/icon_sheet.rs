//! Object → Generate Icon Sheet (`IconSheet.aip` — Illustrator 2027 `31.0`): generates a crisp
//! vector icon sheet grid (configurable category, search filter, columns, cell size, icon size,
//! padding, labels, cell cards, and dedicated artboard) previewed live on the canvas.

use serde_json::{Value, json};
use vectorcraft_color::Color;

use super::{DialogSpec, form};
use crate::panels::c32;
use crate::state::Dialog;
use crate::{VectorcraftApp, widgets};

pub const KIND: &str = "iconSheet";

const CMD: &str = "icon.generateSheet";
const LABEL_W: f32 = 96.0;

pub(super) const SPEC: DialogSpec = DialogSpec {
    heading: |_| tl!("Generate Icon Sheet").into(),
    body,
    confirm,
    preview: true,
    min_width: 356.0,
    ..DialogSpec::FORM
};

pub fn fields() -> Value {
    json!({
        "category": "",
        "query": "",
        "columns": 6,
        "cellSize": 64,
        "iconSize": 24,
        "padding": 20,
        "showLabels": true,
        "showGrid": true,
        "createArtboard": true,
        "preview": true
    })
}

fn sheet_params(d: &Dialog) -> Value {
    let mut p = form::params(d);
    if let Some(o) = p.as_object_mut() {
        if o.get("category").and_then(Value::as_str).is_some_and(|s| s.trim().is_empty()) {
            o.remove("category");
        }
        if o.get("query").and_then(Value::as_str).is_some_and(|s| s.trim().is_empty()) {
            o.remove("query");
        }
    }
    p
}

fn body(app: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) -> bool {
    let categories = [
        ("", tl!("All")),
        ("UI", tl!("UI")),
        ("Arrows", tl!("Arrows")),
        ("Media", tl!("Media")),
        ("Files", tl!("Files")),
        ("Commerce", tl!("Commerce")),
        ("Badges", tl!("Badges")),
    ];
    form::choice(ui, d, "category", tl!("Category"), (LABEL_W, 148.0), &categories);
    widgets::label_row(ui, tl!("Filter"), LABEL_W, |ui| {
        form::text(ui, d, "query", 148.0);
    });

    form::slider_w(ui, d, ("columns", tl!("Columns"), LABEL_W), 1.0..=12.0, "", &|x| c32(&Color::gray(0.25 + 0.55 * x)));
    form::slider_w(ui, d, ("cellSize", tl!("Cell Size"), LABEL_W), 24.0..=160.0, " pt", &|x| c32(&Color::gray(0.25 + 0.55 * x)));
    form::slider_w(ui, d, ("iconSize", tl!("Icon Size"), LABEL_W), 12.0..=96.0, " pt", &|x| c32(&Color::gray(0.25 + 0.55 * x)));
    form::slider_w(ui, d, ("padding", tl!("Padding"), LABEL_W), 0.0..=64.0, " pt", &|x| c32(&Color::gray(0.25 + 0.55 * x)));

    ui.add_space(4.0);
    form::check(ui, d, "showLabels", tl!("Show Labels"));
    form::check(ui, d, "showGrid", tl!("Show Cell Grid"));
    form::check(ui, d, "createArtboard", tl!("Create Artboard"));

    let p = sheet_params(d);
    form::preview(app, ui, d, "Generate Icon Sheet", CMD, p);
    false
}

fn confirm(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    form::commit_preview(app, CMD, sheet_params(d))
}
