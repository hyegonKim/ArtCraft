use crate::{
    FunctionGraphTarget, Result, Session, cmd,
    commands::{CommandSpec, bad, has_graph},
};
use serde_json::{Value as Json, json};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "graph.function.node.info",
        "Copy function node info",
        [],
        None,
        "{id, target?: FunctionGraphTarget} returns text describing a function node without changing the document or node clipboard",
        has_graph,
        info
    )]
}

fn info(session: &mut Session, parameters: &Json) -> Result<Json> {
    let command = "graph.function.node.info";
    let target: FunctionGraphTarget = serde_json::from_value(parameters.get("target").cloned().unwrap_or_else(|| json!({"kind": "graph"})))
        .map_err(|error| bad(command, error.to_string()))?;
    let id = parameters.get("id").and_then(Json::as_u64).and_then(|id| u32::try_from(id).ok()).ok_or_else(|| bad(command, "id must be a function node id"))?;
    let graph = target.current(session)?;
    let node = graph.node(id).ok_or_else(|| bad(command, "missing function node"))?;
    let label = materialcraft_nodes::function::node_label(node);
    let output = materialcraft_nodes::function::output_node(graph).is_some_and(|node| node.id == id);
    let text = serde_json::to_string_pretty(&json!({"target": target, "label": label, "node": node, "output": output}))
        .map_err(|error| bad(command, error.to_string()))?;
    Ok(json!({"text": text}))
}
