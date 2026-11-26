use egui::Ui;
use materialcraft_engine::core::{ExposedParameter, ParameterEditor, Value};
use serde_json::{Value as JsonValue, json};

/// The editor for one exposed input's value; returns the new value as JSON when it changed
pub(super) fn input_editor(ui: &mut Ui, exposed: &materialcraft_engine::core::ExposedParameter, current: &Value) -> Option<JsonValue> {
    let label = if exposed.label.is_empty() { exposed.id.as_str() } else { exposed.label.as_str() };
    let response = ui.vertical(|ui| edit(ui, exposed, current, label));
    if !exposed.description.is_empty() {
        response.response.on_hover_text(&exposed.description);
    }

    response.inner
}

fn edit(ui: &mut Ui, exposed: &ExposedParameter, current: &Value, label: &str) -> Option<JsonValue> {
    if exposed.editor() == ParameterEditor::Input {
        ui.label(label);
        let mut value = current.clone();
        return numeric_fields(ui, exposed, &mut value).then(|| json!(value));
    }

    if exposed.editor() == ParameterEditor::Color && matches!(current, Value::Float3(_) | Value::Float4(_)) {
        let value = current.as_vec4()?;
        let mut rgb = [value[0], value[1], value[2]];
        ui.label(label);
        if crate::widgets::color_swatch(ui, &format!("exp-{}", exposed.id), &mut rgb, 80.0) {
            return Some(match current {
                Value::Float4(value) => json!(Value::Float4([rgb[0], rgb[1], rgb[2], value[3]])),
                _ => json!(Value::Float3(rgb)),
            });
        }

        return None;
    }

    match *current {
        Value::Float(mut value) => crate::widgets::input_slider(ui, label, &mut value, exposed, 3).changed().then(|| json!(value)),
        Value::Int(integer) => {
            let mut value = integer as f32;
            crate::widgets::input_slider(ui, label, &mut value, exposed, 0).changed().then(|| json!(value.round()))
        }
        Value::Bool(mut value) => ui.checkbox(&mut value, label).changed().then(|| json!(value)),
        Value::Enum(selected) => {
            let mut changed = None;
            crate::widgets::combo(ui, label, exposed.options.get(selected as usize).cloned().unwrap_or_default(), |ui| {
                for (index, option) in exposed.options.iter().enumerate() {
                    if ui.selectable_label(index as u32 == selected, option).clicked() {
                        changed = Some(json!(index));
                    }
                }
            });
            changed
        }
        Value::Color(color) => {
            let mut rgb = [color[0], color[1], color[2]];
            let mut changed = None;
            ui.horizontal(|ui| {
                ui.label(label);
                if crate::widgets::color_swatch(ui, &format!("exp-{}", exposed.id), &mut rgb, 80.0) {
                    changed = Some(json!([rgb[0], rgb[1], rgb[2], color[3]]));
                }
            });
            changed
        }
        ref other => {
            ui.label(label);
            let mut value = other.clone();
            numeric_fields(ui, exposed, &mut value).then(|| json!(value))
        }
    }
}

fn numeric_fields(ui: &mut Ui, input: &ExposedParameter, value: &mut Value) -> bool {
    match value {
        Value::Float(value) => number(ui, input, value, "Value"),
        Value::Float2(values) => components(ui, input, values),
        Value::Float3(values) => components(ui, input, values),
        Value::Float4(values) | Value::Color(values) => components(ui, input, values),
        Value::Int(value) => number(ui, input, value, "Value"),
        Value::Int2(values) => components(ui, input, values),
        Value::Int3(values) => components(ui, input, values),
        Value::Int4(values) => components(ui, input, values),
        _ => crate::value_editor::show(ui, value),
    }
}

fn components<N: egui::emath::Numeric>(ui: &mut Ui, input: &ExposedParameter, values: &mut [N]) -> bool {
    let mut changed = false;
    for (value, label) in values.iter_mut().zip(["X", "Y", "Z", "W"]) {
        changed |= number(ui, input, value, label);
    }
    changed
}

fn number<N: egui::emath::Numeric>(ui: &mut Ui, input: &ExposedParameter, value: &mut N, label: &str) -> bool {
    let range = f64::from(input.min)..=f64::from(input.max);
    let response = if input.editor() == ParameterEditor::Slider {
        ui.add(
            egui::Slider::new(value, N::from_f64(*range.start())..=N::from_f64(*range.end()))
                .text(label)
                .step_by(f64::from(input.step()))
                .clamping(if input.clamp { egui::SliderClamping::Edits } else { egui::SliderClamping::Never }),
        )
    } else {
        let mut widget = egui::DragValue::new(value).speed(input.step()).clamp_existing_to_range(false);
        if input.clamp {
            widget = widget.range(range);
        }
        ui.add(widget)
    };
    crate::widgets::name(&response, egui::Role::SpinButton, label);
    response.changed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::{
        Harness,
        kittest::{By, Queryable},
    };

    #[test]
    fn input_descriptions_only_open_nonempty_tooltips() {
        for description in ["", "Controls the amount"] {
            let input: ExposedParameter = serde_json::from_value(json!({
                "id": "amount", "label": "Amount", "default": {"t": "bool", "v": false}, "description": description
            }))
            .unwrap();
            let directory = std::env::var_os("MC_SNAPSHOT_DIR").map(std::path::PathBuf::from);
            let builder = Harness::builder().with_size(egui::vec2(360.0, 160.0)).with_step_dt(1.0 / 60.0);
            let builder = if directory.is_some() { builder.wgpu() } else { builder };
            let mut harness = builder.build_ui(|ui| {
                input_editor(ui, &input, &input.default);
            });
            harness.run();
            let position = harness.get(By::new().role(egui::accesskit::Role::CheckBox).label("Amount")).rect().center();

            harness.event(egui::Event::PointerMoved(position));
            harness.run_steps(90);

            assert_eq!(egui::Tooltip::seconds_since_last_tooltip(&harness.ctx).is_finite(), !description.is_empty());
            if !description.is_empty() {
                assert!(harness.query(By::new().role(egui::accesskit::Role::Label).label(description)).is_some());
            }
            if let Some(directory) = directory {
                std::fs::create_dir_all(&directory).unwrap();
                let name = if description.is_empty() { "input-without-description" } else { "input-description-tooltip" };
                harness.render().unwrap().save(directory.join(format!("{name}.png"))).unwrap();
            }
        }
    }

    #[test]
    fn exposed_sliders_apply_step_and_allow_values_outside_soft_limits() {
        for (clamp, expected) in [(false, 1.5), (true, 1.0)] {
            let input: ExposedParameter = serde_json::from_value(json!({
                "id": "amount", "label": "Amount", "default": {"t": "float", "v": 0.5},
                "min": 0.0, "max": 1.0, "step": 0.25, "clamp": clamp
            }))
            .unwrap();
            let mut harness = Harness::new_ui_state(
                |ui, value: &mut Value| {
                    if let Some(edited) = input_editor(ui, &input, value) {
                        *value = value.coerce_json(&edited).unwrap();
                    }
                },
                Value::Float(0.5),
            );
            harness.run();

            harness.get(By::new().role(egui::accesskit::Role::TextInput).label("Amount")).click();
            harness.run();
            harness.key_combination_modifiers(egui::Modifiers::COMMAND, &[egui::Key::A]);
            harness.event(egui::Event::Text("1.38".into()));
            harness.run();

            assert_eq!(harness.state(), &Value::Float(expected));
        }
    }

    #[test]
    fn editing_a_slider_with_reversed_imported_limits_does_not_panic() {
        let mut harness = Harness::new_ui_state(
            |ui, value: &mut f32| {
                let (min, max) = (3.0, 2.0);
                crate::widgets::slider(ui, "Amount", value, min..=max, 3);
            },
            2.5,
        );
        harness.run();

        harness.get(By::new().role(egui::accesskit::Role::TextInput).label("Amount")).click();
        harness.run();
        harness.key_combination_modifiers(egui::Modifiers::COMMAND, &[egui::Key::A]);
        harness.event(egui::Event::Text("2.75".into()));
        harness.run();

        assert_eq!(*harness.state(), 2.75);
    }
}
