use crate::{
    Result, Session, cmd,
    commands::{CommandSpec, bad, has_graph, has_package, point, strp},
};
use materialcraft_core::Value as GraphValue;
use serde_json::{Value, json};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "file.importVector",
            "Import vector graphics...",
            ["File"],
            None,
            "{path, name?, folder?} imports an SVG into Resources or its subfolders, preserving its original bytes without adding a graph node",
            has_package,
            import_vector
        ),
        cmd!(
            "graph.resource.add",
            "Add resource to graph",
            [],
            None,
            "{resource, document? (source package), at? [x,y]} adds a bitmap or SVG node; assets from another package are copied into Resources",
            has_graph,
            add_node
        ),
    ]
}

fn add_node(session: &mut Session, parameters: &Value) -> Result<Value> {
    let command = "graph.resource.add";
    if session.graph()?.function.is_some() {
        return Err(bad(command, "Resources can only be added to a material graph"));
    }

    let name = strp(parameters, "resource").ok_or_else(|| bad(command, "resource is required"))?;
    let document = match parameters.get("document") {
        None | Some(Value::Null) => session.document_id,
        Some(value) => value.as_u64().ok_or_else(|| bad(command, "document must be an open document id"))?,
    };
    let mut resource = session
        .open_packages()
        .into_iter()
        .find(|entry| entry.0 == document)
        .and_then(|entry| entry.1.resource(name))
        .cloned()
        .ok_or_else(|| bad(command, "The source resource is no longer available"))?;
    if resource.bytes.is_empty() {
        return Err(bad(command, "The source resource is missing"));
    }

    let definition = if resource.file.to_ascii_lowercase().ends_with(".svg") { "svg" } else { "bitmap" };
    let copy = document != session.document_id;
    if copy {
        if session.pkg.resources.len() >= 4096 || resource.bytes.len() > 64 << 20 {
            return Err(bad(command, "Resources are limited to 64 MiB per file and 4096 files per package"));
        }

        resource.folder = "Resources".into();
        resource.source.clear();
        resource.name = crate::file_cmds::resource_name(&session.pkg, "Resources", &resource.name);
    }
    let at = point(parameters, "at").unwrap_or([0.0; 2]);
    session.edit("Add resource to graph", |session| {
        let name = resource.name.clone();
        if copy {
            session.pkg.resources.push(resource);
        }
        let graph = session.graph_mut()?;
        let id = graph.add_node(definition, at);
        graph.node_mut(id)?.params.insert("resource".into(), GraphValue::Text(name));
        session.selection = vec![id];
        Ok(json!({"id": id.0}))
    })
}

fn import_vector(session: &mut Session, parameters: &Value) -> Result<Value> {
    let path = strp(parameters, "path").ok_or_else(|| bad("file.importVector", "`path` is required"))?;
    if !path.to_ascii_lowercase().ends_with(".svg") {
        return Err(bad("file.importVector", "Choose an SVG file"));
    }

    crate::file_cmds::import_resource_file(session, parameters, true)
}
