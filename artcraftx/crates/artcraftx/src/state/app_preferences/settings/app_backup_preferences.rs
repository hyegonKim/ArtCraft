use serde_derive::{Deserialize, Serialize};

/// Backing up generations made on other services (Higgsfield, Midjourney,
/// Runway, ...) to an ArtCraft account, so the user always keeps a copy.
///
/// Missing fields in an older preferences file fall back to the defaults.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppBackupPreferences {
  /// Master switch. Off by default; nothing is uploaded until the user opts
  /// in and picks an account.
  pub enabled: bool,

  /// The ArtCraft credential (`credential_...`) results are uploaded to.
  /// Kept when backups are switched off so re-enabling restores the choice.
  /// Only ArtCraft (cookie-session) credentials are valid here.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub maybe_artcraft_credential_id: Option<String>,
}

impl Default for AppBackupPreferences {
  fn default() -> Self {
    Self {
      enabled: false,
      maybe_artcraft_credential_id: None,
    }
  }
}
