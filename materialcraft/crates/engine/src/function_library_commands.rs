use crate::{
    Result, Session, cmd,
    commands::{CommandSpec, always, strp},
};
use serde_json::{Value, json};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("library.functions", "List library functions", [], None, "{filter?} lists reusable SBS functions with source paths and interfaces", always, list),
        cmd!(
            "library.function.add",
            "Add library function",
            [],
            None,
            "{graph, package? (sbs://functions.sbs), function?: FunctionGraphTarget, at?, snapToGrid?} adds a function instance to the active Function or processor canvas",
            crate::commands::has_graph,
            add
        ),
    ]
}

fn list(session: &mut Session, parameters: &Value) -> Result<Value> {
    let filter = strp(parameters, "filter").unwrap_or("").to_lowercase();
    let mut entries = Vec::new();
    for (path, package) in &session.pkg.dependencies {
        if !path.starts_with("sbs://") {
            continue;
        }

        for graph in &package.graphs {
            let Some(function) = &graph.function else { continue };
            let identifier = graph.sbs_path();
            if !filter.is_empty() && !identifier.to_lowercase().contains(&filter) && !graph.display_name().to_lowercase().contains(&filter) {
                continue;
            }

            let ports = materialcraft_nodes::function::graph_ports(graph);
            let error = ports.as_ref().err().cloned();
            let (inputs, outputs) = ports.unwrap_or_default();
            let category = if graph.attributes.category.is_empty() { &graph.folder } else { &graph.attributes.category };
            entries.push(json!({"package": path, "graph": identifier, "label": graph.display_name(),
                "category": category, "description": graph.attributes.description,
                "inputs": inputs, "outputs": outputs, "error": error,
                "missing": function.output.is_none() || function.nodes.iter().any(|node| node.missing.is_some())}));
        }
    }
    Ok(json!(entries))
}

fn add(session: &mut Session, parameters: &Value) -> Result<Value> {
    let mut parameters = parameters.clone();
    let object = parameters.as_object_mut().ok_or_else(|| crate::commands::bad("library.function.add", "expected an object"))?;
    object.entry("package").or_insert_with(|| json!(crate::function_library::STANDARD_PACKAGE));

    let package = strp(&parameters, "package").and_then(|path| session.pkg.dependencies.get(path));
    let graph = strp(&parameters, "graph").and_then(|name| package?.graph_by_name(name));
    if graph.is_none_or(|(_, graph)| graph.function.is_none()) {
        return Err(crate::commands::bad("library.function.add", "the library function is unavailable"));
    }

    crate::graph_instance_commands::add(session, &parameters)
}
