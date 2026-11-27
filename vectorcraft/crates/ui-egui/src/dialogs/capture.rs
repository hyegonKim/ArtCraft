//! Object → Capture (`Adobe Capture` asset extractor):
//! - `capture.extractColorTheme`: Extract Color Theme dialog (OKLab 5-swatch moods + live swatch bar)
//! - `capture.extractPatternTile`: Extract Pattern Tile dialog (Kaleidoscope / Mirror / Half-Drop)

use egui::{Color32, Stroke, vec2};
use serde_json::{Value, json};
use vectorcraft_color::Color;
use vectorcraft_color::capture::CaptureMood;
use vectorcraft_engine::cmd::capture::{CMD_COLOR_THEME, CMD_PATTERN_TILE};

use super::{DialogSpec, form};
use crate::panels::c32;
use crate::state::Dialog;
use crate::{VectorcraftApp, widgets};

pub const COLOR_THEME_KIND: &str = "captureColorTheme";
pub const PATTERN_TILE_KIND: &str = "capturePatternTile";

const LABEL_W: f32 = 104.0;

pub(super) const THEME_SPEC: DialogSpec = DialogSpec {
    heading: |_| tl!("Extract Color Theme").into(),
    body: theme_body,
    confirm: theme_confirm,
    preview: false,
    min_width: 360.0,
    ..DialogSpec::FORM
};

pub(super) const PATTERN_SPEC: DialogSpec = DialogSpec {
    heading: |_| tl!("Extract Pattern Tile").into(),
    body: pattern_body,
    confirm: pattern_confirm,
    preview: true,
    min_width: 372.0,
    ..DialogSpec::FORM
};

pub fn theme_fields() -> Value {
    json!({
        "mood": "colorful",
        "name": "",
        "global": true,
        "saveTheme": true
    })
}

pub fn pattern_fields() -> Value {
    json!({
        "mode": "kaleidoscope",
        "segments": "6",
        "tileSize": 120,
        "name": "",
        "placePreview": true,
        "preview": true
    })
}

fn theme_body(_app: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) -> bool {
    let moods = [("colorful", tl!("Colorful")), ("bright", tl!("Bright")), ("muted", tl!("Muted")), ("deep", tl!("Deep")), ("dark", tl!("Dark"))];
    form::choice(ui, d, "mood", tl!("Color Mood"), (LABEL_W, 156.0), &moods);

    widgets::label_row(ui, tl!("Theme Name"), LABEL_W, |ui| {
        form::text(ui, d, "name", 172.0);
    });

    // Illustrative 5-swatch mood preview strip in OKLab space.
    let mood = CaptureMood::parse(&d.str("mood")).unwrap_or(CaptureMood::Colorful);
    let sample_seed =
        [Color::rgb8(232, 68, 56), Color::rgb8(245, 194, 48), Color::rgb8(46, 182, 118), Color::rgb8(42, 146, 228), Color::rgb8(152, 74, 188)];
    let preview_swatches = vectorcraft_color::capture::extract_color_theme_colors(&sample_seed, mood);
    ui.add_space(4.0);
    widgets::label_row(ui, tl!("OKLab Preview"), LABEL_W, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for sw in &preview_swatches {
                let (rect, _) = ui.allocate_exact_size(vec2(28.0, 20.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 3.0, c32(sw));
                ui.painter().rect_stroke(rect, 3.0, Stroke::new(1.0, Color32::from_black_alpha(60)), egui::StrokeKind::Inside);
            }
        });
    });

    ui.add_space(4.0);
    form::check(ui, d, "global", tl!("Global Swatches"));
    form::check(ui, d, "saveTheme", tl!("Save to Color Themes"));
    false
}

fn theme_confirm(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    let mut p = json!({
        "mood": d.str("mood"),
        "global": d.bool("global"),
        "saveTheme": d.bool("saveTheme"),
    });
    let raw_name = d.str("name");
    let name = raw_name.trim();
    if !name.is_empty() {
        p["name"] = json!(name);
    }
    app.ui.dialog = None;
    app.run(CMD_COLOR_THEME, p)
}

fn pattern_params(d: &Dialog) -> Value {
    let seg = d.str("segments").parse::<u32>().ok().filter(|n| matches!(n, 4 | 6 | 8 | 12)).unwrap_or(6);
    let mut p = json!({
        "mode": d.str("mode"),
        "segments": seg,
        "tileSize": d.f64("tileSize", 120.0).clamp(16.0, 2048.0),
        "placePreview": d.bool("placePreview"),
    });
    let raw_name = d.str("name");
    let name = raw_name.trim();
    if !name.is_empty() {
        p["name"] = json!(name);
    }
    p
}

fn pattern_body(app: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) -> bool {
    let modes = [("kaleidoscope", tl!("Kaleidoscope")), ("mirror", tl!("Mirror")), ("halfDrop", tl!("Half-Drop"))];
    form::choice(ui, d, "mode", tl!("Pattern Mode"), (LABEL_W, 156.0), &modes);

    if d.str("mode") == "kaleidoscope" {
        let segs = [("4", "4"), ("6", "6"), ("8", "8"), ("12", "12")];
        form::choice(ui, d, "segments", tl!("Segments"), (LABEL_W, 96.0), &segs);
    }

    form::slider_w(ui, d, ("tileSize", tl!("Tile Size"), LABEL_W), 24.0..=512.0, " pt", &|x| c32(&Color::gray(0.30 + 0.50 * x)));

    widgets::label_row(ui, tl!("Pattern Name"), LABEL_W, |ui| {
        form::text(ui, d, "name", 172.0);
    });

    ui.add_space(4.0);
    form::check(ui, d, "placePreview", tl!("Place Tile Preview"));

    let p = pattern_params(d);
    form::preview(app, ui, d, "Extract Pattern Tile", CMD_PATTERN_TILE, p);
    false
}

fn pattern_confirm(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    form::commit_preview(app, CMD_PATTERN_TILE, pattern_params(d))
}
