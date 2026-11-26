use super::{Program, catalog, instruction::Instruction};
use materialcraft_core::FunctionGraph;
use std::collections::{BTreeMap, BTreeSet};

pub fn compile(graph: &FunctionGraph, seed: u32) -> Result<Program, String> {
    compile_nested(graph, seed, 0, &mut 4096)
}

fn compile_nested(graph: &FunctionGraph, seed: u32, depth: usize, remaining: &mut usize) -> Result<Program, String> {
    if depth > 8 || graph.nodes.len() > *remaining {
        return Err("function exceeds the 8-level or 4096-node execution limit".into());
    }
    *remaining -= graph.nodes.len();
    let output = super::output_node(graph).ok_or("function has no result node")?.id;
    let mut nodes = BTreeMap::new();
    for node in &graph.nodes {
        if nodes.insert(node.id, node).is_some() {
            return Err(format!("duplicate function node {}", node.id));
        }
    }

    let mut pending = vec![(output, false)];
    let mut active = BTreeSet::new();
    let mut indices = BTreeMap::new();
    let mut instructions: Vec<Instruction> = Vec::new();
    while let Some((id, visited)) = pending.pop() {
        if indices.contains_key(&id) {
            continue;
        }

        let node = nodes.get(&id).ok_or_else(|| format!("missing function node {id}"))?;
        if node.missing.is_some() {
            return Err(format!("Missing function operation `{}`", node.op));
        }
        let definition = if node.instance.is_some() {
            &super::function_call::DEFINITION
        } else {
            catalog::definition(&node.op).ok_or_else(|| format!("unknown function `{}`", node.op))?
        };
        let names: Vec<_> = super::input_identifiers(node).collect();
        let missing_input = names
            .iter()
            .zip(&node.inputs)
            .any(|(name, input)| input.is_none() && !node.instance.as_ref().is_some_and(|instance| instance.defaults.contains_key(*name)));
        if node.inputs.len() != names.len() || missing_input {
            return Err(format!("{} ({id}) requires connected inputs: {}", super::node_label(node), names.join(", ")));
        }

        // Preserved legacy connectors must not evaluate a duplicate random or stateful input twice
        let ports: Vec<_> = names
            .into_iter()
            .zip(&node.inputs)
            .filter(|(name, _)| node.instance.as_ref().is_none_or(|instance| instance.defaults.contains_key(*name)))
            .collect();
        let sources: Vec<_> = ports.iter().filter_map(|(_, input)| **input).collect();
        if !visited {
            if !active.insert(id) {
                return Err(format!("function cycle at node {id}"));
            }

            pending.push((id, true));
            pending.extend(sources.iter().rev().map(|input| (*input, false)));
            continue;
        }

        active.remove(&id);
        let inputs: Vec<usize> =
            sources.iter().map(|input| indices.get(input).copied().ok_or("unresolved function input".to_string())).collect::<Result<_, _>>()?;
        let call = if let Some(instance) = &node.instance {
            let source = node.source.as_ref().ok_or_else(|| format!("unresolved Function Graph `{}`", instance.graph))?.as_ref().map_err(Clone::clone)?;
            let program = compile_nested(source, seed, depth + 1, remaining)?;
            for (port, index) in instance
                .inputs
                .iter()
                .zip(&node.inputs)
                .filter(|(port, input)| input.is_some() && instance.defaults.contains_key(&port.identifier))
                .map(|(port, _)| port)
                .zip(&inputs)
            {
                if instructions.get(*index).is_some_and(|instruction| instruction.type_code != port.type_code) {
                    return Err(format!("{}: input `{}` has the wrong type", instance.graph, port.identifier));
                }
            }
            let inputs = ports.iter().filter(|(_, input)| input.is_some()).map(|(name, _)| (*name).to_string()).collect();
            Some(super::function_call::FunctionCall { program, inputs, defaults: instance.defaults.clone() })
        } else {
            None
        };
        let value = node.value.clone().or_else(|| catalog::default_value(&node.op));
        let input_type = |index: usize| inputs.get(index).and_then(|index| instructions.get(*index)).map_or(256, |instruction| instruction.type_code);
        let inferred = super::inferred_type::inferred_type(&node.op, value.as_ref(), input_type);
        let type_code = call.as_ref().map_or_else(|| if node.type_code == 0 { inferred } else { node.type_code }, |call| call.program.output_type());
        let pure = call.is_none()
            && !matches!(node.op.as_str(), "set" | "sequence" | "while" | "rand" | "isdefined")
            && !node.op.starts_with("get_")
            && inputs.iter().all(|index| instructions.get(*index).is_some_and(|instruction| instruction.pure));
        indices.insert(id, instructions.len());
        instructions.push(Instruction { id, definition, inputs, value, type_code, pure, call });
    }

    let output = indices.get(&output).copied().ok_or("missing result instruction")?;
    Ok(Program { instructions, output, seed })
}
