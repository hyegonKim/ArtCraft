//! Binding for `GET /v1/jobs/job/{token}` — get the status for a single job.

use crate::api_defs::jobs::get_job_status::{get_job_status_url_path, GetJobStatusSuccessResponse};
use crate::credentials::storyteller_credential_set::StorytellerCredentialSet;
use crate::error::storyteller_error::StorytellerError;
use crate::tokens::generic_inference_jobs::InferenceJobToken;
use crate::utils::api_host::ApiHost;
use crate::utils::basic_json_get_request::basic_json_get_request;

/// Get the status of a single job by its token.
/// Publicly exposed; credentials are optional.
pub async fn get_job_status(
  api_host: &ApiHost,
  maybe_creds: Option<&StorytellerCredentialSet>,
  job_token: &InferenceJobToken,
) -> Result<GetJobStatusSuccessResponse, StorytellerError> {
  basic_json_get_request(
    api_host,
    &get_job_status_url_path(job_token),
    maybe_creds,
  ).await
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  #[ignore] // Live: hits api.storyteller.ai (read-only, no credits)
  async fn live_get_job_status() {
    let host = ApiHost::Storyteller;
    // A known completed job (nano banana smoke test).
    let token = InferenceJobToken::new_from_str("jinf_sxr641zj1qgfaky9j78hwerk1r2");
    let result = get_job_status(&host, None, &token).await.unwrap();

    println!("state.status: {:?}", result.state.status.status);
    println!("maybe_result: entity_token={:?}", result.state.maybe_result.as_ref().map(|r| &r.entity_token));

    assert!(result.success);
    assert_eq!(result.state.job_token.as_str(), token.as_str());
  }
}
