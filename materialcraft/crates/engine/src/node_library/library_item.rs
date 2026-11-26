use super::LibraryCanvas;
use materialcraft_core::PatternNodeKind;
use serde::Serialize;

/// What adding an entry puts in the graph
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LibraryItem {
    /// A native node, by catalog id
    Node {
        def: String,
    },
    Comment,
    Frame,
    Pin,
    /// An instance of a graph from a library package
    Instance {
        package: String,
        graph: String,
    },
    /// A native function operation
    Operation {
        operation: String,
    },
    /// An instance of a function graph from a library package
    FunctionInstance {
        package: String,
        graph: String,
    },
    /// An FX-Map node
    Pattern {
        kind: PatternNodeKind,
    },
    /// An environment image
    Bitmap {
        resource: String,
    },
}

impl LibraryItem {
    /// Return the canvas that accepts this item.
    pub fn canvas(&self) -> LibraryCanvas {
        match self {
            Self::Comment | Self::Frame | Self::Pin => LibraryCanvas::Any,
            Self::Node { def } if def == "dot" => LibraryCanvas::Any,
            Self::Operation { .. } | Self::FunctionInstance { .. } => LibraryCanvas::Function,
            Self::Pattern { .. } => LibraryCanvas::Pattern,
            _ => LibraryCanvas::Material,
        }
    }
}
