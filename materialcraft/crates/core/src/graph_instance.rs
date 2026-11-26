use crate::GraphPort;
use serde::{Deserialize, Serialize};

/// An SBS graph reference and its last known interface, without a copy of its nodes
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphInstance {
    /// Display label resolved from the source; the SBS reference keeps its identifier
    #[serde(skip)]
    pub label: String,
    /// Empty for a graph in the same package
    pub package: String,
    pub graph: String,
    pub inputs: Vec<GraphPort>,
    pub outputs: Vec<GraphPort>,
    /// Resolved function input defaults; the source package remains authoritative
    #[serde(skip)]
    pub defaults: std::collections::BTreeMap<String, crate::Value>,
}
