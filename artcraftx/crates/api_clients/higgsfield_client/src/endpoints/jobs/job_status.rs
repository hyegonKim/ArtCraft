//! GET `/fnf/jobs/{job_id}` — the full state of one job, including its
//! result URLs once complete.

use crate::client::higgsfield_host::HiggsfieldHost;
use crate::client::send_request::{send_json_request, HttpMethod};
use crate::credentials::higgsfield_auth::HiggsfieldAuth;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::types::ids::{JobId, JobSetId, UserId};
use crate::types::job_media::JobResults;
use crate::types::job_params::JobParams;
use crate::types::job_set_type::JobSetType;
use crate::types::job_status::JobStatus;
use serde::Deserialize;
use serde_json::Value;

pub struct JobStatusArgs<'a> {
  pub request: JobStatusRequest,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

#[derive(Clone, Debug)]
pub struct JobStatusRequest {
  pub job_id: JobId,
}

#[derive(Clone, Debug, Deserialize)]
pub struct JobStatusResponse {
  pub id: JobId,

  pub status: JobStatus,

  pub job_set_type: JobSetType,

  pub job_set_id: JobSetId,

  #[serde(default)]
  pub job_set_parent_id: Option<JobSetId>,

  /// The server's normalized parameters. Note `width` / `height` can change
  /// between "in progress" and "completed".
  #[serde(default)]
  pub params: JobParams,

  /// Present once `status` is `completed`.
  #[serde(default)]
  pub results: Option<JobResults>,

  /// Legacy single-result slot; `null` in every capture so far.
  #[serde(default)]
  pub result: Option<Value>,

  /// Server-side notes; a failed job's `fail_reason` lives here (see
  /// [`Self::fail_reason`]).
  #[serde(default)]
  pub meta: serde_json::Map<String, Value>,

  /// Whether the IP (intellectual property) check has run.
  #[serde(default)]
  pub ip_check_finished: Option<bool>,

  /// Whether the IP check flagged the output.
  #[serde(default)]
  pub ip_detected: Option<bool>,

  /// Unix epoch seconds (fractional).
  pub created_at: f64,

  #[serde(default)]
  pub published_at: Option<f64>,

  #[serde(default)]
  pub user_id: Option<UserId>,

  #[serde(default)]
  pub trace_id: Option<String>,

  #[serde(default)]
  pub cluster_hash: Option<String>,

  #[serde(default)]
  pub user: Option<JobUser>,

  #[serde(default)]
  pub board_ids: Vec<String>,

  #[serde(default)]
  pub folder_ids: Vec<String>,

  #[serde(default)]
  pub is_favourite: bool,

  #[serde(default)]
  pub is_viewed: bool,
}

impl JobStatusResponse {
  /// Why a `failed` job failed, when the server says (e.g. "Input audio
  /// duration is not supported. Please try a shorter audio.").
  pub fn fail_reason(&self) -> Option<&str> {
    self.meta.get("fail_reason").and_then(Value::as_str)
  }

  /// The full-resolution output URL, once the job completed.
  pub fn result_url(&self) -> Option<&str> {
    self.results.as_ref().map(|results| results.raw.url.as_str())
  }
}

/// The job's owner, as embedded in the status response.
#[derive(Clone, Debug, Deserialize)]
pub struct JobUser {
  pub id: UserId,

  #[serde(default)]
  pub full_name: Option<String>,

  #[serde(default)]
  pub username: Option<String>,

  #[serde(default)]
  pub avatar_url: Option<String>,
}

pub async fn job_status(args: JobStatusArgs<'_>) -> Result<JobStatusResponse, HiggsfieldError> {
  let path = format!("/fnf/jobs/{}", args.request.job_id.as_str());
  send_json_request(HttpMethod::Get, &path, args.auth, args.host, None::<&()>).await
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn failed_job_exposes_its_reason() {
    // Live 2026-08-31 (ids scrubbed): a Seedance job rejected for its audio reference.
    let json = r#"{"board_ids":[],"cluster_hash":"65b161afd05ea0a6f0de6b28848b60aa","created_at":1788160267.405772,"folder_ids":[],"id":"00000000-0000-0000-0000-00000000ffff","ip_check_finished":false,"ip_detected":null,"is_favourite":false,"is_viewed":false,"job_set_client_meta":null,"job_set_id":"00000000-0000-0000-0000-00000000bbbb","job_set_parent_id":null,"job_set_type":"seedance_2_0_mini","meta":{"fail_reason":"Input audio duration is not supported. Please try a shorter audio."},"published_at":null,"representation":null,"result":null,"results":null,"status":"failed","trace_id":"00000000-0000-0000-0000-00000000ffff","user_id":"user_TESTUSER0000000000000000000","params":{"aspect_ratio":"1:1","duration":4,"medias":[{"data":{"id":"00000000-0000-4000-8000-0000000000cc","type":"video_input","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000cc.mp4"},"role":"video"},{"data":{"id":"00000000-0000-4000-8000-0000000000dd","type":"audio_input","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000dd.wav"},"role":"audio"}],"prompt":"p","resolution":"480p","width":480,"height":480}}"#;
    let job: JobStatusResponse = serde_json::from_str(json).unwrap();
    assert_eq!(job.status, JobStatus::Failed);
    assert_eq!(job.fail_reason(), Some("Input audio duration is not supported. Please try a shorter audio."));
    assert_eq!(job.params.medias[0].data.kind, crate::types::media_input::MediaInputKind::VideoInput);
    assert_eq!(job.params.medias[1].data.kind, crate::types::media_input::MediaInputKind::AudioInput);
  }
  use crate::types::image_quality::ImageQuality;
  use crate::types::image_aspect_ratio::ImageAspectRatio;
  use crate::types::job_media::JobMediaType;

  // Captured responses, with ids / user details scrubbed.

  const NANO_BANANA_IN_PROGRESS: &str = r#"{"job_set_type":"nano_banana_2","job_set_id":"00000000-0000-0000-0000-00000000bbbb","job_set_parent_id":null,"job_set_client_meta":null,"board_ids":[],"params":{"width":864,"height":1184,"aspect_ratio":"3:4","resolution":"1k","batch_size":1,"input_images":[],"input_image":null,"application":null,"surface":null,"prompt":"a dinosaur on a skateboard"},"meta":{},"id":"00000000-0000-0000-0000-00000000cccc","status":"in_progress","ip_check_finished":null,"ip_detected":null,"result":null,"results":null,"published_at":null,"created_at":1788147223.626676,"user_id":"user_TESTUSER0000000000000000000","trace_id":"00000000-0000-0000-0000-00000000cccc","user":{"id":"user_TESTUSER0000000000000000000","full_name":"Test User","username":"testuser","avatar_url":null},"folder_ids":[],"is_favourite":false,"is_viewed":false}"#;

  const NANO_BANANA_COMPLETED: &str = r#"{"job_set_type":"nano_banana_2","job_set_id":"00000000-0000-0000-0000-00000000bbbb","job_set_parent_id":null,"job_set_client_meta":null,"board_ids":[],"params":{"width":5504,"height":3072,"aspect_ratio":"16:9","resolution":"4k","batch_size":1,"input_images":[],"input_image":null,"application":null,"surface":null,"prompt":"a dinosaur on a skateboard"},"meta":{},"id":"00000000-0000-0000-0000-00000000cccc","status":"completed","ip_check_finished":true,"ip_detected":false,"result":null,"results":{"raw":{"type":"image","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/hf_20260831_034933_00000000-0000-0000-0000-00000000cccc.png"},"min":{"type":"image","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/hf_20260831_034933_00000000-0000-0000-0000-00000000cccc_min.webp"}},"published_at":null,"created_at":1788148173.993516,"user_id":"user_TESTUSER0000000000000000000","trace_id":"00000000-0000-0000-0000-00000000cccc","user":{"id":"user_TESTUSER0000000000000000000","full_name":"Test User","username":"testuser","avatar_url":null},"folder_ids":[],"is_favourite":false,"is_viewed":false}"#;

  const GPT_IMAGE_IN_PROGRESS: &str = r#"{"job_set_type":"gpt_image_2","job_set_id":"00000000-0000-0000-0000-00000000dddd","job_set_parent_id":null,"job_set_client_meta":null,"board_ids":[],"params":{"width":1152,"height":2048,"prompt":"a corgi on a bike","medias":[],"aspect_ratio":"9:16","quality":"high","resolution":"2k","model":"videotape-alpha","remove_bg":false,"reference_elements":[]},"meta":{},"id":"00000000-0000-0000-0000-00000000eeee","status":"in_progress","ip_check_finished":null,"ip_detected":null,"result":null,"results":null,"published_at":null,"created_at":1788148993.971706,"user_id":"user_TESTUSER0000000000000000000","trace_id":"00000000-0000-0000-0000-00000000eeee","cluster_hash":"d8cd56aedaaaeedac3a5ed262f5eed0e","representation":null,"user":{"id":"user_TESTUSER0000000000000000000","full_name":"Test User","username":"testuser","avatar_url":null},"folder_ids":[],"is_favourite":false,"is_viewed":false}"#;

  const GPT_IMAGE_COMPLETED: &str = r#"{"job_set_type":"gpt_image_2","job_set_id":"00000000-0000-0000-0000-00000000dddd","job_set_parent_id":null,"job_set_client_meta":null,"board_ids":[],"params":{"width":1520,"height":2688,"prompt":"a corgi on a bike","medias":[],"aspect_ratio":"9:16","quality":"high","resolution":"2k","model":"videotape-alpha","remove_bg":false,"reference_elements":[]},"meta":{},"id":"00000000-0000-0000-0000-00000000eeee","status":"completed","ip_check_finished":true,"ip_detected":false,"result":null,"results":{"raw":{"type":"image","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/hf_20260831_040313_00000000-0000-0000-0000-00000000eeee.png"},"min":{"type":"image","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/hf_20260831_040313_00000000-0000-0000-0000-00000000eeee.png"}},"published_at":null,"created_at":1788148993.971706,"user_id":"user_TESTUSER0000000000000000000","trace_id":"00000000-0000-0000-0000-00000000eeee","cluster_hash":"d8cd56aedaaaeedac3a5ed262f5eed0e","representation":null,"user":{"id":"user_TESTUSER0000000000000000000","full_name":"Test User","username":"testuser","avatar_url":null},"folder_ids":[],"is_favourite":false,"is_viewed":false}"#;

  #[test]
  fn in_progress_nano_banana_parses() {
    let response: JobStatusResponse = serde_json::from_str(NANO_BANANA_IN_PROGRESS).unwrap();
    assert_eq!(response.id.as_str(), "00000000-0000-0000-0000-00000000cccc");
    assert_eq!(response.status, JobStatus::InProgress);
    assert!(!response.status.is_terminal());
    assert_eq!(response.job_set_type, JobSetType::NanoBanana2);
    assert_eq!(response.job_set_id.as_str(), "00000000-0000-0000-0000-00000000bbbb");
    assert!(response.results.is_none());
    assert_eq!(response.result_url(), None);
    assert_eq!(response.params.aspect_ratio, Some(ImageAspectRatio::Portrait3x4));
    assert_eq!(response.user.as_ref().unwrap().username.as_deref(), Some("testuser"));
    assert_eq!(response.ip_check_finished, None);
  }

  #[test]
  fn completed_nano_banana_has_results() {
    let response: JobStatusResponse = serde_json::from_str(NANO_BANANA_COMPLETED).unwrap();
    assert_eq!(response.status, JobStatus::Completed);
    assert!(response.status.is_terminal());
    assert_eq!(response.ip_check_finished, Some(true));
    assert_eq!(response.ip_detected, Some(false));

    let results = response.results.as_ref().unwrap();
    assert_eq!(results.raw.media_type, JobMediaType::Image);
    assert!(results.raw.url.ends_with(".png"));
    assert!(results.min.url.ends_with("_min.webp"));
    assert_eq!(response.result_url(), Some(results.raw.url.as_str()));

    // The server re-derived the size at completion.
    assert_eq!((response.params.width, response.params.height), (Some(5504), Some(3072)));
  }

  #[test]
  fn in_progress_gpt_image_parses() {
    let response: JobStatusResponse = serde_json::from_str(GPT_IMAGE_IN_PROGRESS).unwrap();
    assert_eq!(response.job_set_type, JobSetType::GptImage2);
    assert_eq!(response.params.quality, Some(ImageQuality::High));
    assert_eq!(response.cluster_hash.as_deref(), Some("d8cd56aedaaaeedac3a5ed262f5eed0e"));
  }

  #[test]
  fn completed_gpt_image_min_may_equal_raw() {
    let response: JobStatusResponse = serde_json::from_str(GPT_IMAGE_COMPLETED).unwrap();
    assert_eq!(response.status, JobStatus::Completed);
    let results = response.results.unwrap();
    assert_eq!(results.raw.url, results.min.url);
  }

  #[test]
  fn unknown_status_does_not_fail_parsing() {
    let json = NANO_BANANA_IN_PROGRESS.replace("\"status\":\"in_progress\"", "\"status\":\"some_new_state\"");
    let response: JobStatusResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(response.status, JobStatus::Other("some_new_state".to_string()));
  }

  // ── Live (ignored: needs a real session and a real job id) ──

  /// Drives the binding off the desktop app's saved Higgsfield login
  /// (`~/Artcraft/artcraftx/credentials/higgsfield_cookies.toml`) against
  /// the job ids in `external/credentials/higgsfield/job_ids.txt`, printing
  /// each full record.
  #[tokio::test]
  #[ignore]
  async fn live_job_status_from_app_credential() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_credential_toml::load_higgsfield_session_from_app_credential;
    use crate::test_utils::higgsfield_test_secrets::load_higgsfield_test_job_ids;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let session = load_higgsfield_session_from_app_credential()?;
    let auth = session.auth().await.map_err(|err| anyhow::anyhow!("minting a session token failed: {err}"))?;

    for job_id in load_higgsfield_test_job_ids()? {
      let response = job_status(JobStatusArgs {
        request: JobStatusRequest { job_id: job_id.clone() },
        auth: &auth,
        host: &HiggsfieldHost::Higgsfield,
      }).await.map_err(|err| anyhow::anyhow!("job {job_id}: {err}"))?;

      println!("\n===== /fnf/jobs/{job_id} =====\n{:#?}", response);
      println!("result_url = {:?}", response.result_url());

      assert_eq!(response.id, job_id);
      assert!(response.status.is_terminal(), "job {job_id} should be done by now, got {}", response.status);
      if response.status.is_success() {
        assert!(response.result_url().is_some(), "completed job {job_id} should have a result url");
      }
    }
    Ok(())
  }

  #[tokio::test]
  #[ignore]
  async fn live_job_status() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_test_secrets::{load_higgsfield_test_auth, load_higgsfield_test_job_id};
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let auth = load_higgsfield_test_auth().await?;
    let job_id = load_higgsfield_test_job_id()?;
    let response = job_status(JobStatusArgs {
      request: JobStatusRequest { job_id },
      auth: &auth,
      host: &HiggsfieldHost::Higgsfield,
    }).await.map_err(|err| anyhow::anyhow!("{err}"))?;

    println!("Job {} status={} results={:?}", response.id, response.status, response.results);
    Ok(())
  }
}
