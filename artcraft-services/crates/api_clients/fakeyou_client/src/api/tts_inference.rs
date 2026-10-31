use chrono::{DateTime, Utc};
use tokens::tokens::tts_models::TtsModelToken;

#[derive(Debug, Serialize, Clone)]
pub struct CreateTtsInferenceRequest<'a> {
  pub uuid_idempotency_token: &'a str,
  pub tts_model_token: &'a TtsModelToken,
  pub inference_text: &'a str,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CreateTtsInferenceResponse {
  pub success: bool,
  pub inference_job_token: Option<String>, // TODO: Strongly type
}

#[derive(Debug, Deserialize, Clone)]
pub struct TtsInferenceJobStatus {
  pub success: bool,
  pub state: Option<TtsInferenceJobStatusStatePayload>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TtsInferenceJobStatusStatePayload {
  pub job_token: String, // TODO: Strongly type
  pub status: String, // TODO: Strongly type
  pub maybe_extra_status_description: Option<String>,
  pub attempt_count: u32,
  pub maybe_result_token: Option<String>, // TODO: Strongly type
  pub maybe_public_bucket_wav_audio_path: Option<String>,
  pub model_token: TtsModelToken,
  pub tts_model_type: String,
  pub title: String,
  pub raw_inference_text: String,
  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
}
