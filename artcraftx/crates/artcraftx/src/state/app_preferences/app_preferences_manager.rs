use crate::state::app_preferences::app_preferences::AppPreferences;
use crate::state::data_dir::app_data_root::AppDataRoot;
use errors::AnyhowResult;
use memory_store::clone_cell::CloneCell;

/// Shared, process-wide app preferences: an in-memory copy that every reader
/// clones, and a single write path that persists to disk.
#[derive(Clone)]
pub struct AppPreferencesManager {
  preferences: CloneCell<AppPreferences>,
  data_root: AppDataRoot,
}

impl AppPreferencesManager {
  /// Load from disk (migrating the legacy JSON file if present), or start
  /// from the defaults.
  pub fn load_or_default(data_root: &AppDataRoot) -> Self {
    Self {
      preferences: CloneCell::with_owned(AppPreferences::load_or_default(data_root)),
      data_root: data_root.clone(),
    }
  }

  /// A snapshot of the current preferences.
  pub fn get(&self) -> AnyhowResult<AppPreferences> {
    self.preferences.get_clone()
  }

  /// Apply a change to the current preferences, persist it, and return the
  /// updated snapshot. Nothing changes in memory if the write fails.
  pub fn update(&self, change: impl FnOnce(&mut AppPreferences)) -> AnyhowResult<AppPreferences> {
    let mut prefs = self.get()?;
    change(&mut prefs);
    prefs.save(&self.data_root)?;
    self.preferences.set_clone(&prefs)?;
    Ok(prefs)
  }
}
