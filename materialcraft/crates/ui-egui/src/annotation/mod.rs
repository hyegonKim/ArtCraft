mod comment_properties;
mod frame_properties;
mod pin_properties;

use crate::MaterialCraftApp;
use egui::Ui;
use materialcraft_engine::AnnotationSelection;

pub(crate) fn show(app: &mut MaterialCraftApp, ui: &mut Ui, selection: AnnotationSelection) {
    match selection {
        AnnotationSelection::Frame(id) => frame_properties::show(app, ui, id),
        AnnotationSelection::Comment(id) => comment_properties::show(app, ui, id),
        AnnotationSelection::Pin(id) => pin_properties::show(app, ui, id),
    }
}

/// Resolve the destination before an editor is temporarily borrowed for drawing.
pub(crate) fn target(app: &MaterialCraftApp) -> materialcraft_engine::AnnotationTarget {
    use materialcraft_engine::{AnnotationTarget, FunctionGraphTarget};
    if let Some(target) = app.function_editor.as_ref().and_then(|editor| editor.command_target()) {
        return match target {
            FunctionGraphTarget::Graph => AnnotationTarget::Graph,
            target => AnnotationTarget::Function { target },
        };
    }
    if crate::pattern::active(app)
        && let Some(editor) = &app.pattern_editor
    {
        return AnnotationTarget::Pattern { owner: editor.node };
    }
    AnnotationTarget::Graph
}

pub(crate) fn activate(app: &mut MaterialCraftApp, target: materialcraft_engine::AnnotationTarget) {
    if app.graph.annotation_target != target {
        app.session.annotation_selection = None;
        app.graph.cancel_drag();
    }
    app.graph.annotation_target = target;
}

/// Read the active editor camera without replacing the parent graph's saved view.
pub(crate) fn camera(app: &MaterialCraftApp) -> crate::state::GraphCamera {
    if let Some(editor) = &app.function_editor {
        editor.camera()
    } else if let Some(editor) = &app.pattern_editor {
        editor.camera()
    } else {
        app.ui.graph_camera
    }
}
