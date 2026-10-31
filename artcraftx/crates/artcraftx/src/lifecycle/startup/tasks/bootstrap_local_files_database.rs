use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::state::database::local_files_database::LocalFilesDatabase;
use errors::AnyhowResult;
use tauri::{AppHandle, Manager};

pub async fn bootstrap_local_files_database(app: &AppHandle, root: &AppDataRoot) -> AnyhowResult<LocalFilesDatabase> {
  let local_files_database = LocalFilesDatabase::connect(root).await?;
  app.manage(local_files_database.clone());
  Ok(local_files_database)
}
