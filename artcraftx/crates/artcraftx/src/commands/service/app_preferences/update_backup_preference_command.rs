use crate::commands::service::app_preferences::notify_app_preferences_changed::notify_app_preferences_changed;
use crate::services::backup::backup_target::is_backup_eligible;
use crate::state::app_preferences::app_preferences_manager::AppPreferencesManager;
use crate::state::app_preferences::settings::app_backup_preferences::AppBackupPreferences;
use crate::state::data_dir::app_data_root::AppDataRoot;
use log::{error, info};
use serde_derive::{Deserialize, Serialize};
use tauri::{AppHandle, State};

/// One backup preference change. The frontend sends
/// `{ "preference": "<name>", "value": <value> }`.
#[derive(Deserialize, Debug)]
#[serde(tag = "preference", content = "value", rename_all = "snake_case")]
pub enum UpdateBackupPreferenceRequest {
  /// Switch backups on or off. The selected account is kept either way.
  Enabled(bool),
  /// Pick the ArtCraft account to back up to (`null` clears it). Must name
  /// an existing ArtCraft web-session credential.
  ArtcraftCredentialId(Option<String>),
}

/// The full backup preferences after the change.
#[derive(Serialize)]
pub struct UpdateBackupPreferenceResponse {
  pub backup: AppBackupPreferences,
}

#[tauri::command]
pub async fn update_backup_preference_command(
  request: UpdateBackupPreferenceRequest,
  app: AppHandle,
  app_data_root: State<'_, AppDataRoot>,
  app_prefs: State<'_, AppPreferencesManager>,
) -> Result<UpdateBackupPreferenceResponse, String> {
  info!("update_backup_preference_command called: {:?}", request);

  if let UpdateBackupPreferenceRequest::ArtcraftCredentialId(Some(credential_id)) = &request {
    validate_backup_credential(&app_data_root, credential_id)?;
  }

  let prefs = app_prefs
      .update(|prefs| match request {
        UpdateBackupPreferenceRequest::Enabled(enabled) => prefs.backup.enabled = enabled,
        UpdateBackupPreferenceRequest::ArtcraftCredentialId(maybe_id) => {
          prefs.backup.maybe_artcraft_credential_id = maybe_id.filter(|id| !id.trim().is_empty());
        }
      })
      .map_err(|err| {
        error!("Error updating backup preference: {:?}", err);
        format!("Error updating backup preference: {:?}", err)
      })?;

  notify_app_preferences_changed(&app);

  Ok(UpdateBackupPreferenceResponse { backup: prefs.backup })
}

fn validate_backup_credential(app_data_root: &AppDataRoot, credential_id: &str) -> Result<(), String> {
  let credential = app_data_root
      .credentials_dir()
      .find_credential_by_id(credential_id)
      .map_err(|err| format!("Error looking up credential {}: {}", credential_id, err))?
      .ok_or_else(|| format!("No credential found for id {}", credential_id))?;
  if !is_backup_eligible(&credential) {
    return Err(format!(
      "Credential {} ({}) can't receive backups; pick an ArtCraft account",
      credential_id, credential.service,
    ));
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Pin the wire format the frontend sends (`UpdateBackupPreference.ts`).
  #[test]
  fn decodes_requests() {
    assert!(matches!(
      serde_json::from_str::<UpdateBackupPreferenceRequest>(r#"{"preference":"enabled","value":true}"#).unwrap(),
      UpdateBackupPreferenceRequest::Enabled(true),
    ));
    assert!(matches!(
      serde_json::from_str::<UpdateBackupPreferenceRequest>(r#"{"preference":"artcraft_credential_id","value":"credential_abc"}"#).unwrap(),
      UpdateBackupPreferenceRequest::ArtcraftCredentialId(Some(id)) if id == "credential_abc",
    ));
    assert!(matches!(
      serde_json::from_str::<UpdateBackupPreferenceRequest>(r#"{"preference":"artcraft_credential_id","value":null}"#).unwrap(),
      UpdateBackupPreferenceRequest::ArtcraftCredentialId(None),
    ));
    assert!(serde_json::from_str::<UpdateBackupPreferenceRequest>(r#"{"preference":"enabled","value":"yes"}"#).is_err());
  }
}
