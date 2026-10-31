use crate::state::promptbox::promptbox_state::PromptboxState;
use crate::state::promptbox::promptbox_state_manager::PromptboxStateManager;
use log::{error, info};
use serde_derive::Serialize;
use tauri::State;

#[derive(Serialize)]
pub struct UpdatePromptboxStateResponse {
  pub state: PromptboxState,
}

/// Replaces the whole prompt box state. The frontend is the only writer and
/// always knows the full state, so it sends a complete snapshot (debounced)
/// rather than patches. Missing fields default; `version` is owned by the
/// backend and ignored.
#[tauri::command]
pub async fn update_promptbox_state_command(
  state: PromptboxState,
  promptbox_state: State<'_, PromptboxStateManager>,
) -> Result<UpdatePromptboxStateResponse, String> {
  info!("update_promptbox_state_command called: {:?}", state);

  let state = promptbox_state
      .update(|current| *current = replace_keeping_version(current, state))
      .map_err(|err| {
        error!("Error updating prompt box state: {:?}", err);
        format!("Error updating prompt box state: {:?}", err)
      })?;

  Ok(UpdatePromptboxStateResponse { state })
}

fn replace_keeping_version(current: &PromptboxState, incoming: PromptboxState) -> PromptboxState {
  PromptboxState { version: current.version, ..incoming }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn incoming_state_replaces_everything_but_version() {
    let mut current = PromptboxState::default();
    current.image.selected_model = Some("old_model".to_string());
    current.last_account_by_model.insert("old_model".to_string(), "credential_a".to_string());

    let incoming: PromptboxState = serde_json::from_str(
      r#"{ "version": 999, "video": { "selected_model": "seedance_2p0", "options": { "duration": 5 } } }"#,
    ).unwrap();

    let replaced = replace_keeping_version(&current, incoming);
    assert_eq!(replaced.version, current.version, "backend owns the version");
    assert_eq!(replaced.video.selected_model.as_deref(), Some("seedance_2p0"));
    assert_eq!(replaced.video.options["duration"], 5);
    assert_eq!(replaced.image, Default::default(), "omitted modalities reset, not merged");
    assert!(replaced.last_account_by_model.is_empty(), "omitted map resets, not merged");
  }
}
