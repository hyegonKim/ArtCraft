use crate::error::artcraftx_error::ArtcraftXError;
use crate::state::app_preferences::app_preferences_manager::AppPreferencesManager;
use crate::state::app_preferences::settings::preferred_download_filename::{model_slug_from_model_type_str, DownloadFilenameParts};
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::utils::download::save_local_file_to_download_dir::save_local_file_to_download_dir;
use chrono::Local;
use log::{error, info};
use sqlite_database::queries::task::Task;
use std::path::PathBuf;

/// Extension used when a result file has none.
const DEFAULT_EXTENSION: &str = "bin";

/// Copy every result file into the user's download directory, named per their
/// filename convention. Fails open per file — a copy failure must never block
/// completing the task. Returns the saved paths, in input order.
pub fn save_results_to_download_dir(
  app_data_root: &AppDataRoot,
  app_preferences: &AppPreferencesManager,
  task: &Task,
  fallback_model_slug: &str,
  local_files: &[PathBuf],
) -> Vec<PathBuf> {
  let task_id = task.id.as_str();
  let mut saved = Vec::with_capacity(local_files.len());

  let app_prefs = match app_preferences.get() {
    Ok(prefs) => prefs,
    Err(err) => {
      error!("[TaskCompletion] Can't read app preferences; not saving downloads for task {}: {:?}", task_id, err);
      return saved;
    }
  };

  let model_slug = task.model_type
      .as_ref()
      .map(|model_type| model_slug_from_model_type_str(model_type.to_str()))
      .unwrap_or_else(|| fallback_model_slug.to_string());

  let download_time = Local::now();
  let is_batch = local_files.len() > 1;

  for (index, local_file) in local_files.iter().enumerate() {
    let extension = local_file.extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or(DEFAULT_EXTENSION);

    let filename = app_prefs.downloads.preferred_download_filename.build_filename(&DownloadFilenameParts {
      model_slug: &model_slug,
      download_time,
      maybe_batch_index: is_batch.then(|| index + 1),
      extension,
    });

    match save_local_file_to_download_dir(local_file, &filename, app_data_root, &app_prefs) {
      Ok(path) => {
        info!("[TaskCompletion] Saved task {} result to {:?}", task_id, path);
        saved.push(path);
      }
      Err(ArtcraftXError::CannotDownloadFilePathAlreadyExists { path }) => {
        info!("[TaskCompletion] Task {} result already saved: {:?}", task_id, path);
        saved.push(path);
      }
      Err(err) => error!("[TaskCompletion] Failed to save task {} result {:?}: {:?}", task_id, local_file, err),
    }
  }

  saved
}
