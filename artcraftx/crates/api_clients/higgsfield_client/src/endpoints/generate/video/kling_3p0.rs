//! POST `/fnf/jobs/v2/kling3_0` — enqueue a Kling 3.0 video job (job set
//! type `kling3_0`).
//!
//! Options read off the web app on 2026-08-31: duration 3–15 s, aspect
//! 16:9 / 9:16 / 1:1, a 720p / 1080p / 4K menu the app sends as `mode`
//! `std` / `pro` / `4k`, prompt enhancement, sound on/off, Kling elements,
//! multi-shot, optional start / end frames.

use crate::client::higgsfield_host::HiggsfieldHost;
use crate::client::send_request::{send_json_request, HttpMethod};
use crate::credentials::higgsfield_auth::HiggsfieldAuth;
use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::types::enqueue_jobs_response::EnqueueJobsResponse;
use crate::types::media_reference::{validate_media_roles, MediaReference};
use crate::types::media_role::MediaRole;
use crate::types::video_aspect_ratio::KlingAspectRatio;
use crate::types::video_dimensions::VideoDimensions;
use crate::types::video_duration::{VideoDurationRange, VideoDurationSeconds};
use crate::types::video_mode::VideoMode;
use crate::types::video_resolution::VideoResolution;
use serde::Serialize;

const PATH: &str = "/fnf/jobs/v2/kling3_0";

/// The web app's resolution menu for Kling 3.0. It goes out as the `mode`
/// param (from the app's bundle: `4k` → 4K, `pro` → 1080p, else 720p).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Kling3p0Resolution {
  /// 720p → `std`
  #[default]
  P720,
  /// 1080p → `pro`
  P1080,
  /// 4K → `4k`
  FourK,
}

impl Kling3p0Resolution {
  pub fn all() -> [Self; 3] {
    [Self::P720, Self::P1080, Self::FourK]
  }

  pub fn label(self) -> &'static str {
    match self {
      Self::P720 => "720p",
      Self::P1080 => "1080p",
      Self::FourK => "4K",
    }
  }

  pub fn to_video_mode(self) -> VideoMode {
    match self {
      Self::P720 => VideoMode::Std,
      Self::P1080 => VideoMode::Pro,
      Self::FourK => VideoMode::FourK,
    }
  }

  fn to_video_resolution(self) -> VideoResolution {
    match self {
      Self::P720 => VideoResolution::P720,
      Self::P1080 => VideoResolution::P1080,
      Self::FourK => VideoResolution::FourK,
    }
  }
}

/// Serializes as the menu label so a logged request reads the way the user
/// chose it; the wire `mode` is derived at send.
impl Serialize for Kling3p0Resolution {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(self.label())
  }
}

pub struct Kling3p0Args<'a> {
  pub request: Kling3p0Request,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

/// The material part of a Kling 3.0 request. Serializable so it can be
/// logged or persisted separately from the session.
#[derive(Clone, Debug, Serialize)]
pub struct Kling3p0Request {
  pub prompt: String,

  pub aspect_ratio: KlingAspectRatio,

  pub resolution: Kling3p0Resolution,

  /// Clip length; see [`Self::DURATION`].
  pub duration: VideoDurationSeconds,

  /// Let Kling rewrite the prompt (the web app's "Enhance").
  pub enhance_prompt: bool,

  /// Generate sound (the web app's audio "On").
  pub sound: bool,

  /// Kling "elements" (consistent characters/objects) to include.
  pub kling_element_ids: Vec<String>,

  /// Reference media (frames, reference images / clips / audio), uploaded
  /// first via `endpoints::media` / `HiggsfieldSession::upload_reference_media`
  /// and tagged with a role. The web app offers start + end frames (no free references; consistent subjects go through `kling_element_ids`). With a frame set the web app hides the aspect menu and sends the frame's aspect (e.g. `1:1`, 720×720); set `aspect_ratio` / `maybe_dimensions` to match your frame. Roles this
  /// model takes: [`Self::MEDIA_ROLES`]. Empty for text-to-video.
  pub medias: Vec<MediaReference>,

  /// Spend from the plan's "unlimited" pool instead of credits, if the plan
  /// has one.
  pub use_unlim: bool,

  /// Override the pixel size sent with the request. When `None`, derived
  /// from `aspect_ratio` + `resolution` the way the web app does.
  pub maybe_dimensions: Option<VideoDimensions>,
}

impl Kling3p0Request {
  /// The web app's duration slider range.
  pub const DURATION: VideoDurationRange = VideoDurationRange::new(3, 15);

  /// The media roles this model accepts.
  pub const MEDIA_ROLES: &'static [MediaRole] = &[MediaRole::StartImage, MediaRole::EndImage];

  /// A text-to-video request with the web app's defaults (enhance on, sound
  /// on, credits).
  pub fn text_to_video(prompt: impl Into<String>, aspect_ratio: KlingAspectRatio, resolution: Kling3p0Resolution, duration: VideoDurationSeconds) -> Self {
    Self {
      prompt: prompt.into(),
      aspect_ratio,
      resolution,
      duration,
      enhance_prompt: true,
      sound: true,
      kling_element_ids: Vec::new(),
      medias: Vec::new(),
      use_unlim: false,
      maybe_dimensions: None,
    }
  }

  fn validate(&self) -> Result<(), HiggsfieldClientError> {
    if self.prompt.trim().is_empty() {
      return Err(HiggsfieldClientError::InvalidRequest("prompt is empty".to_string()));
    }
    validate_media_roles(&self.medias, Self::MEDIA_ROLES, "Kling 3.0")?;
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

  fn to_body(&self) -> Result<Kling3p0RequestBody, HiggsfieldClientError> {
    let dimensions = self.dimensions()?;
    Ok(Kling3p0RequestBody {
      params: Kling3p0Params {
        prompt: self.prompt.clone(),
        enhance_prompt: self.enhance_prompt,
        aspect_ratio: self.aspect_ratio,
        mode: self.resolution.to_video_mode(),
        sound: if self.sound { "on" } else { "off" },
        duration: self.duration,
        multi_shots: false,
        multi_shot_mode: "auto",
        kling_element_ids: self.kling_element_ids.clone(),
        medias: self.medias.clone(),
        width: dimensions.width,
        height: dimensions.height,
      },
      use_unlim: self.use_unlim,
      use_free_gens: false,
    })
  }
}

/// Enqueue the job. The response's job ids are what to poll (see
/// `endpoints::jobs`); a finished job's `results.raw.url` is the `.mp4`.
pub async fn kling_3p0(args: Kling3p0Args<'_>) -> Result<EnqueueJobsResponse, HiggsfieldError> {
  args.request.validate()?;
  let body = args.request.to_body()?;
  send_json_request(HttpMethod::Post, PATH, args.auth, args.host, Some(&body)).await
}

// ── Wire format ──

#[derive(Serialize)]
struct Kling3p0RequestBody {
  params: Kling3p0Params,
  use_unlim: bool,
  use_free_gens: bool,
}

#[derive(Serialize)]
struct Kling3p0Params {
  prompt: String,
  enhance_prompt: bool,
  aspect_ratio: KlingAspectRatio,
  mode: VideoMode,
  sound: &'static str,
  duration: VideoDurationSeconds,
  multi_shots: bool,
  multi_shot_mode: &'static str,
  kling_element_ids: Vec<String>,
  medias: Vec<MediaReference>,
  width: u32,
  height: u32,
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::types::media_input::MediaInput;
  use crate::endpoints::generate::video::test_fixtures::enqueue_response;
  use crate::types::job_set_type::JobSetType;
  use serde_json::Value;

  const SERVER_PARAMS: &str = r#"{"width":1280,"height":720,"prompt":"a shiba inu skateboarding down a hill","medias":[],"duration":3,"aspect_ratio":"16:9","multi_shots":false,"multi_prompt":[],"sound":"on","cfg_scale":0.5,"mode":"std","kling_elements":[],"kling_element_ids":[],"multi_shot_mode":"auto","reference_elements":[],"enhance_prompt":true}"#;

  #[test]
  fn resolution_menu_maps_to_mode() {
    let mapping: Vec<(&str, String)> = Kling3p0Resolution::all().iter().map(|r| (r.label(), r.to_video_mode().to_string())).collect();
    assert_eq!(mapping.iter().map(|(l, m)| (*l, m.as_str())).collect::<Vec<_>>(), [("720p", "std"), ("1080p", "pro"), ("4K", "4k")]);
    assert_eq!(Kling3p0Request::DURATION, VideoDurationRange::new(3, 15));
  }

  #[test]
  fn wire_body_matches_captured_request() {
    let request = Kling3p0Request::text_to_video("a shiba inu skateboarding down a hill", KlingAspectRatio::Landscape16x9, Kling3p0Resolution::P720, VideoDurationSeconds::new(3));
    let actual: Value = serde_json::to_value(request.to_body().unwrap()).unwrap();
    let expected: Value = serde_json::from_str(r#"{"params":{"prompt":"a shiba inu skateboarding down a hill","enhance_prompt":true,"aspect_ratio":"16:9","mode":"std","sound":"on","duration":3,"multi_shots":false,"multi_shot_mode":"auto","kling_element_ids":[],"medias":[],"width":1280,"height":720},"use_unlim":false,"use_free_gens":false}"#).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn sound_off_and_pro_mode() {
    let mut request = Kling3p0Request::text_to_video("p", KlingAspectRatio::Portrait9x16, Kling3p0Resolution::P1080, VideoDurationSeconds::new(5));
    request.sound = false;
    let body: Value = serde_json::to_value(request.to_body().unwrap()).unwrap();
    assert_eq!(body["params"]["sound"], "off");
    assert_eq!(body["params"]["mode"], "pro");
    assert_eq!((body["params"]["width"].as_u64(), body["params"]["height"].as_u64()), (Some(1080), Some(1920)));
  }

  #[test]
  fn wire_body_with_start_and_end_frames_matches_captured_request() {
    // Captured from the web app 2026-08-31 (ids scrubbed): with square
    // frames the web app sent `1:1` and 720x720 at 720p.
    let request = Kling3p0Request::text_to_video("a shiba inu skateboarding down a hill", KlingAspectRatio::Square1x1, Kling3p0Resolution::P720, VideoDurationSeconds::new(3))
        .with_media(MediaReference::start_frame(MediaInput::uploaded("00000000-0000-4000-8000-0000000000b1", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b1.png")))
        .with_media(MediaReference::end_frame(MediaInput::uploaded("00000000-0000-4000-8000-0000000000b2", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b2.png")));
    let actual: Value = serde_json::to_value(request.to_body().unwrap()).unwrap();
    let expected: Value = serde_json::from_str(r#"{"params":{"prompt":"a shiba inu skateboarding down a hill","enhance_prompt":true,"aspect_ratio":"1:1","mode":"std","sound":"on","duration":3,"multi_shots":false,"multi_shot_mode":"auto","kling_element_ids":[],"medias":[{"role":"start_image","data":{"id":"00000000-0000-4000-8000-0000000000b1","type":"media_input","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b1.png"}},{"role":"end_image","data":{"id":"00000000-0000-4000-8000-0000000000b2","type":"media_input","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000b2.png"}}],"width":720,"height":720},"use_unlim":false,"use_free_gens":false}"#).unwrap();
    assert_eq!(actual, expected);
  }

  #[test]
  fn media_roles() {
    assert_eq!(Kling3p0Request::MEDIA_ROLES, &[MediaRole::StartImage, MediaRole::EndImage]);
    let base = || Kling3p0Request::text_to_video("p", KlingAspectRatio::Landscape16x9, Kling3p0Resolution::P720, VideoDurationSeconds::new(3));
    let reference = base().with_media(MediaReference::image(MediaInput::uploaded("00000000-0000-4000-8000-0000000000aa", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png")));
    assert!(matches!(reference.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[test]
  fn validation() {
    let bad = Kling3p0Request::text_to_video("p", KlingAspectRatio::Square1x1, Kling3p0Resolution::P720, VideoDurationSeconds::new(2));
    assert!(matches!(bad.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[tokio::test]
  async fn invalid_request_fails_before_any_http() {
    let auth = HiggsfieldAuth::new("token");
    let host = HiggsfieldHost::Custom("http://127.0.0.1:9".to_string());
    let request = Kling3p0Request::text_to_video("", KlingAspectRatio::Square1x1, Kling3p0Resolution::P720, VideoDurationSeconds::new(3));
    let err = kling_3p0(Kling3p0Args { request, auth: &auth, host: &host }).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[test]
  fn enqueue_response_parses() {
    let response: EnqueueJobsResponse = serde_json::from_str(&enqueue_response("kling3_0", 600, SERVER_PARAMS, true)).unwrap();
    let job_set = response.first_job_set().unwrap();
    assert_eq!(job_set.job_set_type, JobSetType::Kling3p0);
    assert_eq!(job_set.params.mode, Some(VideoMode::Std));
    assert_eq!(job_set.params.extra.get("sound"), Some(&Value::String("on".to_string())));
  }

  // ── Live (ignored: needs a real session and spends credits) ──

  #[tokio::test]
  #[ignore]
  async fn live_enqueue_kling_3p0_from_app_credential_and_poll() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_credential_toml::load_higgsfield_session_from_app_credential;
    use crate::test_utils::poll_job_to_completion::poll_job_to_completion;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let session = load_higgsfield_session_from_app_credential()?;
    let auth = session.auth().await.map_err(|err| anyhow::anyhow!("{err}"))?;
    let request = Kling3p0Request::text_to_video("a shiba inu serving a tennis ball", KlingAspectRatio::Landscape16x9, Kling3p0Resolution::P720, Kling3p0Request::DURATION.shortest());
    println!("\n===== request =====\n{:#?}", request);
    let response = kling_3p0(Kling3p0Args { request, auth: &auth, host: &HiggsfieldHost::Higgsfield }).await.map_err(|err| anyhow::anyhow!("{err}"))?;
    println!("\n===== POST {PATH} =====\n{:#?}", response);
    assert_eq!(response.first_job_set().unwrap().job_set_type, JobSetType::Kling3p0);
    let job = poll_job_to_completion(&session, &response.job_ids()[0]).await?;
    assert!(job.result_url().unwrap().ends_with(".mp4"));
    Ok(())
  }
}
