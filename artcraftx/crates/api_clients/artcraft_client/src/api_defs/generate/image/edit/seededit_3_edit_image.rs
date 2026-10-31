use serde_derive::{Deserialize, Serialize};
use crate::tokens::generic_inference_jobs::InferenceJobToken;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;

pub const SEEDEDIT_3_EDIT_IMAGE_PATH: &str = "/v1/generate/image/edit/seededit_3";

#[derive(Serialize, Deserialize)]
pub struct SeedEdit3EditImageRequest {
  /// Idempotency token to prevent duplicate requests.
  pub uuid_idempotency_token: String,

  /// The image we're editing.
  pub image_media_token: MediaFileToken,

  /// Text prompt to generate the image from.
  pub prompt: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct SeedEdit3EditImageResponse {
  pub success: bool,
  pub inference_job_token: InferenceJobToken,
}
