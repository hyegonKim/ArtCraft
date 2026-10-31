use anyhow::anyhow;
use sqlx::MySqlPool;

use errors::AnyhowResult;

use crate::queries::email_sender_jobs::list_available_email_sender_jobs::AvailableEmailSenderJob;

pub async fn mark_email_sender_job_completely_failed(
  pool: &MySqlPool,
  job: &AvailableEmailSenderJob,
  maybe_internal_debugging_failure_reason: Option<&str>,
) -> AnyhowResult<()>
{
  // Max length of column is 512
  let maybe_internal_debugging_failure_reason
      = maybe_internal_debugging_failure_reason.map(|reason| {
    let mut reason = reason.trim().to_string();
    reason.truncate(512); // Max length of column is 512
    reason
  });

  let query_result = sqlx::query!(
        r#"
UPDATE email_sender_jobs
SET
  status = "complete_failure",
  internal_debugging_failure_reason = ?,
  retry_at = NULL
WHERE id = ?
        "#,
        maybe_internal_debugging_failure_reason.as_deref(),
        job.id.0,
    )
      .execute(pool)
      .await;

  match query_result {
    Err(err) => Err(anyhow!("error with query: {:?}", err)),
    Ok(_r) => Ok(()),
  }
}
