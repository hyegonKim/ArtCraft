use anyhow::anyhow;
use sqlx;
use sqlx::MySqlPool;

use errors::AnyhowResult;

use crate::common_inputs::container_environment_arg::ContainerEnvironmentArg;
use crate::queries::generic_inference::job::_keys::GenericInferenceJobId;

// TODO(2022-02-09): It's unclear how stuck "pending" jobs don't become perma-dead
//  I think this logic is flawed and the job system needs to pick up stuck "pending" jobs.

pub struct GenericInferenceJobLockRecord {
  id: i64,
  status: String,
  attempt_count: u16,
}

pub async fn mark_generic_inference_job_pending_and_grab_lock(
  pool: &MySqlPool,
  job_id: GenericInferenceJobId,
  container_environment: &ContainerEnvironmentArg,
) -> AnyhowResult<bool> {

  // NB: We use transactions and "SELECT ... FOR UPDATE" to simulate mutexes.
  let mut transaction = pool.begin().await?;

  let maybe_record = sqlx::query_as!(
    GenericInferenceJobLockRecord,
        r#"
SELECT
  id,
  status,
  attempt_count
FROM generic_inference_jobs
WHERE id = ?
FOR UPDATE
        "#,
        job_id.0,
    )
      .fetch_one(&mut *transaction)
      .await;

  let record : GenericInferenceJobLockRecord = match maybe_record {
    Ok(record) => record,
    Err(err) => {
      match err {
        sqlx::Error::RowNotFound => {
          return Err(anyhow!("could not job"));
        },
        _ => {
          return Err(anyhow!("query error"));
        }
      }
    }
  };

  let can_transact = match record.status.as_ref() {
    "pending" => true, // It's okay for us to take the lock.
    "attempt_failed" => true, // We can retry.
    "started" => false, // Job in progress (another job beat us, and we can't take the lock)
    "complete_success" => false, // Job already complete
    "complete_failure" => false, // Job already complete (permanently dead; no need to retry)
    "dead" => false, // Job already complete (permanently dead; retries exhausted)
    "cancelled_by_user" => false, // Job already complete (permanently dead; killed by user)
    "cancelled_by_system" => false, // Job already complete (permanently dead; killed by system)
    _ => false, // Future-proof
  };

  if !can_transact {
    transaction.rollback().await?;
    return Ok(false);
  }

  if record.attempt_count == 0 {
    let _acquire_lock = sqlx::query!(
        r#"
UPDATE generic_inference_jobs
SET
  status = 'started',
  assigned_worker = ?,
  assigned_cluster = ?,
  attempt_count = attempt_count + 1,
  retry_at = NOW() + interval 2 minute,
  first_started_at = NOW()
WHERE id = ?
        "#,
        &container_environment.hostname,
        &container_environment.cluster_name,
        job_id.0,
    )
        .execute(&mut *transaction)
        .await?;
  } else {
    let _acquire_lock = sqlx::query!(
        r#"
UPDATE generic_inference_jobs
SET
  status = 'started',
  assigned_worker = ?,
  assigned_cluster = ?,
  attempt_count = attempt_count + 1,
  retry_at = NOW() + interval 2 minute
WHERE id = ?
        "#,
        &container_environment.hostname,
        &container_environment.cluster_name,
        job_id.0,
    )
        .execute(&mut *transaction)
        .await?;
  }


  transaction.commit().await?;

  Ok(true)
}
