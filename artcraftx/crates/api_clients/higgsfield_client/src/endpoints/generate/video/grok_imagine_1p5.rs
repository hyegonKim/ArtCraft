//! POST `/fnf/jobs/v2/grok_video_v15` — enqueue a Grok Imagine 1.5 video job
//! (job set type `grok_video_v15`).
//!
//! Options read off the web app on 2026-08-31: duration 1–15 s, 480p /
//! 720p / 1080p; aspect is locked to Auto for text-to-video (it follows the
//! start frame otherwise); optional start frame and references.

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

const PATH: &str = "/fnf/jobs/v2/grok_video_v15";

/// The resolution tiers the web app offers for Grok Imagine 1.5.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum GrokImagine1p5Resolution {
  #[default]
  P480,
  P720,
  P1080,
}

impl GrokImagine1p5Resolution {
  pub fn all() -> [Self; 3] {
    [Self::P480, Self::P720, Self::P1080]
  }

  pub fn to_video_resolution(self) -> VideoResolution {
    match self {
      Self::P480 => VideoResolution::P480,
      Self::P720 => VideoResolution::P720,
      Self::P1080 => VideoResolution::P1080,
    }
  }
}

impl Serialize for GrokImagine1p5Resolution {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    self.to_video_resolution().serialize(serializer)
  }
}

pub struct GrokImagine1p5Args<'a> {
  pub request: GrokImagine1p5Request,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

/// The material part of a Grok Imagine 1.5 request. Serializable so it can
/// be logged or persisted separately from the session.
#[derive(Clone, Debug, Serialize)]
pub struct GrokImagine1p5Request {
  pub prompt: String,

  pub resolution: GrokImagine1p5Resolution,

  /// Clip length; see [`Self::DURATION`].
  pub duration: VideoDurationSeconds,

  /// Reference media (frames, reference images / clips / audio), uploaded
  /// first via `endpoints::media` / `HiggsfieldSession::upload_reference_media`
  /// and tagged with a role. The web app offers a start frame OR up to 7 reference images (role `image`). Roles this
  /// model takes: [`Self::MEDIA_ROLES`]. Empty for text-to-video.
  pub medias: Vec<MediaReference>,

  /// Spend from the plan's "unlimited" pool instead of credits, if the plan
  /// has one.
  pub use_unlim: bool,

  /// Override the pixel size sent with the request. When `None`, the 16:9
  /// frame for `resolution` the web app sends.
  pub maybe_dimensions: Option<VideoDimensions>,
}

impl GrokImagine1p5Request {
  /// The web app's duration slider range.
  pub const DURATION: VideoDurationRange = VideoDurationRange::new(1, 15);

  /// The media roles this model accepts.
  pub const MEDIA_ROLES: &'static [MediaRole] = &[MediaRole::StartImage, MediaRole::Image];

  /// A text-to-video request with the web app's defaults (credits).
  pub fn text_to_video(prompt: impl Into<String>, resolution: GrokImagine1p5Resolution, duration: VideoDurationSeconds) -> Self {
    Self {
      prompt: prompt.into(),
      resolution,
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
    validate_media_roles(&self.medias, Self::MEDIA_ROLES, "Grok Imagine 1.5")?;
    Self::DURATION.validate(self.duration)
  }

  /// Add one reference (see [`MediaReference`]'s constructors:
  /// `start_frame`, `end_frame`, `image`, `video`, `audio`).
  pub fn with_media(mut self, reference: MediaReference) -> Self {
    self.medias.push(reference);
    self
  }

  fn to_body(&self) -> GrokImagine1p5RequestBody {
    let dimensions = self.maybe_dimensions
        .or_else(|| VideoDimensions::for_aspect_ratio(&ImageAspectRatio::Auto, &self.resolution.to_video_resolution()))
        .expect("auto dimensions are derivable for every known resolution");
    GrokImagine1p5RequestBody {
      params: GrokImagine1p5Params {
        prompt: self.prompt.clone(),
        resolution: self.resolution,
        aspect_ratio: ImageAspectRatio::Auto,
        width: dimensions.width,
        height: dimensions.height,
        duration: self.duration,
        medias: self.medias.clone(),
      },
      use_unlim: self.use_unlim,
    }
  }
}

/// Enqueue the job. The response's job ids are what to poll (see
/// `endpoints::jobs`); a finished job's `results.raw.url` is the `.mp4`.
pub async fn grok_imagine_1p5(args: GrokImagine1p5Args<'_>) -> Result<EnqueueJobsResponse, HiggsfieldError> {
  args.request.validate()?;
  let body = args.request.to_body();
  send_json_request(HttpMethod::Post, PATH, args.auth, args.host, Some(&body)).await
}

// ── Wire format ──

#[derive(Serialize)]
struct GrokImagine1p5RequestBody {
  params: GrokImagine1p5Params,
  use_unlim: bool,
}

#[derive(Serialize)]
struct GrokImagine1p5Params {
  prompt: String,
  resolution: GrokImagine1p5Resolution,
  aspect_ratio: ImageAspectRatio,
  width: u32,
  height: u32,
  duration: VideoDurationSeconds,
  medias: Vec<MediaReference>,
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::types::media_input::MediaInput;
  use crate::endpoints::generate::video::test_fixtures::enqueue_response;
  use crate::types::job_set_type::JobSetType;
  use serde_json::Value;

  const SERVER_PARAMS: &str = r#"{"width":854,"height":480,"prompt":"a shiba inu skateboarding down a hill","medias":[],"duration":1,"resolution":"480p","aspect_ratio":"auto"}"#;

  #[test]
  fn resolutions_match_the_web_app_menu() {
    let wire: Vec<String> = GrokImagine1p5Resolution::all().iter().map(|r| r.to_video_resolution().to_string()).collect();
    assert_eq!(wire, ["480p", "720p", "1080p"]);
    assert_eq!(GrokImagine1p5Request::DURATION, VideoDurationRange::new(1, 15));
  }

  #[test]
  fn wire_body_matches_captured_request() {
    let request = GrokImagine1p5Request::text_to_video("a shiba inu skateboarding down a hill", GrokImagine1p5Resolution::P480, VideoDurationSeconds::new(1));
    let actual: Value = serde_json::to_value(request.to_body()).unwrap();
    let expected: Value = serde_json::from_str(r#"{"params":{"prompt":"a shiba inu skateboarding down a hill","resolution":"480p","aspect_ratio":"auto","width":854,"height":480,"duration":1,"medias":[]},"use_unlim":false}"#).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn wire_body_with_start_frame_matches_captured_request() {
    // Captured from the web app 2026-08-31 (ids scrubbed). The web app also
    // sends `ipCheckFinished` / `ipStatus` inside `data`; the server drops
    // them, so they're not part of the expected body.
    let request = GrokImagine1p5Request::text_to_video("a shiba inu skateboarding down a hill", GrokImagine1p5Resolution::P480, VideoDurationSeconds::new(1))
        .with_media(MediaReference::start_frame(MediaInput::uploaded("00000000-0000-4000-8000-0000000000b1", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b1.png")));
    let actual: Value = serde_json::to_value(request.to_body()).unwrap();
    let expected: Value = serde_json::from_str(r#"{"params":{"prompt":"a shiba inu skateboarding down a hill","resolution":"480p","aspect_ratio":"auto","width":854,"height":480,"duration":1,"medias":[{"role":"start_image","data":{"id":"00000000-0000-4000-8000-0000000000b1","type":"media_input","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b1.png"}}]},"use_unlim":false}"#).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn wire_body_with_reference_image_matches_captured_request() {
    let request = GrokImagine1p5Request::text_to_video("a shiba inu skateboarding down a hill", GrokImagine1p5Resolution::P480, VideoDurationSeconds::new(1))
        .with_media(MediaReference::image(MediaInput::uploaded("00000000-0000-4000-8000-0000000000aa", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png")));
    let actual: Value = serde_json::to_value(request.to_body()).unwrap();
    let expected: Value = serde_json::from_str(r#"{"params":{"prompt":"a shiba inu skateboarding down a hill","resolution":"480p","aspect_ratio":"auto","width":854,"height":480,"duration":1,"medias":[{"role":"image","data":{"id":"00000000-0000-4000-8000-0000000000aa","type":"media_input","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png"}}]},"use_unlim":false}"#).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn media_roles() {
    assert_eq!(GrokImagine1p5Request::MEDIA_ROLES, &[MediaRole::StartImage, MediaRole::Image]);
    let base = || GrokImagine1p5Request::text_to_video("p", GrokImagine1p5Resolution::P480, VideoDurationSeconds::new(1));
    assert!(base().with_media(MediaReference::start_frame(MediaInput::uploaded("00000000-0000-4000-8000-0000000000b1", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b1.png"))).with_media(MediaReference::image(MediaInput::uploaded("00000000-0000-4000-8000-0000000000aa", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png"))).validate().is_ok());
    let end = base().with_media(MediaReference::end_frame(MediaInput::uploaded("00000000-0000-4000-8000-0000000000b2", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b2.png")));
    assert!(matches!(end.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
    let two_starts = base().with_media(MediaReference::start_frame(MediaInput::uploaded("00000000-0000-4000-8000-0000000000b1", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b1.png"))).with_media(MediaReference::start_frame(MediaInput::uploaded("00000000-0000-4000-8000-0000000000b2", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b2.png")));
    assert!(matches!(two_starts.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[test]
  fn validation() {
    let bad = GrokImagine1p5Request::text_to_video("p", GrokImagine1p5Resolution::P480, VideoDurationSeconds::new(0));
    assert!(matches!(bad.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[tokio::test]
  async fn invalid_request_fails_before_any_http() {
    let auth = HiggsfieldAuth::new("token");
    let host = HiggsfieldHost::Custom("http://127.0.0.1:9".to_string());
    let request = GrokImagine1p5Request::text_to_video("", GrokImagine1p5Resolution::P480, VideoDurationSeconds::new(1));
    let err = grok_imagine_1p5(GrokImagine1p5Args { request, auth: &auth, host: &host }).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[test]
  fn enqueue_response_parses() {
    let response: EnqueueJobsResponse = serde_json::from_str(&enqueue_response("grok_video_v15", 250, SERVER_PARAMS, true)).unwrap();
    let job_set = response.first_job_set().unwrap();
    assert_eq!(job_set.job_set_type, JobSetType::GrokVideoV15);
    assert_eq!(job_set.cost, Some(250.0));
    assert_eq!(job_set.params.duration, Some(1));
  }

  // ── Live (ignored: needs a real session and spends credits) ──

  #[tokio::test]
  #[ignore]
  async fn live_enqueue_grok_imagine_1p5_from_app_credential_and_poll() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_credential_toml::load_higgsfield_session_from_app_credential;
    use crate::test_utils::poll_job_to_completion::poll_job_to_completion;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let session = load_higgsfield_session_from_app_credential()?;
    let auth = session.auth().await.map_err(|err| anyhow::anyhow!("{err}"))?;
    let request = GrokImagine1p5Request::text_to_video("a shiba inu winning a bicycle race", GrokImagine1p5Resolution::P480, GrokImagine1p5Request::DURATION.shortest());
    println!("\n===== request =====\n{:#?}", request);
    let response = grok_imagine_1p5(GrokImagine1p5Args { request, auth: &auth, host: &HiggsfieldHost::Higgsfield }).await.map_err(|err| anyhow::anyhow!("{err}"))?;
    println!("\n===== POST {PATH} =====\n{:#?}", response);
    assert_eq!(response.first_job_set().unwrap().job_set_type, JobSetType::GrokVideoV15);
    let job = poll_job_to_completion(&session, &response.job_ids()[0]).await?;
    assert!(job.result_url().unwrap().ends_with(".mp4"));
    Ok(())
  }
}
