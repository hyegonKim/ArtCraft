use materialcraft_core::{Node, Package, Value, ValueType};
use materialcraft_nodes::{NodeDefinition, ParameterDescriptor, clamp_params, resolve_params};
use std::collections::BTreeMap;

pub(crate) fn resolve_parameters(
    package: &Package,
    node: &Node,
    definition: Option<&NodeDefinition>,
    variables: &BTreeMap<String, Value>,
    seed: u32,
    size: [u32; 2],
) -> Result<BTreeMap<String, Value>, String> {
    let mut parameters = match definition {
        Some(definition) if node.def != "graph_instance" => resolve_params(definition, &node.params),
        _ => {
            let mut parameters = node.instance.as_ref().map(|instance| instance.defaults.clone()).unwrap_or_default();
            parameters.extend(node.params.clone());
            parameters
        }
    };
    for parameter in node.bindings.keys().chain(node.functions.keys()) {
        if !parameters.contains_key(parameter)
            && let Some(descriptor) = ParameterDescriptor::for_node(package, node, parameter)
        {
            parameters.insert(parameter.clone(), descriptor.default);
        }
    }

    for (parameter, exposed) in &node.bindings {
        if let (Some(value), Some(current)) = (variables.get(exposed), parameters.get(parameter))
            && let Some(coerced) = current.coerce_json(&serde_json::to_value(value).unwrap_or_default())
        {
            parameters.insert(parameter.clone(), coerced);
        }
    }

    for (parameter, function) in &node.functions {
        let current = parameters.get(parameter).ok_or_else(|| format!("unknown function parameter `{parameter}`"))?;
        let expected = ValueType::of(current).ok_or_else(|| format!("parameter `{parameter}` cannot have a function"))?;
        let value =
            crate::parameter_function::evaluate(function, expected, variables, seed, size).map_err(|error| format!("function of `{parameter}`: {error}"))?;
        let value = current.coerce_json(&serde_json::to_value(value).unwrap_or_default()).ok_or_else(|| format!("invalid function value for `{parameter}`"))?;
        parameters.insert(parameter.clone(), value);
    }

    // Catalog limits keep values from files and functions within safe cook bounds
    if let Some(definition) = definition
        && node.def != "graph_instance"
    {
        clamp_params(definition, &mut parameters);
    }

    Ok(parameters)
}
