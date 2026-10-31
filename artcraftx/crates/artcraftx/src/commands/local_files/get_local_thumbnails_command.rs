//! Bulk thumbnail lookup for local files.
//!
//! Takes one or more absolute file paths and returns, per path, the cached
//! thumbnail (and video animated preview) if the worker has produced it.
//! Anything still `pending` is handed to the worker thread; the frontend
//! learns of it through `local_thumbnail_ready_event`. Per-item statuses
//! mean one bad path never fails the batch, so the command is infallible.

use std::path::PathBuf;

use serde_derive::Deserialize;

use crate::commands::utils::response::shorthand::InfallibleResponse;
use crate::commands::utils::response::success_response_wrapper::SerializeMarker;
use crate::services::local_files::thumbnail_service::{lookup_thumbnail, LocalThumbnail, LocalThumbnailStatus};
use crate::state::database::local_files_database::LocalFilesDatabase;
use crate::state::thumbnails::thumbnail_work_queue::ThumbnailWorkQueue;
use log::info;
use serde_derive::Serialize;
use tauri::{AppHandle, State};

// ── Request ──

#[derive(Debug, Deserialize)]
pub struct GetLocalThumbnailsRequest {
  /// Absolute local file paths (typically completed results on disk).
  pub file_paths: Vec<String>,
}

// ── Response ──

#[derive(Debug, Serialize)]
pub struct GetLocalThumbnailsResponse {
  /// One entry per requested path, in request order.
  pub thumbnails: Vec<LocalThumbnail>,
}

impl SerializeMarker for GetLocalThumbnailsResponse {}

#[tauri::command]
pub async fn get_local_thumbnails_command(
  app: AppHandle,
  request: GetLocalThumbnailsRequest,
  local_files_database: State<'_, LocalFilesDatabase>,
) -> Result<InfallibleResponse<GetLocalThumbnailsResponse>, ()> {
  info!("get_local_thumbnails_command called for {} path(s)", request.file_paths.len());

  let file_paths: Vec<PathBuf> = request.file_paths.iter().map(PathBuf::from).collect();
  let lookups = file_paths.iter().map(|file_path| lookup_thumbnail(&local_files_database, file_path));
  let thumbnails = futures::future::join_all(lookups).await;

  let pending: Vec<&str> = thumbnails.iter()
      .filter(|thumbnail| thumbnail.status == LocalThumbnailStatus::Pending)
      .map(|thumbnail| thumbnail.file_path.as_str())
      .collect();
  ThumbnailWorkQueue::enqueue_for_app(&app, &pending);

  Ok(GetLocalThumbnailsResponse { thumbnails }.into())
}
