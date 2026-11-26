use materialcraft_engine::core::NodeId;

pub(crate) struct GraphParent {
    pub document: u64,
    pub graph: usize,
    pub selection: Vec<NodeId>,
    pub function: Option<crate::function::FunctionEditor>,
    pub pattern: Option<crate::pattern::PatternEditor>,
    pub camera: crate::state::GraphCamera,
}

impl GraphParent {
    pub(super) fn capture(app: &mut crate::MaterialCraftApp) -> Self {
        Self {
            document: app.session.document_id,
            graph: app.session.graph,
            selection: app.session.selection.clone(),
            camera: app.ui.graph_camera,
            function: app.function_editor.take(),
            pattern: app.pattern_editor.take(),
        }
    }
}
