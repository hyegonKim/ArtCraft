use super::WireRef;
use crate::{MaterialCraftApp, i18n::tr};
use egui::{Pos2, Ui};
use egui_phosphor::regular as icon;
use serde_json::json;

pub(crate) fn edit(app: &mut MaterialCraftApp, wires: &mut Vec<WireRef>, target: Option<materialcraft_engine::SelectionTarget>, command: &str) -> bool {
    if wires.is_empty() || (command == "graph.wire.swap" && !can_swap(app, wires, target.as_ref())) {
        return false;
    }

    if app.run(command, parameters(app, wires, target.as_ref())).is_some() && command == "graph.wire.delete" {
        wires.clear();
    }
    true
}

fn parameters(app: &MaterialCraftApp, wires: &[WireRef], target: Option<&materialcraft_engine::SelectionTarget>) -> serde_json::Value {
    let mut parameters = json!({"graph": app.session.graph, "wires": wires.iter().map(|(to, port)| json!({"to": to.0, "toPort": port})).collect::<Vec<_>>()});
    match target {
        Some(materialcraft_engine::SelectionTarget::Function(target)) => parameters["function"] = json!(target),
        Some(materialcraft_engine::SelectionTarget::Pattern(owner)) => parameters["pattern"] = json!(owner.0),
        None => {}
    }
    parameters
}

fn can_swap(app: &MaterialCraftApp, wires: &[WireRef], target: Option<&materialcraft_engine::SelectionTarget>) -> bool {
    wires.len() == 2 && materialcraft_engine::wire_swap::available(&app.session, &parameters(app, wires, target))
}

fn invoke_selected(app: &mut MaterialCraftApp, command: &str) -> bool {
    match super::editor_target::EditorTarget::resolve(app) {
        super::editor_target::EditorTarget::Function => {
            let Some(mut editor) = app.function_editor.take() else { return false };
            let target = editor.command_target().map(materialcraft_engine::SelectionTarget::Function);
            let handled = edit(app, &mut editor.selected_wires, target, command);
            app.function_editor = Some(editor);
            handled
        }
        super::editor_target::EditorTarget::Pattern => {
            let Some(mut editor) = app.pattern_editor.take() else { return false };
            let target = Some(materialcraft_engine::SelectionTarget::Pattern(editor.node));
            let handled = edit(app, &mut editor.selected_wires, target, command);
            app.pattern_editor = Some(editor);
            handled
        }
        super::editor_target::EditorTarget::Material => {
            let mut wires = std::mem::take(&mut app.graph.selected_wires);
            let handled = edit(app, &mut wires, None, command);
            app.graph.selected_wires = wires;
            handled
        }
    }
}

pub fn delete_selected_wires(app: &mut MaterialCraftApp) -> bool {
    invoke_selected(app, "graph.wire.delete")
}

pub fn swap_selected_wires(app: &mut MaterialCraftApp) -> bool {
    invoke_selected(app, "graph.wire.swap")
}

pub(crate) fn swap_button(app: &MaterialCraftApp, ui: &mut Ui, wires: &[WireRef], target: Option<&materialcraft_engine::SelectionTarget>) -> bool {
    can_swap(app, wires, target) && ui.add(egui::Button::new(tr("Swap links")).shortcut_text("X")).clicked()
}

pub(crate) fn enabled(app: &MaterialCraftApp, command: &str, wires: &[WireRef], target: Option<&materialcraft_engine::SelectionTarget>) -> Option<bool> {
    match command {
        "graph.wire.swap" => Some(can_swap(app, wires, target)),
        "graph.wire.delete" => Some(!wires.is_empty()),
        "edit.delete" if !wires.is_empty() => Some(true),
        _ => None,
    }
}

pub(crate) fn shortcuts(app: &mut MaterialCraftApp, ctx: &egui::Context) {
    let count = match super::editor_target::EditorTarget::resolve(app) {
        super::editor_target::EditorTarget::Function => app.function_editor.as_ref().map_or(0, |editor| editor.selected_wires.len()),
        super::editor_target::EditorTarget::Pattern => app.pattern_editor.as_ref().map_or(0, |editor| editor.selected_wires.len()),
        super::editor_target::EditorTarget::Material => app.graph.selected_wires.len(),
    };
    if count == 0 {
        return;
    }

    if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Delete) | input.consume_key(egui::Modifiers::NONE, egui::Key::Backspace)) {
        delete_selected_wires(app);
    } else if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::X)) && count == 2 {
        swap_selected_wires(app);
    }
}

pub(super) fn wire_menu(app: &mut MaterialCraftApp, ui: &mut Ui, (to, port): WireRef, at: Pos2) {
    let n = app.graph.selected_wires.len();
    if ui.button(format!("{}  {}", icon::DOT_OUTLINE, tr("Insert reroute dot"))).clicked() {
        app.run("graph.wire.insertDot", json!({"to": to.0, "toPort": port, "at": [at.x, at.y]}));
        app.graph.selected_wires.clear();
        ui.close();
    }
    if swap_button(app, ui, &app.graph.selected_wires, None) {
        swap_selected_wires(app);
        ui.close();
    }
    let label = if n >= 2 { tr("Delete selected links") } else { tr("Delete link") };

    if ui.button(format!("{}  {label}", icon::TRASH)).clicked() {
        if n < 2 {
            app.graph.selected_wires = vec![(to, port)];
        }
        delete_selected_wires(app);
        ui.close();
    }
}
