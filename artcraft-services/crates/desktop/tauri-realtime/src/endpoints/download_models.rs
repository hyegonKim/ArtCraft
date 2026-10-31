use crate::state::app_dir::AppDataRoot;
use tauri::State;

#[tauri::command]
pub async fn download_models(
  _app_data_root: State<'_, AppDataRoot>,
) -> Result<String, String> {

  // NB: This endpoint has been removed for now as we now download models in the app lifecycle.

  Ok(("downloaded".to_string()))
}
