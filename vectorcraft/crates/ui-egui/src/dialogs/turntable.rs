//! Object → Turntable (`TurntableGR6` / `TurntableV8` — Illustrator 2027 `31.0`): multi-angle
//! 2D/3D vector turnaround sheet generator with Orthographic / Isometric / Perspective projection,
//! PBR Metallic & Roughness shading, degree labels, and live canvas preview (`object.turntable`).

use serde_json::{Value, json};
use vectorcraft_color::Color;

use super::{DialogSpec, form};
use crate::panels::c32;
use crate::state::Dialog;
use crate::{VectorcraftApp, widgets};

pub const KIND: &str = "turntable";

const CMD: &str = "object.turntable";
const LABEL_W: f32 = 104.0;
const ANGLE_PRESETS: [&str; 4] = ["4", "6", "8", "12"];

pub(super) const SPEC: DialogSpec =
    DialogSpec { heading: |_| tl!("Turntable").into(), body, confirm, preview: true, min_width: 360.0, ..DialogSpec::FORM };

pub fn fields() -> Value {
    json!({
        "angles": 8,
        "projection": "orthographic",
        "elevation": 15,
        "extrudeDepth": 24,
        "metallic": 0,
        "roughness": 45,
        "spacing": 24,
        "showLabels": true,
        "createArtboard": false,
        "keepOriginal": true,
        "preview": true
    })
}

fn body(app: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) -> bool {
    let cur_angles = (d.f64("angles", 8.0).round() as i64).clamp(2, 36);
    let shown_angles = cur_angles.to_string();
    widgets::label_row(ui, tl!("Angles"), LABEL_W, |ui| {
        if let Some(i) = widgets::dropdown(ui, "tt-angles", &shown_angles, &ANGLE_PRESETS, 84.0)
            && let Some(n) = ANGLE_PRESETS.get(i).and_then(|s| s.parse::<i64>().ok())
        {
            d.fields.insert("angles".into(), json!(n));
        }
    });

    let projections = [("orthographic", tl!("Orthographic")), ("isometric", tl!("Isometric")), ("perspective", tl!("Perspective"))];
    form::choice(ui, d, "projection", tl!("Projection"), (LABEL_W, 148.0), &projections);

    if d.str("projection") != "isometric" {
        form::slider_w(ui, d, ("elevation", tl!("Elevation"), LABEL_W), -89.0..=89.0, "°", &|x| c32(&Color::gray(0.25 + 0.5 * x)));
    }
    form::slider_w(ui, d, ("extrudeDepth", tl!("Extrude Depth"), LABEL_W), 0.0..=200.0, " pt", &|x| c32(&Color::gray(0.2 + 0.6 * x)));
    form::slider_w(ui, d, ("metallic", tl!("Metallic"), LABEL_W), 0.0..=100.0, "%", &|x| c32(&Color::gray(0.2 + 0.7 * x)));
    form::slider_w(ui, d, ("roughness", tl!("Roughness"), LABEL_W), 0.0..=100.0, "%", &|x| c32(&Color::gray(0.85 - 0.6 * x)));
    form::slider_w(ui, d, ("spacing", tl!("Spacing"), LABEL_W), 0.0..=160.0, " pt", &|x| c32(&Color::gray(0.3 + 0.5 * x)));

    ui.add_space(4.0);
    form::check(ui, d, "showLabels", tl!("Show Angle Labels"));
    form::check(ui, d, "createArtboard", tl!("Create Turnaround Artboard"));
    form::check(ui, d, "keepOriginal", tl!("Keep Original"));

    let p = form::params(d);
    form::preview(app, ui, d, "Turntable", CMD, p);
    false
}

fn confirm(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    form::commit_preview(app, CMD, form::params(d))
}
