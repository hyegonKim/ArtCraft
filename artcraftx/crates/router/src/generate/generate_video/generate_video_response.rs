use std::fmt::Debug;
use std::sync::Arc;

use artcraft_client::tokens::generic_inference_jobs::InferenceJobToken;

#[derive(Clone, Debug)]
pub struct ArtcraftVideoResponsePayload {
  pub inference_job_token: InferenceJobToken,
  pub all_inference_job_tokens: Vec<InferenceJobToken>,
}

#[derive(Clone, Debug)]
pub struct Seedance2proVideoResponsePayload {
  pub order_id: String,
  pub task_id: String,
  pub maybe_order_ids: Option<Vec<String>>,
  pub maybe_task_ids: Option<Vec<String>>,
}

#[derive(Clone, Debug)]
pub struct FalVideoResponsePayload {
  pub request_id: Option<String>,
  pub gateway_request_id: Option<String>,

  /// Queue-mode status URL (fal's queue/polling flow). `None` for webhook
  /// dispatch — the webhook callback drives status updates instead.
  pub maybe_status_url: Option<String>,

  /// Queue-mode response URL (fal's queue/polling flow). `None` for webhook
  /// dispatch.
  pub maybe_response_url: Option<String>,

  /// The outbound request that was sent to Fal.
  /// Stored as a trait object so any Request type can be captured.
  /// Use `format!("{:?}", ...)` or `format!("{:#?}", ...)` to print.
  pub maybe_outbound_request: Option<Arc<dyn Debug + Send + Sync>>,
}

#[derive(Clone, Debug)]
pub struct GmiCloudVideoResponsePayload {
  pub request_id: String,
}

#[derive(Clone, Debug)]
pub struct GrokVideoResponsePayload {
  /// xAI request_id; poll with `grok_api_client::api::requests::videos::video_status`
  /// to fetch the rendered video URL once it's ready.
  pub request_id: String,

  /// The outbound request that was sent to Grok (xAI).
  /// Stored as a trait object so any Request type can be captured.
  /// Use `format!("{:?}", ...)` or `format!("{:#?}", ...)` to print.
  pub maybe_outbound_request: Option<Arc<dyn Debug + Send + Sync>>,
}

/// Response from the first-party (cookie-session) Higgsfield provider. One
/// enqueue creates a job set with one job per requested video; the caller
/// polls the jobs (`HiggsfieldSession::job_status_batch`) by id.
#[derive(Clone, Debug)]
pub struct HiggsfieldVideoResponsePayload {
  pub job_set_id: String,
  /// In generation order; never empty.
  pub job_ids: Vec<String>,
}

impl HiggsfieldVideoResponsePayload {
  /// The first job, which stands in for the whole set as the task's
  /// `provider_job_id`.
  pub fn primary_job_id(&self) -> &str {
    &self.job_ids[0]
  }
}

#[derive(Clone, Debug)]
pub enum GenerateVideoResponse {
  Artcraft(ArtcraftVideoResponsePayload),
  GmiCloud(GmiCloudVideoResponsePayload),
  Grok(GrokVideoResponsePayload),
  Higgsfield(HiggsfieldVideoResponsePayload),
  Seedance2Pro(Seedance2proVideoResponsePayload),
  Fal(FalVideoResponsePayload),
}

impl GenerateVideoResponse {
  pub fn get_artcraft_payload(&self) -> Option<ArtcraftVideoResponsePayload> {
    match self {
      Self::Artcraft(p) => Some(p.clone()),
      _ => None,
    }
  }

  pub fn get_seedance2pro_payload(&self) -> Option<Seedance2proVideoResponsePayload> {
    match self {
      Self::Seedance2Pro(p) => Some(p.clone()),
      _ => None,
    }
  }

  pub fn get_gmicloud_payload(&self) -> Option<GmiCloudVideoResponsePayload> {
    match self {
      Self::GmiCloud(p) => Some(p.clone()),
      _ => None,
    }
  }

  pub fn get_grok_payload(&self) -> Option<GrokVideoResponsePayload> {
    match self {
      Self::Grok(p) => Some(p.clone()),
      _ => None,
    }
  }

  pub fn get_higgsfield_payload(&self) -> Option<HiggsfieldVideoResponsePayload> {
    match self {
      Self::Higgsfield(p) => Some(p.clone()),
      _ => None,
    }
  }

  pub fn get_fal_payload(&self) -> Option<FalVideoResponsePayload> {
    match self {
      Self::Fal(p) => Some(p.clone()),
      _ => None,
    }
  }
}
