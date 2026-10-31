use crate::commands::service::app_preferences::notify_app_preferences_changed::notify_app_preferences_changed;
use crate::state::app_preferences::app_preferences_manager::AppPreferencesManager;
use crate::state::app_preferences::settings::app_prompt_preferences::AppPromptPreferences;
use log::{error, info};
use serde_derive::{Deserialize, Serialize};
use tauri::{AppHandle, State};

/// One prompt-box preference change. The frontend sends
/// `{ "preference": "<name>", "value": <value> }`.
#[derive(Deserialize, Debug)]
#[serde(tag = "preference", content = "value", rename_all = "snake_case")]
pub enum UpdatePromptPreferenceRequest {
  EnterToGenerate(bool),
}

/// The full prompt preferences after the change.
#[derive(Serialize)]
pub struct UpdatePromptPreferenceResponse {
  pub prompt: AppPromptPreferences,
}

#[tauri::command]
pub async fn update_prompt_preference_command(
  request: UpdatePromptPreferenceRequest,
  app: AppHandle,
  app_prefs: State<'_, AppPreferencesManager>,
) -> Result<UpdatePromptPreferenceResponse, String> {
  info!("update_prompt_preference_command called: {:?}", request);

  let prefs = app_prefs
      .update(|prefs| match request {
        UpdatePromptPreferenceRequest::EnterToGenerate(enabled) => prefs.prompt.enter_to_generate = enabled,
      })
      .map_err(|err| {
        error!("Error updating prompt preference: {:?}", err);
        format!("Error updating prompt preference: {:?}", err)
      })?;

  notify_app_preferences_changed(&app);

  Ok(UpdatePromptPreferenceResponse { prompt: prefs.prompt })
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Pin the wire format the frontend sends (`UpdatePromptPreference.ts`).
  #[test]
  fn decodes_enter_to_generate() {
    assert!(matches!(
      serde_json::from_str::<UpdatePromptPreferenceRequest>(r#"{"preference":"enter_to_generate","value":false}"#).unwrap(),
      UpdatePromptPreferenceRequest::EnterToGenerate(false),
    ));
    assert!(serde_json::from_str::<UpdatePromptPreferenceRequest>(r#"{"preference":"enter_to_generate","value":"yes"}"#).is_err());
    assert!(serde_json::from_str::<UpdatePromptPreferenceRequest>(r#"{"preference":"tab_to_generate","value":true}"#).is_err());
  }
}
