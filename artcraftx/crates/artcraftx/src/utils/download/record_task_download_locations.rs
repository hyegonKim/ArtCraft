use crate::state::database::task_database::TaskDatabase;
use log::{error, info};
use sqlite_database::queries::update::update_task_download_locations::{
  update_task_download_locations, UpdateTaskDownloadLocationsArgs,
};
use sqlite_identifiers::ids::task_id::TaskId;
use std::path::Path;

/// Record where a completed task's files landed: the containing directory and
/// the first (or only) file. No-op when nothing was downloaded. Fails open —
/// a bookkeeping failure must never undo a completed generation.
pub async fn record_task_download_locations(task_database: &TaskDatabase, task_id: &TaskId, downloaded: &[impl AsRef<Path>]) {
  let Some(first_file) = downloaded.first().map(|p| p.as_ref()) else {
    return;
  };
  let Some(directory) = first_file.parent() else {
    error!("Downloaded file {:?} for task {} has no parent directory", first_file, task_id.as_str());
    return;
  };

  let result = update_task_download_locations(UpdateTaskDownloadLocationsArgs {
    db: task_database.get_connection(),
    task_id,
    directory_location: directory,
    first_file_location: first_file,
  }).await;

  match result {
    Ok(true) => info!("Recorded download location for task {}: {:?}", task_id.as_str(), first_file),
    Ok(false) => error!("Task {} not found while recording its download location", task_id.as_str()),
    Err(err) => error!("Could not record download location for task {}: {:?}", task_id.as_str(), err),
  }
}
