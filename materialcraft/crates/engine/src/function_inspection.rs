use crate::FunctionGraphTarget;
use materialcraft_core::{FunctionGraph, Graph, Value};

pub fn warnings_for(document: &Graph, target: &FunctionGraphTarget, function: &FunctionGraph) -> Vec<String> {
    let mut messages = materialcraft_nodes::function::warnings(function);
    if let Some(output) = materialcraft_nodes::function::output_node(function)
        && let Err(message) = crate::function_output::validate(document, target, function, output.id)
    {
        messages.push(message);
    }
    messages
}

pub fn warnings(graph: &Graph) -> Vec<(FunctionGraphTarget, Vec<String>)> {
    let mut entries = Vec::new();
    let mut inspect = |target: FunctionGraphTarget, function: &FunctionGraph| {
        let messages = warnings_for(graph, &target, function);
        if !messages.is_empty() {
            entries.push((target, messages));
        }
    };
    if let Some(function) = &graph.function {
        inspect(FunctionGraphTarget::Graph, function);
    }

    for node in &graph.nodes {
        for (parameter, function) in &node.base.functions {
            inspect(FunctionGraphTarget::Base { node: node.id, parameter: *parameter }, function);
        }
        for (parameter, function) in &node.functions {
            inspect(FunctionGraphTarget::Dynamic { node: node.id, parameter: parameter.clone() }, function);
        }
        let defaults = materialcraft_nodes::find(&node.def)
            .into_iter()
            .flat_map(|definition| &definition.params)
            .filter(|parameter| !node.params.contains_key(parameter.id))
            .map(|parameter| (parameter.id, &parameter.default));
        for (parameter, value) in node.params.iter().map(|(parameter, value)| (parameter.as_str(), value)).chain(defaults) {
            match value {
                Value::Function(function) => inspect(FunctionGraphTarget::Parameter { node: node.id, parameter: parameter.into() }, function),
                Value::PatternGraph(pattern) => {
                    for child in &pattern.nodes {
                        for (parameter, value) in &child.parameters {
                            if let Value::Function(function) = value {
                                inspect(FunctionGraphTarget::Pattern { owner: node.id, node: child.id, parameter: parameter.clone() }, function);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    entries
}
