use crate::commands::utils::response::shorthand::ResponseOrErrorMessage;
use crate::commands::utils::response::success_response_wrapper::SerializeMarker;
use crate::state::database::task_database::TaskDatabase;
use errors::AnyhowResult;
use log::{error, info};
use serde_derive::{Deserialize, Serialize};
use sqlite_database::queries::update::mark_task_as_dismissed::mark_task_as_dismissed;
use tauri::{AppHandle, State};
use sqlite_identifiers::ids::task_id::TaskId;

#[derive(Deserialize)]
pub struct MarkTaskAsDismissedRequest {
  task: TaskId,
}

#[derive(Serialize)]
pub struct MarkTaskAsDismissedResponse {
  success: bool,
}

impl SerializeMarker for MarkTaskAsDismissedResponse {}

#[tauri::command]
pub async fn mark_task_as_dismissed_command(
  request: MarkTaskAsDismissedRequest,
  _app: AppHandle,
  task_database: State<'_, TaskDatabase>,
) -> ResponseOrErrorMessage<MarkTaskAsDismissedResponse> {

  info!("mark_task_as_dismissed_command called");

  let result = handle_request(
    &request.task,
    &task_database,
  ).await;

  if let Err(err) = &result {
    error!("mark_task_as_dismissed_command failed: {:?}", err);
    return Err("mark_task_as_dismissed_command failed".into())
  }

  Ok(MarkTaskAsDismissedResponse{
    success: true,
  }.into())
}

pub async fn handle_request(
  task_id: &TaskId,
  task_database: &TaskDatabase,
) -> AnyhowResult<()> {
  let _result = mark_task_as_dismissed(task_database.get_connection(), task_id).await?;
  Ok(())
}
