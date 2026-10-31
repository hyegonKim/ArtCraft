use std::time::Duration;

use anyhow::anyhow;
use sqlx::MySqlPool;

use errors::AnyhowResult;

use crate::queries::generic_download::job::list_available_generic_download_jobs::AvailableDownloadJob;

pub async fn mark_generic_download_job_done(
  pool: &MySqlPool,
  job: &AvailableDownloadJob,
  success: bool,
  maybe_entity_token: Option<&str>,
  maybe_entity_type: Option<&str>,
  job_duration: Duration,
) -> AnyhowResult<()>
{
  // NB: MySql's unsigned int (32 bits) can store integers up to 4,294,967,295.
  // Given milliseconds, this is ~49.71 days, which should be plenty for us.
  let truncated_execution_millis = job_duration.as_millis() as u32;

  let query_result = if success {
    sqlx::query!(
        r#"
UPDATE generic_download_jobs
SET
  status = "complete_success",
  on_success_downloaded_entity_token = ?,
  on_success_downloaded_entity_type = ?,
  failure_reason = NULL,
  success_execution_millis = ?,
  retry_at = NULL,
  successfully_completed_at = NOW()
WHERE id = ?
        "#,
        maybe_entity_token,
        maybe_entity_type,
        truncated_execution_millis,
        job.id.0,
    )
        .execute(pool)
        .await
  } else {
    sqlx::query!(
        r#"
UPDATE generic_download_jobs
SET
  status = "complete_failure",
  on_success_downloaded_entity_token = ?,
  on_success_downloaded_entity_type = ?,
  failure_reason = NULL,
  success_execution_millis = ?,
  retry_at = NULL
WHERE id = ?
        "#,
        maybe_entity_token,
        maybe_entity_type,
        truncated_execution_millis,
        job.id.0,
    )
        .execute(pool)
        .await
  };

  match query_result {
    Err(err) => Err(anyhow!("error with query: {:?}", err)),
    Ok(_r) => Ok(()),
  }
}
