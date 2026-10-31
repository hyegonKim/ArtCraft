use crate::connection::TaskDbConnection;
use crate::error::SqliteTasksError;
use crate::queries::task::Task;
use core_types::enums::generation_source::GenerationSource;
use sqlite_identifiers::enums::task_model_type::TaskModelType;
use sqlite_identifiers::enums::task_status::TaskStatus;
use sqlite_identifiers::enums::task_type::TaskType;
use sqlite_identifiers::enums::tauri_command_caller::TauriCommandCaller;
use sqlx::Error;
use sqlite_identifiers::ids::task_id::TaskId;

pub struct GetTaskByProviderAndProviderJobIdArgs<'a> {
  pub db: &'a TaskDbConnection,
  pub provider: GenerationSource,
  pub provider_job_id: &'a str,
}


pub async fn get_task_by_provider_and_provider_job_id(
  args: GetTaskByProviderAndProviderJobIdArgs<'_>,
) -> Result<Option<Task>, SqliteTasksError> {

  // TODO(bt,2025-07-12): Fix this. The sqlx mysql queries never required temporaries
  let temp_provider = args.provider.to_str();
  let temp_provider_job_id = args.provider_job_id;

  let query = sqlx::query!(r#"
    SELECT
      id,
      task_status,
      task_type,
      model_type,
      provider,
      provider_job_id,
      is_batch_generation,
      frontend_caller,
      queue_status_url,
      queue_response_url,
      prompt_token,
      frontend_subscriber_id,
      frontend_subscriber_payload
    FROM tasks
    WHERE
      provider = ?
      AND provider_job_id = ?
  "#,
      temp_provider,
      temp_provider_job_id,
  );

  // info!("Query: {:?}", query.sql());

  let result = query.fetch_one(args.db.get_pool()).await;

  let record = match result {
    Ok(record) => record,
    Err(err) => return match err {
      Error::RowNotFound => Ok(None),
      _ => Err(err.into()),
    },
  };

  // NB: Not sure why query can't figure out this isn't nullable
  let provider = record.provider.as_deref()
      .unwrap_or_else(|| args.provider.to_str());

  Ok(Some(Task {
    id: TaskId::new_from_str(&record.id),
    status: TaskStatus::from_str(&record.task_status)?,
    task_type: TaskType::from_str(&record.task_type)?,
    model_type: record.model_type
        .map(|model| TaskModelType::from_str(&model))
        .transpose()?,
    provider: GenerationSource::from_str(provider)?,
    provider_job_id: record.provider_job_id,
    is_batch_generation: record.is_batch_generation != 0,
    queue_status_url: record.queue_status_url,
    queue_response_url: record.queue_response_url,
    prompt_token: record.prompt_token,
    frontend_caller: record.frontend_caller
        .map(|caller| TauriCommandCaller::from_str(&caller))
        .transpose()?,
    frontend_subscriber_id: record.frontend_subscriber_id,
    frontend_subscriber_payload: record.frontend_subscriber_payload,
  }))
}
