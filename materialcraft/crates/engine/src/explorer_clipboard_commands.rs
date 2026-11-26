use crate::{
    ExplorerClipboard, Result, Session, cmd,
    commands::{CommandSpec, bad, has_package, strp},
    folder_path,
};
use materialcraft_core::{Package, Value as GraphValue};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("explorer.copy", "Copy Explorer item", [], None, "{graph? index, folder?} copies a graph, folder subtree, or package contents", has_package, copy),
        cmd!(
            "explorer.paste",
            "Paste Explorer item",
            [],
            None,
            "{folder?} pastes into a folder or the package root, with unique names",
            |session| {
                has_package(session)?;
                if session.explorer_clipboard.is_some() { Ok(()) } else { Err("the Explorer clipboard is empty".into()) }
            },
            paste
        ),
    ]
}

fn copy(session: &mut Session, parameters: &Value) -> Result<Value> {
    let graph = parameters
        .get("graph")
        .map(|value| value.as_u64().and_then(|index| usize::try_from(index).ok()).ok_or_else(|| bad("explorer.copy", "graph must be an index")))
        .transpose()?;
    let folder = strp(parameters, "folder").map(str::to_string);
    if graph.is_some() && folder.is_some() {
        return Err(bad("explorer.copy", "select one graph or folder"));
    }

    if let Some(index) = graph {
        session.pkg.graph(index)?;
    }
    if let Some(folder) = &folder {
        folder_path::require(&session.pkg, folder)?;
        folder_path::require_graph_folder(folder)?;
    }

    session.explorer_clipboard =
        Some(ExplorerClipboard { source: session.pkg.clone(), document: session.document_id, path: session.path.clone(), graph, folder });
    Ok(Value::Null)
}

fn paste(session: &mut Session, parameters: &Value) -> Result<Value> {
    let parent = strp(parameters, "folder").unwrap_or("");
    folder_path::require(&session.pkg, parent)?;
    folder_path::require_graph_folder(parent)?;
    let clipboard = session.explorer_clipboard.as_ref().ok_or_else(|| bad("explorer.paste", "the Explorer clipboard is empty"))?;
    let mut incoming = clipboard.source.clone();
    incoming.graphs = clipboard
        .source
        .graphs
        .iter()
        .enumerate()
        .filter(|(index, graph)| {
            clipboard.graph.is_none_or(|selected| selected == *index)
                && clipboard.folder.as_ref().is_none_or(|folder| folder.is_empty() || folder_path::contains(folder, &graph.folder))
        })
        .map(|(_, graph)| graph.clone())
        .collect();
    let folders: BTreeSet<_> = if clipboard.graph.is_some() {
        BTreeSet::new()
    } else {
        clipboard
            .source
            .folder_paths()
            .into_iter()
            .filter(|path| clipboard.folder.as_ref().is_none_or(|folder| folder.is_empty() || folder_path::contains(folder, path)))
            .collect()
    };
    if session.pkg.graphs.len().saturating_add(incoming.graphs.len()) > 4096 || session.pkg.folder_paths().len().saturating_add(folders.len()) > 16384 {
        return Err(bad("explorer.paste", "a package supports at most 4096 graphs and 16384 folders"));
    }

    let mut destination = session.pkg.clone();
    let mut folder_names = BTreeMap::new();
    for folder in &folders {
        if folder_path::is_resources(folder) {
            folder_names.insert(folder.clone(), folder.clone());
            continue;
        }

        let (source_parent, leaf) = folder.rsplit_once('/').unwrap_or(("", folder));
        let target_parent = folder_names.get(source_parent).map(String::as_str).unwrap_or(parent);
        let new = folder_path::join(target_parent, &folder_path::unique(&destination, target_parent, leaf));
        folder_path::validate(&new)?;
        destination.folders.push(new.clone());
        folder_names.insert(folder.clone(), new);
    }
    incoming.folders = folder_names.values().cloned().collect();
    incoming.folder_descriptions =
        incoming.folder_descriptions.into_iter().filter_map(|(path, description)| folder_names.get(&path).map(|new| (new.clone(), description))).collect();
    let mut paths = BTreeMap::new();
    for graph in &mut incoming.graphs {
        let old = graph.sbs_path();
        graph.folder = folder_names.get(&graph.folder).cloned().unwrap_or_else(|| parent.to_string());
        graph.name = folder_path::unique(&destination, &graph.folder, &graph.name);
        paths.insert(old, graph.sbs_path());
        destination.graphs.push(graph.clone());
    }
    let names = crate::graph_references::with_leaf_names(&clipboard.source, &paths);
    for graph in &mut incoming.graphs {
        for instance in graph.instances_mut() {
            if !instance.package.is_empty() {
                continue;
            }

            if let Some(new) = names.get(&instance.graph) {
                instance.graph = new.clone();
            } else if clipboard.document != session.document_id {
                instance.package =
                    clipboard.path.clone().ok_or_else(|| bad("explorer.paste", "save the source package or copy the referenced graphs together"))?;
                if let Some((_, source)) = clipboard.source.graph_by_name(&instance.graph) {
                    instance.graph = source.sbs_path();
                }
            }
        }
        for node in graph.nodes.iter_mut().filter(|node| node.def == "graph_instance" && node.instance.is_none()) {
            if let Some(GraphValue::Text(name)) = node.params.get_mut("graph") {
                if let Some(new) = names.get(name) {
                    *name = new.clone();
                } else if clipboard.document != session.document_id {
                    let package =
                        clipboard.path.clone().ok_or_else(|| bad("explorer.paste", "save the source package or copy the referenced graphs together"))?;
                    node.instance = Some(materialcraft_core::GraphInstance {
                        label: String::new(),
                        package,
                        graph: name.clone(),
                        inputs: Vec::new(),
                        outputs: Vec::new(),
                        defaults: Default::default(),
                    });
                }
            }
        }
    }
    for resource in &mut incoming.resources {
        if let Some(folder) = folder_names.get(&resource.folder) {
            resource.folder = folder.clone();
        } else if clipboard.graph.is_none() && resource.folder.is_empty() {
            resource.folder = parent.into();
        }
    }
    transfer_resources(
        &mut incoming,
        &destination,
        clipboard.graph.is_none() && clipboard.folder.is_none(),
        clipboard.folder.as_deref().and_then(|path| folder_names.get(path)).map(String::as_str),
    );
    let dependencies = incoming.dependencies.clone();
    let result = session.edit("Paste Explorer item", |session| {
        let first = session.pkg.graphs.len();
        let count = incoming.graphs.len();
        session.pkg.graphs.extend(incoming.graphs);
        session.pkg.folders.extend(incoming.folders);
        for (path, description) in incoming.folder_descriptions {
            session.pkg.folder_descriptions.entry(path).or_insert(description);
        }
        session.pkg.resources.extend(incoming.resources);
        if count > 0 {
            session.graph = first;
            session.open_material_preview();
            session.selection.clear();
            session.annotation_selection = None;
        }
        Ok(json!({"graph": (count > 0).then_some(first), "graphs": count}))
    })?;
    session.linked_sources.extend(dependencies);
    session.refresh_links();
    Ok(result)
}

fn transfer_resources(incoming: &mut Package, destination: &Package, all: bool, folder: Option<&str>) {
    let used: BTreeSet<_> = incoming
        .graphs
        .iter()
        .flat_map(|graph| {
            graph.input_images.values().cloned().chain(graph.nodes.iter().filter_map(|node| match node.params.get("resource") {
                Some(GraphValue::Text(name)) => Some(name.clone()),
                _ => None,
            }))
        })
        .collect();
    incoming.resources.retain(|resource| all || used.contains(&resource.name) || folder.is_some_and(|folder| folder_path::contains(folder, &resource.folder)));
    let mut occupied: BTreeSet<_> = destination.resources.iter().map(|resource| resource.name.clone()).collect();
    let occupied_paths: BTreeSet<_> = destination.folder_paths().into_iter().chain(destination.graphs.iter().map(|graph| graph.sbs_path())).collect();
    let mut names = BTreeMap::new();
    incoming.resources.retain_mut(|resource| {
        if destination.resources.iter().any(|existing| existing == resource) {
            return false;
        }

        let original = resource.name.clone();
        let free = |name: &str| !occupied.contains(name) && !occupied_paths.contains(&folder_path::join(&resource.folder, name));
        if !free(&original) {
            resource.name = (1..).map(|index| format!("{original}_{index}")).find(|name| free(name)).unwrap_or_else(|| original.clone());
            names.insert(original, resource.name.clone());
        }
        occupied.insert(resource.name.clone());
        true
    });
    for graph in &mut incoming.graphs {
        for resource in graph.input_images.values_mut() {
            if let Some(new) = names.get(resource) {
                *resource = new.clone();
            }
        }
        for node in &mut graph.nodes {
            if let Some(GraphValue::Text(name)) = node.params.get_mut("resource")
                && let Some(new) = names.get(name)
            {
                *name = new.clone();
            }
        }
    }
}
