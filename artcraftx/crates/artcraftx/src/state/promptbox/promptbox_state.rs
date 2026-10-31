use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::state::promptbox::modality_promptbox_state::ModalityPromptboxState;
use crate::state::promptbox::promptbox_modality::PromptboxModality;
use errors::AnyhowResult;
use log::warn;
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// File format version (not semver).
///  - 1: initial.
const CURRENT_VERSION: u32 = 1;

/// Everything the prompt boxes remember across restarts, persisted as
/// `state/promptbox_state.json`.
///
/// Every field defaults independently, so a file from an older build (or one
/// missing pieces) still loads; an unreadable file is replaced by the
/// defaults (logged), never a crash.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PromptboxState {
  pub version: u32,
  pub image: ModalityPromptboxState,
  pub video: ModalityPromptboxState,
  pub mesh: ModalityPromptboxState,
  pub splat: ModalityPromptboxState,
  pub audio: ModalityPromptboxState,

  /// The account last used with each model (model id -> credential id), so
  /// re-selecting a model brings back the account it was last run on, even
  /// when the current account could also run it.
  pub last_account_by_model: BTreeMap<String, String>,
}

impl Default for PromptboxState {
  fn default() -> Self {
    Self {
      version: CURRENT_VERSION,
      image: ModalityPromptboxState::default(),
      video: ModalityPromptboxState::default(),
      mesh: ModalityPromptboxState::default(),
      splat: ModalityPromptboxState::default(),
      audio: ModalityPromptboxState::default(),
      last_account_by_model: BTreeMap::new(),
    }
  }
}

impl PromptboxState {
  /// Load from the state directory, or the defaults when there is no file or
  /// it can't be read (logged).
  pub fn load_or_default(data_root: &AppDataRoot) -> Self {
    let path = data_root.state_dir().get_promptbox_state_path();
    if !path.exists() {
      return Self::default();
    }
    match Self::load_from_file(&path) {
      Ok(state) => state,
      Err(err) => {
        warn!("Could not read prompt box state from {:?}; starting fresh: {}", path, err);
        Self::default()
      }
    }
  }

  pub fn load_from_file(path: &Path) -> AnyhowResult<Self> {
    let contents = std::fs::read_to_string(path)?;
    let mut state: Self = serde_json::from_str(&contents)?;
    state.version = CURRENT_VERSION;
    Ok(state)
  }

  /// Write atomically (temp file + rename) so a crash mid-write can't leave a
  /// truncated file.
  pub fn save_to_file(&self, path: &Path) -> AnyhowResult<()> {
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent)?;
    }
    let contents = serde_json::to_string_pretty(self)?;
    let temp_path = path.with_extension("json.tmp");
    std::fs::write(&temp_path, contents)?;
    std::fs::rename(&temp_path, path)?;
    Ok(())
  }

  pub fn save(&self, data_root: &AppDataRoot) -> AnyhowResult<()> {
    self.save_to_file(&data_root.state_dir().get_promptbox_state_path())
  }

  pub fn modality(&self, modality: PromptboxModality) -> &ModalityPromptboxState {
    match modality {
      PromptboxModality::Image => &self.image,
      PromptboxModality::Video => &self.video,
      PromptboxModality::Mesh => &self.mesh,
      PromptboxModality::Splat => &self.splat,
      PromptboxModality::Audio => &self.audio,
    }
  }

  pub fn modality_mut(&mut self, modality: PromptboxModality) -> &mut ModalityPromptboxState {
    match modality {
      PromptboxModality::Image => &mut self.image,
      PromptboxModality::Video => &mut self.video,
      PromptboxModality::Mesh => &mut self.mesh,
      PromptboxModality::Splat => &mut self.splat,
      PromptboxModality::Audio => &mut self.audio,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn round_trips_through_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("promptbox_state.json");

    let mut state = PromptboxState::default();
    state.image.selected_account_id = Some("credential_abc".to_string());
    state.image.selected_model = Some("midjourney_7".to_string());
    state.image.options = json!({ "aspect_ratio": "square", "batch_count": 4 }).as_object().unwrap().clone();
    state.video.selected_model = Some("seedance_2p0".to_string());
    state.last_account_by_model.insert("midjourney_7".to_string(), "credential_abc".to_string());

    state.save_to_file(&path).unwrap();
    assert_eq!(PromptboxState::load_from_file(&path).unwrap(), state);
  }

  #[test]
  fn partial_or_unknown_fields_still_load() {
    let state: PromptboxState = serde_json::from_str(r#"{
      "version": 1,
      "image": { "selected_model": "nano_banana_pro", "future_field": true },
      "something_new": [1, 2, 3]
    }"#).unwrap();
    assert_eq!(state.image.selected_model.as_deref(), Some("nano_banana_pro"));
    assert_eq!(state.image.selected_account_id, None);
    assert_eq!(state.video, ModalityPromptboxState::default());
    assert!(state.last_account_by_model.is_empty());
  }

  #[test]
  fn unreadable_file_falls_back_to_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("promptbox_state.json");
    std::fs::write(&path, "{ this is not json").unwrap();
    assert!(PromptboxState::load_from_file(&path).is_err());
    // (load_or_default logs and returns Default; exercised via the manager.)
  }
}
