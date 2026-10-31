//! Types for `GET /v1/jobs/job/{token}` — get the status for a single job.
//!
//! Mirrors storyteller-web's `get_inference_job_status_handler`. The batch
//! endpoint (`/v1/jobs/batch`) returns the same per-job payload shape, so
//! its binding reuses these types.

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde::Serialize;
use sqlite_identifiers::ids::prompt_token::PromptToken;

use crate::api_defs::common::responses::job_details::{JobDetailsLipsyncRequest, JobDetailsLivePortraitRequest};
use crate::api_defs::common::responses::media_links::MediaLinks;
use crate::enums::api_safe::by_table::generic_inference_jobs::frontend_failure_category_for_api_clients::FrontendFailureCategoryForApiClients;
use crate::enums::by_table::generic_inference_jobs::inference_category::InferenceCategory;
use crate::enums::common::job_status_plus::JobStatusPlus;
use crate::enums::no_table::style_transfer::style_transfer_name::StyleTransferName;
use crate::tokens::generic_inference_jobs::InferenceJobToken;

pub const GET_JOB_STATUS_URL_PATH_PREFIX: &str = "/v1/jobs/job";

/// Build the request path for a single job's status.
pub fn get_job_status_url_path(job_token: &InferenceJobToken) -> String {
  format!("{}/{}", GET_JOB_STATUS_URL_PATH_PREFIX, job_token.as_str())
}

#[derive(Serialize, Deserialize)]
pub struct GetJobStatusSuccessResponse {
  pub success: bool,
  pub state: JobStatusPayload,
}

/// The full status payload for one job.
#[derive(Serialize, Deserialize)]
pub struct JobStatusPayload {
  pub job_token: InferenceJobToken,

  pub request: JobStatusRequestDetails,
  pub status: JobStatusDetails,
  pub maybe_result: Option<JobStatusResultDetails>,

  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
}

/// Details about what the user requested for generation.
#[derive(Serialize, Deserialize)]
pub struct JobStatusRequestDetails {
  pub inference_category: InferenceCategory,

  pub maybe_prompt_token: Option<PromptToken>,

  pub maybe_model_type: Option<String>,
  pub maybe_model_token: Option<String>,

  /// OPTIONAL. Title of the model, if it has one
  pub maybe_model_title: Option<String>,

  /// OPTIONAL. If the result was TTS, this is the raw inference text.
  pub maybe_raw_inference_text: Option<String>,

  /// OPTIONAL. For Comfy / Video Style Transfer jobs, this might include
  /// the name of the selected style.
  pub maybe_style_name: Option<StyleTransferName>,

  /// OPTIONAL. For Live Portrait jobs, this is additional information on the request.
  pub maybe_live_portrait_details: Option<JobDetailsLivePortraitRequest>,

  /// OPTIONAL. For lipsync jobs (face fusion and sad talker), this is additional
  /// information on the request.
  pub maybe_lipsync_details: Option<JobDetailsLipsyncRequest>,
}

/// Details about the ongoing job status.
#[derive(Serialize, Deserialize)]
pub struct JobStatusDetails {
  /// Primary status from the database (a state machine).
  pub status: JobStatusPlus,

  /// Extra, temporary status from Redis.
  /// This can denote inference progress, and the Python code can write to it.
  pub maybe_extra_status_description: Option<String>,

  pub maybe_assigned_worker: Option<String>,
  pub maybe_assigned_cluster: Option<String>,

  pub maybe_first_started_at: Option<DateTime<Utc>>,

  pub attempt_count: u8,

  /// Whether the frontend needs to maintain a keepalive check.
  /// This is typically only for non-premium users.
  pub requires_keepalive: bool,

  /// An enum the frontend can use to display localized/I18N error
  /// messages. These pertain to both transient and permanent failures.
  ///
  /// NB: The server serializes its closed `FrontendFailureCategory` here; we
  /// deserialize with the api-safe type so new server variants can't kill
  /// the client.
  pub maybe_failure_category: Option<FrontendFailureCategoryForApiClients>,

  /// This is an integer number between 0 and 100 (both inclusive) that
  /// reports the completeness.
  pub progress_percentage: u8,
}

/// Details about the completed result (if any).
#[derive(Serialize, Deserialize)]
pub struct JobStatusResultDetails {
  pub entity_type: String,
  pub entity_token: String,

  /// (DEPRECATED) URL path to the media file.
  /// This field doesn't point to the full URL. Use media_links instead to leverage the CDN.
  pub maybe_public_bucket_media_path: Option<String>,

  /// Rich CDN links to the media, including thumbnails, previews, and more.
  pub media_links: MediaLinks,

  pub maybe_successfully_completed_at: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn url_path_includes_token() {
    let token = InferenceJobToken::new_from_str("jinf_abc123");
    assert_eq!(get_job_status_url_path(&token), "/v1/jobs/job/jinf_abc123");
  }
}
