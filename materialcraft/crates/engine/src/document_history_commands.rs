use crate::{
    HistoryPrefs, Result, Session, cmd,
    commands::{CommandSpec, always, bad},
    document_history::DocumentHistory,
};
use serde_json::{Value, json};

fn can_undo(session: &Session) -> std::result::Result<(), String> {
    if session.can_undo() || session.documents.values().any(|document| document.history.can_undo()) { Ok(()) } else { Err("nothing to undo".into()) }
}

fn can_redo(session: &Session) -> std::result::Result<(), String> {
    if session.can_redo() || session.documents.values().any(|document| document.history.can_redo()) { Ok(()) } else { Err("nothing to redo".into()) }
}

fn document(session: &Session, parameters: &Value, command: &str) -> Result<u64> {
    match parameters.get("document") {
        None => Ok(session.document_id),
        Some(value) => value.as_u64().ok_or_else(|| bad(command, "document must be an unsigned integer")),
    }
}

pub(crate) fn specs() -> Vec<CommandSpec> {
    // Enabled predicates have no parameters; the requested document is checked again when run
    vec![
        cmd!(
            "session.history",
            "History preferences",
            [],
            None,
            "{undoLimit?: 1..10000} sets retained steps per package; {} reads preferences; does not modify packages",
            always,
            |session, parameters| {
                if let Some(value) = parameters.get("undoLimit") {
                    let limit = value
                        .as_u64()
                        .and_then(|value| u32::try_from(value).ok())
                        .filter(|value| (1..=10_000).contains(value))
                        .ok_or_else(|| bad("session.history", "undoLimit must be an integer from 1 to 10000"))?;
                    session.set_history_prefs(HistoryPrefs { undo_limit: limit });
                }
                Ok(json!(session.history_prefs))
            }
        ),
        cmd!("edit.undo", "Undo", ["Edit"], Some("Ctrl+Z"), "{document?} defaults to the active package", can_undo, |session, parameters| {
            let document = document(session, parameters, "edit.undo")?;
            let label = session.undo_document(document)?.ok_or_else(|| bad("edit.undo", "nothing to undo in this package"))?;
            Ok(json!({"undone": label}))
        }),
        cmd!("edit.redo", "Redo", ["Edit"], Some("Ctrl+Y"), "{document?} defaults to the active package", can_redo, |session, parameters| {
            let document = document(session, parameters, "edit.redo")?;
            let label = session.redo_document(document)?.ok_or_else(|| bad("edit.redo", "nothing to redo in this package"))?;
            Ok(json!({"redone": label}))
        }),
    ]
}

impl Session {
    /// Apply an app preference to existing documents and future edits without recording an edit
    pub fn set_history_prefs(&mut self, preferences: HistoryPrefs) {
        self.history_prefs = preferences.sanitized();
        let limit = self.history_prefs.undo_limit as usize;
        self.history.trim(limit);
        for document in self.documents.values_mut() {
            document.history.trim(limit);
        }
        self.finish_edit();
    }

    fn document_history(&self, document: u64) -> Option<&DocumentHistory> {
        if self.has_package() && document == self.document_id { Some(&self.history) } else { self.documents.get(&document).map(|document| &document.history) }
    }

    pub fn can_undo_document(&self, document: u64) -> bool {
        self.document_history(document).is_some_and(DocumentHistory::can_undo)
    }

    pub fn can_redo_document(&self, document: u64) -> bool {
        self.document_history(document).is_some_and(DocumentHistory::can_redo)
    }

    /// Undo labels for an open package, oldest first, without activating it
    pub fn document_history_labels(&self, document: u64) -> Vec<String> {
        self.document_history(document).map(DocumentHistory::labels).unwrap_or_default()
    }

    /// Undo in an open package without changing the active canvas
    pub fn undo_document(&mut self, document: u64) -> Result<Option<String>> {
        self.package(document)?;
        if document == self.document_id {
            return Ok(self.undo());
        }

        let target = self.documents.get_mut(&document).ok_or_else(|| bad("edit.undo", "the package is no longer open"))?;
        let label = target.undo(self.history_prefs.undo_limit as usize);
        if label.is_some() {
            self.finish_edit();
            self.refresh_links();
        }
        Ok(label)
    }

    /// Redo in an open package without changing the active canvas
    pub fn redo_document(&mut self, document: u64) -> Result<Option<String>> {
        self.package(document)?;
        if document == self.document_id {
            return Ok(self.redo());
        }

        let target = self.documents.get_mut(&document).ok_or_else(|| bad("edit.redo", "the package is no longer open"))?;
        let label = target.redo(self.history_prefs.undo_limit as usize);
        if label.is_some() {
            self.finish_edit();
            self.refresh_links();
        }
        Ok(label)
    }
}
