use super::{graph_property_draft::GraphPropertyDraft, section};
use crate::{MaterialCraftApp, i18n::tr};
use materialcraft_engine::GraphTarget;

pub(super) fn show(app: &mut MaterialCraftApp, ui: &mut egui::Ui, target: GraphTarget) {
    if app.graph_property_draft.as_ref().is_some_and(|draft| draft.target != target) {
        GraphPropertyDraft::commit(app);
    }

    let Ok(graph) = target.get(&app.session) else { return };
    let draft = &mut app.graph_property_draft;
    section(ui, "graph-attributes", tr("Attributes"), |ui| {
        for (key, label, current) in [
            ("name", "Identifier", &graph.name),
            ("description", "Description", &graph.attributes.description),
            ("category", "Category", &graph.attributes.category),
            ("label", "Label", &graph.attributes.label),
            ("author", "Author", &graph.attributes.author),
            ("authorUrl", "Author URL", &graph.attributes.author_url),
            ("tags", "Tags", &graph.attributes.tags),
            ("group", "Group", &graph.attributes.group),
            ("userData", "User Data", &graph.attributes.user_data),
        ] {
            if graph.function.is_none() && !matches!(key, "name" | "label") {
                continue;
            }

            let widget = ui.make_persistent_id((target, key));
            if draft.as_ref().is_some_and(|draft| draft.widget == widget && draft.original != *current) {
                *draft = None;
            }

            let pending = draft.as_ref().filter(|draft| draft.widget == widget && draft.original == *current);
            let mut text = pending.map_or_else(|| current.clone(), |draft| draft.text.clone());
            ui.label(tr(label));
            let editor = if key == "description" { egui::TextEdit::multiline(&mut text).desired_rows(6) } else { egui::TextEdit::singleline(&mut text) };
            let response = ui.add(editor.id(widget).desired_width(ui.available_width()));
            crate::widgets::name(&response, egui::Role::TextInput, label);
            if response.changed() {
                *draft = Some(GraphPropertyDraft { target, key, original: current.clone(), text, widget });
            }
            ui.add_space(5.0);
        }
    });
    if app.graph_property_draft.as_ref().is_some_and(|draft| !ui.memory(|memory| memory.has_focus(draft.widget))) {
        GraphPropertyDraft::commit(app);
    }
}
