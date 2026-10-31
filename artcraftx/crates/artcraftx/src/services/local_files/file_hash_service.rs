//! BLAKE3 for local files, memoized in the local_files index: when a path's
//! size and mtime match the indexed row the stored hash is returned without
//! reading the file; otherwise the file is hashed and the row refreshed.

use std::path::Path;
use std::time::UNIX_EPOCH;

use file_hashing::hash_file_blake3;
use local_files_database::queries::get_local_file_by_path::get_local_file_by_path;
use local_files_database::queries::upsert_local_file::{upsert_local_file, UpsertLocalFileArgs};
use log::warn;

use crate::state::database::local_files_database::LocalFilesDatabase;

/// `(size, mtime_ms)` for a regular file, `None` if it's missing.
pub fn stat_file(file_path: &Path) -> Option<(i64, i64)> {
  let stat = match std::fs::metadata(file_path) {
    Ok(metadata) if metadata.is_file() => metadata,
    _ => return None,
  };
  let file_mtime_ms = stat.modified().ok()
      .and_then(|mtime| mtime.duration_since(UNIX_EPOCH).ok())
      .map(|duration| duration.as_millis() as i64)
      .unwrap_or(0);
  Some((stat.len() as i64, file_mtime_ms))
}

/// The file's BLAKE3 hash, from the index when the stat matches, else by
/// reading the file (and indexing the result).
pub async fn hash_local_file_memoized(database: &LocalFilesDatabase, file_path: &Path) -> Result<String, String> {
  let (file_size_bytes, file_mtime_ms) = stat_file(file_path)
      .ok_or_else(|| format!("{} is not a readable file", file_path.display()))?;
  hash_local_file_with_stat(database, file_path, file_size_bytes, file_mtime_ms).await
}

/// [`hash_local_file_memoized`] for a caller that already stat-ed the file.
pub async fn hash_local_file_with_stat(
  database: &LocalFilesDatabase,
  file_path: &Path,
  file_size_bytes: i64,
  file_mtime_ms: i64,
) -> Result<String, String> {
  let path_string = file_path.to_string_lossy().into_owned();

  match get_local_file_by_path(database.get_connection(), &path_string).await {
    Ok(Some(record)) if record.matches_stat(file_size_bytes, file_mtime_ms) => {
      return Ok(record.file_hash_blake3);
    }
    Ok(_) => {}
    Err(err) => warn!("local_files lookup failed for {}: {}", path_string, err),
  }

  let hash_path = file_path.to_path_buf();
  let file_hash = tokio::task::spawn_blocking(move || hash_file_blake3(&hash_path))
      .await
      .map_err(|join_error| join_error.to_string())?
      .map_err(|io_error| io_error.to_string())?;

  let extension = file_extension(file_path);
  let upsert = upsert_local_file(UpsertLocalFileArgs {
    db: database.get_connection(),
    file_path: &path_string,
    maybe_file_type: (!extension.is_empty()).then_some(extension.as_str()),
    file_size_bytes,
    file_mtime_ms,
    file_hash_blake3: &file_hash,
  }).await;
  if let Err(err) = upsert {
    warn!("local_files upsert failed for {}: {}", path_string, err);
  }

  Ok(file_hash)
}

pub fn file_extension(file_path: &Path) -> String {
  file_path.extension()
      .and_then(|extension| extension.to_str())
      .map(str::to_ascii_lowercase)
      .unwrap_or_default()
}
