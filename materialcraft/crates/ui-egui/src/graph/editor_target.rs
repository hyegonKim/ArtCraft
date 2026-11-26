use crate::MaterialCraftApp;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EditorTarget {
    Material,
    Function,
    Pattern,
}

impl EditorTarget {
    pub(crate) fn annotation_active(app: &MaterialCraftApp) -> bool {
        app.session.annotation_selection.is_some() && app.session.annotation_target == app.graph.annotation_target
    }

    pub(crate) fn resolve(app: &MaterialCraftApp) -> Self {
        if app.function_editor.as_ref().is_some_and(|editor| editor.current(app).is_some()) {
            Self::Function
        } else if crate::pattern::active(app) {
            Self::Pattern
        } else if app.session.graph().is_ok_and(|graph| graph.function.is_some()) {
            Self::Function
        } else {
            Self::Material
        }
    }
}
