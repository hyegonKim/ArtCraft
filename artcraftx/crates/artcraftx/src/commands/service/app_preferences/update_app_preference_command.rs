use crate::commands::service::app_preferences::notify_app_preferences_changed::notify_app_preferences_changed;
use crate::state::app_preferences::app_preferences::AppPreferences;
use crate::state::app_preferences::app_preferences_manager::AppPreferencesManager;
use crate::state::app_preferences::settings::preferred_download_directory::PreferredDownloadDirectory;
use crate::state::app_preferences::settings::preferred_download_filename::PreferredDownloadFilename;
use anyhow::anyhow;
use errors::AnyhowResult;
use log::{error, info};
use serde_derive::{Deserialize, Serialize};
use tauri::{AppHandle, State};

/// One preference change, typed per preference. The frontend sends
/// `{ "preference": "<name>", "value": <value> }`.
///
/// Per-event sounds have their own command (`update_sound_preference_command`),
/// since custom files need validating.
#[derive(Deserialize, Debug)]
#[serde(tag = "preference", content = "value", rename_all = "snake_case")]
pub enum UpdateAppPreferencesRequest {
  PreferredDownloadDirectory(PreferredDownloadDirectory),
  PreferredDownloadFilename(PreferredDownloadFilename),
  PlaySounds(bool),
}

#[derive(Serialize)]
pub struct UpdateAppPreferencesResponse {
  pub success: bool,
}

#[tauri::command]
pub async fn update_app_preferences_command(
  request: UpdateAppPreferencesRequest,
  app: AppHandle,
  app_prefs: State<'_, AppPreferencesManager>,
) -> Result<UpdateAppPreferencesResponse, String> {
  info!("update_app_preferences_command called: {:?}", request);

  update_prefs(request, &app_prefs).map_err(|err| {
    error!("Error updating app preferences: {:?}", err);
    format!("Error updating app preferences: {:?}", err)
  })?;

  notify_app_preferences_changed(&app);

  Ok(UpdateAppPreferencesResponse { success: true })
}

fn update_prefs(request: UpdateAppPreferencesRequest, app_prefs: &AppPreferencesManager) -> AnyhowResult<()> {
  validate(&request)?;
  app_prefs.update(|prefs| apply(request, prefs))?;
  Ok(())
}

fn validate(request: &UpdateAppPreferencesRequest) -> AnyhowResult<()> {
  match request {
    UpdateAppPreferencesRequest::PreferredDownloadFilename(PreferredDownloadFilename::Custom(format)) => {
      PreferredDownloadFilename::validate_custom_format(format)
          .map_err(|reason| anyhow!("Invalid filename format: {}", reason))
    }
    _ => Ok(()),
  }
}

fn apply(request: UpdateAppPreferencesRequest, prefs: &mut AppPreferences) {
  use UpdateAppPreferencesRequest::*;
  match request {
    PreferredDownloadDirectory(directory) => prefs.downloads.preferred_download_directory = directory,
    PreferredDownloadFilename(filename) => prefs.downloads.preferred_download_filename = filename,
    PlaySounds(enabled) => prefs.sounds.play_sounds = enabled,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn decode(json: &str) -> UpdateAppPreferencesRequest {
    serde_json::from_str(json).unwrap()
  }

  /// Pin the wire format the frontend sends (`UpdateAppPreference.ts`).
  #[test]
  fn decodes_each_preference() {
    assert!(matches!(
      decode(r#"{"preference":"play_sounds","value":true}"#),
      UpdateAppPreferencesRequest::PlaySounds(true),
    ));
    assert!(matches!(
      decode(r#"{"preference":"preferred_download_filename","value":"artcraft_convention"}"#),
      UpdateAppPreferencesRequest::PreferredDownloadFilename(PreferredDownloadFilename::ArtcraftConvention),
    ));
    assert!(matches!(
      decode(r#"{"preference":"preferred_download_filename","value":{"custom_format":"{model}_{date}"}}"#),
      UpdateAppPreferencesRequest::PreferredDownloadFilename(PreferredDownloadFilename::Custom(_)),
    ));
    assert!(matches!(
      decode(r#"{"preference":"preferred_download_directory","value":{"custom":"/tmp"}}"#),
      UpdateAppPreferencesRequest::PreferredDownloadDirectory(PreferredDownloadDirectory::Custom(_)),
    ));
    assert!(matches!(
      decode(r#"{"preference":"preferred_download_directory","value":{"system":"downloads"}}"#),
      UpdateAppPreferencesRequest::PreferredDownloadDirectory(PreferredDownloadDirectory::System(_)),
    ));
  }

  #[test]
  fn wrong_value_type_is_rejected() {
    assert!(serde_json::from_str::<UpdateAppPreferencesRequest>(r#"{"preference":"play_sounds","value":"yes"}"#).is_err());
    // Per-event sounds moved to `update_sound_preference_command`.
    assert!(serde_json::from_str::<UpdateAppPreferencesRequest>(r#"{"preference":"enqueue_success_sound","value":"done"}"#).is_err());
  }

  #[test]
  fn custom_filename_format_is_validated() {
    let bad = decode(r#"{"preference":"preferred_download_filename","value":{"custom_format":"a/b"}}"#);
    assert!(validate(&bad).is_err());
    let good = decode(r#"{"preference":"preferred_download_filename","value":{"custom_format":"{model}_{date}"}}"#);
    assert!(validate(&good).is_ok());
  }

  #[test]
  fn apply_writes_into_the_right_group() {
    let mut prefs = AppPreferences::default();
    apply(decode(r#"{"preference":"play_sounds","value":false}"#), &mut prefs);
    apply(decode(r#"{"preference":"preferred_download_directory","value":{"custom":"/tmp/out"}}"#), &mut prefs);
    assert!(!prefs.sounds.play_sounds);
    assert_eq!(prefs.downloads.preferred_download_directory, PreferredDownloadDirectory::Custom("/tmp/out".into()));
  }
}
