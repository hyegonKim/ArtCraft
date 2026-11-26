use super::FunctionEditor;
use crate::{MaterialCraftApp, graph::edit_menu, i18n::tr, menus};
use egui::Ui;
use materialcraft_engine::FunctionEdit;
use serde_json::json;

pub(super) fn show(app: &mut MaterialCraftApp, editor: &mut FunctionEditor, ui: &mut Ui, node: Option<u32>) {
    ui.set_min_width(290.0);
    if let Some(node) = node {
        node_actions(app, editor, ui, node);
    } else {
        background_actions(app, editor, ui);
    }
    selection_actions(app, editor, ui);
}

fn node_actions(app: &mut MaterialCraftApp, editor: &mut FunctionEditor, ui: &mut Ui, node: u32) {
    annotations(app, editor, ui);
    if ui.button(tr("Copy info to clipboard")).clicked() {
        if let Some(info) = app.run("graph.function.node.info", json!({"target": editor.command_target(), "id": node}))
            && let Some(text) = info.get("text").and_then(serde_json::Value::as_str)
        {
            ui.ctx().copy_text(text.into());
        }
        ui.close();
    }
    ui.separator();
    if editor.current(app).and_then(|graph| graph.node(node)).is_some_and(|node| node.instance.is_some()) && ui.button(tr("Open reference")).clicked() {
        editor.open_reference = Some(node);
        ui.close();
    }
    if let Some(graph) = editor.current(app)
        && graph.node(node).is_some_and(|node| materialcraft_engine::nodes::function::can_be_output(&node.op))
    {
        let output = graph.output == Some(node);
        let compatible = editor
            .command_target()
            .zip(app.session.graph().ok())
            .is_some_and(|(target, document)| materialcraft_engine::function_output::validate(document, &target, graph, node).is_ok());
        if ui.add_enabled(editor.selection.len() == 1 && !output && compatible, egui::Button::new(tr("Set as output node"))).clicked() {
            editor.edit(app, FunctionEdit::Output { id: node }, false);
            ui.close();
        }
        ui.separator();
    }
    let label = format!("{} ({})", tr("Create graph from selection"), editor.selection.len());
    let command = "graph.createFromSelection";
    if edit_menu::button(ui, &label, command, super::actions::editor_enabled(app, editor, command).unwrap_or(false)) {
        super::actions::invoke_editor(app, editor, command);
    }
}

fn background_actions(app: &mut MaterialCraftApp, editor: &mut FunctionEditor, ui: &mut Ui) {
    let Some(target) = editor.command_target() else { return };

    let mut add = None;
    ui.menu_button(tr("Add node"), |ui| {
        let entries =
            target.available_operations(&app.session).filter(|operation| operation.id != "passthrough").map(|operation| (operation.id, operation.label, ""));
        add = crate::graph::add_node_menu::show(app, ui, entries);
    });
    annotations(app, editor, ui);
    if edit_menu::button(ui, "Add Navigation pin", "graph.pin.add", true) {
        app.run("graph.pin.add", json!({"at": [editor.menu_at.x, editor.menu_at.y]}));
        editor.selection.clear();
    }
    if ui.button(tr("Add Dot node")).clicked() {
        add = Some("passthrough");
        ui.close();
    }
    ui.separator();
    if edit_menu::button(ui, "Save package", "file.save", menus::enabled(app, "file.save")) {
        menus::invoke(app, ui.ctx(), "file.save");
    }
    for (label, command) in [("Paste", "edit.paste"), ("Paste without link", "edit.pasteWithoutLinks"), ("Select all", "edit.selectAll")] {
        selection_action(app, editor, ui, label, command);
    }
    for (label, command) in [("Previous pin", "ui.graph.previousPin"), ("Next pin", "ui.graph.nextPin")] {
        if edit_menu::button(ui, label, command, menus::enabled(app, command)) {
            crate::graph::actions::invoke(app, command);
            editor.set_camera(app.ui.graph_camera);
        }
    }
    ui.separator();
    if let Some(operation) = add {
        editor.edit(app, FunctionEdit::Add { operation: operation.into(), at: editor.menu_at.into(), snap: app.ui.prefs.snap_to_grid }, false);
    }
}

fn annotations(app: &mut MaterialCraftApp, editor: &mut FunctionEditor, ui: &mut Ui) {
    for (label, command, parameters) in [
        ("Add Comment", "graph.comment.add", json!({"at": [editor.menu_at.x, editor.menu_at.y]})),
        ("Add Frame", "graph.frame.add", json!({"rect": [editor.menu_at.x, editor.menu_at.y, 400, 260]})),
    ] {
        if edit_menu::button(ui, label, command, true) {
            app.run(command, parameters);
            editor.selection.clear();
        }
    }
}

fn selection_actions(app: &mut MaterialCraftApp, editor: &mut FunctionEditor, ui: &mut Ui) {
    for (label, command) in [
        ("Copy selection", "edit.copy"),
        ("Delete selection", "edit.delete"),
        ("Delete and relink", "edit.deleteRelink"),
        ("Duplicate selection", "edit.duplicate"),
        ("Duplicate selection without link", "edit.duplicateWithoutLinks"),
        ("Select upstream nodes", "graph.selectUpstream"),
        ("Select downstream nodes", "graph.selectDownstream"),
    ] {
        selection_action(app, editor, ui, label, command);
    }
    let target = editor.command_target().map(materialcraft_engine::SelectionTarget::Function);
    if crate::graph::wire_actions::swap_button(app, ui, &editor.selected_wires, target.as_ref()) {
        crate::graph::wire_actions::edit(app, &mut editor.selected_wires, target, "graph.wire.swap");
        ui.close();
    }
    selection_action(app, editor, ui, "Disable selection", "graph.node.disable");
}

fn selection_action(app: &mut MaterialCraftApp, editor: &mut FunctionEditor, ui: &mut Ui, label: &str, command: &str) {
    let enabled = super::actions::editor_enabled(app, editor, command).unwrap_or_else(|| menus::enabled(app, command));
    if edit_menu::button(ui, label, command, enabled) && !super::actions::invoke_editor(app, editor, command) {
        app.run(command, json!({}));
    }
}
