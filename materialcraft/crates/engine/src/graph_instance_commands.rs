use crate::{
    Result, Session, cmd,
    commands::{CommandSpec, bad, has_graph, point, strp, u64p},
};
use materialcraft_core::{GraphInstance, Package};
use serde_json::{Value, json};
use std::collections::BTreeSet;

const COMMAND: &str = "graph.instance.add";

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "graph.instance.reference",
            "Open graph reference",
            [],
            None,
            "{id, function?: FunctionGraphTarget} resolves the source package and graph without changing the document",
            has_graph,
            reference
        ),
        cmd!(
            "graph.instance.add",
            "Add graph instance",
            [],
            None,
            "{document, graph, function?: FunctionGraphTarget, at? [x,y]} instances a compatible graph from an open SBS package; requires an output and rejects cycles",
            has_graph,
            add
        ),
    ]
}

fn reaches(
    package: &Package,
    source_file: &str,
    graph: &str,
    target_file: &str,
    target_graph: &str,
    sources: &std::collections::BTreeMap<String, std::sync::Arc<Package>>,
) -> bool {
    let mut pending = vec![(source_file.to_string(), graph.to_string())];
    let mut visited = BTreeSet::new();
    while let Some((file, name)) = pending.pop() {
        if !visited.insert((file.clone(), name.clone())) {
            continue;
        }
        if file == target_file && name == target_graph {
            return true;
        }
        let source = if file == source_file { Some(package) } else { sources.get(&file).map(AsRef::as_ref) };
        let Some((_, graph)) = source.and_then(|source| source.graph_by_name(&name)) else { continue };
        for instance in graph.instances() {
            pending.push((if instance.package.is_empty() { file.clone() } else { instance.package.clone() }, instance.graph.clone()));
        }
        for node in &graph.nodes {
            if node.instance.is_none()
                && node.def == "graph_instance"
                && let Some(materialcraft_core::Value::Text(name)) = node.params.get("graph")
            {
                pending.push((file.clone(), name.clone()));
            }
        }
    }
    false
}

pub(crate) fn add(session: &mut Session, parameters: &Value) -> Result<Value> {
    if parameters.get("pattern").is_some() {
        return Err(bad(COMMAND, "FX-Map graphs do not accept graph instances"));
    }
    let target: Option<crate::FunctionGraphTarget> = parameters
        .get("function")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| bad(COMMAND, error.to_string()))?
        .or_else(|| session.graph().ok()?.function.as_ref().map(|_| crate::FunctionGraphTarget::Graph));
    if let Some(target) = &target {
        target.current(session)?;
    }
    let name = strp(parameters, "graph").ok_or_else(|| bad(COMMAND, "graph is required"))?;
    let packages = session.open_packages();
    let document = u64p(parameters, "document");
    let (source, path) = if let Some(path) = strp(parameters, "package") {
        if !path.starts_with("sbs://") {
            return Err(bad(COMMAND, "library instances require an sbs:// package"));
        }

        (session.pkg.dependencies.get(path).map(AsRef::as_ref).ok_or_else(|| bad(COMMAND, "the library package is missing"))?, Some(path))
    } else {
        let document = document.ok_or_else(|| bad(COMMAND, "document is required"))?;
        let (_, source, path, _) = packages.iter().find(|entry| entry.0 == document).copied().ok_or_else(|| bad(COMMAND, "the source package is not open"))?;
        (source, path)
    };
    let (_, graph) = source.graph_by_name(name).ok_or_else(|| bad(COMMAND, "the source graph no longer exists"))?;
    if graph.function.is_some() != target.is_some() {
        return Err(bad(COMMAND, "Function instances require a Function canvas; main instances require a main graph"));
    }
    let (inputs, outputs) = if target.is_some() {
        materialcraft_nodes::function::graph_ports(graph).map_err(|error| bad(COMMAND, error))?
    } else {
        materialcraft_nodes::graph_interface::graph_ports(graph)
    };
    if outputs.is_empty() {
        return Err(bad(COMMAND, "the source graph needs an output before it can be instanced"));
    }
    for ports in [&inputs, &outputs] {
        if ports.iter().map(|port| &port.identifier).collect::<BTreeSet<_>>().len() != ports.len() {
            return Err(bad(COMMAND, "source connector identifiers must be unique"));
        }
    }
    let file = if document == Some(session.document_id) && parameters.get("package").is_none() {
        ""
    } else {
        path.ok_or_else(|| bad(COMMAND, "save the source SBS file before instancing its graph in another package"))?
    };
    let source_file = path.map(str::to_string).unwrap_or_else(|| format!("unsaved:{}", document.unwrap_or(session.document_id)));
    let target_file = session.path.clone().unwrap_or_else(|| format!("unsaved:{}", session.document_id));
    if reaches(source, &source_file, &graph.sbs_path(), &target_file, &session.graph()?.sbs_path(), &session.pkg.dependencies) {
        return Err(bad(COMMAND, "the instance would create a graph dependency cycle"));
    }
    let instance =
        GraphInstance { label: graph.display_name().into(), defaults: Default::default(), package: file.into(), graph: graph.sbs_path(), inputs, outputs };
    let at =
        crate::graph_grid::snap_position(point(parameters, "at").unwrap_or([0.0; 2]), parameters.get("snapToGrid").and_then(Value::as_bool).unwrap_or(false));
    session.edit("Add graph instance", |session| {
        if let Some(target) = target {
            let mut function = target.current(session)?.clone();
            if function.nodes.len() >= 4096 {
                return Err(bad(COMMAND, "function exceeds the 4096-node limit"));
            }
            let id = function.next_id();
            function.nodes.push(materialcraft_core::FunctionNode {
                id,
                missing: None,
                missing_data: String::new(),
                op: "instance".into(),
                type_code: instance.outputs.first().map_or(0, |port| port.type_code),
                inputs: vec![None; instance.inputs.len()],
                value: None,
                pos: at,
                instance: Some(instance),
                source: None,
            });
            target.replace(session, function)?;
            if matches!(target, crate::FunctionGraphTarget::Graph) {
                session.selection = vec![materialcraft_core::NodeId(u64::from(id))];
            }
            return Ok(json!({"id": id}));
        }
        let graph = session.graph_mut()?;
        let id = graph.add_node("graph_instance", at);
        graph.node_mut(id)?.instance = Some(instance);
        session.selection = vec![id];
        Ok(json!({"id": id}))
    })
}

fn reference(session: &mut Session, parameters: &Value) -> Result<Value> {
    let command = "graph.instance.reference";
    let id = u64p(parameters, "id").ok_or_else(|| bad(command, "id is required"))?;
    let instance = if let Some(target) = parameters.get("function") {
        let target: crate::FunctionGraphTarget = serde_json::from_value(target.clone()).map_err(|error| bad(command, error.to_string()))?;
        let id = u32::try_from(id).map_err(|_| bad(command, "invalid function node id"))?;
        target.current(session)?.node(id).and_then(|node| node.instance.as_ref())
    } else {
        session.graph()?.node(materialcraft_core::NodeId(id))?.instance.as_ref()
    }
    .ok_or_else(|| bad(command, "the node has no graph reference"))?;
    let source = if instance.package.is_empty() { Some(&session.pkg) } else { session.pkg.dependencies.get(&instance.package).map(AsRef::as_ref) }
        .ok_or_else(|| bad(command, "the source file is unavailable"))?;
    let (graph, _) = source.graph_by_name(&instance.graph).ok_or_else(|| bad(command, "the source graph is missing"))?;
    let document = if instance.package.is_empty() {
        Some(session.document_id)
    } else {
        session.open_packages().into_iter().find(|entry| entry.2 == Some(instance.package.as_str())).map(|entry| entry.0)
    };
    Ok(json!({"graph": graph, "document": document, "path": instance.package}))
}
