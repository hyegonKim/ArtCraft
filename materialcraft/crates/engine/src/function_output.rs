use crate::FunctionGraphTarget;
use materialcraft_core::{FunctionGraph, Graph, Value, ValueType};

pub fn expected_type(document: &Graph, target: &FunctionGraphTarget) -> Option<ValueType> {
    match target {
        FunctionGraphTarget::Graph => None,
        FunctionGraphTarget::Dynamic { node, parameter } => {
            let node = document.node(*node).ok()?;
            let default = if node.def == "graph_instance" {
                node.instance.as_ref()?.defaults.get(parameter)?
            } else if node.def == "input_value" && parameter == "default" {
                node.params.get(parameter).unwrap_or(&materialcraft_nodes::find(&node.def)?.param(parameter)?.default)
            } else {
                &materialcraft_nodes::find(&node.def)?.param(parameter)?.default
            };
            ValueType::of(default)
        }
        FunctionGraphTarget::Base { parameter, .. } => Some(parameter.value_type()),
        FunctionGraphTarget::Parameter { node, parameter } => {
            let node = document.node(*node).ok()?;
            if node.def != "pixel_processor" || parameter != "function" {
                return None;
            }

            let color = node.params.get("color_mode").and_then(Value::as_bool).unwrap_or(true);
            Some(if color { ValueType::Float4 } else { ValueType::Float })
        }
        FunctionGraphTarget::Pattern { owner, node, parameter } => {
            let Value::PatternGraph(pattern) = document.node(*owner).ok()?.params.get("pattern")? else { return None };

            let node = pattern.nodes.iter().find(|candidate| candidate.id == *node)?;
            ValueType::of(&materialcraft_nodes::pattern::definition(node.kind)?.param(parameter)?.default)
        }
    }
}

pub fn validate(document: &Graph, target: &FunctionGraphTarget, function: &FunctionGraph, id: u32) -> Result<(), String> {
    let node = function.node(id).ok_or("missing output node")?;
    if !materialcraft_nodes::function::can_be_output(&node.op) {
        return Err("Dot nodes cannot be function outputs".into());
    }
    let Some(expected) = expected_type(document, target) else { return Ok(()) };

    let types = materialcraft_nodes::function::node_types(function)?;
    let actual = types.get(&id).copied().and_then(ValueType::from_code).ok_or("output node has an unknown value type")?;
    if actual != expected {
        return Err(format!("Output node returns {}; expected {}", actual.label(), expected.label()));
    }
    Ok(())
}
