use crate::commands::utils::response::shorthand::ResponseOrErrorMessage;
use crate::commands::utils::response::success_response_wrapper::SerializeMarker;
use crate::state::database::task_database::TaskDatabase;
use errors::AnyhowResult;
use log::{error, info};
use serde_derive::Serialize;
use sqlite_database::queries::update::dismiss_all_tasks::dismiss_all_tasks;
use sqlite_database::queries::update::nuke_all_tasks::nuke_all_tasks;
use tauri::State;

#[derive(Serialize)]
pub struct TasksNukeAllResponse {
  success: bool,
}

impl SerializeMarker for TasksNukeAllResponse {}

#[tauri::command]
pub async fn tasks_nuke_all_command(
  task_database: State<'_, TaskDatabase>,
) -> ResponseOrErrorMessage<TasksNukeAllResponse> {

  info!("tasks_nuke_all_command called");

  let result = handle_request(
    &task_database,
  ).await;

  if let Err(err) = &result {
    error!("tasks_nuke_all_command failed: {:?}", err);
    return Err("tasks_nuke_all_command failed".into())
  }

  Ok(TasksNukeAllResponse{
    success: true,
  }.into())
}

pub async fn handle_request(
  task_database: &TaskDatabase,
) -> AnyhowResult<()> {
  let _result = nuke_all_tasks(task_database.get_connection()).await?;
  let _result = dismiss_all_tasks(task_database.get_connection()).await?;
  Ok(())
}
