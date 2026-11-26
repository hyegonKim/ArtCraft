use crate::{PortKind, find};
use materialcraft_core::{Graph, GraphPort, NodeId, Value, ValueType};

pub fn graph_ports(graph: &Graph) -> (Vec<GraphPort>, Vec<GraphPort>) {
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for node in &graph.nodes {
        let type_code = match node.def.as_str() {
            "input_color" => 1,
            "input_grayscale" => 2,
            "input_value" => node.params.get("default").and_then(ValueType::of).unwrap_or(ValueType::Float) as u32,
            "output" => output_type(graph, node.id, "output"),
            _ => continue,
        };
        let identifier = match node.params.get("identifier") {
            Some(Value::Text(name)) if !name.is_empty() => name.clone(),
            _ => if node.def == "output" { "output" } else { "input" }.into(),
        };
        let port = GraphPort { identifier, type_code };
        if node.def == "output" {
            outputs.push(port);
        } else {
            inputs.push(port);
        }
    }
    (inputs, outputs)
}

pub fn output_type(graph: &Graph, id: NodeId, port: &str) -> u32 {
    if let Ok(Some(kind)) = crate::output_port_value_type(graph, id, port) {
        return kind as u32;
    }
    let mut pending = vec![(id, port.to_string())];
    let mut visited = std::collections::BTreeSet::new();
    while let Some((id, port)) = pending.pop() {
        if !visited.insert((id, port.clone())) {
            continue;
        }
        let Ok(node) = graph.node(id) else { continue };
        if let Some(instance) = &node.instance {
            if instance.outputs.iter().any(|output| output.identifier == port && output.type_code == 1) {
                return 1;
            }
            continue;
        }
        let Some(definition) = find(&node.def) else { return 1 };
        if let Some(parameter) = definition.param("color_mode") {
            if node.params.get("color_mode").unwrap_or(&parameter.default).as_bool() == Some(true) {
                return 1;
            }
            continue;
        }
        match definition.outputs.iter().find(|output| output.id == port).map(|output| output.kind) {
            Some(PortKind::Color) => return 1,
            Some(PortKind::Gray) => continue,
            _ => {}
        }
        for source in definition.inputs.iter().filter(|input| input.kind != PortKind::Gray).filter_map(|input| graph.input_of(id, input.id)) {
            pending.push((source.from, source.from_port.clone()));
            if node.def != "blend" {
                break;
            }
        }
    }
    2
}
