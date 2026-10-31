use crate::commands::utils::response::shorthand::ResponseOrErrorMessage;
use crate::commands::utils::response::success_response_wrapper::SerializeMarker;
use log::info;
use serde_derive::Serialize;
use tauri::AppHandle;

#[derive(Serialize)]
pub struct ResetArtcraftCommandResponse {
  pub success: bool,
}

impl SerializeMarker for ResetArtcraftCommandResponse {}

#[tauri::command]
pub async fn reset_artcraft_command(
  _app: AppHandle,
) -> ResponseOrErrorMessage<ResetArtcraftCommandResponse> {
  info!("reset_artcraft_command called");

  // TODO: Reset local vs. production

  Ok(ResetArtcraftCommandResponse {
    success: true,
  }.into())
}

