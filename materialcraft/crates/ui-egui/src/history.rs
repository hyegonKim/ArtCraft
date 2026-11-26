use crate::MaterialCraftApp;

pub(crate) fn document(app: &MaterialCraftApp) -> u64 {
    if let Some((document, _)) = &app.inspected_folder {
        return *document;
    }

    app.inspected_graph.filter(|target| target.get(&app.session).is_ok()).map_or(app.session.document_id, |target| target.document)
}

pub(crate) fn enabled(app: &MaterialCraftApp, command: &str) -> Option<bool> {
    match command {
        "edit.undo" => Some(app.session.can_undo_document(document(app))),
        "edit.redo" => Some(app.session.can_redo_document(document(app))),
        _ => None,
    }
}
