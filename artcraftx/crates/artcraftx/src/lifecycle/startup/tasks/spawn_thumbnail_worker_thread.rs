use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::state::database::local_files_database::LocalFilesDatabase;
use crate::state::database::task_database::TaskDatabase;
use crate::state::thumbnails::thumbnail_work_queue::ThumbnailWorkQueue;
use crate::threads::thumbnail_worker_thread::thumbnail_worker_thread::thumbnail_worker_thread;
use errors::AnyhowResult;
use tauri::{AppHandle, Manager};

/// Start the background thumbnail worker and expose its wake-up queue as
/// managed state, so downloaders (and commands) can hand it files.
pub fn spawn_thumbnail_worker_thread(
  app: &AppHandle,
  app_data_root: &AppDataRoot,
  task_database: &TaskDatabase,
  local_files_database: &LocalFilesDatabase,
) -> AnyhowResult<()> {
  let queue = ThumbnailWorkQueue::new();
  app.manage(queue.clone());

  tauri::async_runtime::spawn(thumbnail_worker_thread(
    app.clone(),
    app_data_root.clone(),
    task_database.clone(),
    local_files_database.clone(),
    queue,
  ));

  Ok(())
}
