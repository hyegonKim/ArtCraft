use serde_json::{Value, json};

use crate::commands::{CommandSpec, bad};
use crate::{FunctionGraphTarget, Session, cmd};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "graph.node.edit",
        "Edit node graph",
        ["Edit"],
        Some("Ctrl+E"),
        "{} resolves the selected FX-Map, processor or graph instance to {kind, node, parameter?, graph?, document?, path?}; UI invocation opens its reference",
        |session| resolve(session).map(|_| ()),
        |session, _| resolve(session).map_err(|error| bad("graph.node.edit", error))
    )]
}

fn resolve(session: &Session) -> std::result::Result<Value, String> {
    if session.annotation_selection.is_some() {
        return Err("select a node with an editable graph".into());
    }
    let id = session.selection.last().copied().ok_or("select a node with an editable graph")?;
    let graph = session.graph().map_err(|error| error.to_string())?;
    let node = graph.node(id).map_err(|error| error.to_string())?;
    match node.def.as_str() {
        "fx_map" => Ok(json!({"kind": "pattern", "node": id, "parameter": "pattern"})),
        "pixel_processor" | "value_processor" => {
            FunctionGraphTarget::Parameter { node: id, parameter: "function".into() }.current(session).map_err(|error| error.to_string())?;
            Ok(json!({"kind": "function", "node": id, "parameter": "function"}))
        }
        "graph_instance" => {
            let (index, _) = materialcraft_eval::instance_graph(&session.pkg, node).ok_or("the referenced graph is missing")?;
            let path = node.instance.as_ref().filter(|instance| !instance.package.is_empty()).map(|instance| instance.package.as_str());
            let document = match path {
                Some(path) => session.open_packages().into_iter().find(|entry| entry.2 == Some(path)).map(|entry| entry.0),
                None => Some(session.document_id),
            };
            Ok(json!({"kind": "graph", "node": id, "graph": index, "document": document, "path": path}))
        }
        _ => Err("the selected node has no editable graph".into()),
    }
}
