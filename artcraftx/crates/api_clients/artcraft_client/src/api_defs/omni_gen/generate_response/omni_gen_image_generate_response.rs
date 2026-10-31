use serde_derive::{Deserialize, Serialize};
use crate::tokens::generic_inference_jobs::InferenceJobToken;

/// Response body for the omni-gen image generation endpoint.
#[derive(Serialize, Deserialize)]
pub struct OmniGenImageGenerateResponse {
  pub success: bool,
  pub inference_job_token: InferenceJobToken,
}
