use anyhow::anyhow;
use sqlx::{MySql, MySqlPool};
use sqlx::mysql::MySqlArguments;
use sqlx::query::Query;

use errors::AnyhowResult;

#[derive(Copy, Clone)]
pub enum JobStatus {
  /// Kill all "pending" jobs
  AllPending,
  /// Kill all "pending" and "attempt_failed" jobs
  AllPendingAndFailed,
  /// Kill "pending" jobs with priority_level = 0.
  ZeroPriorityPending,
}

pub async fn kill_tts_inference_jobs(job_status: JobStatus, mysql_pool: &MySqlPool) -> AnyhowResult<()> {

  let query = match job_status {
    JobStatus::AllPending => kill_all_pending(),
    JobStatus::AllPendingAndFailed => kill_all_pending_and_failed(),
    JobStatus::ZeroPriorityPending => kill_all_zero_priority_pending(),
  };

  let result = query.execute(mysql_pool).await;

  match result {
    Err(err) => {
      Err(anyhow!("error with query: {:?}", err))
    },
    Ok(_r) => Ok(()),
  }
}

fn kill_all_pending() -> Query<'static, MySql, MySqlArguments> {
  sqlx::query!(
        r#"
UPDATE tts_inference_jobs
SET status='complete_failure'
WHERE
  status IN ('pending');
        "#
    )
}

fn kill_all_pending_and_failed() -> Query<'static, MySql, MySqlArguments> {
    sqlx::query!(
        r#"
UPDATE tts_inference_jobs
SET status='complete_failure'
WHERE
  status IN ('pending', 'attempt_failed');
        "#
    )
}

fn kill_all_zero_priority_pending() -> Query<'static, MySql, MySqlArguments> {
    sqlx::query!(
        r#"
UPDATE tts_inference_jobs
SET status='complete_failure'
WHERE
  status IN ('pending')
  AND priority_level = 0;
        "#
    )
}
