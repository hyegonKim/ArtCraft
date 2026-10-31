use crate::state::app_preferences::app_preferences::AppPreferences;
use crate::state::app_preferences::app_preferences_manager::AppPreferencesManager;
use log::{error, info};
use serde_derive::Serialize;
use tauri::State;

/// The preferences as the frontend sees them: the same nested shape as the
/// on-disk file (`sounds`, `downloads`), minus the file version.
#[derive(Serialize)]
pub struct GetAppPreferencesResponse {
  pub preferences: AppPreferences,
}

#[tauri::command]
pub async fn get_app_preferences_command(
  app_prefs: State<'_, AppPreferencesManager>,
) -> Result<GetAppPreferencesResponse, String> {
  info!("get_app_preferences_command called");

  let preferences = app_prefs.get().map_err(|err| {
    error!("Error getting app preferences: {:?}", err);
    format!("Error getting app preferences: {:?}", err)
  })?;

  Ok(GetAppPreferencesResponse { preferences })
}
