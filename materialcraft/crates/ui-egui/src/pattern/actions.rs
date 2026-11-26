use super::PatternEditor;
use crate::MaterialCraftApp;

pub(crate) fn active(app: &MaterialCraftApp) -> bool {
    app.pattern_editor.as_ref().is_some_and(|editor| {
        editor.graph == app.session.graph && app.session.graph().is_ok_and(|graph| graph.node(editor.node).is_ok_and(|node| node.def == "fx_map"))
    })
}

pub(crate) fn enabled(app: &MaterialCraftApp, command: &str) -> Option<bool> {
    if !active(app) {
        return None;
    }

    if let Some(enabled) = editor_enabled(app, app.pattern_editor.as_ref()?, command) {
        return Some(enabled);
    }
    match command {
        "ui.graph.snapToGrid"
        | "graph.cook"
        | "edit.finish"
        | "graph.frame.add"
        | "graph.comment.add"
        | "graph.pin.add"
        | "ui.graph.nextPin"
        | "ui.graph.previousPin" => None,
        command
            if command.starts_with("graph.")
                || command.starts_with("ui.graph.")
                || command.starts_with("edit.") && !matches!(command, "edit.undo" | "edit.redo") =>
        {
            Some(false)
        }
        _ => None,
    }
}

pub(crate) fn invoke(app: &mut MaterialCraftApp, command: &str) -> bool {
    let Some(allowed) = enabled(app, command) else { return false };

    if !allowed {
        return true;
    }

    let Some(mut editor) = app.pattern_editor.take() else { return true };

    invoke_editor(app, &mut editor, command);
    app.pattern_editor = Some(editor);
    true
}

pub(super) fn invoke_editor(app: &mut MaterialCraftApp, editor: &mut PatternEditor, command: &str) {
    if app.session.annotation_selection.is_some() && matches!(command, "edit.delete" | "edit.deleteRelink") {
        app.run(command, serde_json::json!({}));
        return;
    }
    if editor_enabled(app, editor, command) != Some(true) {
        return;
    }
    if command.starts_with("graph.wire.") || (command == "edit.delete" && !editor.selected_wires.is_empty()) {
        let command = if command == "edit.delete" { "graph.wire.delete" } else { command };
        let target = Some(materialcraft_engine::SelectionTarget::Pattern(editor.node));
        crate::graph::wire_actions::edit(app, &mut editor.selected_wires, target, command);
        return;
    }

    let parameters = editor.selection_parameters(command);
    crate::graph::selection_commands::invoke(app, &mut editor.selection, command, parameters);
    if matches!(command, "edit.selectAll" | "edit.selectNone" | "edit.paste" | "edit.pasteWithoutLinks" | "edit.duplicate" | "edit.duplicateWithoutLinks") {
        editor.selected_wires.clear();
    }
}

pub(super) fn editor_enabled(app: &MaterialCraftApp, editor: &PatternEditor, command: &str) -> Option<bool> {
    if app.session.annotation_selection.is_some() && matches!(command, "edit.delete" | "edit.deleteRelink") {
        return Some(true);
    }
    let target = materialcraft_engine::SelectionTarget::Pattern(editor.node);
    if let Some(enabled) = crate::graph::wire_actions::enabled(app, command, &editor.selected_wires, Some(&target)) {
        return Some(enabled && editor.current(app).is_some());
    }
    materialcraft_engine::selection_commands::enabled_for(
        &app.session,
        &materialcraft_engine::SelectionTarget::Pattern(editor.node),
        &editor.selection,
        command,
    )
    .map(|enabled| enabled && editor.current(app).is_some())
}
