//! POST `/fnf/jobs/v2/seedance_2_0` — enqueue a Seedance 2.0 video job (job
//! set type `seedance_2_0`).
//!
//! Options read off the web app on 2026-08-31: duration 4–15 s, aspect
//! Auto / 16:9 / 9:16 / 4:3 / 3:4 / 1:1 / 21:9, 480p / 720p / 1080p / 4k,
//! audio on/off, bitrate High / Standard, uploaded media. The app sends
//! `mode: "std"`.

use crate::client::higgsfield_host::HiggsfieldHost;
use crate::client::send_request::{send_json_request, HttpMethod};
use crate::credentials::higgsfield_auth::HiggsfieldAuth;
use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::types::enqueue_jobs_response::EnqueueJobsResponse;
use crate::types::image_aspect_ratio::ImageAspectRatio;
use crate::types::media_reference::{validate_media_roles, MediaReference};
use crate::types::media_role::MediaRole;
use crate::types::image_batch_size::ImageBatchSize;
use crate::types::video_aspect_ratio::SeedanceVideoAspectRatio;
use crate::types::video_bitrate_mode::VideoBitrateMode;
use crate::types::video_dimensions::VideoDimensions;
use crate::types::video_duration::{VideoDurationRange, VideoDurationSeconds};
use crate::types::video_mode::VideoMode;
use crate::types::video_resolution::VideoResolution;
use serde::Serialize;

const PATH: &str = "/fnf/jobs/v2/seedance_2_0";

/// The `model` field the web app sends on this endpoint.
const MODEL: &str = "seedance_2_0";

/// The resolution tiers the web app offers for Seedance 2.0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Seedance2p0Resolution {
  #[default]
  P480,
  P720,
  P1080,
  FourK,
}

impl Seedance2p0Resolution {
  pub fn all() -> [Self; 4] {
    [Self::P480, Self::P720, Self::P1080, Self::FourK]
  }

  pub fn to_video_resolution(self) -> VideoResolution {
    match self {
      Self::P480 => VideoResolution::P480,
      Self::P720 => VideoResolution::P720,
      Self::P1080 => VideoResolution::P1080,
      Self::FourK => VideoResolution::FourK,
    }
  }
}

impl Serialize for Seedance2p0Resolution {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    self.to_video_resolution().serialize(serializer)
  }
}

pub struct Seedance2p0Args<'a> {
  pub request: Seedance2p0Request,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

/// The material part of a Seedance 2.0 request. Serializable so it can be
/// logged or persisted separately from the session.
#[derive(Clone, Debug, Serialize)]
pub struct Seedance2p0Request {
  pub prompt: String,

  /// `Auto` follows the reference media; for text-to-video the web app
  /// sends it as `16:9`, and so do we.
  pub aspect_ratio: SeedanceVideoAspectRatio,

  pub resolution: Seedance2p0Resolution,

  /// Clip length; see [`Self::DURATION`].
  pub duration: VideoDurationSeconds,

  /// How many clips to generate (1–4). Each costs credits.
  pub batch_size: ImageBatchSize,

  /// Generate a soundtrack (the web app's audio "On").
  pub generate_audio: bool,

  pub bitrate_mode: VideoBitrateMode,

  /// The quality mode; the web app sends `std`.
  pub mode: VideoMode,

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

impl Seedance2p0Request {
  /// The web app's duration slider range.
  pub const DURATION: VideoDurationRange = VideoDurationRange::new(4, 15);

  /// The media roles this model accepts.
  pub const MEDIA_ROLES: &'static [MediaRole] = &[MediaRole::Image, MediaRole::StartImage, MediaRole::EndImage, MediaRole::Video, MediaRole::Audio];

  /// A text-to-video request with the web app's defaults (1 clip, audio on,
  /// high bitrate, `std` mode, credits).
  pub fn text_to_video(prompt: impl Into<String>, aspect_ratio: SeedanceVideoAspectRatio, resolution: Seedance2p0Resolution, duration: VideoDurationSeconds) -> Self {
    Self {
      prompt: prompt.into(),
      aspect_ratio,
      resolution,
      duration,
      batch_size: ImageBatchSize::One,
      generate_audio: true,
      bitrate_mode: VideoBitrateMode::High,
      mode: VideoMode::Std,
      medias: Vec::new(),
      use_unlim: false,
      maybe_dimensions: None,
    }
  }

  fn validate(&self) -> Result<(), HiggsfieldClientError> {
    if self.prompt.trim().is_empty() {
      return Err(HiggsfieldClientError::InvalidRequest("prompt is empty".to_string()));
    }
    validate_media_roles(&self.medias, Self::MEDIA_ROLES, "Seedance 2.0")?;
    Self::DURATION.validate(self.duration)
  }

  /// Add one reference (see [`MediaReference`]'s constructors:
  /// `start_frame`, `end_frame`, `image`, `video`, `audio`).
  pub fn with_media(mut self, reference: MediaReference) -> Self {
    self.medias.push(reference);
    self
  }

  /// `Auto` with no reference media goes out as `16:9`, as observed from
  /// the web app.
  fn wire_aspect_ratio(&self) -> ImageAspectRatio {
    match self.aspect_ratio {
      SeedanceVideoAspectRatio::Auto if self.medias.is_empty() => ImageAspectRatio::Landscape16x9,
      other => other.to_image_aspect_ratio(),
    }
  }

  fn dimensions(&self) -> Result<VideoDimensions, HiggsfieldClientError> {
    if let Some(dimensions) = self.maybe_dimensions {
      return Ok(dimensions);
    }
    VideoDimensions::for_aspect_ratio(&self.wire_aspect_ratio(), &self.resolution.to_video_resolution())
        .ok_or_else(|| HiggsfieldClientError::InvalidRequest("can't derive video dimensions; pass maybe_dimensions".to_string()))
  }

  fn to_body(&self) -> Result<Seedance2p0RequestBody, HiggsfieldClientError> {
    let dimensions = self.dimensions()?;
    Ok(Seedance2p0RequestBody {
      params: Seedance2p0Params {
        model: MODEL,
        mode: self.mode.clone(),
        batch_size: self.batch_size,
        duration: self.duration,
        aspect_ratio: self.wire_aspect_ratio(),
        resolution: self.resolution,
        width: dimensions.width,
        height: dimensions.height,
        generate_audio: self.generate_audio,
        bitrate_mode: self.bitrate_mode.clone(),
        prompt: self.prompt.clone(),
        medias: self.medias.clone(),
      },
      use_unlim: self.use_unlim,
      use_free_gens: false,
    })
  }
}

/// Enqueue the job. The response's job ids are what to poll (see
/// `endpoints::jobs`); a finished job's `results.raw.url` is the `.mp4`.
pub async fn seedance_2p0(args: Seedance2p0Args<'_>) -> Result<EnqueueJobsResponse, HiggsfieldError> {
  args.request.validate()?;
  let body = args.request.to_body()?;
  send_json_request(HttpMethod::Post, PATH, args.auth, args.host, Some(&body)).await
}

// ── Wire format ──

#[derive(Serialize)]
struct Seedance2p0RequestBody {
  params: Seedance2p0Params,
  use_unlim: bool,
  use_free_gens: bool,
}

#[derive(Serialize)]
struct Seedance2p0Params {
  model: &'static str,
  mode: VideoMode,
  batch_size: ImageBatchSize,
  duration: VideoDurationSeconds,
  aspect_ratio: ImageAspectRatio,
  resolution: Seedance2p0Resolution,
  width: u32,
  height: u32,
  generate_audio: bool,
  bitrate_mode: VideoBitrateMode,
  prompt: String,
  medias: Vec<MediaReference>,
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::types::media_input::MediaInput;
  use crate::endpoints::generate::video::test_fixtures::enqueue_response;
  use crate::types::job_set_type::JobSetType;
  use serde_json::Value;

  const SERVER_PARAMS: &str = r#"{"width":854,"height":480,"prompt":"a shiba inu skateboarding down a hill","genre":"auto","medias":[],"duration":8,"resolution":"480p","aspect_ratio":"16:9","generate_audio":true,"multi_shots":false,"multi_shot_mode":"custom","multi_prompt":[],"speedramp":"auto","reference_elements":[],"prompt_language":"en","mode":"std","model":"seedance_2_0","bitrate_mode":"high"}"#;

  #[test]
  fn resolutions_match_the_web_app_menu() {
    let wire: Vec<String> = Seedance2p0Resolution::all().iter().map(|r| r.to_video_resolution().to_string()).collect();
    assert_eq!(wire, ["480p", "720p", "1080p", "4k"]);
    assert_eq!(Seedance2p0Request::DURATION, VideoDurationRange::new(4, 15));
  }

  #[test]
  fn wire_body_matches_captured_request() {
    // Captured with the menu on "Auto" and no media: the app sent 16:9.
    let request = Seedance2p0Request::text_to_video("a shiba inu skateboarding down a hill", SeedanceVideoAspectRatio::Auto, Seedance2p0Resolution::P480, VideoDurationSeconds::new(8));
    let actual: Value = serde_json::to_value(request.to_body().unwrap()).unwrap();
    let expected: Value = serde_json::from_str(r#"{"params":{"model":"seedance_2_0","mode":"std","batch_size":1,"duration":8,"aspect_ratio":"16:9","resolution":"480p","width":854,"height":480,"generate_audio":true,"bitrate_mode":"high","prompt":"a shiba inu skateboarding down a hill","medias":[]},"use_unlim":false,"use_free_gens":false}"#).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn auto_stays_auto_with_reference_media() {
    let mut request = Seedance2p0Request::text_to_video("p", SeedanceVideoAspectRatio::Auto, Seedance2p0Resolution::P480, VideoDurationSeconds::new(4));
    request.medias = vec![MediaReference::image(MediaInput::uploaded("00000000-0000-4000-8000-000000000001", "https://cdn.example.com/user_x/00000000-0000-4000-8000-000000000001.png"))];
    assert_eq!(request.wire_aspect_ratio(), ImageAspectRatio::Auto);
  }

  #[test]
  fn reference_media_serialize_with_roles() {
    // Not browser-captured for this model (its upload picker is gated by a
    // media-upload agreement); the shape and roles come from the web app's
    // bundle and match the other models' captured bodies.
    let request = Seedance2p0Request::text_to_video("p", SeedanceVideoAspectRatio::Landscape16x9, Seedance2p0Resolution::P480, VideoDurationSeconds::new(4))
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
    assert_eq!(Seedance2p0Request::MEDIA_ROLES, &[MediaRole::Image, MediaRole::StartImage, MediaRole::EndImage, MediaRole::Video, MediaRole::Audio]);
    let base = || Seedance2p0Request::text_to_video("p", SeedanceVideoAspectRatio::Landscape16x9, Seedance2p0Resolution::P480, VideoDurationSeconds::new(4));
    assert!(base().with_media(MediaReference::start_frame(MediaInput::uploaded("00000000-0000-4000-8000-0000000000b1", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b1.png"))).with_media(MediaReference::end_frame(MediaInput::uploaded("00000000-0000-4000-8000-0000000000b2", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b2.png"))).validate().is_ok());
    let two_starts = base().with_media(MediaReference::start_frame(MediaInput::uploaded("00000000-0000-4000-8000-0000000000b1", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b1.png"))).with_media(MediaReference::start_frame(MediaInput::uploaded("00000000-0000-4000-8000-0000000000b2", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b2.png")));
    assert!(matches!(two_starts.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[test]
  fn validation() {
    let bad = Seedance2p0Request::text_to_video("p", SeedanceVideoAspectRatio::Landscape16x9, Seedance2p0Resolution::P480, VideoDurationSeconds::new(16));
    assert!(matches!(bad.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
    let empty = Seedance2p0Request::text_to_video("", SeedanceVideoAspectRatio::Landscape16x9, Seedance2p0Resolution::P480, VideoDurationSeconds::new(4));
    assert!(matches!(empty.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[tokio::test]
  async fn invalid_request_fails_before_any_http() {
    let auth = HiggsfieldAuth::new("token");
    let host = HiggsfieldHost::Custom("http://127.0.0.1:9".to_string());
    let request = Seedance2p0Request::text_to_video("", SeedanceVideoAspectRatio::Landscape16x9, Seedance2p0Resolution::P480, VideoDurationSeconds::new(4));
    let err = seedance_2p0(Seedance2p0Args { request, auth: &auth, host: &host }).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[test]
  fn enqueue_response_parses() {
    let response: EnqueueJobsResponse = serde_json::from_str(&enqueue_response("seedance_2_0", 2400, SERVER_PARAMS, true)).unwrap();
    let job_set = response.first_job_set().unwrap();
    assert_eq!(job_set.job_set_type, JobSetType::Seedance2p0);
    assert_eq!(job_set.params.mode, Some(VideoMode::Std));
    assert_eq!(job_set.params.duration, Some(8));
  }

  // ── Live (ignored: needs a real session and spends credits) ──

  #[tokio::test]
  #[ignore]
  async fn live_enqueue_seedance_2p0_from_app_credential_and_poll() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_credential_toml::load_higgsfield_session_from_app_credential;
    use crate::test_utils::poll_job_to_completion::poll_job_to_completion;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let session = load_higgsfield_session_from_app_credential()?;
    let auth = session.auth().await.map_err(|err| anyhow::anyhow!("{err}"))?;
    let request = Seedance2p0Request::text_to_video("a shiba inu scoring a soccer goal", SeedanceVideoAspectRatio::Landscape16x9, Seedance2p0Resolution::P480, Seedance2p0Request::DURATION.shortest());
    println!("\n===== request =====\n{:#?}", request);
    let response = seedance_2p0(Seedance2p0Args { request, auth: &auth, host: &HiggsfieldHost::Higgsfield }).await.map_err(|err| anyhow::anyhow!("{err}"))?;
    println!("\n===== POST {PATH} =====\n{:#?}", response);
    assert_eq!(response.first_job_set().unwrap().job_set_type, JobSetType::Seedance2p0);
    let job = poll_job_to_completion(&session, &response.job_ids()[0]).await?;
    assert!(job.result_url().unwrap().ends_with(".mp4"));
    Ok(())
  }
}
