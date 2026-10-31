use crate::error::artcraftx_error::ArtcraftXError;
use crate::commands::utils::response::shorthand::ResponseOrErrorMessage;
use crate::commands::utils::response::success_response_wrapper::SerializeMarker;
use crate::state::app_preferences::app_preferences_manager::AppPreferencesManager;
use crate::state::data_dir::app_data_root::AppDataRoot;
use anyhow::anyhow;
use log::info;
use serde_derive::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

#[derive(Serialize)]
pub struct DownloadDirectoryRevealSuccessResponse {
}

impl SerializeMarker for DownloadDirectoryRevealSuccessResponse {}

#[tauri::command]
pub async fn download_directory_reveal_command(
  app: AppHandle,
  app_prefs: State<'_, AppPreferencesManager>,
  app_data_root: State<'_, AppDataRoot>,
) -> ResponseOrErrorMessage<DownloadDirectoryRevealSuccessResponse> {

  info!("download_directory_reveal_command called");

  let result = handle_request(
    &app,
    &app_prefs,
    &app_data_root,
  ).await;

  if let Err(err) = result {
    format!("Error revealing download dir: {:?}", err);
    return Err("error revealing download dir".into())
  }

  Ok(DownloadDirectoryRevealSuccessResponse {}.into())
}


pub async fn handle_request(
  app: &AppHandle,
  app_prefs: &AppPreferencesManager,
  app_data_root: &AppDataRoot,
) -> Result<(), ArtcraftXError> {

  let app_prefs = app_prefs.get()?;

  let download_directory = app_prefs
      .downloads
      .preferred_download_directory
      .download_directory(app_data_root);

  info!("Opening download directory: {:?}", download_directory);

  // NB: `open_path` opens the directory ITSELF in the system file manager;
  // `reveal_item_in_dir` would open the PARENT with the directory selected.
  let download_directory = download_directory
      .to_str()
      .ok_or_else(|| anyhow!("Download directory path isn't valid UTF-8: {:?}", download_directory))?
      .to_string();

  app.opener().open_path(download_directory, None::<&str>)
      .map_err(|err| anyhow!("Failed to open directory: {:?}", err))?;

  Ok(())
}
