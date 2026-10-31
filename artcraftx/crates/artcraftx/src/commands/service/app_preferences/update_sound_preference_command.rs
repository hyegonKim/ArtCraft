use crate::commands::service::app_preferences::notify_app_preferences_changed::notify_app_preferences_changed;
use crate::state::app_preferences::app_preferences_manager::AppPreferencesManager;
use crate::state::app_preferences::settings::app_sound_file::{optional_sound, AppSoundFile};
use crate::state::app_preferences::settings::app_sound_preferences::{AppSoundEvent, AppSoundPreferences};
use log::{error, info, warn};
use serde_derive::{Deserialize, Serialize};
use tauri::{AppHandle, State};

/// Change the sound for one event.
///
/// - `set`: `sound` is a catalog key (e.g. `"done"`), `{ "custom_wav":
///   "/abs/path.wav" }`, or `"none"` / `null` / absent for silent. A custom
///   file is checked to exist (and be a `.wav`) before it's accepted.
/// - `reset_to_default`: back to the out-of-the-box sound for the event
///   (whatever `AppSoundPreferences::default()` says).
#[derive(Deserialize, Debug)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum UpdateSoundPreferenceRequest {
  Set {
    event: AppSoundEvent,
    #[serde(default, with = "optional_sound")]
    sound: Option<AppSoundFile>,
  },
  ResetToDefault {
    event: AppSoundEvent,
  },
}

/// The full sound preferences after the change, so the UI can re-render
/// without a second round trip.
#[derive(Serialize)]
pub struct UpdateSoundPreferenceResponse {
  pub sounds: AppSoundPreferences,
}

#[tauri::command]
pub async fn update_sound_preference_command(
  request: UpdateSoundPreferenceRequest,
  app: AppHandle,
  app_prefs: State<'_, AppPreferencesManager>,
) -> Result<UpdateSoundPreferenceResponse, String> {
  info!("update_sound_preference_command called: {:?}", request);

  if let UpdateSoundPreferenceRequest::Set { event, sound: Some(sound) } = &request {
    sound.validate().map_err(|reason| {
      warn!("Rejected sound for {:?}: {}", event, reason);
      reason
    })?;
  }

  let prefs = app_prefs
      .update(|prefs| match request {
        UpdateSoundPreferenceRequest::Set { event, sound } => prefs.sounds.set(event, sound),
        UpdateSoundPreferenceRequest::ResetToDefault { event } => prefs.sounds.reset_to_default(event),
      })
      .map_err(|err| {
        error!("Error updating sound preference: {:?}", err);
        format!("Error updating sound preference: {:?}", err)
      })?;

  notify_app_preferences_changed(&app);

  Ok(UpdateSoundPreferenceResponse { sounds: prefs.sounds })
}

#[cfg(test)]
mod tests {
  use super::*;

  fn decode(json: &str) -> UpdateSoundPreferenceRequest {
    serde_json::from_str(json).unwrap()
  }

  fn set_sound(request: UpdateSoundPreferenceRequest) -> (AppSoundEvent, Option<AppSoundFile>) {
    match request {
      UpdateSoundPreferenceRequest::Set { event, sound } => (event, sound),
      other => panic!("expected a set action, got {other:?}"),
    }
  }

  /// Pin the wire format the frontend sends (`UpdateSoundPreference.ts`).
  #[test]
  fn decodes_catalog_custom_and_silent() {
    let (event, sound) = set_sound(decode(r#"{"action":"set","event":"enqueue_success","sound":"done"}"#));
    assert_eq!(event, AppSoundEvent::EnqueueSuccess);
    assert_eq!(sound, Some(AppSoundFile::Done));

    let (event, sound) = set_sound(decode(r#"{"action":"set","event":"generation_failure","sound":{"custom_wav":"/tmp/x.wav"}}"#));
    assert_eq!(event, AppSoundEvent::GenerationFailure);
    assert_eq!(sound, Some(AppSoundFile::CustomWav("/tmp/x.wav".into())));

    assert_eq!(set_sound(decode(r#"{"action":"set","event":"delete_file","sound":"none"}"#)).1, None);
    assert_eq!(set_sound(decode(r#"{"action":"set","event":"delete_file","sound":null}"#)).1, None);
    assert_eq!(set_sound(decode(r#"{"action":"set","event":"delete_file"}"#)).1, None);
  }

  #[test]
  fn decodes_reset() {
    assert!(matches!(
      decode(r#"{"action":"reset_to_default","event":"generation_success"}"#),
      UpdateSoundPreferenceRequest::ResetToDefault { event: AppSoundEvent::GenerationSuccess },
    ));
  }

  #[test]
  fn rejects_unknown_action_event_or_sound() {
    assert!(serde_json::from_str::<UpdateSoundPreferenceRequest>(r#"{"event":"delete_file","sound":"done"}"#).is_err());
    assert!(serde_json::from_str::<UpdateSoundPreferenceRequest>(r#"{"action":"set","event":"launch","sound":"done"}"#).is_err());
    assert!(serde_json::from_str::<UpdateSoundPreferenceRequest>(r#"{"action":"set","event":"delete_file","sound":"bogus"}"#).is_err());
  }

  /// Resetting reads the default from the `Default` impl, so changing a
  /// default there is the only place it needs changing.
  #[test]
  fn reset_restores_the_default_impl_value() {
    let mut sounds = AppSoundPreferences::default();
    sounds.set(AppSoundEvent::EnqueueFailure, Some(AppSoundFile::CustomWav("/tmp/x.wav".into())));
    sounds.set(AppSoundEvent::DeleteFile, None);

    sounds.reset_to_default(AppSoundEvent::EnqueueFailure);
    sounds.reset_to_default(AppSoundEvent::DeleteFile);

    let defaults = AppSoundPreferences::default();
    assert_eq!(sounds.enqueue_failure, defaults.enqueue_failure);
    assert_eq!(sounds.delete_file, defaults.delete_file);
    assert_eq!(sounds, defaults);
  }
}
