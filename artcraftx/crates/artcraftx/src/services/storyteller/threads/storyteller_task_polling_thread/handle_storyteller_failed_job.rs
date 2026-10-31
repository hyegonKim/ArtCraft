use crate::events::basic_sendable_event_trait::BasicSendableEvent;
use crate::events::generation_events::generation_failed_event::GenerationFailedEvent;
use crate::state::database::task_database::TaskDatabase;
use crate::utils::enum_conversion::generation_source::to_generation_service_provider;
use crate::utils::enum_conversion::task_failure_type::task_failure_type_from_frontend_failure_category_for_api;
use crate::utils::enum_conversion::task_type::to_generation_action;
use artcraft_client::api_defs::jobs::get_job_status::JobStatusPayload;
use errors::AnyhowResult;
use log::info;
use sqlite_database::queries::task::Task;
use sqlite_database::queries::update::update_task_status_with_rich_failure::{update_task_status_with_rich_failure, UpdateTaskWithRichFailureArgs};
use sqlite_identifiers::enums::task_status::TaskStatus;
use tauri::AppHandle;

pub async fn handle_failed_job(
  app_handle: &AppHandle,
  job: &JobStatusPayload,
  task: &Task,
  task_database: &TaskDatabase,
) -> AnyhowResult<()> {
  info!("Marking storyteller job as failed: {:?}", task.id);

  let maybe_failure_type = job.status
      .maybe_failure_category
      .as_ref()
      .map(task_failure_type_from_frontend_failure_category_for_api);

  update_task_status_with_rich_failure(UpdateTaskWithRichFailureArgs {
    db: task_database.get_connection(),
    task_id: &task.id,
    status: TaskStatus::CompleteFailure,
    maybe_failure_type,
    // NB: The job status endpoints don't carry a user-facing failure message.
    maybe_failure_message: None,
  }).await?;

  let service = to_generation_service_provider(task.provider);
  let action = to_generation_action(task.task_type);

  let event = GenerationFailedEvent {
    action,
    service,
    model: None,
    reason: None,
  };

  event.send_infallible(app_handle);

  Ok(())
}
