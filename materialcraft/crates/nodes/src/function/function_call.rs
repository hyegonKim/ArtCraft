use super::{FunctionDefinition, Program};

pub(crate) const DEFINITION: FunctionDefinition = FunctionDefinition { id: "instance", label: "Function instance", inputs: &[], category: "Graph" };

pub(crate) struct FunctionCall {
    pub program: Program,
    pub inputs: Vec<String>,
    pub defaults: std::collections::BTreeMap<String, materialcraft_core::Value>,
}
