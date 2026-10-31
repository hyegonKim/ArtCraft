//! Thumbnails for local files, content-addressed and cached.
//!
//! Two entry points with one cache between them:
//!
//! - [`lookup_thumbnail`] is the cheap read path (a stat and a row lookup,
//!   never a file read) for request handlers. It reports what exists and
//!   says `Pending` for anything the worker hasn't produced yet.
//! - [`generate_thumbnail`] is the write path, run by the thumbnail worker
//!   thread: stat → BLAKE3 (from the local_files index when size+mtime
//!   match, else rehash) → `cache/thumbnails/{hash}.jpg` (static) and
//!   `{hash}.webp` (~1s animated preview, videos only) → record the outcome
//!   (state, cache paths, error, attempt count) on the file's row.
//!
//! Everything on disk is regenerable; the database rows make lookups cheap
//! and let the worker find work without walking the cache directory.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};

use local_files_database::queries::get_local_file_by_path::get_local_file_by_path;
use local_files_database::queries::local_file_record::LocalFileRecord;
use local_files_database::queries::set_thumbnail_result::{set_thumbnail_result, SetThumbnailResultArgs};
use local_files_database::thumbnail_state::ThumbnailState;
use log::{info, warn};
use once_cell::sync::Lazy;
use serde_derive::Serialize;
use thumbnails::{generate_image_thumbnail, generate_video_thumbnails, ThumbnailError, STATIC_THUMBNAIL_MAX_DIMENSION, THUMBNAIL_GENERATOR_VERSION};
use tokio::sync::{Mutex as TokioMutex, Semaphore};

use crate::services::local_files::file_hash_service::{file_extension, hash_local_file_with_stat, stat_file};
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::state::database::local_files_database::LocalFilesDatabase;

/// Failed generations are retried until this many attempts; after that the
/// file is left alone (a placeholder is shown).
pub const MAX_THUMBNAIL_ATTEMPTS: i64 = 3;

/// Cap on concurrent thumbnail generations (decode + encode are CPU-heavy).
const MAX_CONCURRENT_GENERATIONS: usize = 2;

static GENERATION_SEMAPHORE: Lazy<Semaphore> = Lazy::new(|| Semaphore::new(MAX_CONCURRENT_GENERATIONS));

/// Per-path locks so the worker and a stray direct call never hash or
/// generate the same file twice at once.
static PER_PATH_LOCKS: Lazy<StdMutex<HashMap<PathBuf, Arc<TokioMutex<()>>>>> =
    Lazy::new(|| StdMutex::new(HashMap::new()));

const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "bmp", "tiff", "avif"];
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "m4v", "mov"];

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalThumbnailStatus {
  /// A static thumbnail exists (animated preview too, when the path is set).
  Ready,
  /// Not generated yet; the worker thread will get to it and the frontend
  /// hears about it via `local_thumbnail_ready_event`.
  Pending,
  /// The file type (or codec) can't be thumbnailed; show a placeholder.
  Unsupported,
  /// The source file no longer exists on disk.
  MissingFile,
  /// Generation errored and won't be retried; show a placeholder.
  Failed,
}

#[derive(Clone, Debug, Serialize)]
pub struct LocalThumbnail {
  pub file_path: String,
  pub status: LocalThumbnailStatus,
  pub maybe_file_hash: Option<String>,
  pub maybe_thumbnail_path: Option<String>,
  pub maybe_animated_preview_path: Option<String>,
}

impl LocalThumbnail {
  fn without_thumbnail(file_path: &Path, status: LocalThumbnailStatus, maybe_file_hash: Option<String>) -> Self {
    Self {
      file_path: file_path.to_string_lossy().into_owned(),
      status,
      maybe_file_hash,
      maybe_thumbnail_path: None,
      maybe_animated_preview_path: None,
    }
  }

  fn ready(file_path: &Path, file_hash: String, thumbnail_jpg: &Path, animated_webp: Option<&Path>) -> Self {
    Self {
      file_path: file_path.to_string_lossy().into_owned(),
      status: LocalThumbnailStatus::Ready,
      maybe_file_hash: Some(file_hash),
      maybe_thumbnail_path: Some(thumbnail_jpg.to_string_lossy().into_owned()),
      maybe_animated_preview_path: animated_webp.map(|path| path.to_string_lossy().into_owned()),
    }
  }
}

/// The cache paths a `local_files` row points at, if its thumbnail has been
/// generated. No disk access — the row is trusted (the worker's scan
/// re-checks disk and regenerates anything that went missing).
pub fn generated_thumbnail_paths(record: &LocalFileRecord) -> Option<(String, Option<String>)> {
  if record.thumbnail_state() != ThumbnailState::Generated {
    return None;
  }
  let jpg = record.thumbnail_jpg_path.clone()?;
  Some((jpg, record.animated_preview_webp_path.clone()))
}

/// Whether the worker should (re)generate this row's thumbnail: never done,
/// still retryable, made by an older generator, or done but the cache file
/// is gone.
pub fn needs_generation(record: &LocalFileRecord) -> bool {
  match record.thumbnail_state() {
    ThumbnailState::Pending => true,
    ThumbnailState::Failed => record.thumbnail_attempts < MAX_THUMBNAIL_ATTEMPTS,
    ThumbnailState::Unsupported => false,
    ThumbnailState::Generated => {
      if record.thumbnail_generator_version < THUMBNAIL_GENERATOR_VERSION {
        return true;
      }
      match record.thumbnail_jpg_path.as_deref() {
        Some(jpg) => !Path::new(jpg).is_file(),
        None => true,
      }
    }
  }
}

/// Cheap read: what the cache holds for one file right now. Never hashes or
/// generates; anything not yet produced is `Pending`.
pub async fn lookup_thumbnail(database: &LocalFilesDatabase, file_path: &Path) -> LocalThumbnail {
  let Some((file_size_bytes, file_mtime_ms)) = stat_file(file_path) else {
    return LocalThumbnail::without_thumbnail(file_path, LocalThumbnailStatus::MissingFile, None);
  };

  let path_string = file_path.to_string_lossy().into_owned();
  let record = match get_local_file_by_path(database.get_connection(), &path_string).await {
    Ok(Some(record)) if record.matches_stat(file_size_bytes, file_mtime_ms) => record,
    Ok(_) => return LocalThumbnail::without_thumbnail(file_path, LocalThumbnailStatus::Pending, None),
    Err(err) => {
      warn!("local_files lookup failed for {}: {}", path_string, err);
      return LocalThumbnail::without_thumbnail(file_path, LocalThumbnailStatus::Pending, None);
    }
  };

  let file_hash = Some(record.file_hash_blake3.clone());
  match record.thumbnail_state() {
    ThumbnailState::Generated => match generated_thumbnail_paths(&record) {
      Some((jpg, maybe_webp)) if Path::new(&jpg).is_file() => LocalThumbnail {
        file_path: path_string,
        status: LocalThumbnailStatus::Ready,
        maybe_file_hash: file_hash,
        maybe_thumbnail_path: Some(jpg),
        maybe_animated_preview_path: maybe_webp,
      },
      // Cache file gone; the worker regenerates on its next scan.
      _ => LocalThumbnail::without_thumbnail(file_path, LocalThumbnailStatus::Pending, file_hash),
    },
    ThumbnailState::Unsupported => LocalThumbnail::without_thumbnail(file_path, LocalThumbnailStatus::Unsupported, file_hash),
    ThumbnailState::Failed if record.thumbnail_attempts >= MAX_THUMBNAIL_ATTEMPTS => {
      LocalThumbnail::without_thumbnail(file_path, LocalThumbnailStatus::Failed, file_hash)
    }
    ThumbnailState::Failed | ThumbnailState::Pending => {
      LocalThumbnail::without_thumbnail(file_path, LocalThumbnailStatus::Pending, file_hash)
    }
  }
}

/// Produce (or confirm) the cached thumbnails for one file and record the
/// outcome on its row. The worker thread's unit of work.
pub async fn generate_thumbnail(
  database: &LocalFilesDatabase,
  app_data_root: &AppDataRoot,
  file_path: &Path,
) -> LocalThumbnail {
  // Serialize work per path; overlapping calls wait, then hit the cache.
  let path_lock = per_path_lock(file_path);
  let _path_guard = path_lock.lock().await;

  let Some((file_size_bytes, file_mtime_ms)) = stat_file(file_path) else {
    return LocalThumbnail::without_thumbnail(file_path, LocalThumbnailStatus::MissingFile, None);
  };

  let file_hash = match hash_local_file_with_stat(database, file_path, file_size_bytes, file_mtime_ms).await {
    Ok(hash) => hash,
    Err(message) => {
      warn!("Could not hash {}: {}", file_path.display(), message);
      return LocalThumbnail::without_thumbnail(file_path, LocalThumbnailStatus::Failed, None);
    }
  };

  let thumbnails_dir = match app_data_root.cache_dir().get_or_create_thumbnails_dir() {
    Ok(dir) => dir,
    Err(err) => {
      warn!("Could not create thumbnails cache dir: {}", err);
      return LocalThumbnail::without_thumbnail(file_path, LocalThumbnailStatus::Failed, Some(file_hash));
    }
  };
  let thumbnail_jpg = thumbnails_dir.join(format!("{file_hash}.jpg"));
  let animated_webp = thumbnails_dir.join(format!("{file_hash}.webp"));

  let extension = file_extension(file_path);
  let kind = if IMAGE_EXTENSIONS.contains(&extension.as_str()) {
    MediaKind::Image
  } else if VIDEO_EXTENSIONS.contains(&extension.as_str()) {
    MediaKind::Video
  } else {
    record_outcome(database, file_path, ThumbnailState::Unsupported, None, None, Some(&format!("unsupported extension {extension:?}"))).await;
    return LocalThumbnail::without_thumbnail(file_path, LocalThumbnailStatus::Unsupported, Some(file_hash));
  };

  // Already in the cache (same content seen at another path, or a previous
  // run of this generator): just point the row at it. Videos count as
  // cached only with their animated preview, so an earlier partial result
  // gets completed; older generators' output is always redone.
  let made_by_current_generator = get_local_file_by_path(database.get_connection(), &file_path.to_string_lossy()).await
      .ok()
      .flatten()
      .is_some_and(|record| record.thumbnail_generator_version >= THUMBNAIL_GENERATOR_VERSION);
  let cache_complete = made_by_current_generator && thumbnail_jpg.is_file() && match kind {
    MediaKind::Image => true,
    MediaKind::Video => animated_webp.is_file(),
  };
  if cache_complete {
    let has_webp = animated_webp.is_file();
    record_outcome(database, file_path, ThumbnailState::Generated, Some(&thumbnail_jpg), has_webp.then_some(&animated_webp), None).await;
    return LocalThumbnail::ready(file_path, file_hash, &thumbnail_jpg, has_webp.then_some(animated_webp.as_path()));
  }

  let generation = {
    let _permit = GENERATION_SEMAPHORE.acquire().await.expect("semaphore never closes");
    let source = file_path.to_path_buf();
    let jpg = thumbnail_jpg.clone();
    let webp = animated_webp.clone();
    tokio::task::spawn_blocking(move || generate(kind, &source, &jpg, &webp)).await
  };

  match generation {
    Ok(Ok(wrote_animated_preview)) => {
      info!("Generated thumbnail for {} -> {}", file_path.display(), thumbnail_jpg.display());
      record_outcome(database, file_path, ThumbnailState::Generated, Some(&thumbnail_jpg), wrote_animated_preview.then_some(&animated_webp), None).await;
      LocalThumbnail::ready(file_path, file_hash, &thumbnail_jpg, wrote_animated_preview.then_some(animated_webp.as_path()))
    }
    Ok(Err(err)) if err.is_unsupported() => {
      info!("Media not thumbnailable ({}): {}", file_path.display(), err);
      record_outcome(database, file_path, ThumbnailState::Unsupported, None, None, Some(&err.to_string())).await;
      LocalThumbnail::without_thumbnail(file_path, LocalThumbnailStatus::Unsupported, Some(file_hash))
    }
    Ok(Err(err)) => {
      warn!("Thumbnail generation failed for {}: {}", file_path.display(), err);
      record_outcome(database, file_path, ThumbnailState::Failed, None, None, Some(&err.to_string())).await;
      LocalThumbnail::without_thumbnail(file_path, LocalThumbnailStatus::Failed, Some(file_hash))
    }
    Err(join_error) => {
      warn!("Thumbnail generation task panicked for {}: {}", file_path.display(), join_error);
      record_outcome(database, file_path, ThumbnailState::Failed, None, None, Some(&join_error.to_string())).await;
      LocalThumbnail::without_thumbnail(file_path, LocalThumbnailStatus::Failed, Some(file_hash))
    }
  }
}

// ── Private ──

#[derive(Copy, Clone, Debug)]
enum MediaKind {
  Image,
  Video,
}

/// Returns whether an animated preview was written alongside the static jpg.
fn generate(kind: MediaKind, source: &Path, jpg: &Path, webp: &Path) -> Result<bool, ThumbnailError> {
  match kind {
    MediaKind::Image => {
      generate_image_thumbnail(source, jpg, STATIC_THUMBNAIL_MAX_DIMENSION)?;
      Ok(false)
    }
    MediaKind::Video => {
      let outcome = generate_video_thumbnails(source, jpg, webp)?;
      Ok(outcome.wrote_animated_preview)
    }
  }
}

async fn record_outcome(
  database: &LocalFilesDatabase,
  file_path: &Path,
  state: ThumbnailState,
  maybe_jpg: Option<&Path>,
  maybe_webp: Option<&Path>,
  maybe_error: Option<&str>,
) {
  let path_string = file_path.to_string_lossy();
  let jpg = maybe_jpg.map(|path| path.to_string_lossy().into_owned());
  let webp = maybe_webp.map(|path| path.to_string_lossy().into_owned());
  let result = set_thumbnail_result(SetThumbnailResultArgs {
    db: database.get_connection(),
    file_path: &path_string,
    state,
    generator_version: THUMBNAIL_GENERATOR_VERSION,
    maybe_thumbnail_jpg_path: jpg.as_deref(),
    maybe_animated_preview_webp_path: webp.as_deref(),
    maybe_error,
  }).await;
  match result {
    Ok(true) => {}
    Ok(false) => warn!("No local_files row to record thumbnail state for {}", path_string),
    Err(err) => warn!("Could not record thumbnail state for {}: {}", path_string, err),
  }
}

fn per_path_lock(file_path: &Path) -> Arc<TokioMutex<()>> {
  let mut locks = PER_PATH_LOCKS.lock().expect("lock poisoned");
  locks.entry(file_path.to_path_buf()).or_default().clone()
}

#[cfg(test)]
mod tests {
  use super::*;

  // The 4-second H.264 clip the higgsfield_client live tests upload.
  const H264_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../api_clients/higgsfield_client/test_assets/clip_4s.mp4",
  );

  /// The worker's write path against a throwaway data root and index.
  #[tokio::test]
  async fn generates_records_and_then_serves_a_video_thumbnail() {
    let dir = tempfile::tempdir().unwrap();
    let app_data_root = AppDataRoot::create_existing(dir.path().join("root")).unwrap();
    let database = LocalFilesDatabase::connect(&app_data_root).await.unwrap();
    let video = dir.path().join("downloaded.mp4");
    std::fs::copy(H264_FIXTURE, &video).unwrap();

    // Nothing yet: the read path says pending without touching the file.
    let before = lookup_thumbnail(&database, &video).await;
    assert_eq!(before.status, LocalThumbnailStatus::Pending);
    assert!(before.maybe_file_hash.is_none());

    let generated = generate_thumbnail(&database, &app_data_root, &video).await;
    assert_eq!(generated.status, LocalThumbnailStatus::Ready);
    let jpg = PathBuf::from(generated.maybe_thumbnail_path.clone().unwrap());
    let webp = PathBuf::from(generated.maybe_animated_preview_path.clone().unwrap());
    assert!(jpg.is_file(), "static thumbnail written");
    assert!(webp.is_file(), "animated preview written");
    // Content-addressed under the cache dir.
    let hash = generated.maybe_file_hash.clone().unwrap();
    assert_eq!(jpg.file_name().unwrap().to_str().unwrap(), format!("{hash}.jpg"));
    assert!(jpg.starts_with(app_data_root.cache_dir().get_or_create_thumbnails_dir().unwrap()));

    // The row records the outcome, so readers never stat the cache.
    let record = get_local_file_by_path(database.get_connection(), &video.to_string_lossy()).await.unwrap().unwrap();
    assert_eq!(record.thumbnail_state(), ThumbnailState::Generated);
    assert_eq!(record.thumbnail_attempts, 1);
    assert_eq!(record.thumbnail_generator_version, THUMBNAIL_GENERATOR_VERSION);
    assert!(record.has_thumbnail && record.has_animated_preview);
    assert_eq!(record.thumbnail_jpg_path.as_deref(), Some(jpg.to_str().unwrap()));
    assert!(!needs_generation(&record));
    assert_eq!(generated_thumbnail_paths(&record), Some((jpg.to_string_lossy().into_owned(), Some(webp.to_string_lossy().into_owned()))));

    let served = lookup_thumbnail(&database, &video).await;
    assert_eq!(served.status, LocalThumbnailStatus::Ready);
    assert_eq!(served.maybe_thumbnail_path.as_deref(), Some(jpg.to_str().unwrap()));
    assert_eq!(served.maybe_animated_preview_path.as_deref(), Some(webp.to_str().unwrap()));

    // A second pass is a cache hit (no re-decode) that just re-confirms.
    let again = generate_thumbnail(&database, &app_data_root, &video).await;
    assert_eq!(again.status, LocalThumbnailStatus::Ready);

    // Cache file deleted: the read path degrades to pending and the scan
    // wants it regenerated.
    std::fs::remove_file(&jpg).unwrap();
    let record = get_local_file_by_path(database.get_connection(), &video.to_string_lossy()).await.unwrap().unwrap();
    assert!(needs_generation(&record));
    assert_eq!(lookup_thumbnail(&database, &video).await.status, LocalThumbnailStatus::Pending);
    let regenerated = generate_thumbnail(&database, &app_data_root, &video).await;
    assert_eq!(regenerated.status, LocalThumbnailStatus::Ready);
    assert!(jpg.is_file());
  }

  #[tokio::test]
  async fn unsupported_and_missing_files_are_reported_not_retried() {
    let dir = tempfile::tempdir().unwrap();
    let app_data_root = AppDataRoot::create_existing(dir.path().join("root")).unwrap();
    let database = LocalFilesDatabase::connect(&app_data_root).await.unwrap();

    let text = dir.path().join("notes.txt");
    std::fs::write(&text, b"not media").unwrap();
    let result = generate_thumbnail(&database, &app_data_root, &text).await;
    assert_eq!(result.status, LocalThumbnailStatus::Unsupported);
    let record = get_local_file_by_path(database.get_connection(), &text.to_string_lossy()).await.unwrap().unwrap();
    assert_eq!(record.thumbnail_state(), ThumbnailState::Unsupported);
    assert!(!needs_generation(&record));
    assert_eq!(lookup_thumbnail(&database, &text).await.status, LocalThumbnailStatus::Unsupported);

    let missing = dir.path().join("gone.mp4");
    assert_eq!(generate_thumbnail(&database, &app_data_root, &missing).await.status, LocalThumbnailStatus::MissingFile);
    assert_eq!(lookup_thumbnail(&database, &missing).await.status, LocalThumbnailStatus::MissingFile);
  }
}
