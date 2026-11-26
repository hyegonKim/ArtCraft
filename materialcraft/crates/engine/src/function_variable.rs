use materialcraft_core::ValueType;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionVariable {
    pub name: String,
    pub value_type: ValueType,
    pub system: bool,
}
