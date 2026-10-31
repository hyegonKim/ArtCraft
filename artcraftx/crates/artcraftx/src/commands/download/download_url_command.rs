use crate::error::artcraftx_error::ArtcraftXError;
use crate::commands::utils::response::failure_response_wrapper::{CommandErrorResponseWrapper, CommandErrorStatus};
use crate::commands::utils::response::shorthand::ResponseOrErrorType;
use crate::commands::utils::response::success_response_wrapper::SerializeMarker;
use crate::events::basic_sendable_event_trait::BasicSendableEvent;
use crate::events::warning_events::flash_file_download_error_event::{FlashFileDownloadErrorType, FlashFileDownloadErrorEvent};
use crate::state::app_preferences::app_preferences_manager::AppPreferencesManager;
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::utils::download::download_url_to_user_download_dir::download_url_to_user_download_dir;
use log::{error, info};
use serde_derive::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use url::Url;

#[derive(Deserialize, Debug)]
pub struct DownloadUrlRequest {
  pub url: Url,
}

#[derive(Serialize)]
pub struct DownloadUrlSuccessResponse {
}

impl SerializeMarker for DownloadUrlSuccessResponse {}

#[derive(Serialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum DownloadUrlErrorType {
  FilesystemError,
  NetworkError,
  UnknownError,
}


#[tauri::command]
pub async fn download_url_command(
  request: DownloadUrlRequest,
  app: AppHandle,
  app_prefs: State<'_, AppPreferencesManager>,
  app_data_root: State<'_, AppDataRoot>,
) -> ResponseOrErrorType<DownloadUrlSuccessResponse, DownloadUrlErrorType> {

  info!("download_url_command called");

  info!("request: {:?}", request);

  let result = handle_request(
    request,
    &app,
    &app_prefs,
    &app_data_root,
  ).await;

  if let Err(err) = result {
    // Already on disk = success. Download filenames derive from the content
    // hash, so the existing file IS the requested file. This also happens
    // routinely now that completed ArtCraft jobs are auto-downloaded by the
    // backend polling thread while the frontend auto-save races it.
    if let ArtcraftXError::CannotDownloadFilePathAlreadyExists { path } = &err {
      info!("File already downloaded (treating as success): {:?}", path);
      return Ok(DownloadUrlSuccessResponse {}.into());
    }

    error!("Error downloading media: {:?}", err);

    let endpoint_message = "unknown error when downloading file";
    let error_type = DownloadUrlErrorType::UnknownError;

    let flash_error_type = FlashFileDownloadErrorType::UnknownError;
    let flash_filename = None;
    let flash_message = Some("Failed to download file".to_string());

    let event = FlashFileDownloadErrorEvent {
      filename: flash_filename,
      message: flash_message,
      error_type: flash_error_type,
    };

    event.send_infallible(&app);

    return Err(CommandErrorResponseWrapper {
      status: CommandErrorStatus::ServerError,
      error_message: Some(endpoint_message.to_string()),
      error_type: Some(error_type),
      error_details: None,
    });
  }

  Ok(DownloadUrlSuccessResponse {}.into())
}


pub async fn handle_request(
  request: DownloadUrlRequest,
  _app: &AppHandle,
  app_prefs: &AppPreferencesManager,
  app_data_root: &AppDataRoot
) -> Result<(), ArtcraftXError> {

  let app_prefs = app_prefs.get()?;

  let download_path = download_url_to_user_download_dir(
    &request.url,
    app_data_root,
    &app_prefs
  ).await?;

  info!("downloaded to: {:?}", download_path);

  Ok(())
}
