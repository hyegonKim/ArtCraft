use crate::{Cache, identifier};
use materialcraft_core::{Node, Package, Value};
use materialcraft_image::Image;
use std::collections::{BTreeMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex};

pub(crate) fn image_key(i: &Image, cache: &Mutex<Cache>) -> u64 {
    let ptr = Arc::as_ptr(&i.data) as usize;
    let shape = (i.w, i.h, i.kind.channels());
    if let Some(key) = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .input_hashes
        .get(&ptr)
        .and_then(|saved| saved.buffer.upgrade().filter(|data| Arc::ptr_eq(data, &i.data) && saved.shape == shape).map(|_| saved.key))
    {
        return key;
    }
    let mut h = DefaultHasher::new();
    (shape, i.data.len()).hash(&mut h);
    for value in i.data.iter() {
        value.to_bits().hash(&mut h);
    }
    let key = h.finish();
    let mut cache = cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if cache.input_hashes.len() >= 128 {
        cache.input_hashes.retain(|_, saved| saved.buffer.strong_count() > 0);
        if cache.input_hashes.len() >= 128 {
            cache.input_hashes.clear();
        }
    }
    cache.input_hashes.insert(ptr, crate::input_fingerprint::InputFingerprint { buffer: Arc::downgrade(&i.data), shape, key });
    key
}

pub(crate) fn hash_of(v: &impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

#[cfg(test)]
pub(crate) fn graph_tree_hash(pkg: &Package, gi: usize) -> u64 {
    graph_tree_hash_with_registry(pkg, pkg, gi)
}

pub(crate) fn graph_tree_hash_with_registry(registry: &Package, pkg: &Package, gi: usize) -> u64 {
    let mut h = DefaultHasher::new();
    let mut pending = vec![(pkg, gi)];
    let mut seen = HashSet::new();
    while let Some((pkg, index)) = pending.pop() {
        if !seen.insert((std::ptr::from_ref(pkg) as usize, index)) {
            continue;
        }
        let Some(graph) = pkg.graphs.get(index) else { continue };

        index.hash(&mut h);
        serde_json::to_string(graph).unwrap_or_default().hash(&mut h);
        for (id, res) in &graph.input_images {
            pkg.resource(res).map(|r| r.bytes.content_hash()).hash(&mut h);
            id.hash(&mut h);
            graph.input_images_linear.contains(id).hash(&mut h);
        }
        for instance in graph.functions().flat_map(|function| &function.nodes).filter_map(|node| node.instance.as_ref()) {
            let source = if instance.package.is_empty() { Some(pkg) } else { registry.dependencies.get(&instance.package).map(AsRef::as_ref) };
            if let Some(source) = source
                && let Some((index, _)) = source.graph_by_name(&instance.graph)
            {
                pending.push((source, index));
            }
        }
        for n in &graph.nodes {
            if let Some(Value::Text(name)) = n.params.get("resource") {
                pkg.resource(name).map(|r| r.bytes.content_hash()).hash(&mut h);
            }
            if n.def == "graph_instance"
                && let Some((source, index, _)) = crate::graph_instance::resolve(registry, pkg, n)
            {
                pending.push((source, index));
            }
        }
    }
    h.finish()
}

pub(crate) fn variable_key(node: &Node, parameters: &BTreeMap<String, Value>, variables: &BTreeMap<String, Value>) -> u64 {
    let mut hasher = DefaultHasher::new();
    let mut names = std::collections::BTreeSet::new();
    let mut pending: Vec<&Value> = parameters.values().collect();
    while let Some(value) = pending.pop() {
        match value {
            Value::Function(graph) => {
                let mut pending = vec![graph.as_ref()];
                while let Some(graph) = pending.pop() {
                    serde_json::to_string(graph).unwrap_or_default().hash(&mut hasher);
                    for function_node in &graph.nodes {
                        // Resolved call defaults are omitted from graph serialization
                        if let Some(instance) = &function_node.instance {
                            serde_json::to_string(&instance.defaults).unwrap_or_default().hash(&mut hasher);
                        }

                        match &function_node.source {
                            Some(Ok(source)) => pending.push(source.as_ref()),
                            Some(Err(error)) => error.hash(&mut hasher),
                            None => {}
                        }
                        if (function_node.op.starts_with("get_") || function_node.op == "isdefined")
                            && let Some(Value::Text(name)) = &function_node.value
                        {
                            names.insert(name.as_str());
                        }
                    }
                }
            }
            Value::PatternGraph(graph) => {
                pending.extend(graph.nodes.iter().flat_map(|node| node.parameters.values()));
            }
            _ => {}
        }
    }

    let input_identifier = identifier(node);
    if node.def == "input_value" {
        names.insert(&input_identifier);
    }

    for name in names {
        name.hash(&mut hasher);
        variables.get(name).map(|value| serde_json::to_string(value).unwrap_or_default()).hash(&mut hasher);
    }

    hasher.finish()
}
