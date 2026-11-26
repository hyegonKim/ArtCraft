use materialcraft_core::{Package, ValueType};
use materialcraft_nodes::function;

pub(super) fn package(bytes: &[u8], source: &str) -> Result<Package, Box<dyn std::error::Error>> {
    let imported = materialcraft_formats::sbs::read_sbs(bytes, source, &|_| None)?;
    if !imported.warnings.is_empty() {
        return Err(imported.warnings.join("\n").into());
    }

    if !imported.pkg.resources.is_empty() {
        return Err("function library conversion does not copy resource assets".into());
    }

    let mut package = imported.pkg.clone();
    for graph in &mut package.graphs {
        let path = graph.sbs_path();
        let body = graph.function.as_mut().ok_or_else(|| format!("{path}: expected a Function Graph"))?;
        if body.nodes.iter().any(|node| node.missing.is_some()) {
            return Err(format!("{path}: contains unsupported operations").into());
        }

        // Resolve a temporary body so validation cannot rewrite the source connector contract
        let mut resolved = body.clone();
        function::resolve_instances(&mut resolved, &imported.pkg, &imported.pkg.dependencies);
        function::compile(&resolved, 0).map_err(|error| format!("{path}: {error}"))?;
        crate::layout::reposition(body).map_err(|error| format!("{path}: {error}"))?;

        let result = ValueType::from_code(graph.function_type).ok_or_else(|| format!("{path}: unknown result type"))?;
        let inputs = if graph.exposed.is_empty() {
            "none".to_string()
        } else {
            graph.exposed.iter().map(|parameter| parameter.id.as_str()).collect::<Vec<_>>().join(", ")
        };
        graph.attributes.description = format!("Evaluates the {} function graph. Result type: {}. Inputs: {inputs}.", graph.name, result.label());
        for input in &mut graph.exposed {
            input.description.clear();
        }
        graph.attributes.author.clear();
        graph.attributes.author_url.clear();
        graph.attributes.user_data.clear();
        graph.frames.clear();
        graph.comments.clear();
        graph.pins.clear();
    }

    Ok(package)
}
