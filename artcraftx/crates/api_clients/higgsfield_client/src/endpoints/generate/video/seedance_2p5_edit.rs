//! POST `/fnf/jobs/v2/seedance_2_5` with `model: "video_edit"` — the web
//! app's "Seedance 2.5 Edit": rewrite an existing clip (≤30 s) from a prompt
//! plus optional image / audio references. Same job set type as Seedance
//! 2.5 (`seedance_2_5`); the web app's picker labels the variant
//! `seedance_2_5_edit` but posts it to the base pipeline.
//!
//! Shape from the web app's bundle (2026-08-31): the edit body is the
//! Seedance 2.5 body minus `duration` and `aspect_ratio` (the clip decides
//! both), with the source clip in `medias` as role `video`. The Edit tab's
//! upload picker is gated by a media-upload agreement, so this was not
//! browser-captured; the live test below is the verification.

use crate::client::higgsfield_host::HiggsfieldHost;
use crate::client::send_request::{send_json_request, HttpMethod};
use crate::credentials::higgsfield_auth::HiggsfieldAuth;
use crate::endpoints::generate::video::seedance_2p5::Seedance2p5Resolution;
use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::types::enqueue_jobs_response::EnqueueJobsResponse;
use crate::types::image_aspect_ratio::ImageAspectRatio;
use crate::types::image_batch_size::ImageBatchSize;
use crate::types::media_input::MediaInput;
use crate::types::media_reference::{validate_media_roles, MediaReference};
use crate::types::media_role::MediaRole;
use crate::types::video_bitrate_mode::VideoBitrateMode;
use crate::types::video_dimensions::VideoDimensions;
use serde::Serialize;

const PATH: &str = "/fnf/jobs/v2/seedance_2_5";

/// The `model` field that selects the edit pipeline (the web app's
/// `videoEdit` constant; text-to-video sends `default`).
const MODEL: &str = "video_edit";

pub struct Seedance2p5EditArgs<'a> {
  pub request: Seedance2p5EditRequest,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

/// The material part of a Seedance 2.5 Edit request.
#[derive(Clone, Debug, Serialize)]
pub struct Seedance2p5EditRequest {
  /// What to change.
  pub prompt: String,

  /// The clip to edit (≤30 s): an uploaded `video_input` or a previous
  /// generation as `video_job`. Sent first in `medias` with role `video`.
  pub source_video: MediaInput,

  pub resolution: Seedance2p5Resolution,

  /// Image / audio references ("elements or references", up to 50). Roles
  /// this model takes: [`Self::MEDIA_ROLES`].
  pub references: Vec<MediaReference>,

  /// How many edits to generate (1–4). Each costs credits.
  pub batch_size: ImageBatchSize,

  /// Generate a soundtrack (the web app's audio "On").
  pub generate_audio: bool,

  pub bitrate_mode: VideoBitrateMode,

  /// Spend from the plan's "unlimited" pool instead of credits, if the plan
  /// has one.
  pub use_unlim: bool,

  /// The pixel size sent with the request. The web app derives it from the
  /// clip; with `None` a 16:9 frame at `resolution` is sent, which the
  /// server treats as advisory.
  pub maybe_dimensions: Option<VideoDimensions>,
}

impl Seedance2p5EditRequest {
  /// Reference roles the edit form takes ("up to 50 image or audio").
  pub const MEDIA_ROLES: &'static [MediaRole] = &[MediaRole::Image, MediaRole::Audio];

  /// An edit with the web app's defaults (1 clip, audio on, high bitrate,
  /// credits).
  pub fn new(prompt: impl Into<String>, source_video: MediaInput, resolution: Seedance2p5Resolution) -> Self {
    Self {
      prompt: prompt.into(),
      source_video,
      resolution,
      references: Vec::new(),
      batch_size: ImageBatchSize::One,
      generate_audio: true,
      bitrate_mode: VideoBitrateMode::High,
      use_unlim: false,
      maybe_dimensions: None,
    }
  }

  /// Add one image / audio reference.
  pub fn with_reference(mut self, reference: MediaReference) -> Self {
    self.references.push(reference);
    self
  }

  fn validate(&self) -> Result<(), HiggsfieldClientError> {
    if self.prompt.trim().is_empty() {
      return Err(HiggsfieldClientError::InvalidRequest("prompt is empty".to_string()));
    }
    validate_media_roles(&self.references, Self::MEDIA_ROLES, "Seedance 2.5 Edit")
  }

  fn dimensions(&self) -> VideoDimensions {
    self.maybe_dimensions
        .or_else(|| VideoDimensions::for_aspect_ratio(&ImageAspectRatio::Landscape16x9, &self.resolution.to_video_resolution()))
        .expect("16:9 dimensions are derivable for every known resolution")
  }

  fn to_body(&self) -> Seedance2p5EditRequestBody {
    let dimensions = self.dimensions();
    let mut medias = Vec::with_capacity(self.references.len() + 1);
    medias.push(MediaReference::video(self.source_video.clone()));
    medias.extend(self.references.iter().cloned());
    Seedance2p5EditRequestBody {
      params: Seedance2p5EditParams {
        prompt: self.prompt.clone(),
        width: dimensions.width,
        height: dimensions.height,
        medias,
        resolution: self.resolution,
        generate_audio: self.generate_audio,
        bitrate_mode: self.bitrate_mode.clone(),
        batch_size: self.batch_size,
        model: MODEL,
        use_unlim: self.use_unlim,
      },
      use_unlim: self.use_unlim,
    }
  }
}

/// Enqueue the edit. The response's job ids are what to poll (see
/// `endpoints::jobs`); a finished job's `results.raw.url` is the `.mp4`.
pub async fn seedance_2p5_edit(args: Seedance2p5EditArgs<'_>) -> Result<EnqueueJobsResponse, HiggsfieldError> {
  args.request.validate()?;
  let body = args.request.to_body();
  send_json_request(HttpMethod::Post, PATH, args.auth, args.host, Some(&body)).await
}

// ── Wire format ──

#[derive(Serialize)]
struct Seedance2p5EditRequestBody {
  params: Seedance2p5EditParams,
  use_unlim: bool,
}

/// The Seedance 2.5 params without `duration` / `aspect_ratio`.
#[derive(Serialize)]
struct Seedance2p5EditParams {
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
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::Value;

  fn source() -> MediaInput {
    MediaInput::uploaded_video("00000000-0000-4000-8000-0000000000cc", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000cc.mp4")
  }

  #[test]
  fn wire_body_matches_the_bundles_edit_shape() {
    let request = Seedance2p5EditRequest::new("make it snow", source(), Seedance2p5Resolution::P480)
        .with_reference(MediaReference::image(MediaInput::uploaded("00000000-0000-4000-8000-0000000000aa", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png")));
    let actual: Value = serde_json::to_value(request.to_body()).unwrap();
    let expected: Value = serde_json::from_str(r#"{"params":{"prompt":"make it snow","width":854,"height":480,"medias":[{"role":"video","data":{"id":"00000000-0000-4000-8000-0000000000cc","type":"video_input","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000cc.mp4"}},{"role":"image","data":{"id":"00000000-0000-4000-8000-0000000000aa","type":"media_input","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png"}}],"resolution":"480p","generate_audio":true,"bitrate_mode":"high","batch_size":1,"model":"video_edit","use_unlim":false},"use_unlim":false}"#).unwrap();
    assert_eq!(actual, expected);
    assert!(actual["params"].get("duration").is_none());
    assert!(actual["params"].get("aspect_ratio").is_none());
  }

  #[test]
  fn validation() {
    assert!(matches!(Seedance2p5EditRequest::new(" ", source(), Seedance2p5Resolution::P480).validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
    let frame = Seedance2p5EditRequest::new("p", source(), Seedance2p5Resolution::P480)
        .with_reference(MediaReference::start_frame(MediaInput::uploaded("a", "https://cdn.example.com/a.png")));
    assert!(matches!(frame.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
    assert!(Seedance2p5EditRequest::new("p", source(), Seedance2p5Resolution::P480).validate().is_ok());
  }

  #[tokio::test]
  async fn invalid_request_fails_before_any_http() {
    let auth = HiggsfieldAuth::new("token");
    let err = seedance_2p5_edit(Seedance2p5EditArgs {
      request: Seedance2p5EditRequest::new("", source(), Seedance2p5Resolution::P480),
      auth: &auth,
      host: &HiggsfieldHost::Custom("http://127.0.0.1:9".into()),
    }).await.unwrap_err();
    assert!(matches!(err, HiggsfieldError::Client(HiggsfieldClientError::InvalidRequest(_))));
  }
}
