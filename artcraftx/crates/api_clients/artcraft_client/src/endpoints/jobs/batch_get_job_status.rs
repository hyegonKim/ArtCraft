//! Binding for `GET /v1/jobs/batch` — get job statuses for a batch of job tokens.

use crate::api_defs::jobs::batch_get_job_status::{batch_get_job_status_url_path, BatchGetJobStatusSuccessResponse};
use crate::credentials::storyteller_credential_set::StorytellerCredentialSet;
use crate::error::storyteller_error::StorytellerError;
use crate::tokens::generic_inference_jobs::InferenceJobToken;
use crate::utils::api_host::ApiHost;
use crate::utils::basic_json_get_request::basic_json_get_request;

/// Get the statuses of multiple jobs in one request.
/// Publicly exposed; credentials are optional. Unknown tokens are simply
/// absent from the response's `job_states`.
pub async fn batch_get_job_status(
  api_host: &ApiHost,
  maybe_creds: Option<&StorytellerCredentialSet>,
  job_tokens: &[InferenceJobToken],
) -> Result<BatchGetJobStatusSuccessResponse, StorytellerError> {
  basic_json_get_request(
    api_host,
    &batch_get_job_status_url_path(job_tokens),
    maybe_creds,
  ).await
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  #[ignore] // Live: hits api.storyteller.ai (read-only, no credits)
  async fn live_batch_get_job_status() {
    let host = ApiHost::Storyteller;
    let tokens = vec![
      InferenceJobToken::new_from_str("jinf_sxr641zj1qgfaky9j78hwerk1r2"),
      InferenceJobToken::new_from_str("jinf_vrbjye4tzfczz3z81e0trz8zmx1"),
    ];
    let result = batch_get_job_status(&host, None, &tokens).await.unwrap();

    for state in &result.job_states {
      println!("job {}: {:?}", state.job_token.as_str(), state.status.status);
    }

    assert!(result.success);
    assert_eq!(result.job_states.len(), 2);
  }
}
