use core_types::enums::generation_source::GenerationSource;
use sqlite_identifiers::enums::task_model_type::TaskModelType;
use sqlite_identifiers::enums::task_status::TaskStatus;
use sqlite_identifiers::enums::task_type::TaskType;
use sqlite_identifiers::enums::tauri_command_caller::TauriCommandCaller;
use sqlite_identifiers::ids::task_id::TaskId;

#[derive(Debug, Clone)]
pub struct Task {
  pub id: TaskId,
  pub status: TaskStatus,
  pub task_type: TaskType,
  pub model_type: Option<TaskModelType>,
  pub provider: GenerationSource,
  pub provider_job_id: Option<String>,
  /// The job produces more than one file. Written on enqueue.
  pub is_batch_generation: bool,
  pub queue_status_url: Option<String>,
  pub queue_response_url: Option<String>,
  pub prompt_token: Option<String>,
  pub frontend_caller: Option<TauriCommandCaller>,
  pub frontend_subscriber_id: Option<String>,
  pub frontend_subscriber_payload: Option<String>,
}

#[derive(Debug)]
#[derive(sqlx::FromRow)]
pub (crate) struct RawTask {
  pub (crate) id: String,
  pub (crate) task_status: String,
  pub (crate) task_type: String,
  pub (crate) model_type: Option<String>,
  pub (crate) provider: String,
  pub (crate) provider_job_id: Option<String>,
  pub (crate) is_batch_generation: bool,
  pub (crate) queue_status_url: Option<String>,
  pub (crate) queue_response_url: Option<String>,
  pub (crate) prompt_token: Option<String>,
  pub (crate) frontend_caller: Option<String>,
  pub (crate) frontend_subscriber_id: Option<String>,
  pub (crate) frontend_subscriber_payload: Option<String>,
}
