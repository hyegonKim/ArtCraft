use anyhow::anyhow;
use sqlx;
use sqlx::MySqlPool;

use errors::AnyhowResult;

use crate::queries::tts::tts_inference_jobs::_keys::TtsInferenceJobId;

pub async fn mark_tts_inference_job_done(
  pool: &MySqlPool,
  job_id: TtsInferenceJobId,
  success: bool,
  maybe_result_token: Option<&str>,
  last_assigned_worker: &str,
) -> AnyhowResult<()> {
  let status = if success { "complete_success" } else { "complete_failure" };

  let query_result = sqlx::query!(
        r#"
UPDATE tts_inference_jobs
SET
  status = ?,
  on_success_result_token = ?,
  last_assigned_worker = ?,
  failure_reason = NULL,
  retry_at = NULL
WHERE id = ?
        "#,
        status,
        maybe_result_token,
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

