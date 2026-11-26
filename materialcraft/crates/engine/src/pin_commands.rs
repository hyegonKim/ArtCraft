use crate::commands::{CommandSpec, bad, has_graph, point, strp, u64p};
use crate::{AnnotationSelection, AnnotationTarget, Result, Session, cmd};
use materialcraft_core::Pin;
use serde_json::{Value, json};

/// Register navigation pin creation, description/position editing and deletion.
pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("graph.pin.add", "Add Navigation pin", [], None, "{name?, description?, at? [x, y], function?, pattern?}", has_graph, add),
        cmd!("graph.pin.edit", "Edit Navigation pin", [], None, "{id, description?, at? [x, y], function?, pattern?}", has_graph, edit),
        cmd!("graph.pin.delete", "Delete pin", [], None, "{id, function?, pattern?}", has_graph, delete),
    ]
}

fn add(session: &mut Session, parameters: &Value) -> Result<Value> {
    let target = AnnotationTarget::resolve(session, parameters)?;
    let mut items = target.items(session)?;
    let id = target.next_id(session, &items)?;
    let name =
        strp(parameters, "description").or_else(|| strp(parameters, "name")).map(str::to_string).unwrap_or_else(|| format!("Pin {}", items.pins.len() + 1));
    let pos = point(parameters, "at").unwrap_or([0.0; 2]);
    items.pins.push(Pin { id, name, pos });

    session.edit("Add pin", |session| {
        target.replace(session, items)?;
        session.selection.clear();
        session.annotation_target = target;
        session.annotation_selection = Some(AnnotationSelection::Pin(id));
        Ok(json!({"id": id}))
    })
}

fn edit(session: &mut Session, parameters: &Value) -> Result<Value> {
    let target = AnnotationTarget::resolve(session, parameters)?;
    let id = u64p(parameters, "id").ok_or_else(|| bad("graph.pin.edit", "`id` is required"))?;
    let mut items = target.items(session)?;
    let pin = items.pins.iter_mut().find(|pin| pin.id == id).ok_or_else(|| bad("graph.pin.edit", format!("no pin {id}")))?;
    if let Some(description) = strp(parameters, "description").or_else(|| strp(parameters, "name")) {
        pin.name = description.into();
    }
    if let Some(position) = point(parameters, "at") {
        pin.pos = position;
    }

    session.edit_merged(&format!("pin:{target:?}:{id}"), "Edit pin", |session| {
        target.replace(session, items)?;
        Ok(Value::Null)
    })
}

fn delete(session: &mut Session, parameters: &Value) -> Result<Value> {
    let target = AnnotationTarget::resolve(session, parameters)?;
    let id = u64p(parameters, "id").ok_or_else(|| bad("graph.pin.delete", "`id` is required"))?;
    let mut items = target.items(session)?;
    let previous = items.pins.len();
    items.pins.retain(|pin| pin.id != id);
    if previous == items.pins.len() {
        return Err(bad("graph.pin.delete", format!("no pin {id}")));
    }

    session.edit("Delete pin", |session| {
        target.replace(session, items)?;
        Ok(Value::Null)
    })
}
