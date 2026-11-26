use crate::commands::{CommandSpec, bad, has_graph, strp, u64p};
use crate::{AnnotationSelection, Result, Session, cmd};
use serde_json::{Value, json};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "graph.annotation.select",
        "Select annotation",
        [],
        None,
        "{kind?: frame|comment|pin, id?} clears node selection; omit kind to deselect",
        has_graph,
        select
    )]
}

fn select(session: &mut Session, parameters: &Value) -> Result<Value> {
    let target = crate::AnnotationTarget::resolve(session, parameters)?;
    let items = target.items(session)?;
    let selection = match strp(parameters, "kind") {
        None => None,
        Some(kind) => {
            let id = u64p(parameters, "id").ok_or_else(|| bad("graph.annotation.select", "`id` is required"))?;
            let selection = match kind {
                "frame" => AnnotationSelection::Frame(id),
                "comment" => AnnotationSelection::Comment(id),
                "pin" => AnnotationSelection::Pin(id),
                _ => return Err(bad("graph.annotation.select", "kind must be frame, comment or pin")),
            };
            if !selection.exists(&items) {
                return Err(bad("graph.annotation.select", "annotation does not exist"));
            }

            Some(selection)
        }
    };

    session.annotation_target = target;
    session.annotation_selection = selection;
    if selection.is_some() {
        session.selection.clear();
    }

    Ok(json!({"selection": selection}))
}
