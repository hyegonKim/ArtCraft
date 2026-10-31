use anyhow::anyhow;
use sqlx;
use sqlx::MySqlPool;

use errors::AnyhowResult;

use crate::queries::generic_download::job::list_available_generic_download_jobs::AvailableDownloadJob;

/// Mark a single download job failure. The job may be re-run.
pub async fn mark_generic_download_job_failure(
  pool: &MySqlPool,
  job: &AvailableDownloadJob,
  failure_reason: &str,
  max_attempts: i32
) -> AnyhowResult<()> {

  // statuses: "attempt_failed", "complete_failure", "dead"
  let mut next_status = "attempt_failed";

  if job.attempt_count >= max_attempts {
    // NB: Job attempt count is incremented at start
    next_status = "dead";
  }

  let query_result = sqlx::query!(
        r#"
UPDATE generic_download_jobs
SET
  status = ?,
  failure_reason = ?,
  retry_at = NOW() + interval 2 minute
WHERE id = ?
        "#,
        next_status,
        failure_reason.to_string(),
        job.id.0,
    )
      .execute(pool)
      .await;

  match query_result {
    Err(err) => {
      Err(anyhow!("error with query: {:?}", err))
    },
    Ok(_r) => Ok(()),
  }
}
