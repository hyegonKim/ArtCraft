use crate::state::app_preferences::legacy_json_preferences::LegacyJsonPreferences;
use crate::state::app_preferences::settings::app_backup_preferences::AppBackupPreferences;
use crate::state::app_preferences::settings::app_download_preferences::AppDownloadPreferences;
use crate::state::app_preferences::settings::app_prompt_preferences::AppPromptPreferences;
use crate::state::app_preferences::settings::app_sound_preferences::AppSoundPreferences;
use crate::state::data_dir::app_data_root::AppDataRoot;
use errors::AnyhowResult;
use log::{info, warn};
use serde_derive::{Deserialize, Serialize};
use std::path::Path;

/// File format version (not semver).
///  - 1..=3: the legacy JSON layout (see `legacy_json_preferences`).
///  - 4: TOML, grouped into `[sounds]` / `[downloads]` tables.
///  - 5: added the `[prompt]` table.
///  - 6: added the `[backup]` table.
const CURRENT_VERSION: u32 = 6;

/// User-adjustable app preferences, persisted as `settings/app_preferences.toml`.
///
/// Grouped by concern so the file reads as nested tables:
///
/// ```toml
/// version = 4
///
/// [sounds]
/// play_sounds = true
/// enqueue_success = "done"
///
/// [downloads]
/// preferred_download_filename = "artcraft_convention"
///
/// [prompt]
/// enter_to_generate = true
///
/// [backup]
/// enabled = true
/// maybe_artcraft_credential_id = "credential_01j9dq3v5x8k2m7n4p6r9t0wxy"
///
/// [downloads.preferred_download_directory]
/// system = "downloads"
/// ```
///
/// Every group defaults independently, so a file missing a group (or a field)
/// still loads.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppPreferences {
  pub version: u32,
  pub sounds: AppSoundPreferences,
  pub downloads: AppDownloadPreferences,
  pub prompt: AppPromptPreferences,
  pub backup: AppBackupPreferences,
}

impl Default for AppPreferences {
  fn default() -> Self {
    Self {
      version: CURRENT_VERSION,
      sounds: AppSoundPreferences::default(),
      downloads: AppDownloadPreferences::default(),
      prompt: AppPromptPreferences::default(),
      backup: AppBackupPreferences::default(),
    }
  }
}

impl AppPreferences {
  /// Load from the settings directory. Falls back to the defaults when there
  /// is no file or it can't be read (logged), and migrates a legacy JSON file
  /// (rewriting it as TOML) the first time.
  pub fn load_or_default(data_root: &AppDataRoot) -> Self {
    let settings_dir = data_root.settings_dir();
    let toml_path = settings_dir.get_app_preferences_path();

    if toml_path.exists() {
      return match Self::load_from_file(&toml_path) {
        Ok(prefs) => prefs,
        Err(err) => {
          warn!("Could not read app preferences from {:?}; using defaults: {}", toml_path, err);
          Self::default()
        }
      };
    }

    let legacy_path = settings_dir.get_legacy_app_preferences_json_path();
    if legacy_path.exists() {
      match LegacyJsonPreferences::load(&legacy_path) {
        Ok(legacy) => {
          let prefs = legacy.into_preferences();
          info!("Migrating legacy app preferences {:?} -> {:?}", legacy_path, toml_path);
          if let Err(err) = prefs.save_to_file(&toml_path) {
            warn!("Could not write migrated app preferences: {}", err);
          }
          return prefs;
        }
        Err(err) => {
          warn!("Could not read legacy app preferences from {:?}; using defaults: {}", legacy_path, err);
        }
      }
    }

    Self::default()
  }

  pub fn load_from_file(path: &Path) -> AnyhowResult<Self> {
    let contents = std::fs::read_to_string(path)?;
    let mut prefs: Self = toml::from_str(&contents)?;
    prefs.version = CURRENT_VERSION;
    Ok(prefs)
  }

  /// Write atomically (temp file + rename) so a crash mid-write can't leave a
  /// truncated preferences file.
  pub fn save_to_file(&self, path: &Path) -> AnyhowResult<()> {
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent)?;
    }
    let contents = toml::to_string_pretty(self)?;
    let temp_path = path.with_extension("toml.tmp");
    std::fs::write(&temp_path, contents)?;
    std::fs::rename(&temp_path, path)?;
    Ok(())
  }

  pub fn save(&self, data_root: &AppDataRoot) -> AnyhowResult<()> {
    self.save_to_file(&data_root.settings_dir().get_app_preferences_path())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::state::app_preferences::settings::app_sound_file::AppSoundFile;
  use crate::state::app_preferences::settings::preferred_download_directory::PreferredDownloadDirectory;
  use crate::state::app_preferences::settings::preferred_download_filename::PreferredDownloadFilename;

  #[test]
  fn defaults_serialize_to_nested_toml() {
    let toml = toml::to_string_pretty(&AppPreferences::default()).unwrap();
    assert!(toml.starts_with("version = 6\n"), "{toml}");
    assert!(toml.contains("[sounds]\nplay_sounds = true\n"), "{toml}");
    assert!(toml.contains("enqueue_success = \"done\""), "{toml}");
    assert!(toml.contains("[downloads]\n"), "{toml}");
    assert!(toml.contains("[downloads.preferred_download_directory]\nsystem = \"downloads\""), "{toml}");
    assert!(toml.contains("[prompt]\nenter_to_generate = true"), "{toml}");
    assert!(toml.contains("[backup]\nenabled = false\n"), "{toml}");
  }

  #[test]
  fn round_trips_through_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app_preferences.toml");

    let mut prefs = AppPreferences::default();
    prefs.sounds.play_sounds = false;
    prefs.sounds.enqueue_failure = None;
    prefs.sounds.generation_success = Some(AppSoundFile::CustomWav("/tmp/ding.wav".into()));
    prefs.downloads.preferred_download_directory = PreferredDownloadDirectory::Custom("/tmp/out".into());
    prefs.downloads.preferred_download_filename = PreferredDownloadFilename::Custom("{model}_{date}".into());
    prefs.prompt.enter_to_generate = false;
    prefs.backup.enabled = true;
    prefs.backup.maybe_artcraft_credential_id = Some("credential_backup".to_string());

    prefs.save_to_file(&path).unwrap();
    let loaded = AppPreferences::load_from_file(&path).unwrap();
    assert_eq!(loaded, prefs);
  }

  #[test]
  fn partial_file_falls_back_to_defaults_per_field() {
    let prefs: AppPreferences = toml::from_str("[sounds]\nplay_sounds = false\n").unwrap();
    assert!(!prefs.sounds.play_sounds);
    assert_eq!(prefs.sounds.enqueue_success, Some(AppSoundFile::Done));
    assert_eq!(prefs.downloads, AppPreferences::default().downloads);
    assert!(prefs.prompt.enter_to_generate, "prompt table absent -> default");
    assert!(!prefs.backup.enabled, "backup table absent -> off");
  }

  #[test]
  fn silent_event_persists_as_none_string() {
    let mut prefs = AppPreferences::default();
    prefs.sounds.delete_file = None;
    let toml = toml::to_string_pretty(&prefs).unwrap();
    assert!(toml.contains("delete_file = \"none\""), "{toml}");
    let loaded: AppPreferences = toml::from_str(&toml).unwrap();
    assert_eq!(loaded.sounds.delete_file, None);
  }
}
