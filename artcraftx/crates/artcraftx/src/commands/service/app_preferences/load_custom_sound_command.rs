use crate::state::app_preferences::app_preferences_manager::AppPreferencesManager;
use crate::state::app_preferences::settings::app_sound_preferences::AppSoundEvent;
use log::{error, info, warn};
use tauri::ipc::Response;
use tauri::State;

/// The bytes of the custom `.wav` configured for an event, for the frontend
/// to play. The path comes from the saved preference — the frontend never
/// names a file — so this can only read files the user picked in Settings.
///
/// Errors (returned as a plain message, logged at `warn`) when the event has
/// no custom sound or the file has since gone missing; the frontend treats
/// that as "play nothing".
#[tauri::command]
pub async fn load_custom_sound_command(
  event: AppSoundEvent,
  app_prefs: State<'_, AppPreferencesManager>,
) -> Result<Response, String> {
  info!("load_custom_sound_command called for {:?}", event);

  let prefs = app_prefs.get().map_err(|err| {
    error!("Error reading app preferences: {:?}", err);
    format!("Error reading app preferences: {:?}", err)
  })?;

  let path = prefs
      .sounds
      .get(event)
      .and_then(|sound| sound.custom_wav_path())
      .ok_or_else(|| format!("No custom sound is configured for {:?}", event))?;

  let bytes = std::fs::read(path).map_err(|err| {
    warn!("Custom sound for {:?} could not be read from {}: {}", event, path.display(), err);
    format!("Custom sound file could not be read: {}", path.display())
  })?;

  Ok(Response::new(bytes))
}
