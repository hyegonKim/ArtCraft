use crate::{Cache, Cooked, NodeOutput, NodeResult, Options, identifier, now_ms};
use crate::{
    cache_key::{graph_tree_hash_with_registry, hash_of, image_key, variable_key},
    graph_instance::cook_instance,
    parameter::resolve_parameters,
    prepared_graph::PreparedGraph,
    value_execution::evaluate_value,
};
use materialcraft_core::{Node, Package, Value};
use materialcraft_image::{Image, Kind};
use materialcraft_nodes::{CookContext, find};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

/// The magenta image a failed node shows
pub fn error_image(w: u32, h: u32) -> Image {
    Image::filled(w, h, Kind::Color, [1.0, 0.0, 1.0, 1.0])
}

/// Lock the cache, recovering from a poisoned lock (a cache entry is never half-written)
fn lock(cache: &Mutex<Cache>) -> std::sync::MutexGuard<'_, Cache> {
    cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub(crate) fn cook_node(
    package: &Package,
    graph_index: usize,
    prepared: &PreparedGraph<'_>,
    node: &Node,
    cooked: &Cooked,
    graph_variables: &BTreeMap<String, Value>,
    options: &Options,
    cache: &Mutex<Cache>,
) -> NodeResult {
    let (input_ports, output_ports) = prepared.ports(node.id);
    let definition = find(&node.def);

    // Gather inputs by port order, with their keys
    let mut inputs = Vec::new();
    let mut incoming = Vec::new();
    let mut input_keys = Vec::new();
    let mut variables = std::borrow::Cow::Borrowed(graph_variables);
    let mut input_error = None;
    for port in input_ports {
        let source = prepared.index.input_of(node.id, &port.id).and_then(|connection| {
            let result = cooked.nodes.get(&connection.from)?;
            let index = prepared.ports(connection.from).1.iter().position(|output| output.id == connection.from_port)?;
            Some((result.outputs.get(index)?, result.key ^ hash_of(&index)))
        });
        input_keys.push(source.as_ref().map_or(0, |source| source.1));
        let output = source.map(|source| source.0);

        let expected = node.input_values.get(&port.id).copied().or_else(|| {
            node.instance
                .as_ref()?
                .inputs
                .iter()
                .find(|input| input.identifier == port.id)
                .and_then(|input| materialcraft_core::ValueType::from_code(input.type_code))
        });
        if let Some(expected) = expected {
            let value = match output.as_ref() {
                Some(NodeOutput::Value(value)) if materialcraft_core::ValueType::of(value) == Some(expected) => value.clone(),
                Some(_) => {
                    input_error = Some(format!("input `{}` requires {}", port.id, expected.label()));
                    expected.default_value()
                }
                None => expected.default_value(),
            };

            variables.to_mut().insert(port.id.clone(), value);
        } else {
            if output.as_ref().is_some_and(|output| output.as_value().is_some()) && !matches!(node.def.as_str(), "output" | "dot") {
                input_error = Some(format!("input `{}` requires an image", port.id));
            }
            inputs.push(output.and_then(NodeOutput::as_image).cloned());
        }

        incoming.push(output);
    }

    let input_base = prepared.input_base(node.id, cooked);
    let (mut base, base_values, base_error) = match crate::resolved_base_parameters::evaluate(&node.base, input_base, prepared.parent_base, &variables) {
        Ok((base, values)) => (base, values, None),
        Err(error) => (crate::ResolvedBaseParameters::resolve(&node.base, input_base, prepared.parent_base), BTreeMap::new(), Some(error)),
    };
    if node.disabled
        && let Some(input) = input_base
    {
        base = input;
    }

    let (w, h) = crate::node_size::limit(base.size, options.max_size);
    let fail = |message: String, key: u64| NodeResult {
        base,
        base_values: base_values.clone(),
        parameters: BTreeMap::new(),
        outputs: Arc::new(vec![NodeOutput::Image(error_image(w, h)); output_ports.len().max(1)]),
        error: Some(message),
        ms: 0.0,
        key,
        cached: false,
    };
    if let Some(error) = base_error {
        return fail(error, 0);
    }

    if let Some(error) = input_error {
        return fail(error, 0);
    }

    if !node.functions.is_empty() || matches!(node.def.as_str(), "pixel_processor" | "value_processor" | "fx_map") {
        base.variables(variables.to_mut());
    }

    let variables = &variables;
    let seed = base.seed;
    let mut parameters = match resolve_parameters(package, node, definition, variables, seed, base.size) {
        Ok(parameters) => parameters,
        Err(error) => return fail(error, 0),
    };

    let tile = base.tiling[0] && base.tiling[1];

    // Input nodes read the caller's image for their identifier
    let input_image = node.def.starts_with("input_").then(|| options.inputs.get(&identifier(node))).flatten();
    let resource_hash = match parameters.get("resource") {
        Some(Value::Text(name)) => package.resource(name).map_or(0, |r| r.bytes.content_hash()),
        _ => 0,
    };

    let registry = options.registry.unwrap_or(package);
    let subgraph_hash = if node.def == "graph_instance" {
        crate::graph_instance::resolve(registry, package, node).map_or(0, |(source, index, _)| graph_tree_hash_with_registry(registry, source, index))
    } else {
        0
    };

    let key = hash_of(&(
        &node.def,
        serde_json::to_string(&parameters).unwrap_or_default(),
        (w, h, seed, tile, node.disabled, base.size, base.format, base.tiling, base.pixel_ratio, base.pixel_size.map(f32::to_bits)),
        &input_keys,
        resource_hash,
        subgraph_hash,
        // A cached graph instance includes results evaluated with its host environment
        (node.def == "graph_instance").then(|| serde_json::to_string(&options.environment).unwrap_or_default()),
        input_image.map(|image| image_key(image, cache)),
        variable_key(node, &parameters, variables),
    ));
    if let Some(outputs) = lock(cache).get(key) {
        parameters.retain(|_, value| materialcraft_core::ValueType::of(value).is_some());
        return NodeResult { base, base_values, parameters, outputs, error: None, ms: 0.0, key, cached: true };
    }

    let started_ms = now_ms();
    let result: Result<Vec<NodeOutput>, String> = if node.disabled {
        let input = if let Some(primary) = definition.and_then(|definition| definition.primary_input) {
            input_ports.iter().position(|port| port.id == primary).and_then(|index| incoming.get(index)).copied().flatten()
        } else {
            incoming.iter().find_map(|output| *output)
        };
        let output = input.cloned().unwrap_or_else(|| NodeOutput::Image(Image::filled(w, h, Kind::Gray, [0.0; 4])));
        Ok(vec![output; output_ports.len().max(1)])
    } else if node.def == "input_value" {
        let value = variables.get(&identifier(node)).or_else(|| parameters.get("default")).cloned().unwrap_or(Value::Float(0.0));
        Ok(vec![NodeOutput::Value(value)])
    } else if node.def == "value_processor" {
        evaluate_value(&parameters, variables, seed, [w, h], &inputs, tile).map(|value| vec![NodeOutput::Value(value)])
    } else if matches!(node.def.as_str(), "output" | "dot") && incoming.first().and_then(Option::as_ref).is_some_and(|output| output.as_value().is_some()) {
        incoming.first().copied().flatten().cloned().map(|output| vec![output]).ok_or("missing value input".into())
    } else if let Some(image) = input_image {
        let image = image.resize(w, h, tile);
        let image = if node.def == "input_grayscale" { image.to_gray() } else { image };

        Ok(vec![NodeOutput::Image(image)])
    } else if node.def == materialcraft_core::MISSING_NODE {
        let name = if node.label.is_empty() { "this node" } else { node.label.as_str() };

        Err(format!("unsupported node `{name}`"))
    } else if node.def == "graph_instance" {
        cook_instance(package, graph_index, node, input_ports, output_ports, &incoming, &parameters, (w, h), base, options, cache)
    } else if let Some(definition) = definition {
        let context = CookContext { w, h, seed, tile, inputs: &inputs, params: &parameters, resource: options.resource, vars: variables };
        (definition.cook)(&context).map(|images| images.into_iter().map(NodeOutput::Image).collect())
    } else {
        Err(format!("unknown node `{}`", node.def))
    };

    // Cooked values serve parameter controls; authoring graphs stay in the document
    parameters.retain(|_, value| materialcraft_core::ValueType::of(value).is_some());
    match result {
        Ok(outputs) if outputs.len() == output_ports.len().max(1) || output_ports.is_empty() => {
            let outputs = Arc::new(outputs);
            lock(cache).put(key, outputs.clone());
            NodeResult { base, base_values, parameters, outputs, error: None, ms: (now_ms() - started_ms) as f32, key, cached: false }
        }
        Ok(outputs) => fail(format!("`{}` made {} images for {} outputs", node.def, outputs.len(), output_ports.len()), key),
        Err(error) => fail(error, key),
    }
}
