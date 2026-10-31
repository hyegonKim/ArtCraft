use serde_derive::{Deserialize, Serialize};
use crate::tokens::generic_inference_jobs::InferenceJobToken;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;

pub const BEEBLE_SWITCHX_EDIT_VIDEO_PATH: &str = "/v1/generate/video/edit/beeble_switchx";

#[derive(Serialize, Deserialize, Debug)]
pub struct BeebleSwitchXEditVideoRequest {
  pub uuid_idempotency_token: String,
  pub source_video_media_token: Option<MediaFileToken>,
  pub reference_image_media_token: Option<MediaFileToken>,
  pub prompt: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct BeebleSwitchXEditVideoResponse {
  pub success: bool,
  pub inference_job_token: InferenceJobToken,
}
