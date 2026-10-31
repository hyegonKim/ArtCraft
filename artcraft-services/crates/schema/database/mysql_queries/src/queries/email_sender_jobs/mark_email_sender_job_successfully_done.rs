use anyhow::anyhow;
use sqlx::MySqlPool;

use errors::AnyhowResult;

use crate::queries::email_sender_jobs::list_available_email_sender_jobs::AvailableEmailSenderJob;

pub async fn mark_email_sender_job_successfully_done(
  pool: &MySqlPool,
  job: &AvailableEmailSenderJob,
) -> AnyhowResult<()>
{
  let query_result = sqlx::query!(
        r#"
UPDATE email_sender_jobs
SET
  status = "complete_success",
  internal_debugging_failure_reason = NULL,
  retry_at = NULL,
  successfully_completed_at = NOW()
WHERE id = ?
        "#,
        job.id.0,
    )
      .execute(pool)
      .await;

  match query_result {
    Err(err) => Err(anyhow!("error with query: {:?}", err)),
    Ok(_r) => Ok(()),
  }
}
