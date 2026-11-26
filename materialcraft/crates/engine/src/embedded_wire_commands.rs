use crate::{Result, SelectionTarget, Session, commands::bad, graph_edit};
use materialcraft_core::Value;
use serde_json::{Value as Json, json};

pub(crate) fn execute(session: &mut Session, command: &str, parameters: &Json, swapping: bool) -> Option<Result<Json>> {
    match SelectionTarget::resolve(session, parameters) {
        Ok(Some(target)) => Some(run(session, command, parameters, target, swapping)),
        Ok(None) => None,
        Err(error) => Some(Err(error)),
    }
}

fn run(session: &mut Session, command: &str, parameters: &Json, target: SelectionTarget, swapping: bool) -> Result<Json> {
    let entries = parameters.get("wires").and_then(Json::as_array).ok_or_else(|| bad(command, "wires must be a list of {to, toPort}"))?;
    if entries.len() > 4096 {
        return Err(bad(command, "at most 4096 links"));
    }

    let wires = entries
        .iter()
        .map(|entry| {
            let id = entry.get("to").and_then(Json::as_u64).and_then(|id| u32::try_from(id).ok()).ok_or_else(|| bad(command, "invalid node id"))?;
            let port = entry.get("toPort").and_then(Json::as_str).ok_or_else(|| bad(command, "missing port"))?;
            let slot = match &target {
                SelectionTarget::Function(_) => port.parse().ok(),
                SelectionTarget::Pattern(owner) => SelectionTarget::pattern(session, *owner)?
                    .nodes
                    .iter()
                    .find(|node| node.id == id)
                    .and_then(|node| (0..node.children.len()).find(|slot| node.kind.child_port(*slot) == Some(port))),
            }
            .ok_or_else(|| bad(command, "invalid link port"))?;
            Ok((id, slot))
        })
        .collect::<Result<Vec<_>>>()?;
    let label = if swapping { "Swap links" } else { "Delete links" };
    match target {
        SelectionTarget::Function(target) => {
            let mut graph = target.current(session)?.clone();
            graph_edit::edit_links(&mut graph, &wires, swapping).map_err(|error| bad(command, error))?;
            crate::graph_validation::function(&graph).map_err(|error| bad(command, error))?;
            session.edit(label, |session| target.replace(session, graph))?;
        }
        SelectionTarget::Pattern(owner) => {
            let mut graph = SelectionTarget::pattern(session, owner)?.clone();
            graph_edit::edit_links(&mut graph, &wires, swapping).map_err(|error| bad(command, error))?;
            crate::graph_validation::pattern(&graph).map_err(|error| bad(command, error))?;
            session.edit(label, |session| {
                session.graph_mut()?.node_mut(owner)?.params.insert("pattern".into(), Value::PatternGraph(Box::new(graph)));
                Ok(())
            })?;
        }
    }
    Ok(if swapping { json!({"swapped": wires.len()}) } else { json!({"deleted": wires.len()}) })
}
