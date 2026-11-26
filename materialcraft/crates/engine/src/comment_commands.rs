use crate::commands::{CommandSpec, bad, has_graph, point, strp, u64p};
use crate::graph_cmds::nodes_rect;
use crate::{AnnotationSelection, Result, Session, cmd};
use materialcraft_core::Comment;
use serde_json::{Value as J, json};

pub(crate) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("graph.comment.add", "Add Comment", [], None, "{text?, at? [x, y]}", has_graph, comment_add),
        cmd!("graph.comment.edit", "Edit comment", [], None, "{id, text?, at?, size?}", has_graph, comment_edit),
        cmd!("graph.comment.delete", "Delete comment", [], None, "{id}", has_graph, comment_delete),
    ]
}

fn comment_add(session: &mut Session, parameters: &J) -> Result<J> {
    let target = crate::AnnotationTarget::resolve(session, parameters)?;
    let at = point(parameters, "at").or_else(|| nodes_rect(session, &session.selection).map(|rect| [rect[0], rect[1] - 90.0])).unwrap_or([0.0, -120.0]);
    let text = strp(parameters, "text").unwrap_or("Comment").to_string();

    session.edit("Add comment", |session| {
        let mut items = target.items(session)?;
        let id = target.next_id(session, &items)?;
        items.comments.push(Comment { id, text, pos: at, size: [180.0, 70.0] });
        target.replace(session, items)?;
        session.annotation_target = target;
        session.selection.clear();
        session.annotation_selection = Some(AnnotationSelection::Comment(id));
        Ok(json!({"id": id}))
    })
}

fn comment_edit(session: &mut Session, parameters: &J) -> Result<J> {
    let target = crate::AnnotationTarget::resolve(session, parameters)?;
    let id = u64p(parameters, "id").ok_or_else(|| bad("graph.comment.edit", "`id` is required"))?;

    session.edit_merged(&format!("comment:{target:?}:{id}"), "Edit comment", |session| {
        let mut items = target.items(session)?;
        let comment = items.comments.iter_mut().find(|comment| comment.id == id).ok_or_else(|| bad("graph.comment.edit", format!("no comment {id}")))?;
        if let Some(text) = strp(parameters, "text") {
            comment.text = text.into();
        }
        if let Some(position) = point(parameters, "at") {
            comment.pos = position;
        }
        if let Some(size) = point(parameters, "size") {
            comment.size = [size[0].max(40.0), size[1].max(24.0)];
        }

        target.replace(session, items)?;
        Ok(J::Null)
    })
}

fn comment_delete(session: &mut Session, parameters: &J) -> Result<J> {
    let target = crate::AnnotationTarget::resolve(session, parameters)?;
    let id = u64p(parameters, "id").ok_or_else(|| bad("graph.comment.delete", "`id` is required"))?;

    session.edit("Delete comment", |session| {
        let mut items = target.items(session)?;
        items.comments.retain(|comment| comment.id != id);

        target.replace(session, items)?;
        Ok(J::Null)
    })
}
