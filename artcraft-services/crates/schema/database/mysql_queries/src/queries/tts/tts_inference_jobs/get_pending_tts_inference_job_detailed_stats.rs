use anyhow::anyhow;
use sqlx::MySqlPool;

use errors::AnyhowResult;

#[derive(Debug)]
pub struct PendingCountResult {
  pub seconds_since_first: i64,
  pub pending_count: i64,
  pub pending_priority_nonzero_count: i64,
  pub pending_priority_gt_one_count: i64,
  pub attempt_failed_count: i64,
}

pub async fn get_pending_tts_inference_job_detailed_stats(mysql_pool: &MySqlPool) -> AnyhowResult<Option<PendingCountResult>> {
  // NB(old?): Lookup failure is Err(RowNotFound).
  // NB(2022-03-05): The "seconds_since_first" result might return null if no pending, so IFNULL
  //  means we won't fail the full query.
  let maybe_result = sqlx::query_as!(
      PendingCountResult,
        r#"
SELECT
  IFNULL((
    SELECT
      NOW() - t1.created_at AS seconds_since_first
    FROM tts_inference_jobs AS t1
    WHERE t1.status = "pending"
    ORDER BY t1.id ASC
    LIMIT 1
  ), 0) as seconds_since_first,
  sub2.pending_count,
  sub3.pending_priority_nonzero_count,
  sub4.pending_priority_gt_one_count,
  sub5.attempt_failed_count
FROM
  (
    SELECT
      count(t2.id) as pending_count
    FROM tts_inference_jobs AS t2
    WHERE t2.status = "pending"
  ) as sub2,
  (
    SELECT
      count(t3.id) as pending_priority_nonzero_count
    FROM tts_inference_jobs AS t3
    WHERE t3.status = "pending"
    AND t3.priority_level > 0
  ) as sub3,
  (
    SELECT
      count(t4.id) as pending_priority_gt_one_count
    FROM tts_inference_jobs AS t4
    WHERE t4.status = "pending"
    AND t4.priority_level > 1
  ) as sub4,
  (
    SELECT
      count(t5.id) as attempt_failed_count
    FROM tts_inference_jobs AS t5
    WHERE t5.status = "attempt_failed"
  ) as sub5
        "#,
    )
      .fetch_one(mysql_pool)
      .await;

  match maybe_result {
    Ok(result) => Ok(Some(result)),
    Err(ref err) => match err {
      sqlx::Error::RowNotFound => {
        // NB: Not Found for null results means nothing is pending in the queue (not an error!)
        Ok(None)
      },
      _ => {
        Err(anyhow!("error querying tts stats: {:?}", err))
      }
    }
  }
}
