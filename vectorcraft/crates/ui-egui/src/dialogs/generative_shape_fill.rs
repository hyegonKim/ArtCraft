//! Object → Generative Shape Fill (`생성형 모양 채우기` — Illustrator 2027 `31.0`): fills selected
//! closed vector shapes with procedural prompt-guided geometric, botanical, halftone or mosaic
//! vector motifs clipped cleanly via boolean intersection (`object.generativeShapeFill`).

use serde_json::{Value, json};
use vectorcraft_color::Color;

use super::{DialogSpec, form};
use crate::panels::c32;
use crate::state::Dialog;
use crate::{VectorcraftApp, widgets};

pub const KIND: &str = "generativeShapeFill";

const CMD: &str = "object.generativeShapeFill";
const LABEL_W: f32 = 92.0;

pub(super) const SPEC: DialogSpec =
    DialogSpec { heading: |_| tl!("Generative Shape Fill").into(), body, confirm, preview: true, min_width: 360.0, ..DialogSpec::FORM };

pub fn fields() -> Value {
    json!({
        "style": "geometric",
        "prompt": "",
        "seed": 42,
        "density": 50,
        "scale": 100,
        "keepOutline": true,
        "preview": true
    })
}

fn body(app: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) -> bool {
    let styles = [("geometric", tl!("Geometric")), ("botanical", tl!("Botanical")), ("halftone", tl!("Halftone")), ("mosaic", tl!("Mosaic"))];
    form::choice(ui, d, "style", tl!("Style"), (LABEL_W, 152.0), &styles);

    widgets::label_row(ui, tl!("Prompt"), LABEL_W, |ui| {
        form::text(ui, d, "prompt", 180.0);
    });

    ui.horizontal(|ui| {
        let seed = d.f64("seed", 42.0).round().clamp(0.0, 9999.0);
        widgets::label_row(ui, tl!("Seed"), LABEL_W, |ui| {
            if let Some(n) = widgets::range_field(ui, "gsf-seed", seed, 0.0..=9999.0, "", 0, 72.0) {
                d.fields.insert("seed".into(), json!(n as u64));
            }
            if ui.button(tl!("Variation")).clicked() {
                let next = ((seed as u64).wrapping_mul(1103515245).wrapping_add(12345) % 9973) + 1;
                d.fields.insert("seed".into(), json!(next));
            }
        });
    });

    form::slider_w(ui, d, ("density", tl!("Density"), LABEL_W), 5.0..=100.0, "%", &|x| c32(&Color::gray(0.85 - 0.65 * x)));
    form::slider_w(ui, d, ("scale", tl!("Scale"), LABEL_W), 25.0..=300.0, "%", &|x| c32(&Color::gray(0.25 + 0.55 * x)));

    ui.add_space(4.0);
    form::check(ui, d, "keepOutline", tl!("Keep Shape Outline"));

    let p = form::params(d);
    form::preview(app, ui, d, "Generative Shape Fill", CMD, p);
    false
}

fn confirm(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    form::commit_preview(app, CMD, form::params(d))
}
