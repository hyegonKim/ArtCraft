use super::sbs_value::{constant_xml, sbs_constant, sbs_constant_element, to_natural_value};
use super::sbs_writer::SbsWriter;
use super::xml::{child_element, child_value, children_named, parse_floats, xml_escape};
use materialcraft_core::{FunctionGraph, FunctionNode, Value};
use materialcraft_nodes::function;
use roxmltree::Node as XmlNode;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

pub(super) fn read(element: XmlNode<'_, '_>) -> Result<FunctionGraph, String> {
    let mut graph = FunctionGraph {
        output: child_value(element, "rootnode").and_then(|value| value.parse().ok()),
        nodes: Vec::new(),
        annotations: super::canvas::read_embedded(element),
    };
    let mut ids = BTreeSet::new();
    for element in child_element(element, "paramNodes").into_iter().flat_map(|nodes| children_named(nodes, "paramNode")) {
        if graph.nodes.len() >= 4096 {
            return Err("function exceeds the 4096-node limit".into());
        }
        let id = child_value(element, "uid").and_then(|value| value.parse().ok()).ok_or("function node has no valid uid")?;
        if !ids.insert(id) {
            return Err(format!("duplicate function uid {id}"));
        }
        let op = child_value(element, "function").ok_or("function node has no operation")?;
        let value = child_element(element, "funcDatas")
            .into_iter()
            .flat_map(|data| children_named(data, "funcData"))
            .find(|data| child_value(*data, "name") == Some(op))
            .and_then(|data| data.children().filter(|child| child.is_element()).find_map(|child| sbs_constant_element(child).or_else(|| sbs_constant(child))))
            .map(|value| to_natural_value(&value));
        let type_code = child_value(element, "type").and_then(|value| value.parse().ok()).unwrap_or(0);
        let mut instance = if op == "instance" { Some(super::function_instance::read(element, value.as_ref(), type_code)?) } else { None };
        let mut missing = instance.is_none() && function::definition(op).is_none();
        let mut names: Vec<String> = if let Some(instance) = &instance {
            instance.inputs.iter().map(|port| port.identifier.clone()).collect()
        } else {
            function::definition(op).into_iter().flat_map(|definition| definition.inputs.iter().map(|name| (*name).into())).collect()
        };
        let mut inputs = vec![None; names.len()];
        for connection in child_element(element, "connections").into_iter().flat_map(|connections| children_named(connections, "connection")) {
            let port = child_value(connection, "identifier").unwrap_or("");
            let index = if let Some(index) = names.iter().position(|name| name == port) {
                index
            } else {
                if names.len() >= 4096 {
                    return Err("Function instance exceeds the 4096-input limit".into());
                }
                if let Some(instance) = &mut instance {
                    instance.inputs.push(materialcraft_core::GraphPort { identifier: port.into(), type_code: 0 });
                } else {
                    // A newer connector contract must remain editable without running the older operation
                    missing = true;
                }
                names.push(port.into());
                inputs.push(None);
                names.len() - 1
            };
            let source = child_value(connection, "connRef").and_then(|value| value.parse().ok()).ok_or("invalid function connection")?;
            let input = inputs.get_mut(index).ok_or("invalid input index")?;
            if input.replace(source).is_some() {
                return Err(format!("{op}: duplicate input `{port}`"));
            }
        }
        let position = child_element(element, "GUILayout").and_then(|layout| child_value(layout, "gpos")).map(parse_floats).unwrap_or_default();
        graph.nodes.push(FunctionNode {
            missing: missing.then_some(names),
            missing_data: if missing {
                child_element(element, "funcDatas").map(|data| element.document().input_text()[data.range()].to_string()).unwrap_or_default()
            } else {
                String::new()
            },
            instance,
            source: None,
            id,
            op: op.into(),
            inputs,
            type_code,
            value: if op == "instance" { None } else { value.or_else(|| native_default(op)) },
            pos: [position.first().copied().unwrap_or(0.0), position.get(1).copied().unwrap_or(0.0)],
        });
    }

    if graph.output.is_some_and(|id| !ids.contains(&id)) {
        return Err("function result references a missing node".into());
    }
    if graph.nodes.iter().flat_map(|node| node.inputs.iter().flatten()).any(|id| !ids.contains(id)) {
        return Err("function connection references a missing node".into());
    }
    // Earlier files may designate a Dot; keep the graph editable without promoting another node
    if graph.output.and_then(|id| graph.node(id)).is_some_and(|node| node.missing.is_none() && !function::can_be_output(&node.op)) {
        graph.output = None;
    }
    Ok(graph)
}

fn native_default(op: &str) -> Option<Value> {
    Some(match op {
        "const_float1" => Value::Float(0.0),
        "const_float4" => Value::Float4([0.0; 4]),
        "swizzle2" | "iswizzle2" => Value::Int2([0; 2]),
        "swizzle3" | "iswizzle3" => Value::Int3([0; 3]),
        "swizzle4" | "iswizzle4" => Value::Int4([0; 4]),
        _ => return function::default_value(op),
    })
}

pub(super) fn write(graph: &FunctionGraph, writer: &mut SbsWriter) -> Result<String, String> {
    if graph.nodes.len() > 4096 {
        return Err("function exceeds the 4096-node limit".into());
    }
    let types = function::node_types(graph)?;
    let identifiers: BTreeMap<_, _> = graph.nodes.iter().map(|node| (node.id, writer.next_uid())).collect();
    let mut output = String::from("<dynamicValue>");
    if let Some(root) = function::output_node(graph).and_then(|root| identifiers.get(&root.id)) {
        let _ = write!(output, "<rootnode v=\"{root}\"/>");
    }
    output.push_str("<paramNodes>");
    for node in &graph.nodes {
        let inputs = function::input_identifiers(node);
        let inferred = types.get(&node.id).copied().unwrap_or(256);
        let code = if node.type_code == 0 { inferred } else { node.type_code };
        let _ = write!(
            output,
            "<paramNode><uid v=\"{}\"/><function v=\"{}\"/><type v=\"{code}\"/><GUILayout><gpos v=\"{} {} 0\"/></GUILayout><connections>",
            identifiers.get(&node.id).copied().ok_or("missing function uid")?,
            xml_escape(&node.op),
            node.pos[0],
            node.pos[1]
        );
        for (input, source) in inputs.zip(&node.inputs) {
            if let Some(source) = source.and_then(|source| identifiers.get(&source)) {
                let _ = write!(output, "<connection><identifier v=\"{}\"/><connRef v=\"{source}\"/></connection>", xml_escape(input));
            }
        }
        output.push_str("</connections>");
        if node.missing.is_some() && !node.missing_data.is_empty() {
            // Only a bounded data element may be emitted; command input must not inject package XML
            let options = roxmltree::ParsingOptions { allow_dtd: false, nodes_limit: 100_000 };
            if node.missing_data.len() > (1 << 20) {
                return Err("invalid preserved function data".into());
            }

            let document = roxmltree::Document::parse_with_options(&node.missing_data, options).map_err(|_| "invalid preserved function data")?;
            let root = document.root_element();
            if !root.has_tag_name("funcDatas") {
                return Err("invalid preserved function data".into());
            }

            output.push_str(node.missing_data.get(root.range()).ok_or("invalid preserved function data")?);
        } else if let Some(instance) = &node.instance {
            output.push_str(&super::function_instance::write(instance, writer));
        } else if let Some(value) = &node.value {
            let constant = constant_xml(value).ok_or("function data must be a scalar or vector constant")?;
            let _ =
                write!(output, "<funcDatas><funcData><name v=\"{}\"/><constantValue>{constant}</constantValue></funcData></funcDatas>", xml_escape(&node.op));
        }
        output.push_str("</paramNode>");
    }
    output.push_str("</paramNodes>");
    if !graph.annotations.is_empty() {
        super::canvas::write_items(&mut output, &graph.annotations.frames, &graph.annotations.comments, &graph.annotations.pins, writer);
    }
    output.push_str("</dynamicValue>");
    Ok(output)
}

pub(super) fn binding(identifier: &str, value: &Value) -> Result<FunctionGraph, String> {
    let value_type = materialcraft_core::ValueType::of(value).ok_or("only scalar and vector parameters can be exposed")?;

    Ok(FunctionGraph::input(identifier, value_type))
}
