//! POST `/fnf/jobs/v2/minimax_h3` — enqueue a MiniMax H3 video job (job set
//! type `minimax_h3`).
//!
//! Options read off the web app on 2026-08-31: duration 5–15 s; 2K only
//! (the app sends 2560×1440); aspect is locked to Auto for text-to-video
//! (it follows the start frame otherwise); optional start / end frames and
//! references.

use crate::client::higgsfield_host::HiggsfieldHost;
use crate::client::send_request::{send_json_request, HttpMethod};
use crate::credentials::higgsfield_auth::HiggsfieldAuth;
use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::types::enqueue_jobs_response::EnqueueJobsResponse;
use crate::types::image_aspect_ratio::ImageAspectRatio;
use crate::types::media_reference::{validate_media_roles, MediaReference};
use crate::types::media_role::MediaRole;
use crate::types::video_dimensions::VideoDimensions;
use crate::types::video_duration::{VideoDurationRange, VideoDurationSeconds};
use crate::types::video_resolution::VideoResolution;
use serde::Serialize;

const PATH: &str = "/fnf/jobs/v2/minimax_h3";

/// The only resolution this pipeline runs at.
const RESOLUTION: VideoResolution = VideoResolution::TwoK;

pub struct MinimaxH3Args<'a> {
  pub request: MinimaxH3Request,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

/// The material part of a MiniMax H3 request. Serializable so it can be
/// logged or persisted separately from the session.
#[derive(Clone, Debug, Serialize)]
pub struct MinimaxH3Request {
  pub prompt: String,

  /// Clip length; see [`Self::DURATION`].
  pub duration: VideoDurationSeconds,

  /// Reference media (frames, reference images / clips / audio), uploaded
  /// first via `endpoints::media` / `HiggsfieldSession::upload_reference_media`
  /// and tagged with a role. The web app offers start + end frames, or references ("up to 9 images and elements, 3 videos, and 3 audio files"). Roles this
  /// model takes: [`Self::MEDIA_ROLES`]. Empty for text-to-video.
  pub medias: Vec<MediaReference>,

  /// Spend from the plan's "unlimited" pool instead of credits, if the plan
  /// has one.
  pub use_unlim: bool,

  /// Override the pixel size sent with the request. When `None`, the 2K
  /// 16:9 frame the web app sends (2560×1440).
  pub maybe_dimensions: Option<VideoDimensions>,
}

impl MinimaxH3Request {
  /// The web app's duration slider range.
  pub const DURATION: VideoDurationRange = VideoDurationRange::new(5, 15);

  /// The media roles this model accepts. `video` / `audio` are
  /// from the web app's bundle; only image roles have been sent live.
  pub const MEDIA_ROLES: &'static [MediaRole] = &[MediaRole::StartImage, MediaRole::EndImage, MediaRole::Image, MediaRole::Video, MediaRole::Audio];

  /// A text-to-video request with the web app's defaults (credits).
  pub fn text_to_video(prompt: impl Into<String>, duration: VideoDurationSeconds) -> Self {
    Self {
      prompt: prompt.into(),
      duration,
      medias: Vec::new(),
      use_unlim: false,
      maybe_dimensions: None,
    }
  }

  fn validate(&self) -> Result<(), HiggsfieldClientError> {
    if self.prompt.trim().is_empty() {
      return Err(HiggsfieldClientError::InvalidRequest("prompt is empty".to_string()));
    }
    validate_media_roles(&self.medias, Self::MEDIA_ROLES, "MiniMax H3")?;
    Self::DURATION.validate(self.duration)
  }

  /// Add one reference (see [`MediaReference`]'s constructors:
  /// `start_frame`, `end_frame`, `image`, `video`, `audio`).
  pub fn with_media(mut self, reference: MediaReference) -> Self {
    self.medias.push(reference);
    self
  }

  fn to_body(&self) -> MinimaxH3RequestBody {
    let dimensions = self.maybe_dimensions
        .or_else(|| VideoDimensions::for_aspect_ratio(&ImageAspectRatio::Auto, &RESOLUTION))
        .expect("2K auto dimensions are derivable");
    MinimaxH3RequestBody {
      params: MinimaxH3Params {
        prompt: self.prompt.clone(),
        duration: self.duration,
        aspect_ratio: ImageAspectRatio::Auto,
        resolution: RESOLUTION,
        width: dimensions.width,
        height: dimensions.height,
        medias: self.medias.clone(),
      },
      use_unlim: self.use_unlim,
    }
  }
}

/// Enqueue the job. The response's job ids are what to poll (see
/// `endpoints::jobs`); a finished job's `results.raw.url` is the `.mp4`.
pub async fn minimax_h3(args: MinimaxH3Args<'_>) -> Result<EnqueueJobsResponse, HiggsfieldError> {
  args.request.validate()?;
  let body = args.request.to_body();
  send_json_request(HttpMethod::Post, PATH, args.auth, args.host, Some(&body)).await
}

// ── Wire format ──

#[derive(Serialize)]
struct MinimaxH3RequestBody {
  params: MinimaxH3Params,
  use_unlim: bool,
}

#[derive(Serialize)]
struct MinimaxH3Params {
  prompt: String,
  duration: VideoDurationSeconds,
  aspect_ratio: ImageAspectRatio,
  resolution: VideoResolution,
  width: u32,
  height: u32,
  medias: Vec<MediaReference>,
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::types::media_input::MediaInput;
  use crate::endpoints::generate::video::test_fixtures::enqueue_response;
  use crate::types::job_set_type::JobSetType;
  use serde_json::Value;

  const SERVER_PARAMS: &str = r#"{"width":2560,"height":1440,"prompt":"a shiba inu skateboarding down a hill","medias":[],"reference_elements":[],"duration":5,"resolution":"2K","aspect_ratio":"auto","aigc_watermark":false,"batch_size":1}"#;

  #[test]
  fn wire_body_matches_captured_request() {
    let request = MinimaxH3Request::text_to_video("a shiba inu skateboarding down a hill", VideoDurationSeconds::new(5));
    let actual: Value = serde_json::to_value(request.to_body()).unwrap();
    let expected: Value = serde_json::from_str(r#"{"params":{"prompt":"a shiba inu skateboarding down a hill","duration":5,"aspect_ratio":"auto","resolution":"2K","width":2560,"height":1440,"medias":[]},"use_unlim":false}"#).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn wire_body_with_start_and_end_frames_matches_captured_request() {
    // Captured from the web app 2026-08-31 (ids scrubbed): with square
    // frames the web app sent 2048x2048; pass `maybe_dimensions` to match.
    let mut request = MinimaxH3Request::text_to_video("a shiba inu skateboarding down a hill", VideoDurationSeconds::new(5))
        .with_media(MediaReference::start_frame(MediaInput::uploaded("00000000-0000-4000-8000-0000000000b1", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b1.png")))
        .with_media(MediaReference::end_frame(MediaInput::uploaded("00000000-0000-4000-8000-0000000000b2", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b2.png")));
    request.maybe_dimensions = Some(VideoDimensions::new(2048, 2048));
    let actual: Value = serde_json::to_value(request.to_body()).unwrap();
    let expected: Value = serde_json::from_str(r#"{"params":{"prompt":"a shiba inu skateboarding down a hill","duration":5,"aspect_ratio":"auto","resolution":"2K","width":2048,"height":2048,"medias":[{"role":"start_image","data":{"id":"00000000-0000-4000-8000-0000000000b1","type":"media_input","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b1.png"}},{"role":"end_image","data":{"id":"00000000-0000-4000-8000-0000000000b2","type":"media_input","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b2.png"}}]},"use_unlim":false}"#).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn media_roles() {
    assert_eq!(MinimaxH3Request::MEDIA_ROLES, &[MediaRole::StartImage, MediaRole::EndImage, MediaRole::Image, MediaRole::Video, MediaRole::Audio]);
    let base = || MinimaxH3Request::text_to_video("p", VideoDurationSeconds::new(5));
    assert!(base().with_media(MediaReference::video(MediaInput::uploaded("00000000-0000-4000-8000-0000000000aa", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png"))).with_media(MediaReference::audio(MediaInput::uploaded("00000000-0000-4000-8000-0000000000b2", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b2.png"))).validate().is_ok());
    let two_ends = base().with_media(MediaReference::end_frame(MediaInput::uploaded("00000000-0000-4000-8000-0000000000b1", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b1.png"))).with_media(MediaReference::end_frame(MediaInput::uploaded("00000000-0000-4000-8000-0000000000b2", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b2.png")));
    assert!(matches!(two_ends.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[test]
  fn validation() {
    assert_eq!(MinimaxH3Request::DURATION, VideoDurationRange::new(5, 15));
    let bad = MinimaxH3Request::text_to_video("p", VideoDurationSeconds::new(4));
    assert!(matches!(bad.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
    let empty = MinimaxH3Request::text_to_video("", VideoDurationSeconds::new(5));
    assert!(matches!(empty.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[tokio::test]
  async fn invalid_request_fails_before_any_http() {
    let auth = HiggsfieldAuth::new("token");
    let host = HiggsfieldHost::Custom("http://127.0.0.1:9".to_string());
    let request = MinimaxH3Request::text_to_video("p", VideoDurationSeconds::new(16));
    let err = minimax_h3(MinimaxH3Args { request, auth: &auth, host: &host }).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[test]
  fn enqueue_response_parses() {
    let response: EnqueueJobsResponse = serde_json::from_str(&enqueue_response("minimax_h3", 1000, SERVER_PARAMS, true)).unwrap();
    let job_set = response.first_job_set().unwrap();
    assert_eq!(job_set.job_set_type, JobSetType::MinimaxH3);
    assert_eq!(job_set.params.aspect_ratio, Some(ImageAspectRatio::Auto));
    assert_eq!(job_set.params.duration, Some(5));
    assert_eq!(job_set.params.extra.get("aigc_watermark"), Some(&Value::Bool(false)));
  }

  // ── Live (ignored: needs a real session and spends credits) ──

  #[tokio::test]
  #[ignore]
  async fn live_enqueue_minimax_h3_from_app_credential_and_poll() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_credential_toml::load_higgsfield_session_from_app_credential;
    use crate::test_utils::poll_job_to_completion::poll_job_to_completion;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let session = load_higgsfield_session_from_app_credential()?;
    let auth = session.auth().await.map_err(|err| anyhow::anyhow!("{err}"))?;
    let request = MinimaxH3Request::text_to_video("a shiba inu skiing down a mountain", MinimaxH3Request::DURATION.shortest());
    println!("\n===== request =====\n{:#?}", request);
    let response = minimax_h3(MinimaxH3Args { request, auth: &auth, host: &HiggsfieldHost::Higgsfield }).await.map_err(|err| anyhow::anyhow!("{err}"))?;
    println!("\n===== POST {PATH} =====\n{:#?}", response);
    assert_eq!(response.first_job_set().unwrap().job_set_type, JobSetType::MinimaxH3);
    let job = poll_job_to_completion(&session, &response.job_ids()[0]).await?;
    assert!(job.result_url().unwrap().ends_with(".mp4"));
    Ok(())
  }
}
