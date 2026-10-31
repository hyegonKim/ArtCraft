use artcraft_client::utils::api_host::ApiHost;
use crate::error::artcraftx_error::ArtcraftXError;
use crate::commands::utils::response::failure_response_wrapper::{CommandErrorResponseWrapper, CommandErrorStatus};
use crate::commands::utils::response::shorthand::ResponseOrErrorType;
use crate::commands::utils::response::success_response_wrapper::SerializeMarker;
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::utils::download::download_url_to_user_download_dir::download_url_to_user_download_dir;
use log::{error, info};
use serde_derive::{Deserialize, Serialize};
use artcraft_client::endpoints::media_files::get_media_file::get_media_file;
use tauri::{AppHandle, State};
use sqlite_identifiers::ids::media_file_token::MediaFileToken;
use crate::state::app_preferences::app_preferences_manager::AppPreferencesManager;

#[derive(Deserialize, Debug)]
pub struct DownloadMediaFileRequest {
  pub media_token: MediaFileToken,
}

#[derive(Serialize)]
pub struct DownloadMediaFileSuccessResponse {
}

impl SerializeMarker for DownloadMediaFileSuccessResponse {}

#[derive(Serialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum DownloadMediaFileErrorType {
  FilesystemError,
  NetworkError,
  UnknownError,
}


#[tauri::command]
pub async fn download_media_file_command(
  request: DownloadMediaFileRequest,
  app: AppHandle,
  app_prefs: State<'_, AppPreferencesManager>,
  app_data_root: State<'_, AppDataRoot>,
) -> ResponseOrErrorType<DownloadMediaFileSuccessResponse, DownloadMediaFileErrorType> {

  info!("download_media_file_command called");

  info!("request: {:?}", request);

  let result = handle_request(
    request,
    &app,
    &app_prefs,
    &app_data_root,
  ).await;

  if let Err(err) = result {
    error!("Error downloading media: {:?}", err);
    // TODO: This error is semantically incorrect - just trying to get the code done
    return Err(CommandErrorResponseWrapper {
      status: CommandErrorStatus::ServerError,
      error_message: Some("error downloading file".to_string()),
      error_type: Some(DownloadMediaFileErrorType::UnknownError),
      error_details: None,
    });
  }

  Ok(DownloadMediaFileSuccessResponse {}.into())
}


pub async fn handle_request(
  request: DownloadMediaFileRequest,
  _app: &AppHandle,
  app_prefs: &AppPreferencesManager,
  app_data_root: &AppDataRoot
) -> Result<(), ArtcraftXError> {

  let app_prefs = app_prefs.get()?;

  // TODO: Api should return the extension and suggested filename so we can better construct something.
  let media_file = get_media_file(
    &ApiHost::Storyteller,
    &request.media_token,
  ).await?;

  let asset_url = media_file.media_file.media_links.cdn_url;

  let download_path = download_url_to_user_download_dir(
    &asset_url,
    app_data_root,
    &app_prefs
  ).await?;

  info!("downloaded to: {:?}", download_path);

  Ok(())
}
