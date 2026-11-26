//! Create a paint style or colour variable directly from a colour sampled by the eyedropper
//! (`dialog.createFromColor`).

use egui::{Color32, Context, CornerRadius, Sense, Stroke, StrokeKind, vec2};
use figcraft_color::Color;
use figcraft_doc::StyleKind;
use serde_json::{Value, json};

use crate::FigApp;
use crate::i18n::tr;
use crate::theme::{Tokens, regular, semibold};

#[derive(Clone, Debug)]
pub struct SampledColor {
    /// Normalized `#RRGGBB` (or `#RRGGBBAA`) colour string.
    pub color: String,
    /// `false` = Paint Style, `true` = Colour Variable.
    pub variable: bool,
    pub name: String,
    /// Selected variable collection id, if any exist.
    pub collection: Option<String>,
}

fn next_color_name(doc: &figcraft_doc::Document) -> String {
    for i in 1..=9999 {
        let cand = format!("Color {i}");
        let taken = doc.styles.iter().any(|s| matches!(s.kind, StyleKind::Paint { .. }) && s.name == cand)
            || doc.variables.variables.iter().any(|v| v.name == cand);
        if !taken {
            return cand;
        }
    }
    "Color".into()
}

/// Handle `dialog.createFromColor` UI request (`{"color": "#ff8800"}`).
pub fn request(app: &mut FigApp, data: &Value) {
    let Some(raw) = data.get("color").and_then(Value::as_str) else { return };
    let Some(parsed) = Color::from_hex(raw) else { return };
    let color = format!("#{}", parsed.to_hex());
    let (name, collection) = match app.session.doc() {
        Ok(doc) => (
            next_color_name(doc),
            doc.variables.collections.first().map(|c| c.id.clone()),
        ),
        Err(_) => ("Color 1".into(), None),
    };
    app.ui.sampled_color = Some(SampledColor { color, variable: false, name, collection });
}

/// Build the engine command and params that create the style or variable described by `state`.
pub fn create_params(app: &FigApp, state: &SampledColor) -> (&'static str, Value) {
    let trimmed = state.name.trim();
    let name = if trimmed.is_empty() { "Color 1" } else { trimmed };
    if !state.variable {
        return (
            "style.create",
            json!({
                "type": "paint",
                "name": name,
                "value": [state.color],
            }),
        );
    }

    let chosen = state
        .collection
        .clone()
        .or_else(|| app.session.doc().ok().and_then(|d| d.variables.collections.first().map(|c| c.id.clone())));
    match chosen {
        Some(coll) => (
            "variable.create",
            json!({
                "collection": coll,
                "name": name,
                "type": "color",
                "value": state.color,
            }),
        ),
        None => (
            "command.batch",
            json!({
                "commands": [
                    {"command": "variable.createCollection", "params": {"name": "Collection"}},
                    {"command": "variable.create", "params": {"collection": "Collection", "name": name, "type": "color", "value": state.color}},
                ]
            }),
        ),
    }
}

pub fn show(app: &mut FigApp, ctx: &Context) {
    let Some(mut state) = app.ui.sampled_color.clone() else { return };
    let t = Tokens::get(ctx);
    let mut open = true;
    let mut create_now = false;
    let mut cancel_now = false;

    crate::dialogs::window(tr("Create from color"), ctx)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_width(280.0);

            // Swatch + hex readout without leading '#'
            ui.horizontal(|ui| {
                let c32 = Color::from_hex(&state.color)
                    .map(|c| crate::theme::doc_c32(ctx, c))
                    .unwrap_or(Color32::GRAY);
                let (swatch, _) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::hover());
                ui.painter().rect_filled(swatch, CornerRadius::same(4), c32);
                ui.painter().rect_stroke(swatch, CornerRadius::same(4), Stroke::new(1.0, t.border), StrokeKind::Inside);
                ui.label(egui::RichText::new(state.color.trim_start_matches('#')).font(semibold(12.0)).color(t.text));
            });

            ui.add_space(8.0);
            let opts = [tr("Style"), tr("Variable")];
            if let Some(idx) = crate::widgets::segmented_text(ui, &opts, usize::from(state.variable)) {
                state.variable = idx == 1;
            }

            ui.add_space(8.0);
            ui.label(egui::RichText::new(tr("Name")).font(regular(11.0)).color(t.text_secondary));
            ui.add(egui::TextEdit::singleline(&mut state.name).desired_width(f32::INFINITY));

            if state.variable
                && let Ok(doc) = app.session.doc()
            {
                ui.add_space(8.0);
                ui.label(egui::RichText::new(tr("Collection")).font(regular(11.0)).color(t.text_secondary));
                if doc.variables.collections.is_empty() {
                    ui.label(egui::RichText::new(tr("New collection")).font(regular(11.0)).color(t.text_tertiary));
                } else {
                    let current_id = state
                        .collection
                        .clone()
                        .unwrap_or_else(|| doc.variables.collections[0].id.clone());
                    let current_name = doc
                        .variables
                        .collections
                        .iter()
                        .find(|c| c.id == current_id)
                        .map_or_else(|| current_id.clone(), |c| c.name.clone());
                    let collections: Vec<(String, String)> = doc
                        .variables
                        .collections
                        .iter()
                        .map(|c| (c.id.clone(), c.name.clone()))
                        .collect();
                    egui::ComboBox::from_id_salt("sampled_color_collection")
                        .selected_text(current_name)
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for (cid, cname) in collections {
                                if ui.selectable_label(state.collection.as_deref() == Some(cid.as_str()), cname).clicked() {
                                    state.collection = Some(cid);
                                }
                            }
                        });
                }
            }

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if crate::widgets::button(ui, tr("Create"), true, true).clicked() {
                    create_now = true;
                }
                if crate::widgets::button(ui, tr("Cancel"), false, true).clicked() {
                    cancel_now = true;
                }
            });
        });

    if create_now {
        let (cmd, params) = create_params(app, &state);
        app.ui.sampled_color = None;
        app.run(cmd, params);
    } else if !open || cancel_now {
        app.ui.sampled_color = None;
    } else {
        app.ui.sampled_color = Some(state);
    }
}
