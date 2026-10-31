use crate::error::artcraftx_error::ArtcraftXError;
use crate::state::app_preferences::app_preferences::AppPreferences;
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::utils::download::download_url_to_user_download_dir::check_download_file_name;
use anyhow::anyhow;
use log::info;
use std::path::{Path, PathBuf};

/// Copy an already-fetched local file (eg. a result sitting in the app's temp
/// directory) into the user's configured download directory under `filename`.
///
/// Returns the final path. If a file with that name already exists, nothing is
/// copied and [`ArtcraftXError::CannotDownloadFilePathAlreadyExists`] is
/// returned — callers treat that as "already saved".
pub fn save_local_file_to_download_dir(
  source: &Path,
  filename: &str,
  app_data_root: &AppDataRoot,
  app_prefs: &AppPreferences,
) -> Result<PathBuf, ArtcraftXError> {
  check_download_file_name(filename)?;

  let download_directory = app_prefs
      .downloads
      .preferred_download_directory
      .download_directory(app_data_root);

  let destination = download_directory.join(filename);

  if destination == download_directory {
    return Err(ArtcraftXError::AnyhowError(anyhow!("Download filename resolved to directory: {:?}", destination)));
  }

  if destination.exists() {
    if destination.is_dir() {
      return Err(ArtcraftXError::AnyhowError(anyhow!("Download path exists and resolved to directory: {:?}", destination)));
    }
    return Err(ArtcraftXError::CannotDownloadFilePathAlreadyExists { path: destination });
  }

  std::fs::create_dir_all(&download_directory)?;
  std::fs::copy(source, &destination)?;

  info!("Saved {:?} to {:?}", source, destination);

  Ok(destination)
}
