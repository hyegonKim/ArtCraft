use materialcraft_core::{Package, Value};
use std::collections::BTreeMap;

pub(crate) fn with_leaf_names(package: &Package, paths: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut names = paths.clone();
    for (old, new) in paths {
        let leaf = old.rsplit('/').next().unwrap_or(old);
        if package.graph_by_name(leaf).is_some_and(|(_, graph)| graph.sbs_path() == *old) {
            names.entry(leaf.into()).or_insert_with(|| new.clone());
        }
    }
    names
}

pub(crate) fn remap(package: &mut Package, names: &BTreeMap<String, String>) {
    for graph in &mut package.graphs {
        for instance in graph.instances_mut() {
            if instance.package.is_empty()
                && let Some(new) = names.get(&instance.graph)
            {
                instance.graph = new.clone();
            }
        }
        for node in
            graph.nodes.iter_mut().filter(|node| node.def == "graph_instance" && node.instance.as_ref().is_none_or(|instance| instance.package.is_empty()))
        {
            if let Some(Value::Text(old)) = node.params.get_mut("graph")
                && let Some(new) = names.get(old)
            {
                *old = new.clone();
            }
        }
    }
}
