use crate::{
    FunctionGraphTarget, Result, Session, cmd,
    commands::{CommandSpec, bad, has_graph},
};
use serde_json::{Value as Json, json};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "graph.function.variables",
        "List function variables",
        [],
        None,
        "{id, target?: FunctionGraphTarget} lists variable suggestions matching the selected Get node's type and caller context",
        has_graph,
        variables
    )]
}

fn variables(session: &mut Session, parameters: &Json) -> Result<Json> {
    let command = "graph.function.variables";
    let target: FunctionGraphTarget = serde_json::from_value(parameters.get("target").cloned().unwrap_or_else(|| json!({"kind": "graph"})))
        .map_err(|error| bad(command, error.to_string()))?;
    let id = parameters.get("id").and_then(Json::as_u64).and_then(|id| u32::try_from(id).ok()).ok_or_else(|| bad(command, "id must be a function node id"))?;
    Ok(json!({"variables": target.variables(session, id)?}))
}
