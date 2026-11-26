use crate::{
    Result, Session, cmd,
    commands::{CommandSpec, bad, has_graph, u64p},
};
use materialcraft_core::NodeId;
use serde_json::{Value, json};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "graph.node.info",
        "Copy node info",
        [],
        None,
        "{id} returns text describing a node without changing the document or node clipboard",
        has_graph,
        info
    )]
}

fn info(session: &mut Session, parameters: &Value) -> Result<Value> {
    let command = "graph.node.info";
    let id = NodeId(u64p(parameters, "id").ok_or_else(|| bad(command, "id is required"))?);
    let node = session.graph()?.node(id)?;
    let text = serde_json::to_string_pretty(node).map_err(|error| bad(command, error.to_string()))?;
    Ok(json!({"text": text}))
}
