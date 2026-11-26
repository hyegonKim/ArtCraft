use crate::{MaterialCraftApp, graph::editor_target::EditorTarget, i18n::tr};
use materialcraft_engine::{LibraryCanvas, LibraryEntry, LibraryItem};

/// The graph the library adds to, and which native function operations it can take
pub(crate) struct LibraryTarget {
    pub(super) canvas: LibraryCanvas,
    operations: Vec<&'static str>,
}

impl LibraryTarget {
    /// Resolve the active editor and its permitted operations.
    pub(crate) fn of(app: &MaterialCraftApp) -> LibraryTarget {
        match EditorTarget::resolve(app) {
            EditorTarget::Material => LibraryTarget { canvas: LibraryCanvas::Material, operations: Vec::new() },
            EditorTarget::Pattern => LibraryTarget { canvas: LibraryCanvas::Pattern, operations: Vec::new() },
            EditorTarget::Function => LibraryTarget { canvas: LibraryCanvas::Function, operations: crate::function::library::available_operations(app) },
        }
    }

    /// Whether `entry` can go into the open graph now
    pub(crate) fn takes(&self, entry: &LibraryEntry) -> bool {
        entry.missing.is_none()
            && (entry.canvas == LibraryCanvas::Any || entry.canvas == self.canvas)
            && match &entry.item {
                LibraryItem::Operation { operation } => self.operations.contains(&operation.as_str()),
                _ => true,
            }
    }

    /// Why `entry` can't go into the open graph, when it can't
    pub(super) fn refusal(&self, entry: &LibraryEntry) -> Option<String> {
        if let Some(missing) = &entry.missing {
            return Some(missing.clone());
        }

        if entry.canvas != LibraryCanvas::Any && entry.canvas != self.canvas {
            return Some(
                match entry.canvas {
                    LibraryCanvas::Any | LibraryCanvas::Material => tr("Open a graph to add this"),
                    LibraryCanvas::Function => tr("Open a function graph or a processor's function to add this"),
                    LibraryCanvas::Pattern => tr("Open an FX-Map to add this"),
                }
                .to_string(),
            );
        }
        (!self.takes(entry)).then(|| tr("This function can't use this node").to_string())
    }
}
