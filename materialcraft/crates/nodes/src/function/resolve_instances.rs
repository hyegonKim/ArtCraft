use materialcraft_core::{FunctionGraph, GraphPort, Package};
use std::{collections::BTreeMap, sync::Arc};

/// Resolve calls once per document edit, with bounded source expansion
pub fn resolve_instances(function: &mut FunctionGraph, local: &Package, sources: &BTreeMap<String, Arc<Package>>) {
    resolve(function, local, sources, &mut Vec::new(), &mut 4096);
}

fn resolve(function: &mut FunctionGraph, local: &Package, sources: &BTreeMap<String, Arc<Package>>, stack: &mut Vec<(usize, String)>, remaining: &mut usize) {
    *remaining = remaining.saturating_sub(function.nodes.len());
    for node in &mut function.nodes {
        let Some(instance) = &mut node.instance else {
            node.source = None;
            continue;
        };

        let source = if instance.package.is_empty() { Some(local) } else { sources.get(&instance.package).map(AsRef::as_ref) };
        let resolved = (|| {
            let source = source.ok_or_else(|| format!("Missing source file for `{}`", instance.graph))?;
            let (_, graph) = source.graph_by_name(&instance.graph).ok_or_else(|| format!("Missing Function Graph `{}`", instance.graph))?;
            instance.label = graph.display_name().into();
            let body = graph.function.as_ref().ok_or("Only Function Graphs can be used in a function")?;
            let key = (std::ptr::from_ref(source) as usize, graph.sbs_path());
            if stack.contains(&key) {
                return Err("Function Graph dependency cycle".into());
            }

            if stack.len() >= 8 || body.nodes.len() > *remaining {
                return Err("Function instances exceed the 8-level or 4096-node limit".into());
            }

            if graph.exposed.len() > 4096
                || graph.exposed.iter().map(|parameter| &parameter.id).collect::<std::collections::BTreeSet<_>>().len() != graph.exposed.len()
            {
                return Err("Function inputs must have unique identifiers and at most 4096 entries".into());
            }

            let previous: BTreeMap<_, _> = instance.inputs.iter().zip(&node.inputs).map(|(port, value)| (port.identifier.clone(), *value)).collect();
            instance.defaults =
                graph.exposed.iter().map(|parameter| (parameter.id.clone(), graph.input_value(&parameter.id).unwrap_or(&parameter.default).clone())).collect();
            instance.inputs = graph
                .exposed
                .iter()
                .map(|parameter| GraphPort { identifier: parameter.id.clone(), type_code: super::value_type::type_code(&parameter.default) })
                .collect();
            let declared_sources: std::collections::BTreeSet<_> =
                instance.inputs.iter().filter_map(|port| previous.get(&port.identifier).copied().flatten()).collect();
            let mut conflicting_input = false;

            // Keep connected removed inputs visible so saving never discards a wire
            for (name, value) in &previous {
                if value.is_some() && !instance.inputs.iter().any(|port| port.identifier == *name) {
                    instance.inputs.push(GraphPort { identifier: name.clone(), type_code: 0 });

                    // A legacy connector may repeat a declared wire without supplying another argument
                    conflicting_input |= value.is_some_and(|source| !declared_sources.contains(&source));
                }
            }

            node.inputs = instance.inputs.iter().map(|port| previous.get(&port.identifier).copied().flatten()).collect();

            let mut body = body.clone();
            stack.push(key);
            resolve(&mut body, source, sources, stack, remaining);
            stack.pop();

            let output = super::output_node(&body).ok_or_else(|| format!("{}: no output node defined", instance.graph))?;
            let types = super::node_types(&body)?;
            let type_code = types.get(&output.id).copied().unwrap_or(256);
            instance.outputs = vec![GraphPort { identifier: "output".into(), type_code }];
            node.type_code = type_code;
            if conflicting_input {
                return Err(format!("{}: a connected input no longer exists in the source", instance.graph));
            }

            Ok(Arc::new(body))
        })();
        node.source = Some(resolved);
    }
}
