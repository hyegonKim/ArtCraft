//! The pre-TOML `app_preferences.json` layout, read once to migrate existing
//! installs. Never written.

use crate::state::app_preferences::app_preferences::AppPreferences;
use crate::state::app_preferences::settings::app_sound_file::AppSoundFile;
use crate::state::app_preferences::settings::preferred_download_directory::PreferredDownloadDirectory;
use crate::state::app_preferences::settings::preferred_download_filename::PreferredDownloadFilename;
use errors::AnyhowResult;
use log::warn;
use serde_derive::Deserialize;
use std::path::Path;

#[derive(Deserialize)]
pub(super) struct LegacyJsonPreferences {
  preferred_download_directory: Option<PreferredDownloadDirectory>,
  #[serde(default)]
  preferred_download_filename: Option<PreferredDownloadFilename>,
  play_sounds: Option<bool>,
  delete_file_sound: Option<String>,
  generation_success_sound: Option<String>,
  generation_failure_sound: Option<String>,
  /// Was the enqueue-success sound before the enqueue sounds were split.
  generation_enqueue_sound: Option<String>,
}

impl LegacyJsonPreferences {
  pub(super) fn load(path: &Path) -> AnyhowResult<Self> {
    let contents = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&contents)?)
  }

  /// Overlay the legacy values onto the defaults. Sound keys the catalog no
  /// longer knows are dropped (logged) rather than failing the migration.
  pub(super) fn into_preferences(self) -> AppPreferences {
    let mut prefs = AppPreferences::default();

    if let Some(directory) = self.preferred_download_directory {
      prefs.downloads.preferred_download_directory = directory;
    }
    if let Some(filename) = self.preferred_download_filename {
      prefs.downloads.preferred_download_filename = filename;
    }
    if let Some(play_sounds) = self.play_sounds {
      prefs.sounds.play_sounds = play_sounds;
    }
    prefs.sounds.delete_file = legacy_sound(self.delete_file_sound, prefs.sounds.delete_file);
    prefs.sounds.enqueue_success = legacy_sound(self.generation_enqueue_sound, prefs.sounds.enqueue_success);
    prefs.sounds.generation_success = legacy_sound(self.generation_success_sound, prefs.sounds.generation_success);
    prefs.sounds.generation_failure = legacy_sound(self.generation_failure_sound, prefs.sounds.generation_failure);

    prefs
  }
}

/// A legacy sound key is the catalog key string; unknown keys keep the default.
fn legacy_sound(maybe_key: Option<String>, default: Option<AppSoundFile>) -> Option<AppSoundFile> {
  let Some(key) = maybe_key else {
    return default;
  };
  match serde_json::from_value::<AppSoundFile>(serde_json::Value::String(key.clone())) {
    Ok(sound) => Some(sound),
    Err(_) => {
      warn!("Dropping unknown legacy sound key `{}`; using the default.", key);
      default
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn migrates_known_fields_and_drops_unknown_sounds() {
    let legacy: LegacyJsonPreferences = serde_json::from_str(r#"{
      "version": "3",
      "preferred_download_directory": {"custom": "/tmp/out"},
      "play_sounds": false,
      "delete_file_sound": "ghost",
      "generation_enqueue_sound": "not_a_real_sound",
      "generation_failure_sound": "wrong"
    }"#).unwrap();

    let prefs = legacy.into_preferences();
    assert_eq!(prefs.downloads.preferred_download_directory, PreferredDownloadDirectory::Custom("/tmp/out".into()));
    assert_eq!(prefs.downloads.preferred_download_filename, PreferredDownloadFilename::ArtcraftConvention);
    assert!(!prefs.sounds.play_sounds);
    assert_eq!(prefs.sounds.delete_file, Some(AppSoundFile::Ghost));
    assert_eq!(prefs.sounds.enqueue_success, Some(AppSoundFile::Done)); // unknown key -> default
    assert_eq!(prefs.sounds.generation_failure, Some(AppSoundFile::Wrong));
    assert_eq!(prefs.sounds.generation_success, Some(AppSoundFile::SpecialFlower)); // absent -> default
  }
}
