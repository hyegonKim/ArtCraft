use serde::{Deserialize, Serialize};

use crate::Value;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FunctionNode {
    pub id: u32,
    // Zero requests type inference
    #[serde(default)]
    pub type_code: u32,
    pub op: String,
    /// Connector names for an unavailable operation; its wires remain editable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missing: Option<Vec<String>>,
    /// Uninterpreted function data, retained for unavailable operations on save.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub missing_data: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<crate::GraphInstance>,
    /// Resolved call body shared by cooks; SBS stores only the source reference
    #[serde(skip)]
    pub source: Option<Result<std::sync::Arc<crate::FunctionGraph>, String>>,
    // Slots follow the native operation's connector order
    #[serde(default)]
    pub inputs: Vec<Option<u32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(default)]
    pub pos: [f32; 2],
}
