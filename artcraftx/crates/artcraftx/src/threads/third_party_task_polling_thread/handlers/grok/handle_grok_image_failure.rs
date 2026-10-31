use crate::events::basic_sendable_event_trait::BasicSendableEvent;
use crate::events::generation_events::common::{GenerationAction, GenerationServiceProvider};
use crate::events::generation_events::generation_failed_event::GenerationFailedEvent;
use crate::state::database::task_database::TaskDatabase;
use log::{error, info};
use sqlite_database::queries::task::Task;
use sqlite_database::queries::update::update_task_status_with_rich_failure::{
  update_task_status_with_rich_failure, UpdateTaskWithRichFailureArgs,
};
use sqlite_identifiers::enums::task_failure_type::TaskFailureType;
use sqlite_identifiers::enums::task_status::TaskStatus;
use tauri::AppHandle;

/// Mark a Grok image task failed and tell the frontend why.
pub async fn handle_grok_image_failure(
  app_handle: &AppHandle,
  task_database: &TaskDatabase,
  task: &Task,
  reason: &str,
) {
  info!("[GrokPolling] Marking task {} as failed: {}", task.id.as_str(), reason);

  let update_result = update_task_status_with_rich_failure(UpdateTaskWithRichFailureArgs {
    db: task_database.get_connection(),
    task_id: &task.id,
    status: TaskStatus::CompleteFailure,
    maybe_failure_type: Some(TaskFailureType::GenerationFailed),
    maybe_failure_message: Some(reason),
  }).await;

  if let Err(err) = update_result {
    error!("[GrokPolling] Failed to update task status for {}: {:?}", task.id.as_str(), err);
  }

  GenerationFailedEvent {
    action: GenerationAction::GenerateImage,
    service: GenerationServiceProvider::Grok,
    model: None,
    reason: Some(reason.to_string()),
  }.send_infallible(app_handle);
}
