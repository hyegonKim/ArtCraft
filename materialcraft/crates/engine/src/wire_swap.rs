use crate::{Result, SelectionTarget, Session, commands::bad};
use materialcraft_core::NodeId;
use serde_json::Value as Json;

const COMMAND: &str = "graph.wire.swap";

/// Whether two different source pins can feed each other's destinations
pub fn available(session: &Session, parameters: &Json) -> bool {
    validate(session, parameters).is_ok()
}

pub(crate) fn validate(session: &Session, parameters: &Json) -> Result<()> {
    let target = SelectionTarget::resolve(session, parameters)?;
    let wires = parameters.get("wires").and_then(Json::as_array).ok_or_else(|| bad(COMMAND, "wires must be a list of {to, toPort}"))?;
    let [first, second] = wires.as_slice() else { return Err(bad(COMMAND, "select exactly two links to swap")) };

    let first_source = source_pin(session, target.as_ref(), first)?;
    let second_source = source_pin(session, target.as_ref(), second)?;
    if first_source == second_source {
        return Err(bad(COMMAND, "the selected links start at the same output pin"));
    }
    if target.is_none() {
        for (wire, (from, from_port)) in [(first, second_source), (second, first_source)] {
            let (to, to_port) = destination(wire)?;
            crate::connection_commands::validate_connection(session, from, &from_port, to, to_port)?;
        }
    }
    Ok(())
}

fn destination(wire: &Json) -> Result<(NodeId, &str)> {
    let id = wire.get("to").and_then(Json::as_u64).map(NodeId).ok_or_else(|| bad(COMMAND, "invalid node id"))?;
    let port = wire.get("toPort").and_then(Json::as_str).ok_or_else(|| bad(COMMAND, "missing port"))?;
    Ok((id, port))
}

fn source_pin(session: &Session, target: Option<&SelectionTarget>, wire: &Json) -> Result<(NodeId, String)> {
    let (id, port) = destination(wire)?;
    let missing = || bad(COMMAND, format!("no link at port {port} of node {}", id.0));
    match target {
        Some(SelectionTarget::Function(target)) => {
            let graph = target.current(session)?;
            let source = u32::try_from(id.0)
                .ok()
                .and_then(|id| graph.node(id))
                .and_then(|node| port.parse::<usize>().ok().and_then(|slot| node.inputs.get(slot)))
                .copied()
                .flatten()
                .ok_or_else(missing)?;
            Ok((NodeId(u64::from(source)), "output".into()))
        }
        Some(SelectionTarget::Pattern(owner)) => {
            let graph = SelectionTarget::pattern(session, *owner)?;
            let node = graph.nodes.iter().find(|node| u64::from(node.id) == id.0).ok_or_else(missing)?;
            let connected = node.children.iter().enumerate().any(|(slot, child)| node.kind.child_port(slot) == Some(port) && child.is_some());
            if !connected {
                return Err(missing());
            }

            // FX-Map links are addressed by their parent output, rather than the destination input
            Ok((id, port.into()))
        }
        None => {
            let graph = session.graph()?;
            let connection = graph.input_of(id, port).ok_or_else(missing)?;
            Ok((connection.from, connection.from_port.clone()))
        }
    }
}
