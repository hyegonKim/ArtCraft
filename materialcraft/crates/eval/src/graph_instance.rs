use crate::{Cache, MAX_DEPTH, NodeOutput, Options, Port, cook, identifier};
use materialcraft_core::{Graph, Node, Package, Value};
use std::{collections::BTreeMap, sync::Mutex};

pub(crate) fn resolve<'a>(registry: &'a Package, current: &'a Package, node: &Node) -> Option<(&'a Package, usize, &'a Graph)> {
    let package = match &node.instance {
        Some(instance) if !instance.package.is_empty() => registry.dependencies.get(&instance.package)?.as_ref(),
        _ => current,
    };
    let name = node.instance.as_ref().map(|instance| instance.graph.as_str()).or_else(|| match node.params.get("graph") {
        Some(Value::Text(name)) => Some(name.as_str()),
        _ => None,
    })?;
    let (index, graph) = package.graph_by_name(name)?;
    Some((package, index, graph))
}

/// Whether graph `from` reaches `target`, without recursive stack growth
#[cfg(test)]
pub(crate) fn instances_reach(pkg: &Package, from: usize, target: usize) -> bool {
    let mut pending = vec![from];
    let mut seen = std::collections::HashSet::new();
    while let Some(index) = pending.pop() {
        if !seen.insert(index) {
            continue;
        }
        let Some(graph) = pkg.graphs.get(index) else { continue };

        for (si, _) in graph.nodes.iter().filter(|n| n.def == "graph_instance").filter_map(|n| crate::instance_graph(pkg, n)) {
            if si == target {
                return true;
            }
            pending.push(si);
        }
    }
    false
}

pub(crate) fn cook_instance(
    pkg: &Package,
    gi: usize,
    node: &Node,
    in_ports: &[Port],
    out_ports: &[Port],
    inputs: &[Option<&NodeOutput>],
    params: &BTreeMap<String, Value>,
    size: (u32, u32),
    base: crate::resolved_base_parameters::ResolvedBaseParameters,
    opts: &Options,
    cache: &Mutex<Cache>,
) -> Result<Vec<NodeOutput>, String> {
    if opts.depth >= MAX_DEPTH {
        return Err("graph instances nest too deep (does a graph use itself?)".into());
    }
    let registry = opts.registry.unwrap_or(pkg);
    let (source, sub_gi, sub) = resolve(registry, pkg, node).ok_or("the instanced graph does not exist")?;
    let identity = std::ptr::from_ref(sub) as usize;
    if pkg.graphs.get(gi).is_some_and(|graph| std::ptr::eq(graph, sub)) || opts.graph_stack.contains(&identity) {
        return Err(format!("graph `{}` uses itself through a graph instance", sub.name));
    }
    if sub.function.is_some() || !sub.nodes.iter().any(|node| node.def == "output") {
        return Err("the referenced graph must be a main graph with an output".into());
    }
    let root_resource = opts.resource_root.unwrap_or(opts.resource);
    let resource = |name: &str| match &node.instance {
        Some(instance) if !instance.package.is_empty() => {
            let (name, linear) = materialcraft_image::split_resource_key(name);
            root_resource(&materialcraft_image::resource_key(&format!("{}\0{name}", instance.package), linear))
        }
        _ => (opts.resource)(name),
    };
    let mut o = Options::new(&resource);
    o.environment = opts.environment;
    o.parent_tiling = Some(base.tiling);
    o.parent_pixel_ratio = Some(base.pixel_ratio);
    o.parent_base = Some(base);
    o.registry = Some(registry);
    o.resource_root = Some(root_resource);
    o.max_size = opts.max_size;
    o.cancel = opts.cancel;
    o.depth = opts.depth + 1;
    o.graph_stack = opts.graph_stack.clone();
    o.graph_stack.push(identity);
    o.only = Some(sub.nodes.iter().filter(|node| node.def == "output").map(|node| node.id).collect());
    o.inputs = in_ports.iter().zip(inputs).filter_map(|(port, input)| Some((port.id.clone(), input.as_ref()?.as_image()?.clone()))).collect();
    o.overrides = params.iter().filter(|(k, _)| sub.exposed.iter().any(|e| &e.id == *k)).map(|(k, v)| (k.clone(), v.clone())).collect();
    o.overrides.extend(in_ports.iter().zip(inputs).filter_map(|(port, input)| Some((port.id.clone(), input.as_ref()?.as_value()?.clone()))));
    let cooked = cook(source, sub_gi, &o, cache);
    if cooked.cancelled {
        return Err("cancelled".into());
    }
    if let Some(error) = cooked.nodes.values().find_map(|result| result.error.as_deref()) {
        return Err(format!("{}: {error}", sub.name));
    }
    out_ports
        .iter()
        .map(|p| {
            let output = sub
                .nodes
                .iter()
                .find(|n| n.def == "output" && identifier(n) == p.id)
                .and_then(|node| cooked.nodes.get(&node.id))
                .and_then(|result| result.outputs.first())
                .ok_or_else(|| format!("the source output `{}` is missing", p.id))?;
            Ok(match output {
                NodeOutput::Image(image) => NodeOutput::Image(image.resize(size.0, size.1, true)),
                NodeOutput::Value(value) => NodeOutput::Value(value.clone()),
            })
        })
        .collect()
}
