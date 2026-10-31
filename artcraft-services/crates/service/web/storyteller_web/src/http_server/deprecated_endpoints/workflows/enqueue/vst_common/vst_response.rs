use utoipa::ToSchema;

use tokens::tokens::generic_inference_jobs::InferenceJobToken;

#[derive(Serialize, ToSchema)]
pub struct VstSuccessResponse {
  pub success: bool,
  pub inference_job_token: InferenceJobToken,
}
