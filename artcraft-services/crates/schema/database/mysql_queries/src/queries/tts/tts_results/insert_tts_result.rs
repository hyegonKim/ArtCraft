use std::path::Path;

use anyhow::anyhow;
use log::warn;
use sqlx;
use sqlx::MySqlPool;

use errors::AnyhowResult;
use tokens::tokens::tts_results::TtsResultToken;

use crate::column_types::vocoder_type::VocoderType;
use crate::queries::generic_inference::job::list_available_generic_inference_jobs::AvailableInferenceJob;
use crate::queries::tts::tts_inference_jobs::list_available_tts_inference_jobs::AvailableTtsInferenceJob;

/// Used to give user-facing order to logged in user inference requests
pub struct SyntheticIdRecord {
  pub next_id: i64,
}

// TODO: Remove once all inference sits atop generic jobs
pub enum JobType<'a> {
  TtsJob(&'a AvailableTtsInferenceJob),
  GenericInferenceJob(&'a AvailableInferenceJob),
}

pub async fn insert_tts_result<P: AsRef<Path>>(
  pool: &MySqlPool,
  job: JobType<'_>,
  text_hash: &str,
  maybe_pretrained_vocoder_used: Option<VocoderType>,
  bucket_audio_results_path: P,
  bucket_spectrogram_results_path: P,
  file_size_bytes: u64,
  duration_millis: u64,
  is_on_prem: bool,
  worker_hostname: &str,
  is_debug_worker: bool,
) -> AnyhowResult<(u64, String)>
{
  let inference_result_token = TtsResultToken::generate().to_string();

  let bucket_audio_result_path = &bucket_audio_results_path
      .as_ref()
      .display()
      .to_string();

  let bucket_spectrogram_result_path = &bucket_spectrogram_results_path
      .as_ref()
      .display()
      .to_string();

  let maybe_pretrained_vocoder_used = maybe_pretrained_vocoder_used
      .map(|v| v.to_str());

  let raw_inference_text;
  let maybe_creator_user_token;
  let tts_model_token;
  let creator_ip_address;
  let creator_set_visibility;

  match job {
    JobType::TtsJob(tts_job) => {
      raw_inference_text = tts_job.raw_inference_text.clone();
      maybe_creator_user_token = tts_job.maybe_creator_user_token.clone();
      tts_model_token = tts_job.model_token.clone();
      creator_ip_address = tts_job.creator_ip_address.clone();
      creator_set_visibility = tts_job.creator_set_visibility;
    }
    JobType::GenericInferenceJob(generic_job) => {
      raw_inference_text = generic_job.maybe_raw_inference_text.as_deref()
          .unwrap_or("")
          .to_string();
      maybe_creator_user_token = generic_job.maybe_creator_user_token.clone();
      tts_model_token = generic_job.maybe_model_token.as_deref()
          .unwrap_or("")
          .to_string();
      creator_ip_address = generic_job.creator_ip_address.clone();
      creator_set_visibility = generic_job.creator_set_visibility;
    }
  }

  let normalized_inference_text = raw_inference_text.clone(); // TODO

  let mut maybe_creator_synthetic_id : Option<u64> = None;

  let mut transaction = pool.begin().await?;

  if let Some(creator_user_token) = maybe_creator_user_token.as_deref() {
    let query_result = sqlx::query!(
        r#"
INSERT INTO tts_result_synthetic_ids
SET
  user_token = ?,
  next_id = 1
ON DUPLICATE KEY UPDATE
  user_token = ?,
  next_id = next_id + 1
        "#,
      creator_user_token,
      creator_user_token
    )
        .execute(&mut *transaction)
        .await;

    match query_result {
      Ok(_) => {},
      Err(err) => {
        //transaction.rollback().await?;
        warn!("Transaction failure: {:?}", err);
      }
    }

    let query_result = sqlx::query_as!(
    SyntheticIdRecord,
        r#"
SELECT
  next_id
FROM
  tts_result_synthetic_ids
WHERE
  user_token = ?
LIMIT 1
        "#,
      creator_user_token,
    )
        .fetch_one(&mut *transaction)
        .await;

    let record : SyntheticIdRecord = match query_result {
      Ok(record) => record,
      Err(err) => {
        warn!("Transaction failure: {:?}", err);
        transaction.rollback().await?;
        return Err(anyhow!("Transaction failure: {:?}", err));
      }
    };

    let next_id = record.next_id as u64;
    maybe_creator_synthetic_id = Some(next_id);
  }

  let record_id = {
    let query_result = sqlx::query!(
        r#"
INSERT INTO tts_results
SET
  token = ?,

  model_token = ?,
  maybe_pretrained_vocoder_used = ?,
  raw_inference_text = ?,
  raw_inference_text_hash_sha2 = ?,
  normalized_inference_text = ?,

  maybe_creator_user_token = ?,
  maybe_creator_synthetic_id = ?,

  creator_ip_address = ?,
  creator_set_visibility = ?,

  public_bucket_wav_audio_path = ?,
  public_bucket_spectrogram_path = ?,

  file_size_bytes = ?,
  duration_millis = ?,

  is_generated_on_prem = ?,
  generated_by_worker = ?,

  is_debug_request = ?
        "#,
      inference_result_token,
      tts_model_token,
      maybe_pretrained_vocoder_used,
      raw_inference_text,
      text_hash,
      normalized_inference_text,

      maybe_creator_user_token,
      maybe_creator_synthetic_id,

      creator_ip_address,
      creator_set_visibility.to_str(),

      bucket_audio_result_path,
      bucket_spectrogram_result_path,

      file_size_bytes,
      duration_millis,
      is_on_prem,
      worker_hostname,
      is_debug_worker,
    )
        .execute(&mut *transaction)
        .await;

    let record_id = match query_result {
      Ok(res) => {
        res.last_insert_id()
      },
      Err(err) => {
        // TODO: handle better
        //transaction.rollback().await?;
        return Err(anyhow!("Mysql error: {:?}", err));
      }
    };

    record_id
  };

  transaction.commit().await?;

  Ok((record_id, inference_result_token.clone()))
}

