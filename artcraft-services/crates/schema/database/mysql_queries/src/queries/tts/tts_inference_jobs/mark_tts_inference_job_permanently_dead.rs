use anyhow::anyhow;
use sqlx;
use sqlx::MySqlPool;

use errors::AnyhowResult;

use crate::queries::tts::tts_inference_jobs::_keys::TtsInferenceJobId;

/// Mark a TTS job as dead ("do not ever retry").
pub async fn mark_tts_inference_job_permanently_dead(
  pool: &MySqlPool,
  job_id: TtsInferenceJobId,
  failure_reason: &str,
  internal_debugging_failure_reason: &str,
  last_assigned_worker: &str,
) -> AnyhowResult<()> {

  let mut internal_debugging_failure_reason = internal_debugging_failure_reason.to_string();
  internal_debugging_failure_reason.truncate(255); // Field is VARCHAR(255)

  let query_result = sqlx::query!(
        r#"
UPDATE tts_inference_jobs
SET
  status = "dead",
  failure_reason = ?,
  internal_debugging_failure_reason = ?,
  last_assigned_worker = ?,
  retry_at = NULL
WHERE id = ?
        "#,
        failure_reason,
        internal_debugging_failure_reason,
        last_assigned_worker,
        job_id.0,
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
