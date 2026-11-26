use serde::{Deserialize, Serialize};

/// Application-wide history preferences, independent of package contents
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HistoryPrefs {
    pub undo_limit: u32,
}

impl Default for HistoryPrefs {
    fn default() -> Self {
        Self { undo_limit: 200 }
    }
}

impl HistoryPrefs {
    pub fn sanitized(self) -> Self {
        // Bound snapshot retention even when preferences arrive from an external client
        Self { undo_limit: self.undo_limit.clamp(1, 10_000) }
    }
}
