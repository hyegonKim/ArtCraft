use chrono::NaiveDateTime;
use sqlx::MySqlPool;

use errors::AnyhowResult;

use crate::helpers::numeric_converters::try_i64_to_u64_or_min;

#[derive(Clone)]
pub struct InferenceQueueLengthResult {
  pub record_count: u64,
  pub present_time: NaiveDateTime,
}

struct QueryResultInternal {
  record_count: i64,
  present_time: NaiveDateTime,
}

pub async fn get_pending_inference_job_count(
  pool: &MySqlPool,
) -> AnyhowResult<InferenceQueueLengthResult> {
  // NB (1): We query as a union since "started" jobs can get picked off mid-run and go stale
  // forever. We currently have no automated means of collecting those jobs.
  // NB (2): We query the server timestamp so we can cache the results.
  // The frontend can then monotonically adjust the count based on timestamp by ignoring
  // past times.
  let result : QueryResultInternal = sqlx::query_as!(
      QueryResultInternal,
        r#"
SELECT
  COUNT(distinct token) as record_count,
  NOW() as present_time
FROM
(
  SELECT token
  FROM generic_inference_jobs
  WHERE status = "started"
  AND created_at > (CURDATE() - INTERVAL 5 MINUTE)
UNION
  SELECT token
  FROM generic_inference_jobs
  WHERE status IN ("pending", "attempt_failed")
) as t
        "#,
    )
      .fetch_one(pool)
      .await?;

  Ok(InferenceQueueLengthResult {
    record_count: try_i64_to_u64_or_min(result.record_count),
    present_time: result.present_time,
  })
}
