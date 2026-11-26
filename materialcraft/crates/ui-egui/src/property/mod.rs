mod curve;
mod expose_parameter_dialog;
mod folder_property_draft;
mod gradient;
mod graph;
mod graph_attributes;
mod graph_property_draft;
mod input;
mod node;
mod parameter;
mod parameter_control;
mod section;

pub use curve::curve_editor;
pub(crate) use expose_parameter_dialog::ExposeParameterDialog;
pub(crate) use folder_property_draft::FolderPropertyDraft;
pub use gradient::gradient_editor;
pub(crate) use graph_property_draft::GraphPropertyDraft;
pub(crate) use section::section;

use crate::{MaterialCraftApp, graph::editor_target::EditorTarget, i18n::tr};
use egui::Ui;

/// The panel title: "<node> - Properties" or "<graph> - Properties"
pub fn title(app: &MaterialCraftApp) -> String {
    if let Some((_, path)) = &app.inspected_folder {
        return format!("{} - {}", path.rsplit('/').next().unwrap_or(path), tr("Properties"));
    }

    if let Some(target) = app.inspected_graph
        && let Ok(graph) = target.get(&app.session)
    {
        return format!("{} - {}", graph.name, tr("Properties"));
    }

    match EditorTarget::resolve(app) {
        EditorTarget::Pattern if !EditorTarget::annotation_active(app) => return crate::pattern::properties::title(app),
        EditorTarget::Function if !EditorTarget::annotation_active(app) => {
            return crate::function::properties::title(app).unwrap_or_else(|| {
                let owner = app
                    .function_editor
                    .as_ref()
                    .and_then(|editor| editor.node)
                    .and_then(|id| app.session.graph().ok()?.node(id).ok())
                    .map(crate::graph::node_label);
                let name = owner
                    .or_else(|| app.session.graph().ok().filter(|graph| graph.function.is_some()).map(|graph| graph.name.clone()))
                    .unwrap_or_else(|| tr("Function Graph").into());
                format!("{name} - {}", tr("Properties"))
            });
        }
        _ => {}
    }

    if let Some(selection) = app.session.annotation_selection {
        let name = match selection {
            materialcraft_engine::AnnotationSelection::Frame(_) => tr("Frame"),
            materialcraft_engine::AnnotationSelection::Comment(_) => tr("Comment"),
            materialcraft_engine::AnnotationSelection::Pin(_) => tr("Navigation Pin"),
        };
        return format!("{name} - {}", tr("Properties"));
    }

    let sel = crate::graph::node_selection::inspected(&app.session.selection, app.graph.inspected_node)
        .and_then(|id| app.session.graph().ok()?.node(id).ok().map(crate::graph::node_label));
    let name = sel.or_else(|| app.session.graph().ok().map(|g| g.name.clone())).unwrap_or_default();
    format!("{name} - {}", tr("Properties"))
}

pub fn show(app: &mut MaterialCraftApp, ui: &mut Ui) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.set_width(ui.available_width());
        if let Some((document, path)) = app.inspected_folder.clone() {
            folder_property_draft::show(app, ui, document, &path);
            return;
        }

        if let Some(target) = app.inspected_graph {
            if target.get(&app.session).is_ok() {
                graph::show(app, ui, target);
                return;
            }

            app.inspected_graph = None;
        }

        match EditorTarget::resolve(app) {
            EditorTarget::Pattern if !EditorTarget::annotation_active(app) => {
                crate::pattern::properties::show(app, ui);
                return;
            }
            EditorTarget::Function if !EditorTarget::annotation_active(app) => {
                if !crate::function::properties::show(app, ui) {
                    if app.function_editor.as_ref().is_some_and(|editor| matches!(editor.target, crate::function::FunctionTarget::Graph)) {
                        graph::show(app, ui, materialcraft_engine::GraphTarget::active(&app.session));
                    } else if let Some(owner) =
                        app.function_editor.as_ref().and_then(|editor| editor.node).and_then(|id| app.session.graph().ok()?.node(id).ok()).cloned()
                    {
                        node::show(app, ui, &owner);
                    } else {
                        ui.label(tr("Select a node"));
                    }
                }
                return;
            }
            _ => {}
        }

        if let Some(selection) = app.session.annotation_selection {
            ui.push_id((app.session.graph, format!("{selection:?}")), |ui| crate::annotation::show(app, ui, selection));
            return;
        }

        let node = crate::graph::node_selection::inspected(&app.session.selection, app.graph.inspected_node)
            .and_then(|id| app.session.graph().ok()?.node(id).ok().cloned());
        ui.push_id((app.session.graph, node.as_ref().map(|n| n.id)), |ui| match node {
            Some(n) => node::show(app, ui, &n),
            None => graph::show(app, ui, materialcraft_engine::GraphTarget::active(&app.session)),
        });
    });
}
