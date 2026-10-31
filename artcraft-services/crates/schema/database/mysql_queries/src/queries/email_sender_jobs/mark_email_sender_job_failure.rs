use anyhow::anyhow;
use sqlx;
use sqlx::MySqlPool;

use errors::AnyhowResult;

use crate::queries::email_sender_jobs::list_available_email_sender_jobs::AvailableEmailSenderJob;

/// Mark a single inference job failure. The job may be re-run.
pub async fn mark_email_sender_job_failure(
  pool: &MySqlPool,
  job: &AvailableEmailSenderJob,
  internal_debugging_failure_reason: &str,
  max_attempts: u16
) -> AnyhowResult<()> {

  // statuses: "attempt_failed", "complete_failure", "dead"
  let mut next_status = "attempt_failed";

  // Max length of column is 512
  let mut internal_debugging_failure_reason = internal_debugging_failure_reason.trim().to_string();
  internal_debugging_failure_reason.truncate(512);

  if job.attempt_count >= max_attempts {
    // NB: Job attempt count is incremented at start
    next_status = "dead";
  }

  let query_result = sqlx::query!(
        r#"
UPDATE email_sender_jobs
SET
  status = ?,
  internal_debugging_failure_reason = ?,
  retry_at = NOW() + interval 2 minute
WHERE id = ?
        "#,
        next_status,
        &internal_debugging_failure_reason,
        job.id.0,
    )
      .execute(pool)
      .await;

  match query_result {
    Err(err) => Err(anyhow!("error with query: {:?}", err)),
    Ok(_r) => Ok(()),
  }
}
