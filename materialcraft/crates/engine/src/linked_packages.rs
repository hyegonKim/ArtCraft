use crate::Session;
use materialcraft_core::{GraphInstance, Package};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

fn references(package: &Package) -> impl Iterator<Item = &GraphInstance> {
    package.graphs.iter().flat_map(|graph| graph.instances())
}

fn reaches(package: &Package, changed: &BTreeSet<String>, sources: &BTreeMap<String, Arc<Package>>) -> bool {
    let mut pending: Vec<_> = references(package).filter(|instance| !instance.package.is_empty()).map(|instance| instance.package.as_str()).collect();
    let mut visited = BTreeSet::new();
    while let Some(file) = pending.pop() {
        if !visited.insert(file) {
            continue;
        }
        if changed.contains(file) {
            return true;
        }
        if let Some(source) = sources.get(file) {
            pending.extend(references(source).filter(|instance| !instance.package.is_empty()).map(|instance| instance.package.as_str()));
        }
    }
    false
}

fn interfaces(package: &mut Package, sources: &BTreeMap<String, Arc<Package>>) -> bool {
    let updates: Vec<_> = package
        .graphs
        .iter()
        .enumerate()
        .flat_map(|(graph_index, graph)| graph.nodes.iter().enumerate().map(move |(node_index, node)| (graph_index, node_index, graph, node)))
        .filter_map(|(graph_index, node_index, owner, node)| {
            let mut instance = node.instance.clone().or_else(|| {
                if node.def != "graph_instance" {
                    return None;
                }

                let materialcraft_core::Value::Text(name) = node.params.get("graph")? else { return None };
                Some(GraphInstance {
                    label: String::new(),
                    defaults: Default::default(),
                    package: String::new(),
                    graph: name.clone(),
                    inputs: Vec::new(),
                    outputs: Vec::new(),
                })
            })?;
            let source = if instance.package.is_empty() { &*package } else { sources.get(&instance.package)?.as_ref() };
            let (_, graph) = source.graph_by_name(&instance.graph)?;
            if graph.function.is_some() {
                return None;
            }

            let (mut inputs, mut outputs) = materialcraft_nodes::graph_interface::graph_ports(graph);

            // Keep connected missing ports so saving cannot silently discard the user's wires
            for port in &instance.inputs {
                if owner.connections.iter().any(|wire| wire.to == node.id && wire.to_port == port.identifier)
                    && !inputs.iter().any(|input| input.identifier == port.identifier)
                {
                    inputs.push(port.clone());
                }
            }
            for port in &instance.outputs {
                if owner.connections.iter().any(|wire| wire.from == node.id && wire.from_port == port.identifier)
                    && !outputs.iter().any(|output| output.identifier == port.identifier)
                {
                    outputs.push(port.clone());
                }
            }

            instance.inputs = inputs;
            instance.outputs = outputs;
            instance.defaults =
                graph.exposed.iter().map(|parameter| (parameter.id.clone(), graph.input_value(&parameter.id).unwrap_or(&parameter.default).clone())).collect();
            instance.label = graph.display_name().into();
            (node.instance.as_ref() != Some(&instance)).then_some((graph_index, node_index, instance))
        })
        .collect();

    let mut changed = !updates.is_empty();
    for (graph, node, instance) in updates {
        if let Some(node) = package.graphs.get_mut(graph).and_then(|graph| graph.nodes.get_mut(node)) {
            node.instance = Some(instance);
        }
    }

    let local = package.clone();
    for function in package.graphs.iter_mut().flat_map(|graph| graph.functions_mut()) {
        if function.nodes.iter().any(|node| node.instance.is_some()) {
            let previous = function.clone();
            materialcraft_nodes::function::resolve_instances(function, &local, sources);
            changed |= *function != previous;
        }
    }

    changed
}

impl Session {
    /// Refresh source snapshots and invalidate open dependents without merging their undo histories
    pub(crate) fn refresh_links(&mut self) {
        self.refresh_links_from(BTreeSet::new());
    }

    /// Refresh linked interfaces and retain ownership of installed library snapshots.
    pub(crate) fn refresh_links_from(&mut self, mut changed: BTreeSet<String>) {
        // Replaced snapshots belong to the caller, even when their library alias is unchanged
        self.library_sources.retain(|path, owned| self.linked_sources.get(path).is_some_and(|source| Arc::ptr_eq(source, owned)));
        let open_paths: BTreeSet<_> = self.open_packages().into_iter().filter_map(|(_, _, path, _)| path.map(str::to_string)).collect();
        self.library_sources.retain(|path, _| !open_paths.contains(path));

        let mut sources = self.linked_sources.clone();

        // Interface types can propagate through several files; the workspace has at most 64 open packages
        for _ in 0..64 {
            for (_, package, path, _) in self.open_packages() {
                let Some(path) = path else { continue };
                let same = sources
                    .get(path)
                    .is_some_and(|source| source.graphs == package.graphs && source.resources == package.resources && source.name == package.name);
                if !same {
                    let mut source = package.clone();
                    source.dependencies.clear();
                    sources.insert(path.to_string(), Arc::new(source));
                    changed.insert(path.to_string());
                }
            }
            let previous_sources = sources.clone();
            let mut updated = false;
            for (path, source) in &previous_sources {
                let mut package = source.as_ref().clone();
                if interfaces(&mut package, &previous_sources) {
                    sources.insert(path.clone(), Arc::new(package));
                    updated = true;
                }
            }
            updated |= interfaces(&mut self.pkg, &sources);
            for document in self.documents.values_mut() {
                updated |= interfaces(&mut document.pkg, &sources);
            }
            if !updated {
                break;
            }
        }
        if reaches(&self.pkg, &changed, &sources) {
            self.modified = true;
            self.dirty = true;
            self.cook_epoch = self.cook_epoch.wrapping_add(1);
        }
        self.pkg.dependencies = sources.clone();
        for document in self.documents.values_mut() {
            if reaches(&document.pkg, &changed, &sources) {
                document.modified = true;
            }
            document.pkg.dependencies = sources.clone();
        }

        // Interface refreshes replace Arcs without changing who owns the source
        for (path, owned) in &mut self.library_sources {
            if let Some(source) = sources.get(path) {
                *owned = source.clone();
            }
        }
        self.linked_sources = sources;
    }
}
