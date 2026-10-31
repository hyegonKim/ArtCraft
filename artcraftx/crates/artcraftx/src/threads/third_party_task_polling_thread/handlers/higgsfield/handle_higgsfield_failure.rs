use crate::events::basic_sendable_event_trait::BasicSendableEvent;
use crate::events::generation_events::common::{GenerationAction, GenerationServiceProvider};
use crate::events::generation_events::generation_failed_event::GenerationFailedEvent;
use crate::state::database::task_database::TaskDatabase;
use crate::threads::third_party_task_polling_thread::handlers::higgsfield::higgsfield_failure_reason::HiggsfieldFailureReport;
use log::{error, info};
use sqlite_database::queries::task::Task;
use sqlite_database::queries::update::update_task_status_with_rich_failure::{
  update_task_status_with_rich_failure, UpdateTaskWithRichFailureArgs,
};
use sqlite_identifiers::enums::task_status::TaskStatus;
use sqlite_identifiers::enums::task_type::TaskType;
use tauri::AppHandle;

/// Mark the task failed, persisting the failure type + friendly message to
/// the tasks database (so the task queue can render it later) and notifying
/// the frontend. The technical record goes to the logs.
pub async fn handle_higgsfield_failure(
  app_handle: &AppHandle,
  task_database: &TaskDatabase,
  task: &Task,
  report: &HiggsfieldFailureReport,
) {
  info!(
    "[HiggsfieldPolling] Marking task {} as failed ({}): {} | details: {}",
    task.id.as_str(), report.failure_type, report.user_message, report.log_details,
  );

  let update_result = update_task_status_with_rich_failure(UpdateTaskWithRichFailureArgs {
    db: task_database.get_connection(),
    task_id: &task.id,
    status: TaskStatus::CompleteFailure,
    maybe_failure_type: Some(report.failure_type),
    maybe_failure_message: Some(&report.user_message),
  }).await;

  if let Err(err) = update_result {
    error!("[HiggsfieldPolling] Failed to update task status for {}: {:?}", task.id.as_str(), err);
  }

  let event = GenerationFailedEvent {
    action: task_type_to_generation_action(task.task_type),
    service: GenerationServiceProvider::Higgsfield,
    model: None,
    reason: Some(report.user_message.clone()),
  };

  event.send_infallible(app_handle);
}

fn task_type_to_generation_action(task_type: TaskType) -> GenerationAction {
  match task_type {
    TaskType::ImageGeneration => GenerationAction::GenerateImage,
    TaskType::VideoGeneration => GenerationAction::GenerateVideo,
    TaskType::AudioGeneration => GenerationAction::GenerateAudio,
    TaskType::MeshGeneration => GenerationAction::ImageTo3d,
    TaskType::SplatGeneration => GenerationAction::GenerateGaussian,
  }
}
