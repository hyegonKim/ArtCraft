use crate::state::data_dir::subdirectory::trait_data_subdir::DataSubdir;
use std::path::{Path, PathBuf};

/// Loose per-concern settings files. App preferences live in
/// `app_preferences.toml` (see `AppPreferences`).
#[derive(Clone)]
pub struct AppSettingsDir {
  path: PathBuf,
}

impl DataSubdir for AppSettingsDir {
  const DIRECTORY_NAME: &'static str = "settings";

  fn new_from<P: AsRef<Path>> (dir: P) -> Self {
    Self {
      path: dir.as_ref().to_path_buf(),
    }
  }

  fn path(&self) -> &Path {
    &self.path
  }
}

impl AppSettingsDir {
  pub fn get_app_preferences_path(&self) -> PathBuf {
    self.path.join("app_preferences.toml")
  }

  /// The pre-TOML preferences file, read once to migrate existing installs.
  pub fn get_legacy_app_preferences_json_path(&self) -> PathBuf {
    self.path.join("app_preferences.json")
  }

  pub fn get_provider_preferences_path(&self) -> PathBuf {
    self.path.join("provider_preferences.json")
  }
}
