use anyhow::anyhow;
use sqlx;
use sqlx::pool::PoolConnection;
use sqlx::{MySql, MySqlPool};

use errors::AnyhowResult;

/// Kill a handful of jobs when called. 
/// This should only be used in development to keep the job queue empty.
pub async fn kill_jobs_in_development(mysql_pool: &MySqlPool) -> AnyhowResult<()>
{
  let mut connection = mysql_pool.acquire().await?;
  kill_jobs_in_development_using_connection(&mut connection).await
}

/// Kill a handful of jobs when called. 
/// This should only be used in development to keep the job queue empty.
pub async fn kill_jobs_in_development_using_connection(
  mysql_connection: &mut PoolConnection<MySql>,
) -> AnyhowResult<()> {
  // We cancel any non-complete / non-terminal jobs
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
WHERE status IN (
  'pending',
  'started',
  'attempt_failed'
 )
LIMIT 100
        "#
    )
      .execute(&mut **mysql_connection)
      .await;

  if let Err(err) = query_result {
    return Err(anyhow!("error with terminate job query (1): {:?}", err));
  }

  Ok(())
}
