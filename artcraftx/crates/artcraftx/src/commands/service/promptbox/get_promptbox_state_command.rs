use crate::state::promptbox::promptbox_state::PromptboxState;
use crate::state::promptbox::promptbox_state_manager::PromptboxStateManager;
use log::{error, info};
use serde_derive::Serialize;
use tauri::State;

/// What every prompt box last had selected. The frontend hydrates its stores
/// from this on boot, dropping anything that no longer exists.
#[derive(Serialize)]
pub struct GetPromptboxStateResponse {
  pub state: PromptboxState,
}

#[tauri::command]
pub async fn get_promptbox_state_command(
  promptbox_state: State<'_, PromptboxStateManager>,
) -> Result<GetPromptboxStateResponse, String> {
  info!("get_promptbox_state_command called");

  let state = promptbox_state.get().map_err(|err| {
    error!("Error reading prompt box state: {:?}", err);
    format!("Error reading prompt box state: {:?}", err)
  })?;

  Ok(GetPromptboxStateResponse { state })
}
