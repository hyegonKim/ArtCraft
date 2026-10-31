use serde_derive::{Deserialize, Serialize};

/// How the prompt boxes behave (image, video, mesh, splat, ...).
///
/// Missing fields in an older preferences file fall back to the defaults.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppPromptPreferences {
  /// When on, Enter submits the prompt and Shift+Enter inserts a newline.
  /// When off, both insert a newline and only the button submits.
  pub enter_to_generate: bool,
}

impl Default for AppPromptPreferences {
  fn default() -> Self {
    Self {
      enter_to_generate: true,
    }
  }
}
