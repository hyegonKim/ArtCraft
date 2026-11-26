use crate::{
    GraphTarget, Result, Session, cmd,
    commands::{CommandSpec, always, bad, strp},
};
use serde_json::{Value, json};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("graph.rename", "Rename graph", [], None, "{name, graph?, document?}", always, rename),
        cmd!("graph.properties", "Inspect graph properties", [], None, "{graph?, document?}", always, |session, parameters| {
            let target = GraphTarget::resolve(session, parameters)?;
            let graph = target.get(session)?;
            Ok(json!({"document": target.document, "graph": target.graph, "identifier": graph.name, "attributes": graph.attributes}))
        }),
        cmd!(
            "graph.attributes",
            "Graph attributes",
            [],
            None,
            "{graph?, document?, label?, category?, description?, author?, authorUrl?, tags?, group?, userData?}",
            always,
            attributes
        ),
    ]
}

fn attributes(session: &mut Session, parameters: &Value) -> Result<Value> {
    let target = GraphTarget::resolve(session, parameters)?;
    target.edit(session, "Graph attributes", |package| {
        let attributes = &mut package.graph_mut(target.graph)?.attributes;
        for (key, slot) in [
            ("label", &mut attributes.label),
            ("category", &mut attributes.category),
            ("description", &mut attributes.description),
            ("author", &mut attributes.author),
            ("authorUrl", &mut attributes.author_url),
            ("tags", &mut attributes.tags),
            ("group", &mut attributes.group),
            ("userData", &mut attributes.user_data),
        ] {
            if let Some(value) = parameters.get(key) {
                *slot = value.as_str().ok_or_else(|| bad("graph.attributes", format!("`{key}` must be text")))?.into();
            }
        }
        Ok(Value::Null)
    })
}

fn rename(session: &mut Session, parameters: &Value) -> Result<Value> {
    let target = GraphTarget::resolve(session, parameters)?;
    let index = target.graph;
    let package = target.package(session)?;
    let name = strp(parameters, "name").map(str::trim).filter(|name| !name.is_empty()).ok_or_else(|| bad("graph.rename", "`name` is required"))?.to_string();
    let source = package.graph(index)?;
    if package.graphs.iter().enumerate().any(|(other, graph)| other != index && graph.folder == source.folder && graph.name == name) {
        return Err(bad("graph.rename", format!("a graph is already called `{name}`")));
    }

    crate::folder_path::name(&name)?;
    let new_path = crate::folder_path::join(&source.folder, &name);
    if package.folder_paths().contains(&new_path) || package.resources.iter().any(|resource| resource.sbs_path() == new_path) {
        return Err(bad("graph.rename", "a folder or resource already has that name"));
    }

    let paths = [(source.sbs_path(), new_path)].into();
    let names = crate::graph_references::with_leaf_names(package, &paths);
    target.edit(session, "Rename graph", |package| {
        package.graph_mut(index)?.name = name.clone();
        crate::graph_references::remap(package, &names);
        Ok(json!({"name": name}))
    })
}
