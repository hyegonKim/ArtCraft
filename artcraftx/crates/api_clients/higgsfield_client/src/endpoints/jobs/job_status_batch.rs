//! POST `/fnf/jobs/status-batch` — lightweight status for many jobs at once.
//! Returns just the status (no params or results); use
//! [`job_status`](crate::endpoints::jobs::job_status) to fetch a finished
//! job's outputs.

use crate::client::higgsfield_host::HiggsfieldHost;
use crate::client::send_request::{send_json_request, HttpMethod};
use crate::credentials::higgsfield_auth::HiggsfieldAuth;
use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::types::ids::JobId;
use crate::types::job_set_type::JobSetType;
use crate::types::job_status::JobStatus;
use serde::{Deserialize, Serialize};

const PATH: &str = "/fnf/jobs/status-batch";

pub struct JobStatusBatchArgs<'a> {
  pub request: JobStatusBatchRequest,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

#[derive(Clone, Debug, Serialize)]
pub struct JobStatusBatchRequest {
  pub ids: Vec<JobId>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct JobStatusBatchResponse {
  /// One entry per known job, in the order the server chose.
  pub items: Vec<JobStatusBatchItem>,

  /// Requested ids the server doesn't know.
  #[serde(default)]
  pub missing: Vec<JobId>,
}

impl JobStatusBatchResponse {
  pub fn find(&self, job_id: &JobId) -> Option<&JobStatusBatchItem> {
    self.items.iter().find(|item| &item.id == job_id)
  }
}

#[derive(Clone, Debug, Deserialize)]
pub struct JobStatusBatchItem {
  pub id: JobId,

  pub status: JobStatus,

  #[serde(default)]
  pub job_set_type: Option<JobSetType>,

  #[serde(default)]
  pub ip_check_finished: Option<bool>,

  #[serde(default)]
  pub ip_detected: Option<bool>,
}

pub async fn job_status_batch(args: JobStatusBatchArgs<'_>) -> Result<JobStatusBatchResponse, HiggsfieldError> {
  if args.request.ids.is_empty() {
    return Err(HiggsfieldClientError::InvalidRequest("ids is empty".to_string()).into());
  }

  send_json_request(HttpMethod::Post, PATH, args.auth, args.host, Some(&args.request)).await
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::Value;

  // Captured, with ids scrubbed.
  const IN_PROGRESS_RESPONSE: &str = r#"{"items":[{"id":"00000000-0000-0000-0000-00000000cccc","status":"in_progress","ip_check_finished":null,"ip_detected":null,"job_set_type":"nano_banana_2"}],"missing":[]}"#;

  #[test]
  fn wire_body_matches_captured_request() {
    let request = JobStatusBatchRequest { ids: vec![JobId::new("00000000-0000-0000-0000-00000000cccc")] };
    let actual: Value = serde_json::to_value(&request).unwrap();
    let expected: Value = serde_json::from_str(r#"{"ids":["00000000-0000-0000-0000-00000000cccc"]}"#).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn response_parses() {
    let response: JobStatusBatchResponse = serde_json::from_str(IN_PROGRESS_RESPONSE).unwrap();
    assert_eq!(response.items.len(), 1);
    assert!(response.missing.is_empty());

    let job_id = JobId::new("00000000-0000-0000-0000-00000000cccc");
    let item = response.find(&job_id).unwrap();
    assert_eq!(item.status, JobStatus::InProgress);
    assert_eq!(item.job_set_type, Some(JobSetType::NanoBanana2));
    assert!(response.find(&JobId::new("nope")).is_none());
  }

  #[test]
  fn missing_ids_parse() {
    let json = r#"{"items":[],"missing":["00000000-0000-0000-0000-00000000ffff"]}"#;
    let response: JobStatusBatchResponse = serde_json::from_str(json).unwrap();
    assert!(response.items.is_empty());
    assert_eq!(response.missing, vec![JobId::new("00000000-0000-0000-0000-00000000ffff")]);
  }

  #[tokio::test]
  async fn empty_ids_fails_before_any_http() {
    let auth = HiggsfieldAuth::new("token");
    let host = HiggsfieldHost::Custom("http://127.0.0.1:9".to_string());
    let err = job_status_batch(JobStatusBatchArgs {
      request: JobStatusBatchRequest { ids: vec![] },
      auth: &auth,
      host: &host,
    }).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
  }

  // ── Live (ignored: needs a real session and a real job id) ──

  /// Drives the binding off the desktop app's saved Higgsfield login
  /// (`~/Artcraft/artcraftx/credentials/higgsfield_cookies.toml`) against
  /// the job ids in `external/credentials/higgsfield/job_ids.txt` — plus one
  /// made-up id, to show the `missing` list working — printing the whole
  /// response.
  #[tokio::test]
  #[ignore]
  async fn live_job_status_batch_from_app_credential() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_credential_toml::load_higgsfield_session_from_app_credential;
    use crate::test_utils::higgsfield_test_secrets::load_higgsfield_test_job_ids;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let session = load_higgsfield_session_from_app_credential()?;
    let auth = session.auth().await.map_err(|err| anyhow::anyhow!("minting a session token failed: {err}"))?;

    let unknown = JobId::new("00000000-0000-4000-8000-000000000000");
    let mut ids = load_higgsfield_test_job_ids()?;
    ids.push(unknown.clone());

    let response = job_status_batch(JobStatusBatchArgs {
      request: JobStatusBatchRequest { ids: ids.clone() },
      auth: &auth,
      host: &HiggsfieldHost::Higgsfield,
    }).await.map_err(|err| anyhow::anyhow!("{err}"))?;

    println!("\n===== /fnf/jobs/status-batch ({} ids) =====\n{:#?}", ids.len(), response);
    for item in &response.items {
      println!("{} => {} ({:?})", item.id, item.status, item.job_set_type);
    }
    println!("missing => {:?}", response.missing);

    for job_id in load_higgsfield_test_job_ids()? {
      let item = response.find(&job_id).unwrap_or_else(|| panic!("known job {job_id} absent from batch response"));
      assert!(item.status.is_terminal(), "job {job_id} should be done by now, got {}", item.status);
    }
    Ok(())
  }

  #[tokio::test]
  #[ignore]
  async fn live_job_status_batch() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_test_secrets::{load_higgsfield_test_auth, load_higgsfield_test_job_id};
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let auth = load_higgsfield_test_auth().await?;
    let job_id = load_higgsfield_test_job_id()?;
    let response = job_status_batch(JobStatusBatchArgs {
      request: JobStatusBatchRequest { ids: vec![job_id] },
      auth: &auth,
      host: &HiggsfieldHost::Higgsfield,
    }).await.map_err(|err| anyhow::anyhow!("{err}"))?;

    println!("Batch status: {:?}", response);
    Ok(())
  }
}
