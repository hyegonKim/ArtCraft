use crate::{MaterialCraftApp, i18n::tr};
use serde_json::json;

pub(crate) struct FolderPropertyDraft {
    document: u64,
    folder: String,
    key: &'static str,
    original: String,
    text: String,
    widget: egui::Id,
}

impl FolderPropertyDraft {
    pub fn commit(app: &mut MaterialCraftApp) {
        let Some(draft) = app.folder_property_draft.take() else { return };
        if draft.text == draft.original {
            return;
        }

        if let Some(path) = app
            .run("folder.attributes", json!({"document": draft.document, "folder": draft.folder, draft.key: draft.text}))
            .and_then(|result| result.get("folder").and_then(|path| path.as_str()).map(str::to_string))
        {
            app.inspected_folder = Some((draft.document, path.clone()));
            app.ui.explorer.selected = Some(crate::explorer::ExplorerTarget::Folder { document: draft.document, path });
        }
    }
}

pub(super) fn show(app: &mut MaterialCraftApp, ui: &mut egui::Ui, document: u64, path: &str) {
    if app.folder_property_draft.as_ref().is_some_and(|draft| draft.document != document || draft.folder != path) {
        FolderPropertyDraft::commit(app);
    }

    let Ok(package) = app.session.package(document) else { return };
    let identifier = path.rsplit('/').next().unwrap_or(path).to_string();
    let description = package.folder_descriptions.get(path).cloned().unwrap_or_default();

    super::section(ui, "folder-attributes", tr("Attributes"), |ui| {
        for (key, label, current) in [("identifier", "Identifier", identifier), ("description", "Description", description)] {
            let widget = ui.make_persistent_id(("folder-attribute", document, path, key));
            if app.folder_property_draft.as_ref().is_some_and(|draft| draft.widget == widget && draft.original != current) {
                app.folder_property_draft = None;
            }

            let mut text =
                app.folder_property_draft.as_ref().filter(|draft| draft.widget == widget).map_or_else(|| current.clone(), |draft| draft.text.clone());
            ui.label(tr(label));
            let editor = if key == "description" { egui::TextEdit::multiline(&mut text).desired_rows(6) } else { egui::TextEdit::singleline(&mut text) };
            let response =
                ui.add_enabled(key != "identifier" || !path.eq_ignore_ascii_case("Resources"), editor.id(widget).desired_width(ui.available_width()));
            crate::widgets::name(&response, egui::Role::TextInput, label);

            if response.changed() {
                // Switching fields commits the previous draft first
                if app.folder_property_draft.as_ref().is_some_and(|draft| draft.widget != widget) {
                    FolderPropertyDraft::commit(app);
                }
                app.folder_property_draft = Some(FolderPropertyDraft { document, folder: path.into(), key, original: current, text, widget });
            }
            ui.add_space(5.0);
        }
    });

    if app.folder_property_draft.as_ref().is_some_and(|draft| !ui.memory(|memory| memory.has_focus(draft.widget))) {
        FolderPropertyDraft::commit(app);
    }
}
