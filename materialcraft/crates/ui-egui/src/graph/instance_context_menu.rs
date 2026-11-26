use crate::{MaterialCraftApp, i18n::tr};
use egui::{Pos2, Ui};
use materialcraft_engine::core::NodeId;
use serde_json::{Value, json};

use super::context_menu::{action, add_nodes, outputs};

pub(super) fn show(app: &mut MaterialCraftApp, ui: &mut Ui, at: Pos2, node: NodeId) {
    outputs(app, ui, Some(node));
    if let Some(parameters) = crate::material_preview_menu::show(ui, &app.session, app.session.graph, node) {
        app.run("view.3d.assign", parameters);
    }
    ui.separator();
    action(app, ui, "Add Comment", "graph.comment.add", json!({"at": [at.x, at.y]}));
    action(app, ui, "Add Frame", "graph.frame.add", json!({"rect": [at.x, at.y, 400.0, 260.0]}));
    if ui.button(tr("Copy info to clipboard")).clicked() {
        if let Some(info) = app.run("graph.node.info", json!({"id": node}))
            && let Some(text) = info.get("text").and_then(Value::as_str)
        {
            ui.ctx().copy_text(text.into());
        }
        ui.close();
    }
    ui.separator();
    ui.menu_button(tr("Manage parameters"), |ui| {
        action(app, ui, "Reset parameters", "graph.node.reset", json!({"id": node}));
    });
    ui.separator();
    ui.menu_button(tr("Create"), |ui| {
        ui.menu_button(tr("Add node"), |ui| add_nodes(app, ui, at));
        action(app, ui, "Add Dot node", "graph.node.add", json!({"def": "dot", "at": [at.x, at.y]}));
        action(app, ui, "Add Navigation pin", "graph.pin.add", json!({"at": [at.x, at.y]}));
    });
    ui.separator();
    if ui.add_enabled(crate::menus::enabled(app, "graph.node.edit"), egui::Button::new(tr("Open reference"))).clicked() {
        crate::menus::invoke(app, ui.ctx(), "graph.node.edit");
        ui.close();
    }
    ui.separator();
    let label = format!("{} ({})", tr("Create graph from selection"), app.session.selection.len());
    action(app, ui, &label, "graph.createFromSelection", Value::Null);
    for (label, command) in [
        ("Copy selection", "edit.copy"),
        ("Delete selection", "edit.delete"),
        ("Delete and relink", "edit.deleteRelink"),
        ("Duplicate selection", "edit.duplicate"),
        ("Duplicate selection without link", "edit.duplicateWithoutLinks"),
        ("Select upstream nodes", "graph.selectUpstream"),
        ("Select downstream nodes", "graph.selectDownstream"),
        ("Disable selection", "graph.node.disable"),
    ] {
        action(app, ui, label, command, Value::Null);
    }
    if super::wire_actions::swap_button(app, ui, &app.graph.selected_wires, None) {
        super::wire_actions::swap_selected_wires(app);
        ui.close();
    }
}
