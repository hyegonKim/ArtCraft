//! Types for `GET /v1/jobs/batch` — get job statuses for a batch of job tokens.
//!
//! Mirrors storyteller-web's `batch_get_inference_job_status_handler`. The
//! per-job payload shape is identical to the single-job endpoint, so this
//! reuses [`JobStatusPayload`](crate::api_defs::jobs::get_job_status::JobStatusPayload).

use serde::Deserialize;
use serde::Serialize;

use crate::api_defs::jobs::get_job_status::JobStatusPayload;
use crate::tokens::generic_inference_jobs::InferenceJobToken;

pub const BATCH_GET_JOB_STATUS_URL_PATH: &str = "/v1/jobs/batch";

/// Build the request path for a batch of job tokens.
/// The server reads repeated `tokens=` query keys into a set.
pub fn batch_get_job_status_url_path(job_tokens: &[InferenceJobToken]) -> String {
  let query = job_tokens.iter()
      .map(|token| format!("tokens={}", token.as_str()))
      .collect::<Vec<_>>()
      .join("&");
  format!("{}?{}", BATCH_GET_JOB_STATUS_URL_PATH, query)
}

#[derive(Serialize, Deserialize)]
pub struct BatchGetJobStatusSuccessResponse {
  pub success: bool,

  /// One entry per job the server knows about. Unknown tokens are simply
  /// absent from the list.
  pub job_states: Vec<JobStatusPayload>,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn url_path_repeats_tokens_key() {
    let tokens = vec![
      InferenceJobToken::new_from_str("jinf_aaa"),
      InferenceJobToken::new_from_str("jinf_bbb"),
    ];
    assert_eq!(
      batch_get_job_status_url_path(&tokens),
      "/v1/jobs/batch?tokens=jinf_aaa&tokens=jinf_bbb",
    );
  }

  #[test]
  fn url_path_with_no_tokens() {
    assert_eq!(batch_get_job_status_url_path(&[]), "/v1/jobs/batch?");
  }
}
