use crate::commands::utils::response::shorthand::ResponseOrErrorMessage;
use crate::commands::utils::response::success_response_wrapper::SerializeMarker;
use log::{error, info};
use serde_derive::{Deserialize, Serialize};
use std::path::Path;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

/// How to open a local file.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenLocalFileMode {
  /// Open the file with the OS default application (image viewer, video player, ...).
  Open,
  /// Open the containing directory in the OS file manager (Finder, Explorer, ...)
  /// with the file selected.
  Reveal,
}

#[derive(Serialize)]
pub struct OpenLocalFileSuccessResponse {}

impl SerializeMarker for OpenLocalFileSuccessResponse {}

/// Open a file that a completed task downloaded (see the task's
/// `on_complete_first_file_location`) natively, either in its default viewer
/// or revealed in the file manager. The file must exist.
#[tauri::command]
pub async fn open_local_file_command(
  app: AppHandle,
  path: String,
  mode: OpenLocalFileMode,
) -> ResponseOrErrorMessage<OpenLocalFileSuccessResponse> {
  info!("open_local_file_command called: mode={:?} path={:?}", mode, path);

  let file = Path::new(&path);

  if !file.is_file() {
    error!("open_local_file_command: not an existing file: {:?}", path);
    return Err("file not found".into());
  }

  let result = match mode {
    OpenLocalFileMode::Open => app.opener().open_path(&path, None::<&str>),
    OpenLocalFileMode::Reveal => app.opener().reveal_item_in_dir(file),
  };

  if let Err(err) = result {
    error!("open_local_file_command failed ({:?}) for {:?}: {:?}", mode, path, err);
    return Err("could not open file".into());
  }

  Ok(OpenLocalFileSuccessResponse {}.into())
}
