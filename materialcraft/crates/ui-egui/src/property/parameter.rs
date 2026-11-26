use super::{curve_editor, gradient_editor};
use crate::{MaterialCraftApp, i18n::tr, theme::Tokens};
use egui::{RichText, Ui};
use egui_phosphor::regular as icon;
use materialcraft_engine::core::{NodeId, Value};
use materialcraft_engine::nodes::ParameterDefinition;
use serde_json::{Value as JsonValue, json};

fn set(app: &mut MaterialCraftApp, node: NodeId, parameter: &str, value: JsonValue) {
    app.run("graph.node.param", json!({"id": node.0, "param": parameter, "value": value}));
}

fn typed(value: &Value) -> JsonValue {
    serde_json::to_value(value).unwrap_or(JsonValue::Null)
}

/// The literal editor inside a parameter control
pub(super) fn param_row(app: &mut MaterialCraftApp, ui: &mut Ui, node: NodeId, parameter: &ParameterDefinition, current: Value) {
    let tokens = Tokens::get(ui.ctx());
    match current {
        Value::Float(mut value) => {
            if parameter.angle {
                let mut degrees = value * 360.0;
                let response = crate::widgets::slider(ui, tr(parameter.label), &mut degrees, -360.0..=360.0, 1);
                if response.changed() {
                    value = degrees / 360.0;
                    set(app, node, parameter.id, json!(value));
                }
            } else {
                let (minimum, maximum) = (parameter.min.min(value), parameter.max.max(value));
                let response = crate::widgets::slider(ui, tr(parameter.label), &mut value, minimum..=maximum, if maximum - minimum > 20.0 { 1 } else { 3 });
                if response.changed() {
                    set(app, node, parameter.id, json!(value));
                }
            }
        }
        Value::Int(integer) => {
            let mut value = integer as f32;
            let range = parameter.min.min(value)..=parameter.max.max(value);
            let response = crate::widgets::slider(ui, tr(parameter.label), &mut value, range, 0);
            if response.changed() && value.round() as i32 != integer {
                set(app, node, parameter.id, json!(value.round() as i32));
            }
        }
        Value::Float2(value) => {
            ui.label(RichText::new(tr(parameter.label)).color(tokens.text));
            let mut components = value;
            for (index, axis) in ["X", "Y"].iter().enumerate() {
                let range = parameter.min.min(components[index])..=parameter.max.max(components[index]);
                let response = crate::widgets::slider(ui, axis, &mut components[index], range, 3);
                if response.changed() {
                    set(app, node, parameter.id, json!(components));
                }
            }
        }
        Value::Int2(value) => {
            ui.label(RichText::new(tr(parameter.label)).color(tokens.text));
            for index in 0..2 {
                let mut number = value[index] as f32;
                if crate::widgets::slider(ui, ["X", "Y"][index], &mut number, parameter.min..=parameter.max, 0).changed() {
                    let mut components = value;
                    components[index] = number.round() as i32;
                    set(app, node, parameter.id, json!(components));
                }
            }
        }
        Value::Bool(mut value) => {
            let response = ui.checkbox(&mut value, tr(parameter.label));
            if response.changed() {
                set(app, node, parameter.id, json!(value));
            }
        }
        Value::Float3(_) | Value::Float4(_) | Value::Int3(_) | Value::Int4(_) => {
            ui.label(tr(parameter.label));
            let mut value = current.clone();
            if crate::value_editor::show(ui, &mut value) {
                set(app, node, parameter.id, serde_json::to_value(value).unwrap_or_default());
            }
        }
        Value::Color(color) => {
            ui.horizontal(|ui| {
                ui.label(RichText::new(tr(parameter.label)).color(tokens.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut rgb = [color[0], color[1], color[2]];
                    if crate::widgets::color_swatch(ui, &format!("{}-{}", node.0, parameter.id), &mut rgb, 90.0) {
                        set(app, node, parameter.id, json!([rgb[0], rgb[1], rgb[2], color[3]]));
                    }
                });
            });
        }
        Value::Enum(selected) => {
            let selected_label = parameter.options.get(selected as usize).copied().unwrap_or("?");
            crate::widgets::combo(ui, tr(parameter.label), tr(selected_label), |ui| {
                for (index, option) in parameter.options.iter().enumerate() {
                    if ui.selectable_label(index as u32 == selected, tr(option)).clicked() {
                        set(app, node, parameter.id, json!(index));
                    }
                }
            });
        }
        Value::Text(text) if parameter.resource => {
            let names: Vec<String> = app.session.pkg.resources.iter().map(|response| response.name.clone()).collect();
            crate::widgets::combo(ui, tr(parameter.label), if text.is_empty() { tr("(checker)").to_string() } else { text.clone() }, |ui| {
                for name in &names {
                    if ui.selectable_label(*name == text, name).clicked() {
                        set(app, node, parameter.id, json!(name));
                    }
                }
            });
            if ui.button(format!("{}  {}", icon::IMAGE, tr("Import bitmap..."))).clicked() {
                crate::menus::invoke(app, ui.ctx(), "file.importBitmap");
            }
        }
        Value::Text(text) => {
            ui.horizontal(|ui| {
                ui.label(RichText::new(tr(parameter.label)).color(tokens.text));
                let mut draft = text.clone();
                let response = ui.add(egui::TextEdit::singleline(&mut draft).desired_width(ui.available_width()));
                crate::widgets::name(&response, egui::Role::TextInput, parameter.label);
                if response.changed() {
                    set(app, node, parameter.id, json!(draft));
                }
            });
        }
        Value::Curve(curve) => {
            ui.label(RichText::new(tr(parameter.label)).color(tokens.text));
            if let Some(edited_curve) = curve_editor(ui, &format!("curve-{}-{}", node.0, parameter.id), &curve) {
                set(app, node, parameter.id, json!(edited_curve.points.iter().map(|point| [point.x, point.y]).collect::<Vec<_>>()));
            }
        }
        Value::Gradient(gradient) => {
            ui.label(RichText::new(tr(parameter.label)).color(tokens.text));
            if let Some(edited_gradient) = gradient_editor(ui, &format!("grad-{}-{}", node.0, parameter.id), &gradient) {
                set(app, node, parameter.id, typed(&Value::Gradient(edited_gradient)));
            }
        }
        Value::PatternGraph(graph) => {
            ui.label(format!("{}: {} nodes", parameter.label, graph.nodes.len()));
        }
        Value::Function(function) => {
            ui.label(RichText::new(tr(parameter.label)).color(tokens.text));
            crate::windows::function_summary(app, ui, node, parameter.id, &function);
        }
    }
}
