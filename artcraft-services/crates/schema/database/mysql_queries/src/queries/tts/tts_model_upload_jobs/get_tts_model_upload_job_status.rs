use anyhow::anyhow;
use chrono::{DateTime, Utc};
use sqlx::MySqlPool;

use errors::AnyhowResult;

pub struct TtsUploadModelJobStatusRecord {
  pub job_token: String,

  pub status: String,
  pub attempt_count: i32,
  pub maybe_model_token: Option<String>,

  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
}

pub async fn get_tts_model_upload_job_status(
  job_token: &str,
  mysql_pool: &MySqlPool,
) -> AnyhowResult<Option<TtsUploadModelJobStatusRecord>> {
  // NB: Lookup failure is Err(RowNotFound).
  // NB: Since this is publicly exposed, we don't query sensitive data.
  let maybe_status = sqlx::query_as!(
      TtsUploadModelJobStatusRecord,
        r#"
SELECT
    jobs.token as job_token,

    jobs.status,
    jobs.attempt_count,
    jobs.on_success_result_token as maybe_model_token,

    jobs.created_at,
    jobs.updated_at

FROM tts_model_upload_jobs as jobs

WHERE jobs.token = ?
        "#,
      job_token
    )
      .fetch_one(mysql_pool)
      .await;

  match maybe_status {
    Ok(record) => Ok(Some(record)),
    Err(err) => match err {
      sqlx::Error::RowNotFound => Ok(None),
      _ => Err(anyhow!("tts template query error: {:?}", err)),
    }
  }
}
