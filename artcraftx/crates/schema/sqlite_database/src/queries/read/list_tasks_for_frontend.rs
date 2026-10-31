use crate::connection::TaskDbConnection;
use crate::error::SqliteTasksError;
use chrono::{DateTime, Utc};
use core_types::enums::generation_source::GenerationSource;
use sqlite_identifiers::enums::task_failure_type::TaskFailureType;
use sqlite_identifiers::enums::task_media_file_class::TaskMediaFileClass;
use sqlite_identifiers::enums::task_model_type::TaskModelType;
use sqlite_identifiers::enums::task_status::TaskStatus;
use sqlite_identifiers::enums::task_type::TaskType;
use sqlite_identifiers::enums::tauri_command_caller::TauriCommandCaller;
use sqlite_identifiers::ids::batch_generation_token::BatchGenerationToken;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;
use sqlite_identifiers::ids::task_id::TaskId;

pub struct TaskList {
  pub tasks: Vec<TaskItem>,
}

pub struct TaskItem {
  pub id: TaskId,
  pub status: TaskStatus,
  pub task_type: TaskType,
  pub model_type: Option<TaskModelType>,
  pub provider: Option<GenerationSource>,
  pub provider_job_id: Option<String>,
  pub is_batch_generation: bool,
  pub frontend_caller: Option<TauriCommandCaller>,
  pub frontend_subscriber_id: Option<String>,
  pub frontend_subscriber_payload: Option<String>,
  pub on_complete_primary_media_file_token: Option<MediaFileToken>,
  pub on_complete_primary_media_file_class: Option<TaskMediaFileClass>,
  pub on_complete_batch_token: Option<BatchGenerationToken>,
  pub on_complete_primary_media_file_cdn_url: Option<String>,
  pub on_complete_primary_media_file_thumbnail_url_template: Option<String>,
  /// Where the results were downloaded, if they were (absolute paths).
  pub on_complete_directory_location: Option<String>,
  pub on_complete_first_file_location: Option<String>,
  pub on_failure_type: Option<TaskFailureType>,
  pub on_failure_message: Option<String>,
  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
  pub completed_at: Option<DateTime<Utc>>,
}

pub async fn list_tasks_for_frontend(
  db: & TaskDbConnection
) -> Result<TaskList, SqliteTasksError> {
  let query = sqlx::query_as!(
    TaskItemRaw,
    r#"
    SELECT
      id,
      task_status,
      task_type,
      model_type,
      provider,
      provider_job_id,
      is_batch_generation,
      frontend_caller,
      frontend_subscriber_id,
      frontend_subscriber_payload,
      on_complete_primary_media_file_token,
      on_complete_primary_media_file_class,
      on_complete_batch_token,
      on_complete_primary_media_file_cdn_url,
      on_complete_primary_media_file_thumbnail_url_template,
      on_complete_directory_location,
      on_complete_first_file_location,
      on_failure_type,
      on_failure_message,
      created_at as "created_at: DateTime<Utc>",
      updated_at as "updated_at: DateTime<Utc>",
      completed_at as "completed_at: DateTime<Utc>"
    FROM tasks
    WHERE is_dismissed_by_user == 0
  "#);

  let result = query.fetch_all(db.get_pool())
      .await?;

  let mut tasks = Vec::with_capacity(result.len());

  for raw in result.into_iter() {
    tasks.push(TaskItem {
      id: TaskId::new(raw.id),
      status: TaskStatus::from_str(&raw.task_status)?,
      task_type: TaskType::from_str(&raw.task_type)?,
      model_type: raw.model_type
          .map(|model| TaskModelType::from_str(&model))
          .transpose()?,
      provider: raw.provider
          .map(|provider| GenerationSource::from_str(&provider))
          .transpose()?,
      provider_job_id: raw.provider_job_id,
      is_batch_generation: raw.is_batch_generation != 0,
      frontend_caller: raw.frontend_caller
          .map(|caller| TauriCommandCaller::from_str(&caller))
          .transpose()?,
      frontend_subscriber_id: raw.frontend_subscriber_id,
      frontend_subscriber_payload: raw.frontend_subscriber_payload,
      on_complete_primary_media_file_token: raw.on_complete_primary_media_file_token.map(|t| MediaFileToken::new_from_str(&t)),
      on_complete_primary_media_file_class: raw.on_complete_primary_media_file_class
          .map(|c| TaskMediaFileClass::from_str(&c))
          .transpose()?,
      on_complete_batch_token: raw.on_complete_batch_token.map(|t| BatchGenerationToken::new_from_str(&t)),
      on_complete_primary_media_file_cdn_url: raw.on_complete_primary_media_file_cdn_url,
      on_complete_primary_media_file_thumbnail_url_template: raw.on_complete_primary_media_file_thumbnail_url_template,
      on_complete_directory_location: raw.on_complete_directory_location,
      on_complete_first_file_location: raw.on_complete_first_file_location,
      on_failure_type: raw.on_failure_type
          .map(|t| TaskFailureType::from_str(&t))
          .transpose()?,
      on_failure_message: raw.on_failure_message,
      created_at: raw.created_at,
      updated_at: raw.updated_at,
      completed_at: raw.completed_at,
    })
  }

  Ok(TaskList {
    tasks,
  })
}

struct TaskItemRaw {
  id: String,
  task_status: String,
  task_type: String,
  model_type: Option<String>,
  provider: Option<String>,
  provider_job_id: Option<String>,
  is_batch_generation: i64,
  frontend_caller: Option<String>,
  frontend_subscriber_id: Option<String>,
  frontend_subscriber_payload: Option<String>,
  on_complete_primary_media_file_token: Option<String>,
  on_complete_primary_media_file_class: Option<String>,
  on_complete_batch_token: Option<String>,
  on_complete_primary_media_file_cdn_url: Option<String>,
  on_complete_primary_media_file_thumbnail_url_template: Option<String>,
  on_complete_directory_location: Option<String>,
  on_complete_first_file_location: Option<String>,
  on_failure_type: Option<String>,
  on_failure_message: Option<String>,
  created_at: DateTime<Utc>,
  updated_at: DateTime<Utc>,
  completed_at: Option<DateTime<Utc>>,
}
