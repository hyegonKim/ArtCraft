use serde_derive::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// What one modality's prompt box last had selected.
///
/// The backend stores these verbatim and never interprets them: the frontend
/// validates on hydration (a model or account that no longer exists is simply
/// dropped), so stale or hand-edited state can't break the app.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModalityPromptboxState {
  /// The stored credential (`credential_{entropy}`) the prompt box generates
  /// with. An account, not a provider: the user may have several accounts on
  /// one service.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub selected_account_id: Option<String>,

  /// The model id (as served by the `models` crate).
  #[serde(skip_serializing_if = "Option::is_none")]
  pub selected_model: Option<String>,

  /// Model/mode-specific options (aspect ratio, resolution, batch count,
  /// duration, ...), as an opaque JSON object owned by the frontend.
  pub options: Map<String, Value>,
}
