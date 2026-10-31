use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::state::database::task_database::TaskDatabase;
use errors::AnyhowResult;
use tauri::{AppHandle, Manager};

pub async fn bootstrap_task_database(app: &AppHandle, root: &AppDataRoot) -> AnyhowResult<TaskDatabase> {
  let task_database = TaskDatabase::connect(root).await?;
  app.manage(task_database.clone());
  Ok(task_database)
}
