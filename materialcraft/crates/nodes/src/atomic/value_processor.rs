use super::ATOMIC;
use crate::{NodeDefinition, node};

pub(super) fn definition() -> NodeDefinition {
    node("value_processor", "Value processor", ATOMIC)
        .desc("Computes a typed value with a function graph.")
        .atomic()
        .any_in("input0", "Input image 0")
        .function("function", "Function", materialcraft_core::FunctionGraph::default())
        .output("output", "Value", crate::PortKind::Value)
        .cook(crate::node_definition_builder::no_cook)
}
