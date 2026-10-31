use anyhow::anyhow;
use log::info;
use sqlx;
use sqlx::pool::PoolConnection;
use sqlx::{MySql, MySqlPool};

use errors::AnyhowResult;

/// Kill FakeYou jobs that are "stale".
///
/// - This leaves Fal jobs alone, as they do not use the job runner.
/// - This kills jobs that are over a threshold (e.g. an hour) overdue.
pub async fn reap_stale_fakeyou_jobs(mysql_pool: &MySqlPool) -> AnyhowResult<()>
{
  let mut connection = mysql_pool.acquire().await?;
  reap_stale_fakeyou_jobs_using_connection(&mut connection).await
}

pub async fn reap_stale_fakeyou_jobs_using_connection(
  mysql_connection: &mut PoolConnection<MySql>,
) -> AnyhowResult<()> {
  let query_result = sqlx::query!(
        r#"
UPDATE generic_inference_jobs
SET
  status = 'cancelled_by_system',
  retry_at = NULL,
  internal_debugging_failure_reason = 'reap_stale_fakeyou_jobs'
WHERE
  job_type != 'fal_queue'
  AND status IN (
    'pending',
    'started',
    'attempt_failed'
   )
   AND created_at > NOW() - INTERVAL 30 MINUTE
LIMIT 5000
        "#
    )
      .execute(&mut **mysql_connection)
      .await;

  match query_result {
    Ok(result) => {
      info!("Reaped stale FakeYou jobs: {} rows affected.", result.rows_affected());
      Ok(())
    },
    Err(err) => {
      Err(anyhow!("error with reaping stale jobs: {:?}", err))
    }
  }
}
