use materialcraft_core::{FunctionGraph, FunctionNode};

pub fn can_be_output(operation: &str) -> bool {
    operation == "instance" || (operation != "passthrough" && super::definition(operation).is_some())
}

pub fn output_node(graph: &FunctionGraph) -> Option<&FunctionNode> {
    graph.output.and_then(|id| graph.node(id)).filter(|node| node.missing.is_some() || can_be_output(&node.op))
}
