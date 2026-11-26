use crate::{AnnotationSelection, Clip, EmbeddedClipboard, MaterialPreview, Session, View2d, View3d, snapshot::Snapshot};
use materialcraft_core::{NodeId, Package};

/// Editing state retained while another SBS package is active
pub struct OpenDocument {
    pub id: u64,
    pub pkg: Package,
    pub path: Option<String>,
    pub graph: usize,
    pub modified: bool,
    selection: Vec<NodeId>,
    annotation_selection: Option<AnnotationSelection>,
    annotation_target: crate::AnnotationTarget,
    view2d: View2d,
    view3d: View3d,
    material_preview: MaterialPreview,
    load_warnings: Vec<String>,
    clipboard: Option<Clip>,
    embedded_clipboard: Option<EmbeddedClipboard>,
    pub(crate) history: crate::document_history::DocumentHistory,
}

impl OpenDocument {
    fn snapshot(&self, label: &str) -> Snapshot {
        Snapshot {
            pkg: self.pkg.clone(),
            graph: self.graph,
            selection: self.selection.clone(),
            annotation_selection: self.annotation_selection,
            annotation_target: self.annotation_target.clone(),
            material_preview: self.material_preview.clone(),
            label: label.into(),
            modified: self.modified,
            // The active document's cook cache cannot be reused after activation
            dirty: true,
            cook_epoch: 0,
        }
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.pkg = snapshot.pkg;
        self.graph = snapshot.graph.min(self.pkg.graphs.len().saturating_sub(1));
        self.selection = snapshot.selection;
        self.annotation_selection = snapshot.annotation_selection;
        self.annotation_target = snapshot.annotation_target;
        self.material_preview = snapshot.material_preview;
        self.modified = snapshot.modified;
    }

    pub(crate) fn edit<T>(&mut self, label: &str, key: Option<&str>, limit: usize, edit: impl FnOnce(&mut Package) -> crate::Result<T>) -> crate::Result<T> {
        let snapshot = self.snapshot(label);
        let result = match edit(&mut self.pkg) {
            Ok(result) => result,
            Err(error) => {
                self.restore(snapshot);
                return Err(error);
            }
        };
        self.history.record(snapshot, key, limit);
        self.modified = true;
        Ok(result)
    }

    pub(crate) fn undo(&mut self, limit: usize) -> Option<String> {
        if !self.history.can_undo() {
            return None;
        }

        let snapshot = self.history.undo(self.snapshot(""), limit)?;
        let label = snapshot.label.clone();
        self.restore(snapshot);
        Some(label)
    }

    pub(crate) fn redo(&mut self, limit: usize) -> Option<String> {
        if !self.history.can_redo() {
            return None;
        }

        let snapshot = self.history.redo(self.snapshot(""), limit)?;
        let label = snapshot.label.clone();
        self.restore(snapshot);
        Some(label)
    }

    pub(crate) fn new(id: u64, pkg: Package, path: Option<String>) -> Self {
        Self {
            id,
            material_preview: MaterialPreview::for_graph(&pkg, 0),
            pkg,
            path,
            graph: 0,
            modified: false,
            selection: Vec::new(),
            annotation_selection: None,
            annotation_target: Default::default(),
            view2d: View2d::default(),
            view3d: View3d::default(),
            load_warnings: Vec::new(),
            clipboard: None,
            embedded_clipboard: None,
            history: Default::default(),
        }
    }

    pub(crate) fn exchange(&mut self, session: &mut Session) {
        session.end_group();
        session.finish_edit();
        self.history.finish_edit();
        std::mem::swap(&mut self.id, &mut session.document_id);
        macro_rules! exchange {
            ($($field:ident),+ $(,)?) => { $(std::mem::swap(&mut self.$field, &mut session.$field);)+ };
        }
        exchange!(
            pkg,
            path,
            graph,
            modified,
            selection,
            annotation_selection,
            annotation_target,
            view2d,
            view3d,
            material_preview,
            load_warnings,
            clipboard,
            embedded_clipboard,
            history
        );
        // A worker from the previous document must never publish into this one
        session.cook_epoch = session.cook_epoch.wrapping_add(1);
        session.clear_preview_results();
        session.cooked = Default::default();
        session.clear_decoded();
        session.dirty = true;
    }
}
