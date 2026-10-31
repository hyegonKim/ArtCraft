//! Background thumbnail generation for downloaded results.
//!
//! Work arrives two ways:
//!
//! 1. Downloaders push the files they just wrote onto the
//!    [`ThumbnailWorkQueue`], which wakes this thread immediately.
//! 2. Every [`SCAN_INTERVAL`] the thread asks both databases what's owed:
//!    `local_files` rows still pending (retryable, or made by an older
//!    generator), and the most recently completed tasks' downloads that have
//!    no current thumbnail yet — so history from before this thread existed,
//!    and anything a missed wake-up skipped, still gets done.
//!
//! Each finished thumbnail is announced with `local_thumbnail_ready_event`
//! so the task queue can show it without re-polling.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use log::{debug, error, info, warn};
use local_files_database::queries::get_local_files_by_paths::get_local_files_by_paths;
use local_files_database::queries::list_local_files_needing_thumbnails::list_local_files_needing_thumbnails;
use sqlite_database::queries::read::list_completed_task_download_files::list_completed_task_download_files;
use tauri::AppHandle;
use thumbnails::THUMBNAIL_GENERATOR_VERSION;

use crate::events::basic_sendable_event_trait::BasicSendableEvent;
use crate::events::functional_events::local_thumbnail_ready_event::LocalThumbnailReadyEvent;
use crate::services::local_files::thumbnail_service::{generate_thumbnail, needs_generation, LocalThumbnailStatus, MAX_THUMBNAIL_ATTEMPTS};
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::state::database::local_files_database::LocalFilesDatabase;
use crate::state::database::task_database::TaskDatabase;
use crate::state::thumbnails::thumbnail_work_queue::ThumbnailWorkQueue;

/// How often the databases are scanned for owed thumbnails when nobody
/// wakes the thread.
const SCAN_INTERVAL: Duration = Duration::from_secs(60);

/// Per scan, how many `local_files` rows to pick up.
const MAX_LOCAL_FILE_ROWS_PER_SCAN: i64 = 50;

/// Per scan, how far back through completed tasks to look.
const RECENT_COMPLETED_TASKS_TO_SCAN: i64 = 200;

pub async fn thumbnail_worker_thread(
  app_handle: AppHandle,
  app_data_root: AppDataRoot,
  task_database: TaskDatabase,
  local_files_database: LocalFilesDatabase,
  queue: ThumbnailWorkQueue,
) -> ! {
  info!("[ThumbnailWorker] Started");
  // Scan right away so history is covered as soon as the app is up.
  let mut next_scan = Instant::now();

  loop {
    let queued = queue.drain();
    if !queued.is_empty() {
      info!("[ThumbnailWorker] Woken with {} queued file(s)", queued.len());
      process_all(&app_handle, &app_data_root, &local_files_database, queued).await;
    }

    if Instant::now() >= next_scan {
      match scan_for_work(&task_database, &local_files_database).await {
        Ok(paths) => {
          if !paths.is_empty() {
            info!("[ThumbnailWorker] Scan found {} file(s) owed a thumbnail", paths.len());
          }
          process_all(&app_handle, &app_data_root, &local_files_database, paths).await;
        }
        Err(err) => error!("[ThumbnailWorker] Scan failed: {}", err),
      }
      next_scan = Instant::now() + SCAN_INTERVAL;
    }

    let until_next_scan = next_scan.saturating_duration_since(Instant::now());
    tokio::select! {
      _ = queue.notified() => {},
      _ = tokio::time::sleep(until_next_scan) => {},
    }
  }
}

async fn process_all(
  app_handle: &AppHandle,
  app_data_root: &AppDataRoot,
  local_files_database: &LocalFilesDatabase,
  paths: Vec<PathBuf>,
) {
  for path in paths {
    let thumbnail = generate_thumbnail(local_files_database, app_data_root, &path).await;
    match thumbnail.status {
      LocalThumbnailStatus::Ready => {
        let Some(thumbnail_path) = thumbnail.maybe_thumbnail_path else {
          warn!("[ThumbnailWorker] Ready without a thumbnail path for {}", path.display());
          continue;
        };
        LocalThumbnailReadyEvent {
          file_path: thumbnail.file_path,
          thumbnail_path,
          maybe_animated_preview_path: thumbnail.maybe_animated_preview_path,
        }.send_infallible(app_handle);
      }
      LocalThumbnailStatus::MissingFile => debug!("[ThumbnailWorker] File is gone, skipping: {}", path.display()),
      // Already logged (and recorded on the row) by the service.
      LocalThumbnailStatus::Pending | LocalThumbnailStatus::Unsupported | LocalThumbnailStatus::Failed => {}
    }
  }
}

/// Everything the databases say is owed a thumbnail, deduplicated, with the
/// newest downloads first.
async fn scan_for_work(
  task_database: &TaskDatabase,
  local_files_database: &LocalFilesDatabase,
) -> anyhow::Result<Vec<PathBuf>> {
  let mut seen: HashSet<String> = HashSet::new();
  let mut work: Vec<PathBuf> = Vec::new();

  // Recently completed tasks whose download has no (surviving) thumbnail.
  let recent_downloads = list_completed_task_download_files(task_database.get_connection(), RECENT_COMPLETED_TASKS_TO_SCAN).await?;
  let known = get_local_files_by_paths(local_files_database.get_connection(), &recent_downloads).await?;
  for path in recent_downloads {
    let owed = match known.get(&path) {
      Some(record) => needs_generation(record),
      None => true,
    };
    if owed && seen.insert(path.clone()) {
      work.push(PathBuf::from(path));
    }
  }

  // Rows the index already knows are unfinished (files seen by other means).
  let pending_rows = list_local_files_needing_thumbnails(local_files_database.get_connection(), MAX_THUMBNAIL_ATTEMPTS, THUMBNAIL_GENERATOR_VERSION, MAX_LOCAL_FILE_ROWS_PER_SCAN).await?;
  for record in pending_rows {
    if seen.insert(record.file_path.clone()) {
      work.push(PathBuf::from(record.file_path));
    }
  }

  Ok(work)
}
