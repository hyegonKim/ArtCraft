use anyhow::anyhow;
use sqlx;
use sqlx::MySql;
use sqlx::pool::PoolConnection;

use errors::AnyhowResult;
use tokens::tokens::generic_inference_jobs::InferenceJobToken;

/// Mark a single job as cancelled by the user.
pub async fn mark_generic_inference_job_cancelled_by_user(
  mysql_connection: &mut PoolConnection<MySql>,
  job_token: &InferenceJobToken
) -> AnyhowResult<()> {
  // First, we cancel any non-complete / non-terminal jobs
  // We also mark "started" jobs in the event that they're left in a bad state.
  // Any started jobs that are actually running should still run to completion, and
  // the user should still see the final result even if the job system UI doesn't
  // indicate it.
  let query_result = sqlx::query!(
        r#"
UPDATE generic_inference_jobs
SET
  status = 'cancelled_by_user',
  retry_at = NULL
WHERE token = ?
AND status IN (
  'pending',
  'started',
  'attempt_failed'
 )
        "#,
        job_token,
    )
      .execute(&mut **mysql_connection)
      .await;

  if let Err(err) = query_result {
    return Err(anyhow!("error with terminate job query (1): {:?}", err));
  }

  // Next, we dismiss any jobs from UI view.
  // We don't need a where clause because this should include *all* states given
  // the previous query.
  let query_result = sqlx::query!(
        r#"
UPDATE generic_inference_jobs
SET
  is_dismissed_by_user = TRUE
WHERE token = ?
        "#,
        job_token,
    )
      .execute(&mut **mysql_connection)
      .await;

  if let Err(err) = query_result {
    return Err(anyhow!("error with terminate job query (2): {:?}", err));
  }

  Ok(())
}
