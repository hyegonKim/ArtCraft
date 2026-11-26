use materialcraft_engine::core::{NodeId, Package};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ExplorerTarget {
    Package { document: u64 },
    Folder { document: u64, path: String },
    Graph { document: u64, index: usize },
    Node { document: u64, graph: usize, node: NodeId },
    Resource { document: u64, name: String },
}

impl ExplorerTarget {
    pub fn document(&self) -> u64 {
        match self {
            Self::Package { document }
            | Self::Folder { document, .. }
            | Self::Graph { document, .. }
            | Self::Node { document, .. }
            | Self::Resource { document, .. } => *document,
        }
    }

    pub fn parameters(&self) -> Value {
        match self {
            Self::Package { .. } => json!({}),
            Self::Folder { path, .. } => json!({"folder": path}),
            Self::Graph { index, .. } => json!({"graph": index}),
            Self::Node { graph, node, .. } => json!({"graph": graph, "id": node.0}),
            Self::Resource { name, .. } => json!({"resource": name}),
        }
    }

    pub fn folder(&self, package: &Package) -> Option<String> {
        match self {
            Self::Package { .. } => Some(String::new()),
            Self::Folder { path, .. } => package.folder_paths().contains(path).then(|| path.clone()),
            Self::Graph { index, .. } => package.graphs.get(*index).map(|graph| graph.folder.clone()),
            Self::Node { graph, .. } => package.graphs.get(*graph).map(|graph| graph.folder.clone()),
            Self::Resource { name, .. } => package.resource(name).map(|resource| resource.folder.clone()),
        }
    }

    pub fn name(&self, package: &Package) -> Option<String> {
        match self {
            Self::Package { .. } => Some(package.name.clone()),
            Self::Folder { path, .. } => Some(path.rsplit('/').next().unwrap_or(path).into()),
            Self::Graph { index, .. } => package.graphs.get(*index).map(|graph| graph.name.clone()),
            Self::Node { graph, node, .. } => package.graphs.get(*graph).and_then(|graph| graph.node(*node).ok()).map(crate::graph::node_label),
            Self::Resource { name, .. } => Some(name.clone()),
        }
    }

    pub fn is_resources(&self) -> bool {
        matches!(self, Self::Folder { path, .. } if path.split('/').next().is_some_and(|part| part.eq_ignore_ascii_case("Resources")))
    }

    pub fn is_resources_root(&self) -> bool {
        matches!(self, Self::Folder { path, .. } if path.eq_ignore_ascii_case("Resources"))
    }
}
