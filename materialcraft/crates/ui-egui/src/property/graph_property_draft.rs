use crate::MaterialCraftApp;
use materialcraft_engine::GraphTarget;
use serde_json::json;

/// Text retained until focus leaves the field or the inspector changes target
pub(crate) struct GraphPropertyDraft {
    pub target: GraphTarget,
    pub key: &'static str,
    pub original: String,
    pub text: String,
    pub widget: egui::Id,
}

impl GraphPropertyDraft {
    pub fn commit(app: &mut MaterialCraftApp) {
        super::FolderPropertyDraft::commit(app);
        let Some(draft) = app.graph_property_draft.take() else { return };
        if draft.text == draft.original || draft.target.get(&app.session).is_err() {
            return;
        }

        let command = if draft.key == "name" { "graph.rename" } else { "graph.attributes" };
        app.run(command, json!({"document": draft.target.document, "graph": draft.target.graph, draft.key: draft.text}));
    }
}
