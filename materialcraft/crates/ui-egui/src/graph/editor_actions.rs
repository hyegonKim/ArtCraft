use super::editor_target::EditorTarget;
use crate::MaterialCraftApp;

pub(crate) fn shortcut_enabled(app: &MaterialCraftApp, command: &str) -> bool {
    if command != "graph.node.edit" {
        return true;
    }

    // Ctrl+E edits embedded canvases; linked sources require an explicit reference action
    match EditorTarget::resolve(app) {
        EditorTarget::Function | EditorTarget::Pattern => false,
        EditorTarget::Material => {
            app.session.selection.last().and_then(|id| app.session.graph().ok()?.node(*id).ok()).is_none_or(|node| node.def != "graph_instance")
        }
    }
}

pub(crate) fn enabled(app: &MaterialCraftApp, command: &str) -> Option<bool> {
    if command == "ui.graph.parent" {
        return Some(super::subgraph_navigation::can_return(app));
    }
    match EditorTarget::resolve(app) {
        EditorTarget::Function => crate::function::enabled(app, command),
        EditorTarget::Pattern => crate::pattern::enabled(app, command),
        EditorTarget::Material => None,
    }
}

pub(crate) fn invoke(app: &mut MaterialCraftApp, command: &str) -> bool {
    if command == "ui.graph.parent" {
        super::subgraph_navigation::parent(app);
        return true;
    }
    match EditorTarget::resolve(app) {
        EditorTarget::Function => crate::function::invoke(app, command),
        EditorTarget::Pattern => crate::pattern::invoke(app, command),
        EditorTarget::Material => false,
    }
}
