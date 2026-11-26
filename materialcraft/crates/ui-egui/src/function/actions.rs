use super::FunctionEditor;
use crate::MaterialCraftApp;

pub(super) fn editor_enabled(app: &MaterialCraftApp, editor: &FunctionEditor, command: &str) -> Option<bool> {
    if command == "graph.node.edit" {
        return Some(
            editor.selection.len() == 1
                && editor.current(app).zip(editor.selection.first()).and_then(|(graph, id)| graph.node(*id)).is_some_and(|node| node.instance.is_some()),
        );
    }
    let target = editor.command_target().map(materialcraft_engine::SelectionTarget::Function);
    if let Some(enabled) = crate::graph::wire_actions::enabled(app, command, &editor.selected_wires, target.as_ref()) {
        return Some(enabled && editor.current(app).is_some());
    }

    if app.session.annotation_selection.is_some() && matches!(command, "edit.delete" | "edit.deleteRelink") {
        return None;
    }
    if let Some(target) = editor.command_target()
        && let Some(enabled) = materialcraft_engine::selection_commands::enabled_for(
            &app.session,
            &materialcraft_engine::SelectionTarget::Function(target),
            &editor.selection,
            command,
        )
    {
        return Some(enabled && editor.current(app).is_some());
    }
    match command {
        "edit.undo" | "edit.redo" | "edit.finish" | "graph.cook" | "ui.graph.snapToGrid" => None,
        "ui.graph.previousPin" | "ui.graph.nextPin" | "graph.comment.add" | "graph.frame.add" | "graph.pin.add" => None,
        command if command.starts_with("graph.") || command.starts_with("ui.graph.") || command.starts_with("edit.") => Some(false),
        _ => None,
    }
}

pub(crate) fn enabled(app: &MaterialCraftApp, command: &str) -> Option<bool> {
    app.function_editor.as_ref().and_then(|editor| editor_enabled(app, editor, command))
}

pub(super) fn invoke_editor(app: &mut MaterialCraftApp, editor: &mut FunctionEditor, command: &str) -> bool {
    let Some(allowed) = editor_enabled(app, editor, command) else { return false };

    if !allowed {
        return true;
    }

    if command.starts_with("graph.wire.") || (command == "edit.delete" && !editor.selected_wires.is_empty()) {
        let command = if command == "edit.delete" { "graph.wire.delete" } else { command };
        let target = editor.command_target().map(materialcraft_engine::SelectionTarget::Function);
        crate::graph::wire_actions::edit(app, &mut editor.selected_wires, target, command);
        return true;
    }

    if command == "graph.node.edit" {
        editor.open_reference = editor.selection.first().copied();
        return true;
    }
    let parameters = editor.selection_parameters(command);
    crate::graph::selection_commands::invoke(app, &mut editor.selection, command, parameters);
    if matches!(command, "edit.selectAll" | "edit.selectNone" | "edit.paste" | "edit.pasteWithoutLinks" | "edit.duplicate" | "edit.duplicateWithoutLinks") {
        editor.selected_wires.clear();
    }
    true
}

pub(crate) fn invoke(app: &mut MaterialCraftApp, command: &str) -> bool {
    let Some(mut editor) = app.function_editor.take() else { return false };

    let handled = invoke_editor(app, &mut editor, command);
    finish_editor(app, editor);
    handled
}

pub(super) fn finish_editor(app: &mut MaterialCraftApp, mut editor: FunctionEditor) {
    let reference =
        editor.open_reference.take().and_then(|id| app.run("graph.instance.reference", serde_json::json!({"id": id, "function": editor.command_target()})));
    app.function_editor = Some(editor);
    if let Some(reference) = reference
        && let Some(graph) = reference.get("graph").and_then(serde_json::Value::as_u64).and_then(|graph| usize::try_from(graph).ok())
    {
        crate::graph::subgraph_navigation::open_external_instance(app, &reference, graph);
    }
}
