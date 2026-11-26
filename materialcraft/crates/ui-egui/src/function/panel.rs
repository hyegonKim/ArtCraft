use super::{FunctionEditor, FunctionTarget, canvas::canvas};
use crate::MaterialCraftApp;
use egui::Ui;

pub(crate) fn show_graph(app: &mut MaterialCraftApp, ui: &mut Ui) {
    let index = app.session.graph;
    let mut editor = app.function_editor.take().filter(|editor| editor.graph == index).unwrap_or_else(|| FunctionEditor::standalone(index));
    if editor.target == FunctionTarget::Graph {
        editor.selection = app.session.selection.iter().filter_map(|id| u32::try_from(id.0).ok()).collect();
    }
    if let Some(graph) = editor.current(app)
        && let Some(target) = editor.command_target()
        && let Ok(document) = app.session.graph()
    {
        super::warning_banner::show(ui, document, &target, graph);
    }
    canvas(app, &mut editor, ui);
    super::actions::finish_editor(app, editor);
}
