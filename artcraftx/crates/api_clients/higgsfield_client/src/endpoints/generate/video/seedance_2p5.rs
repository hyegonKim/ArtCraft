//! POST `/fnf/jobs/v2/seedance_2_5` — enqueue a Seedance 2.5 video job (job
//! set type `seedance_2_5`).
//!
//! Options read off the web app on 2026-08-31: duration 4–30 s, aspect
//! 21:9 / 16:9 / 4:3 / 1:1 / 3:4 / 9:16, 480p / 720p / 1080p, audio on/off,
//! bitrate High / Standard, reference media (image, video or audio).

use crate::client::higgsfield_host::HiggsfieldHost;
use crate::client::send_request::{send_json_request, HttpMethod};
use crate::credentials::higgsfield_auth::HiggsfieldAuth;
use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::types::enqueue_jobs_response::EnqueueJobsResponse;
use crate::types::image_batch_size::ImageBatchSize;
use crate::types::media_reference::{validate_media_roles, MediaReference};
use crate::types::media_role::MediaRole;
use crate::types::video_aspect_ratio::SeedanceVideoAspectRatio;
use crate::types::video_bitrate_mode::VideoBitrateMode;
use crate::types::video_dimensions::VideoDimensions;
use crate::types::video_duration::{VideoDurationRange, VideoDurationSeconds};
use crate::types::video_resolution::VideoResolution;
use serde::Serialize;

const PATH: &str = "/fnf/jobs/v2/seedance_2_5";

/// The `model` field the web app sends on this endpoint.
const MODEL: &str = "default";

/// The resolution tiers the web app offers for Seedance 2.5.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Seedance2p5Resolution {
  #[default]
  P480,
  P720,
  P1080,
}

impl Seedance2p5Resolution {
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

impl Serialize for Seedance2p5Resolution {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    self.to_video_resolution().serialize(serializer)
  }
}

pub struct Seedance2p5Args<'a> {
  pub request: Seedance2p5Request,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

/// The material part of a Seedance 2.5 request. Serializable so it can be
/// logged or persisted separately from the session.
#[derive(Clone, Debug, Serialize)]
pub struct Seedance2p5Request {
  pub prompt: String,

  /// `Auto` isn't on this model's menu and is rejected.
  pub aspect_ratio: SeedanceVideoAspectRatio,

  pub resolution: Seedance2p5Resolution,

  /// Clip length; see [`Self::DURATION`].
  pub duration: VideoDurationSeconds,

  /// How many clips to generate (1–4). Each costs credits.
  pub batch_size: ImageBatchSize,

  /// Generate a soundtrack (the web app's audio "On").
  pub generate_audio: bool,

  pub bitrate_mode: VideoBitrateMode,

  /// Reference media (frames, reference images / clips / audio), uploaded
  /// first via `endpoints::media` / `HiggsfieldSession::upload_reference_media`
  /// and tagged with a role. The web app's "Upload media" takes images, video and audio (video / audio roles from its bundle; only image roles have been sent live). Roles this
  /// model takes: [`Self::MEDIA_ROLES`]. Empty for text-to-video.
  pub medias: Vec<MediaReference>,

  /// Spend from the plan's "unlimited" pool instead of credits, if the plan
  /// has one.
  pub use_unlim: bool,

  /// Override the pixel size sent with the request. When `None`, derived
  /// from `aspect_ratio` + `resolution` the way the web app does.
  pub maybe_dimensions: Option<VideoDimensions>,
}

impl Seedance2p5Request {
  /// The web app's duration slider range.
  pub const DURATION: VideoDurationRange = VideoDurationRange::new(4, 30);

  /// The media roles this model accepts.
  pub const MEDIA_ROLES: &'static [MediaRole] = &[MediaRole::Image, MediaRole::StartImage, MediaRole::EndImage, MediaRole::Video, MediaRole::Audio];

  /// A text-to-video request with the web app's defaults (1 clip, audio on,
  /// high bitrate, credits).
  pub fn text_to_video(prompt: impl Into<String>, aspect_ratio: SeedanceVideoAspectRatio, resolution: Seedance2p5Resolution, duration: VideoDurationSeconds) -> Self {
    Self {
      prompt: prompt.into(),
      aspect_ratio,
      resolution,
      duration,
      batch_size: ImageBatchSize::One,
      generate_audio: true,
      bitrate_mode: VideoBitrateMode::High,
      medias: Vec::new(),
      use_unlim: false,
      maybe_dimensions: None,
    }
  }

  fn validate(&self) -> Result<(), HiggsfieldClientError> {
    if self.prompt.trim().is_empty() {
      return Err(HiggsfieldClientError::InvalidRequest("prompt is empty".to_string()));
    }
    if self.aspect_ratio == SeedanceVideoAspectRatio::Auto {
      return Err(HiggsfieldClientError::InvalidRequest("Seedance 2.5 has no Auto aspect ratio; pick one".to_string()));
    }
    validate_media_roles(&self.medias, Self::MEDIA_ROLES, "Seedance 2.5")?;
    Self::DURATION.validate(self.duration)
  }

  /// Add one reference (see [`MediaReference`]'s constructors:
  /// `start_frame`, `end_frame`, `image`, `video`, `audio`).
  pub fn with_media(mut self, reference: MediaReference) -> Self {
    self.medias.push(reference);
    self
  }

  fn dimensions(&self) -> Result<VideoDimensions, HiggsfieldClientError> {
    if let Some(dimensions) = self.maybe_dimensions {
      return Ok(dimensions);
    }
    VideoDimensions::for_aspect_ratio(&self.aspect_ratio.to_image_aspect_ratio(), &self.resolution.to_video_resolution())
        .ok_or_else(|| HiggsfieldClientError::InvalidRequest("can't derive video dimensions; pass maybe_dimensions".to_string()))
  }

  fn to_body(&self) -> Result<Seedance2p5RequestBody, HiggsfieldClientError> {
    let dimensions = self.dimensions()?;
    Ok(Seedance2p5RequestBody {
      params: Seedance2p5Params {
        prompt: self.prompt.clone(),
        width: dimensions.width,
        height: dimensions.height,
        medias: self.medias.clone(),
        resolution: self.resolution,
        generate_audio: self.generate_audio,
        bitrate_mode: self.bitrate_mode.clone(),
        batch_size: self.batch_size,
        model: MODEL,
        use_unlim: self.use_unlim,
        duration: self.duration,
        aspect_ratio: self.aspect_ratio,
      },
      use_unlim: self.use_unlim,
    })
  }
}

/// Enqueue the job. The response's job ids are what to poll (see
/// `endpoints::jobs`); a finished job's `results.raw.url` is the `.mp4`.
pub async fn seedance_2p5(args: Seedance2p5Args<'_>) -> Result<EnqueueJobsResponse, HiggsfieldError> {
  args.request.validate()?;
  let body = args.request.to_body()?;
  send_json_request(HttpMethod::Post, PATH, args.auth, args.host, Some(&body)).await
}

// ── Wire format ──

#[derive(Serialize)]
struct Seedance2p5RequestBody {
  params: Seedance2p5Params,
  use_unlim: bool,
}

#[derive(Serialize)]
struct Seedance2p5Params {
  prompt: String,
  width: u32,
  height: u32,
  medias: Vec<MediaReference>,
  resolution: Seedance2p5Resolution,
  generate_audio: bool,
  bitrate_mode: VideoBitrateMode,
  batch_size: ImageBatchSize,
  model: &'static str,
  use_unlim: bool,
  duration: VideoDurationSeconds,
  aspect_ratio: SeedanceVideoAspectRatio,
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::types::media_input::MediaInput;
  use crate::endpoints::generate::video::test_fixtures::enqueue_response;
  use crate::types::job_set_type::JobSetType;
  use serde_json::Value;

  const SERVER_PARAMS: &str = r#"{"width":854,"height":480,"prompt":"a shiba inu skateboarding down a hill","genre":"auto","medias":[],"duration":4,"resolution":"480p","aspect_ratio":"16:9","generate_audio":true,"multi_shots":false,"multi_shot_mode":"custom","multi_prompt":[],"speedramp":"auto","reference_elements":[],"prompt_language":"en","model":"default","extension_mode":null,"bitrate_mode":"high"}"#;

  #[test]
  fn resolutions_match_the_web_app_menu() {
    let wire: Vec<String> = Seedance2p5Resolution::all().iter().map(|r| r.to_video_resolution().to_string()).collect();
    assert_eq!(wire, ["480p", "720p", "1080p"]);
    assert_eq!(Seedance2p5Request::DURATION, VideoDurationRange::new(4, 30));
  }

  #[test]
  fn wire_body_matches_captured_request() {
    let request = Seedance2p5Request::text_to_video("a shiba inu skateboarding down a hill", SeedanceVideoAspectRatio::Landscape16x9, Seedance2p5Resolution::P480, VideoDurationSeconds::new(4));
    let actual: Value = serde_json::to_value(request.to_body().unwrap()).unwrap();
    let expected: Value = serde_json::from_str(r#"{"params":{"prompt":"a shiba inu skateboarding down a hill","width":854,"height":480,"medias":[],"resolution":"480p","generate_audio":true,"bitrate_mode":"high","batch_size":1,"model":"default","use_unlim":false,"duration":4,"aspect_ratio":"16:9"},"use_unlim":false}"#).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn reference_media_serialize_with_roles() {
    // Not browser-captured for this model (its upload picker is gated by a
    // media-upload agreement); the shape and roles come from the web app's
    // bundle and match the other models' captured bodies.
    let request = Seedance2p5Request::text_to_video("p", SeedanceVideoAspectRatio::Landscape16x9, Seedance2p5Resolution::P480, VideoDurationSeconds::new(4))
        .with_media(MediaReference::image(MediaInput::uploaded("00000000-0000-4000-8000-0000000000aa", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png")))
        .with_media(MediaReference::audio(MediaInput::uploaded("00000000-0000-4000-8000-0000000000b2", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b2.png")));
    let body: Value = serde_json::to_value(request.to_body().unwrap()).unwrap();
    let medias = body["params"]["medias"].as_array().unwrap();
    assert_eq!(medias.len(), 2);
    assert_eq!(medias[0]["role"], "image");
    assert_eq!(medias[0]["data"], serde_json::from_str::<Value>(r#"{"id":"00000000-0000-4000-8000-0000000000aa","type":"media_input","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png"}"#).unwrap());
    assert_eq!(medias[1]["role"], "audio");
  }

  #[test]
  fn media_roles() {
    assert_eq!(Seedance2p5Request::MEDIA_ROLES, &[MediaRole::Image, MediaRole::StartImage, MediaRole::EndImage, MediaRole::Video, MediaRole::Audio]);
    let base = || Seedance2p5Request::text_to_video("p", SeedanceVideoAspectRatio::Landscape16x9, Seedance2p5Resolution::P480, VideoDurationSeconds::new(4));
    assert!(base().with_media(MediaReference::start_frame(MediaInput::uploaded("00000000-0000-4000-8000-0000000000b1", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b1.png"))).with_media(MediaReference::end_frame(MediaInput::uploaded("00000000-0000-4000-8000-0000000000b2", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b2.png"))).validate().is_ok());
    let two_starts = base().with_media(MediaReference::start_frame(MediaInput::uploaded("00000000-0000-4000-8000-0000000000b1", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b1.png"))).with_media(MediaReference::start_frame(MediaInput::uploaded("00000000-0000-4000-8000-0000000000b2", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b2.png")));
    assert!(matches!(two_starts.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[test]
  fn validation() {
    let ok = Seedance2p5Request::text_to_video("p", SeedanceVideoAspectRatio::Landscape16x9, Seedance2p5Resolution::P480, VideoDurationSeconds::new(30));
    assert!(ok.validate().is_ok());
    let too_long = Seedance2p5Request::text_to_video("p", SeedanceVideoAspectRatio::Landscape16x9, Seedance2p5Resolution::P480, VideoDurationSeconds::new(31));
    assert!(matches!(too_long.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
    let auto = Seedance2p5Request::text_to_video("p", SeedanceVideoAspectRatio::Auto, Seedance2p5Resolution::P480, VideoDurationSeconds::new(4));
    assert!(matches!(auto.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
    let empty = Seedance2p5Request::text_to_video(" ", SeedanceVideoAspectRatio::Landscape16x9, Seedance2p5Resolution::P480, VideoDurationSeconds::new(4));
    assert!(matches!(empty.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[tokio::test]
  async fn invalid_request_fails_before_any_http() {
    let auth = HiggsfieldAuth::new("token");
    let host = HiggsfieldHost::Custom("http://127.0.0.1:9".to_string());
    let request = Seedance2p5Request::text_to_video("p", SeedanceVideoAspectRatio::Landscape16x9, Seedance2p5Resolution::P480, VideoDurationSeconds::new(3));
    let err = seedance_2p5(Seedance2p5Args { request, auth: &auth, host: &host }).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[test]
  fn enqueue_response_parses() {
    let response: EnqueueJobsResponse = serde_json::from_str(&enqueue_response("seedance_2_5", 1000, SERVER_PARAMS, true)).unwrap();
    let job_set = response.first_job_set().unwrap();
    assert_eq!(job_set.job_set_type, JobSetType::Seedance2p5);
    assert_eq!(job_set.cost, Some(1000.0));
    assert_eq!(job_set.params.duration, Some(4));
    assert_eq!(job_set.params.generate_audio, Some(true));
    assert_eq!(job_set.params.bitrate_mode, Some(VideoBitrateMode::High));
  }

  // ── Live (ignored: needs a real session and spends credits) ──

  #[tokio::test]
  #[ignore]
  async fn live_enqueue_seedance_2p5_from_app_credential_and_poll() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_credential_toml::load_higgsfield_session_from_app_credential;
    use crate::test_utils::poll_job_to_completion::poll_job_to_completion;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let session = load_higgsfield_session_from_app_credential()?;
    let auth = session.auth().await.map_err(|err| anyhow::anyhow!("{err}"))?;
    let request = Seedance2p5Request::text_to_video("a shiba inu surfing a wave", SeedanceVideoAspectRatio::Landscape16x9, Seedance2p5Resolution::P480, Seedance2p5Request::DURATION.shortest());
    println!("\n===== request =====\n{:#?}", request);
    let response = seedance_2p5(Seedance2p5Args { request, auth: &auth, host: &HiggsfieldHost::Higgsfield }).await.map_err(|err| anyhow::anyhow!("{err}"))?;
    println!("\n===== POST {PATH} =====\n{:#?}", response);
    assert_eq!(response.first_job_set().unwrap().job_set_type, JobSetType::Seedance2p5);
    let job = poll_job_to_completion(&session, &response.job_ids()[0]).await?;
    assert!(job.result_url().unwrap().ends_with(".mp4"));
    Ok(())
  }
}
