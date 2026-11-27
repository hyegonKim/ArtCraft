//! Attached AI agent presence (#1416): tracks which layers an external agent just edited or
//! selected so the canvas can highlight where it works for a few seconds.

use std::sync::Arc;
use std::time::Duration;

use figcraft_doc::{Document, NodeId};
use web_time::Instant;

use crate::Session;

/// How long an agent's presence stays visible after its last command.
pub const LINGER: Duration = Duration::from_secs(6);

/// Where an attached AI agent was last seen working.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentPresence {
    pub name: String,
    pub doc: u64,
    pub layers: Vec<NodeId>,
    pub at: Instant,
}

impl AgentPresence {
    /// Time remaining before this presence fades out.
    pub fn left(&self, now: Instant) -> Duration {
        (self.at + LINGER).saturating_duration_since(now)
    }
}

/// Changed layers in `after` vs `before`, omitting containers whose only change was their `children`
/// list when child layers were added, removed or reordered inside them.
fn edited_layers(before: &Document, after: &Document) -> Vec<NodeId> {
    let changed = crate::derived::changed_nodes(after, before);
    let mut out = Vec::new();
    for &id in &changed {
        let Some(an) = after.node(id) else { continue };
        if an.is_page() {
            continue;
        }
        if let Some(bn) = before.node(id)
            && bn.children != an.children
        {
            let mut probe = bn.clone();
            probe.children.clone_from(&an.children);
            if probe == *an {
                continue;
            }
        }
        out.push(id);
    }
    out
}

impl Session {
    pub(crate) fn agent_before(&self) -> Option<(u64, Arc<Document>, Vec<NodeId>)> {
        let ds = self.ds().ok()?;
        Some((ds.uid, ds.doc.clone(), ds.selection.clone()))
    }

    pub(crate) fn note_agent(&mut self, before: Option<(u64, Arc<Document>, Vec<NodeId>)>) {
        let Some(name) = self.agent.clone() else { return };
        let Ok(ds) = self.ds() else { return };
        let uid = ds.uid;
        let doc = ds.doc.clone();
        let sel = ds.selection.clone();

        let layers = match before {
            Some((b_uid, b_doc, b_sel)) if b_uid == uid => {
                if !Arc::ptr_eq(&b_doc, &doc) {
                    let changed = edited_layers(&b_doc, &doc);
                    if !changed.is_empty() {
                        changed
                    } else if b_sel != sel {
                        sel
                    } else {
                        self.agent_presence.as_ref().filter(|p| p.doc == uid).map(|p| p.layers.clone()).unwrap_or_default()
                    }
                } else if b_sel != sel {
                    sel
                } else {
                    self.agent_presence.as_ref().filter(|p| p.doc == uid).map(|p| p.layers.clone()).unwrap_or_default()
                }
            }
            _ => {
                if !sel.is_empty() {
                    sel
                } else {
                    Vec::new()
                }
            }
        };

        self.agent_presence = Some(AgentPresence { name, doc: uid, layers, at: Instant::now() });
    }

    /// The attached AI agent's presence if it is still within [`LINGER`] of `now`.
    pub fn agent_presence_at(&self, now: Instant) -> Option<&AgentPresence> {
        self.agent_presence.as_ref().filter(|p| now < p.at + LINGER)
    }
}
