use crate::{Result, Session, commands::bad};
use materialcraft_core::{Graph, Package};
use serde_json::Value;

/// A graph being inspected, independent of the active canvas
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GraphTarget {
    pub document: u64,
    pub graph: usize,
}

impl GraphTarget {
    pub fn active(session: &Session) -> Self {
        Self { document: session.document_id, graph: session.graph }
    }

    pub(crate) fn resolve(session: &Session, parameters: &Value) -> Result<Self> {
        let document = match parameters.get("document") {
            None | Some(Value::Null) => session.document_id,
            Some(value) => value.as_u64().ok_or_else(|| bad("graph", "`document` must be an open document id"))?,
        };
        let mut target = Self { document, graph: session.graph };
        let package = target.package(session)?;
        target.graph = match parameters.get("graph") {
            None | Some(Value::Null) if document == session.document_id => session.graph,
            None | Some(Value::Null) => session.documents.get(&document).map_or(0, |document| document.graph),
            Some(Value::String(name)) => package.graph_by_name(name).map(|(index, _)| index).ok_or_else(|| bad("graph", format!("no graph `{name}`")))?,
            Some(value) => value.as_u64().and_then(|index| usize::try_from(index).ok()).ok_or_else(|| bad("graph", "`graph` must be an index or name"))?,
        };
        target.get(session)?;
        Ok(target)
    }

    pub fn package(self, session: &Session) -> Result<&Package> {
        session.package(self.document)
    }

    pub fn get(self, session: &Session) -> Result<&Graph> {
        Ok(self.package(session)?.graph(self.graph)?)
    }

    pub(crate) fn edit<T>(self, session: &mut Session, label: &str, edit: impl FnOnce(&mut Package) -> Result<T>) -> Result<T> {
        session.finish_edit();
        self.edit_with_key(session, label, None, edit)
    }

    pub(crate) fn edit_merged<T>(self, session: &mut Session, key: &str, label: &str, edit: impl FnOnce(&mut Package) -> Result<T>) -> Result<T> {
        self.edit_with_key(session, label, Some(key), edit)
    }

    fn edit_with_key<T>(self, session: &mut Session, label: &str, key: Option<&str>, edit: impl FnOnce(&mut Package) -> Result<T>) -> Result<T> {
        session.edit_package(self.document, label, key, edit)
    }
}
