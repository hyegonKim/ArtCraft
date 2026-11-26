use super::{input::input_editor, parameter::param_row, section};
use crate::{MaterialCraftApp, i18n::tr, theme::Tokens};
use egui::{Rect, RichText, Ui, vec2};
use egui_phosphor::regular as icon;
use materialcraft_engine::core::{BaseParameter, ExposedParameter, ParameterInheritance, Value};
use materialcraft_engine::nodes::ParameterDescriptor;
use serde_json::json;

pub(super) fn show(app: &mut MaterialCraftApp, ui: &mut Ui, node: &materialcraft_engine::core::Node) {
    let tokens = Tokens::get(ui.ctx());
    let definition = materialcraft_engine::nodes::find(&node.def);

    // Header: label (renamable) and description
    ui.horizontal(|ui| {
        ui.painter().rect_filled(Rect::from_min_size(ui.cursor().min + vec2(0.0, 2.0), vec2(4.0, 16.0)), 1.0, crate::graph::header_color(&node.def));
        ui.add_space(8.0);
        let mut label = if node.label.is_empty() { crate::graph::node_label(node) } else { node.label.clone() };
        let response = ui.add(egui::TextEdit::singleline(&mut label).desired_width(ui.available_width() - 4.0).font(egui::TextStyle::Heading));
        crate::widgets::name(&response, egui::Role::TextInput, "Node label");
        if response.changed() {
            app.run("graph.node.rename", json!({"id": node.id.0, "label": label}));
        }
    });
    if let Some(definition) = definition
        && !definition.description.is_empty()
    {
        ui.label(RichText::new(definition.description).size(11.0).color(tokens.text_dim));
    }
    if let Some(response) = app.session.cooked.nodes.get(&node.id)
        && let Some(entry) = &response.error
    {
        ui.label(RichText::new(format!("{}  {entry}", icon::WARNING)).color(tokens.danger));
    }
    ui.add_space(4.0);
    section(ui, "base", tr("Base parameters"), |ui| base_params(app, ui, node));
    crate::value_input_editor::show(app, ui, node);
    if node.def == "input_value"
        && let Some(descriptor) = ParameterDescriptor::for_node(&app.session.pkg, node, "default")
    {
        super::parameter_control::show(app, ui, node, &descriptor, None, |app, ui| {
            ui.label(tr("Default value"));
            crate::value_input_editor::default_value(app, ui, node);
        });
    }
    if node.def == "graph_instance" {
        section(ui, "instance", tr("Instance parameters"), |ui| instance_params(app, ui, node));
        return;
    }

    let Some(definition) = definition else { return };

    if definition.params.is_empty() {
        return;
    }

    section(ui, "specific", tr("Specific parameters"), |ui| {
        if node.def == "levels" {
            crate::levels_editor::show(app, ui, node);
            ui.add_space(6.0);
        }
        for parameter in &definition.params {
            if node.def == "input_value" && parameter.id == "default" {
                continue;
            }

            let current = node.params.get(parameter.id).cloned().unwrap_or_else(|| parameter.default.clone());
            if let Some(descriptor) = ParameterDescriptor::for_node(&app.session.pkg, node, parameter.id)
                && descriptor.supports_function
            {
                super::parameter_control::show(app, ui, node, &descriptor, None, |app, ui| param_row(app, ui, node.id, parameter, current));
            } else {
                param_row(app, ui, node.id, parameter, current);
            }
        }
    });
}

fn base_params(app: &mut MaterialCraftApp, ui: &mut Ui, node: &materialcraft_engine::core::Node) {
    for parameter in BaseParameter::ALL {
        let descriptor = ParameterDescriptor::base(parameter);
        let driven = super::parameter_control::header(app, ui, node, &descriptor, parameter);
        let inheritance = parameter.inheritance(&node.base);
        ui.push_id(parameter, |ui| {
            if !driven && parameter == BaseParameter::OutputSize {
                size_control(app, ui, node);
            } else if !driven {
                let mut value = parameter.value(&node.base);
                let editable = inheritance == ParameterInheritance::Absolute || matches!(parameter, BaseParameter::PixelSize | BaseParameter::RandomSeed);
                if !editable && let Some(result) = app.session.cooked.nodes.get(&node.id) {
                    value = match parameter {
                        BaseParameter::Format => {
                            BaseParameter::Format.value(&materialcraft_engine::core::BaseParameters { format: result.base.format, ..Default::default() })
                        }
                        BaseParameter::PixelRatio => Value::Int(result.base.pixel_ratio),
                        BaseParameter::Tiling => Value::Int(i32::from(result.base.tiling[0]) | (i32::from(result.base.tiling[1]) << 1)),
                        _ => value,
                    };
                }

                let value = if descriptor.options.is_empty() { value } else { Value::Enum(value.as_i32().unwrap_or(0).max(0) as u32) };
                let input = ExposedParameter {
                    description: String::new(),
                    editor: None,
                    clamp: false,
                    step: None,
                    id: descriptor.id.clone(),
                    label: if parameter == BaseParameter::RandomSeed { descriptor.label.clone() } else { tr("Value").into() },
                    group: String::new(),
                    default: descriptor.default,
                    min: descriptor.min,
                    max: descriptor.max,
                    options: descriptor.options,
                };
                if let Some(value) = ui.add_enabled_ui(editable, |ui| input_editor(ui, &input, &value)).inner {
                    app.run("graph.node.base", json!({"id": node.id, "param": parameter.identifier(), "value": value}));
                }
                if parameter == BaseParameter::RandomSeed && ui.button(format!("{}  {}", icon::SHUFFLE, tr("New random seed"))).clicked() {
                    let seed = (crate::now_ms() as u64 ^ node.id.0).wrapping_mul(2654435761) % 1000;
                    app.run("graph.node.base", json!({"id": node.id, "param": parameter.identifier(), "value": seed}));
                }
            }
        });
        ui.add_space(4.0);
    }
}

fn size_control(app: &mut MaterialCraftApp, ui: &mut Ui, node: &materialcraft_engine::core::Node) {
    let link_id = ui.make_persistent_id("size-axes-linked");
    let mut linked = ui.data(|data| data.get_temp::<bool>(link_id)).unwrap_or(node.base.size[0] == node.base.size[1]);
    if ui.checkbox(&mut linked, tr("Link width and height")).changed() {
        ui.data_mut(|data| data.insert_temp(link_id, linked));
    }

    let absolute = node.base.size_mode == materialcraft_engine::core::SizeMode::Absolute;
    for (axis, label) in [(0, "Width"), (1, "Height")] {
        let mut exponent = node.base.size[axis] as f32;
        let caption = if absolute {
            format!("{}: {} px", tr(label), 1u32 << node.base.size[axis].clamp(4, 13))
        } else {
            format!("{}: ×{}", tr(label), 2f32.powi(node.base.size[axis].clamp(-12, 12)))
        };
        if crate::widgets::slider(ui, &caption, &mut exponent, if absolute { 4.0..=13.0 } else { -12.0..=12.0 }, 0).changed() {
            let mut size = node.base.size;
            size[axis] = exponent.round() as i32;
            if linked {
                size[1 - axis] = size[axis];
            }

            app.run("graph.node.base", json!({"id": node.id, "param": "outputsize", "value": size}));
        }
    }
}

fn instance_params(app: &mut MaterialCraftApp, ui: &mut Ui, node: &materialcraft_engine::core::Node) {
    let tokens = Tokens::get(ui.ctx());
    let Some((_, subgraph)) = materialcraft_engine::eval::instance_graph(&app.session.pkg, node) else {
        ui.label(RichText::new(tr("The instanced graph is missing")).color(tokens.danger));
        return;
    };

    let exposed = subgraph.exposed.clone();
    if ui.button(format!("{}  {}", icon::ARROW_SQUARE_OUT, tr("Open graph"))).clicked() {
        app.run("graph.select", json!({"ids": [node.id.0]}));
        crate::menus::invoke(app, ui.ctx(), "graph.node.edit");
        return;
    }

    if exposed.is_empty() {
        ui.label(RichText::new(tr("This graph exposes no inputs")).color(tokens.text_dim));
    }

    for entry in exposed {
        let descriptor = ParameterDescriptor::for_node(&app.session.pkg, node, &entry.id);
        let current = node.params.get(&entry.id).unwrap_or_else(|| descriptor.as_ref().map_or(&entry.default, |descriptor| &descriptor.default)).clone();
        let literal = |app: &mut MaterialCraftApp, ui: &mut Ui| {
            if let Some(value) = input_editor(ui, &entry, &current) {
                app.run("graph.node.param", json!({"id": node.id.0, "param": entry.id, "value": value}));
            }
        };
        if let Some(descriptor) = descriptor
            && descriptor.supports_function
        {
            super::parameter_control::show(app, ui, node, &descriptor, None, literal);
        } else {
            literal(app, ui);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::{
        Harness,
        kittest::{By, Queryable},
    };
    use materialcraft_engine::core::NodeId;

    #[test]
    fn instance_properties_show_current_source_values_and_local_overrides() {
        let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), crate::UiState::default());
        app.session.graph_mut().unwrap().name = "Source".into();
        app.run("graph.node.add", json!({"def": "output"})).unwrap();
        app.run("graph.exposed.add", json!({"id": "amount", "label": "Amount", "default": 0.25})).unwrap();
        app.run("graph.exposed.set", json!({"id": "amount", "value": 0.75})).unwrap();
        app.run("file.new", json!({"name": "Target", "template": "empty"})).unwrap();
        let instance = NodeId(app.run("graph.instance.add", json!({"document": 0, "graph": "Source"})).unwrap()["id"].as_u64().unwrap());
        let directory = std::env::var_os("MC_SNAPSHOT_DIR").map(std::path::PathBuf::from);
        let builder = Harness::builder().with_size(egui::vec2(420.0, 200.0)).with_context_setup(crate::theme::install);
        let builder = if directory.is_some() { builder.wgpu() } else { builder };
        let mut harness = builder.build_ui_state(
            move |ui, app: &mut MaterialCraftApp| {
                let node = app.session.graph().unwrap().node(instance).unwrap().clone();
                instance_params(app, ui, &node);
            },
            app,
        );
        harness.run();
        let input = || By::new().role(egui::accesskit::Role::TextInput).label("Amount");

        assert_eq!(harness.get(input()).value().as_deref(), Some("0.750"));
        if let Some(directory) = &directory {
            std::fs::create_dir_all(directory).unwrap();
            harness.render().unwrap().save(directory.join("instance-current-input.png")).unwrap();
        }

        harness.state_mut().run("graph.node.param", json!({"id": instance, "param": "amount", "value": 0.5})).unwrap();
        harness.run();

        assert_eq!(harness.get(input()).value().as_deref(), Some("0.500"));
        if let Some(directory) = directory {
            harness.render().unwrap().save(directory.join("instance-local-override.png")).unwrap();
        }
    }
}
