//! Wire commands: edit wires (connections) of the open graph, each addressed by the input it
//! feeds (`to`, `toPort`): split a wire with a reroute dot so it can be repositioned, delete
//! several wires at once, and exchange the sources of a pair of wires.

use serde_json::{Value as J, json};

use materialcraft_core::{Connection, NodeId};

use crate::commands::{CommandSpec, bad, has_graph, point, strp, u64p};
use crate::{Result, Session, cmd};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "graph.wire.insertDot",
            "Insert reroute dot",
            [],
            None,
            "{to, toPort, at? [x, y] (graph point, default: midway)} splits the wire feeding to.toPort with a Dot node there, selected",
            has_graph,
            insert_dot
        ),
        cmd!(
            "graph.wire.delete",
            "Delete links",
            [],
            None,
            "{function?: FunctionGraphTarget, pattern?: owner, wires: [{to, toPort}, ...]} deletes the wires feeding those inputs, in one undo step",
            has_graph,
            delete
        ),
        cmd!(
            "graph.wire.swap",
            "Swap links",
            [],
            None,
            "{function?: FunctionGraphTarget, pattern?: owner, wires: [{to, toPort}, {to, toPort}]} exchanges the sources of exactly two connected inputs in one undo step",
            has_graph,
            swap
        ),
    ]
}

/// Most wires one command takes (a graph has far fewer inputs in practice).
const MAX_WIRES: usize = 4096;

/// The wires named by `wires: [{to, toPort}, ...]`, each existing and named once.
fn wires_param(s: &Session, id: &str, p: &J) -> Result<Vec<Connection>> {
    let list = p.get("wires").and_then(J::as_array).ok_or_else(|| bad(id, "`wires` is a list of {to, toPort}"))?;
    if list.len() > MAX_WIRES {
        return Err(bad(id, format!("at most {MAX_WIRES} wires")));
    }
    let g = s.graph()?;
    let mut out: Vec<Connection> = Vec::with_capacity(list.len());
    for w in list {
        let to = w.get("to").and_then(J::as_u64).map(NodeId).ok_or_else(|| bad(id, "each wire needs `to`"))?;
        let port = w.get("toPort").and_then(J::as_str).ok_or_else(|| bad(id, "each wire needs `toPort`"))?;
        let c = g.input_of(to, port).ok_or_else(|| bad(id, format!("no wire feeds `{port}` of node {}", to.0)))?;
        if out.iter().any(|o| o.to == c.to && o.to_port == c.to_port) {
            return Err(bad(id, format!("the wire into `{port}` of node {} is listed twice", to.0)));
        }
        out.push(c.clone());
    }
    Ok(out)
}

fn delete(s: &mut Session, p: &J) -> Result<J> {
    if let Some(result) = crate::embedded_wire_commands::execute(s, "graph.wire.delete", p, false) {
        return result;
    }

    let wires = wires_param(s, "graph.wire.delete", p)?;
    if wires.is_empty() {
        return Err(bad("graph.wire.delete", "no wire given"));
    }
    s.edit("Delete links", |s| {
        let g = s.graph_mut()?;
        for c in &wires {
            g.disconnect(c.to, &c.to_port);
        }
        Ok(json!({"deleted": wires.len()}))
    })
}

fn swap(session: &mut Session, parameters: &J) -> Result<J> {
    const ID: &str = "graph.wire.swap";
    crate::wire_swap::validate(session, parameters)?;
    if let Some(result) = crate::embedded_wire_commands::execute(session, ID, parameters, true) {
        return result;
    }

    let wires = wires_param(session, ID, parameters)?;
    if wires.len() != 2 {
        return Err(bad(ID, "select exactly two links to swap"));
    }

    session.edit("Swap links", |session| {
        let graph = session.graph_mut()?;
        // Disconnect both first so an intermediate connection cannot create a false cycle
        for connection in &wires {
            graph.disconnect(connection.to, &connection.to_port);
        }
        for (connection, source) in wires.iter().zip(wires.iter().rev()) {
            graph
                .connect(source.from, &source.from_port, connection.to, &connection.to_port)
                .map_err(|_| bad(ID, format!("swapping would feed node {} into itself (a cycle)", connection.to.0)))?;
        }
        Ok(json!({"swapped": wires.len()}))
    })
}

fn insert_dot(s: &mut Session, p: &J) -> Result<J> {
    const ID: &str = "graph.wire.insertDot";
    let to = NodeId(u64p(p, "to").ok_or_else(|| bad(ID, "`to` is required"))?);
    let to_port = strp(p, "toPort").ok_or_else(|| bad(ID, "`toPort` is required"))?.to_string();
    let g = s.graph()?;
    let c = g.input_of(to, &to_port).cloned().ok_or_else(|| bad(ID, format!("no wire feeds `{to_port}` of node {}", to.0)))?;
    let at = match point(p, "at") {
        Some(at) => at,
        None => {
            let (a, b) = (g.node(c.from)?.pos, g.node(to)?.pos);
            [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5]
        }
    };
    // Hostile but finite coordinates stay within a sane canvas.
    let at = crate::graph_grid::snap_position(at, crate::commands::boolp(p, "snapToGrid").unwrap_or(false));
    s.edit("Insert reroute dot", |s| {
        let id = s.graph_mut()?.add_node("dot", at);
        let input = s.input_ports(id).into_iter().next().ok_or_else(|| bad(ID, "the dot has no input"))?;
        let output = s.output_ports(id).into_iter().next().ok_or_else(|| bad(ID, "the dot has no output"))?;
        let g = s.graph_mut()?;
        g.connect(c.from, &c.from_port, id, &input)?;
        g.connect(id, &output, to, &to_port)?;
        s.selection = vec![id];
        Ok(json!({"id": id.0}))
    })
}
