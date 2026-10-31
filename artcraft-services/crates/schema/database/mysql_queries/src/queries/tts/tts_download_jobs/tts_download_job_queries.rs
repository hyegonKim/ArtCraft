use std::path::Path;

use anyhow::anyhow;
use chrono::Utc;
use sqlx::MySqlPool;

use errors::AnyhowResult;
use tokens::tokens::tts_models::TtsModelToken;
use tokens::tokens::users::UserToken;

/// table: tts_model_upload_jobs
#[derive(Debug)]
pub struct TtsUploadJobRecord {
  pub id: i64,
  pub token: String,
  pub uuid_idempotency_token: String,
  pub on_success_result_token: Option<String>,
  pub creator_user_token: UserToken,
  pub creator_ip_address: String,
  pub creator_set_visibility: String, // TODO
  pub title: String,
  pub tts_model_type: String, // TODO
  pub download_url: Option<String>,
  pub status: String, // TODO
  pub attempt_count: i32,
  pub failure_reason: Option<String>,
  pub created_at: chrono::DateTime<Utc>,
  pub updated_at: chrono::DateTime<Utc>,
  pub retry_at: Option<chrono::DateTime<Utc>>,
}

pub async fn query_tts_upload_job_records(pool: &MySqlPool, num_records: u32)
  -> AnyhowResult<Vec<TtsUploadJobRecord>>
{
  let job_records = sqlx::query_as!(
      TtsUploadJobRecord,
        r#"
SELECT
  id,
  token,
  uuid_idempotency_token,
  on_success_result_token,
  creator_user_token as `creator_user_token: tokens::tokens::users::UserToken`,
  creator_ip_address,
  creator_set_visibility,
  title,
  tts_model_type,
  download_url,
  status,
  attempt_count,
  failure_reason,
  created_at,
  updated_at,
  retry_at
FROM tts_model_upload_jobs
WHERE
  (
    status IN ("pending", "attempt_failed")
  )
  AND
  (
    retry_at IS NULL
    OR
    retry_at < CURRENT_TIMESTAMP
  )
  LIMIT ?
        "#,
      num_records,
    )
    .fetch_all(pool)
    .await?;

  Ok(job_records)
}

pub struct TtsUploadLockRecord {
  id: i64,
  status: String,
}

pub async fn grab_job_lock_and_mark_pending(
  pool: &MySqlPool,
  job: &TtsUploadJobRecord
) -> AnyhowResult<bool> {

  // NB: We use transactions and "SELECT ... FOR UPDATE" to simulate mutexes.
  let mut transaction = pool.begin().await?;

  let maybe_record = sqlx::query_as!(
    TtsUploadLockRecord,
        r#"
SELECT
  id,
  status
FROM tts_model_upload_jobs
WHERE id = ?
FOR UPDATE
        "#,
        job.id,
    )
      .fetch_one(&mut *transaction)
      .await;

  let record : TtsUploadLockRecord = match maybe_record {
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
    _ => false, // Future-proof
  };

  if !can_transact {
    transaction.rollback().await?;
    return Ok(false);
  }

  let _acquire_lock = sqlx::query!(
        r#"
UPDATE tts_model_upload_jobs
SET
  status = 'started',
  attempt_count = attempt_count + 1,
  retry_at = NOW() + interval 2 minute
WHERE id = ?
        "#,
        job.id,
    )
      .execute(&mut *transaction)
      .await?;

  transaction.commit().await?;

  Ok(true)
}

pub async fn mark_tts_upload_job_failure(
  pool: &MySqlPool,
  job: &TtsUploadJobRecord,
  failure_reason: &str,
  max_attempts: i32
) -> AnyhowResult<()> {

  // statuses: "attempt_failed", "complete_failure", "dead"
  let mut next_status = "attempt_failed";

  if job.attempt_count >= max_attempts {
    // NB: Job attempt count is incremented at start
    next_status = "dead";
  }

  let _query_result = sqlx::query!(
        r#"
UPDATE tts_model_upload_jobs
SET
  status = ?,
  failure_reason = ?,
  retry_at = NOW() + interval 2 minute
WHERE id = ?
        "#,
        next_status,
        failure_reason.to_string(),
        job.id,
    )
    .execute(pool)
    .await?;

  Ok(())
}

pub async fn mark_tts_upload_job_done(
  pool: &MySqlPool,
  job: &TtsUploadJobRecord,
  success: bool,
  maybe_model_token: Option<&str>,
) -> AnyhowResult<()>
{
  let status = if success { "complete_success" } else { "complete_failure" };

  let _query_result = sqlx::query!(
        r#"
UPDATE tts_model_upload_jobs
SET
  status = ?,
  on_success_result_token = ?,
  failure_reason = NULL,
  retry_at = NULL
WHERE id = ?
        "#,
        status,
        maybe_model_token,
        job.id,
    )
    .execute(pool)
    .await?;

  Ok(())
}

pub async fn insert_tts_model<P: AsRef<Path>>(
  pool: &MySqlPool,
  job: &TtsUploadJobRecord,
  private_bucket_hash: &str,
  private_bucket_object_name: P,
  file_size_bytes: u64
) -> AnyhowResult<(u64, String)> {

  let model_token = TtsModelToken::generate().to_string();

  let private_bucket_object_name = &private_bucket_object_name
      .as_ref()
      .display()
      .to_string();

  let query_result = sqlx::query!(
        r#"
INSERT INTO tts_models
SET
  token = ?,
  tts_model_type = "tacotron2",
  title = ?,
  description_markdown = '',
  description_rendered_html = '',
  creator_user_token = ?,
  creator_ip_address_creation = ?,
  creator_ip_address_last_update = ?,
  original_download_url = ?,
  private_bucket_hash = ?,
  private_bucket_object_name = ?,
  file_size_bytes = ?
        "#,
      &model_token,
      job.title.to_string(),
      job.creator_user_token.as_str(),
      job.creator_ip_address.clone(),
      job.creator_ip_address.clone(),
      job.download_url.clone(),
      private_bucket_hash.to_string(),
      private_bucket_object_name.to_string(),
      file_size_bytes
    )
    .execute(pool)
    .await;

  let record_id = match query_result {
    Ok(res) => {
      res.last_insert_id()
    },
    Err(err) => {
      // TODO: handle better
      return Err(anyhow!("Mysql error: {:?}", err));
    }
  };

  Ok((record_id, model_token))
}
