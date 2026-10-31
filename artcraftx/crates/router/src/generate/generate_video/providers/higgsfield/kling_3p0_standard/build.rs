//! Kling 3.0 Standard on Higgsfield: the "std" mode of the Kling 3.0
//! endpoint — 720p, 16:9 / 9:16 / 1:1, 3–15s, one clip, start and end
//! frames only (no other references).

use higgsfield_client::endpoints::generate::video::kling_3p0::{Kling3p0Request, Kling3p0Resolution};

use crate::api::router_resolution::RouterResolution;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::generate::generate_video::generate_video_request_builder::GenerateVideoRequestBuilder;
use crate::generate::generate_video::providers::higgsfield::common::{
  finish, plan_duration, plan_kling_aspect_ratio, plan_single_video, require_prompt, snap_resolution, warn_ignored,
  HiggsfieldVideoReferences,
};
use crate::generate::generate_video::providers::higgsfield::draft::HiggsfieldVideoPlan;
use crate::generate::generate_video::providers::higgsfield::video_request::HiggsfieldVideoRequest;
use crate::generate::generate_video::video_generation_draft_or_request::VideoGenerationDraftOrRequest;

const MODEL: &str = "Kling 3.0 Standard";
const DEFAULT_DURATION_SECONDS: u32 = 5;

/// Standard is the 720p tier; the other modes belong to Kling 3.0 Pro.
const RESOLUTION_TIERS: &[(RouterResolution, Kling3p0Resolution)] = &[
  (RouterResolution::SevenTwentyP, Kling3p0Resolution::P720),
];

pub fn build_higgsfield_kling_3p0_standard(builder: GenerateVideoRequestBuilder) -> Result<VideoGenerationDraftOrRequest, ArtcraftRouterError> {
  build_kling_3p0(builder, MODEL, RESOLUTION_TIERS, Kling3p0Resolution::P720)
}

/// Shared by Standard and Pro: only the resolution tiers differ.
pub(crate) fn build_kling_3p0(
  mut builder: GenerateVideoRequestBuilder,
  model: &str,
  resolution_tiers: &[(RouterResolution, Kling3p0Resolution)],
  default_resolution: Kling3p0Resolution,
) -> Result<VideoGenerationDraftOrRequest, ArtcraftRouterError> {
  let strategy = builder.request_mismatch_mitigation_strategy;
  let prompt = require_prompt(builder.prompt.take())?;
  warn_ignored(model, "negative prompt", builder.negative_prompt.take());
  warn_ignored(model, "bitrate", builder.bitrate.take());
  plan_single_video(builder.video_batch_count.take(), strategy, model)?;

  let mut references = HiggsfieldVideoReferences::take_from(&mut builder, model);
  references.retain_roles(Kling3p0Request::MEDIA_ROLES, model);

  let aspect_ratio = plan_kling_aspect_ratio(builder.aspect_ratio.take());
  let resolution = snap_resolution(builder.resolution.take(), resolution_tiers, default_resolution, strategy)?;
  let duration = plan_duration(builder.duration_seconds.take(), Kling3p0Request::DURATION, DEFAULT_DURATION_SECONDS, strategy)?;

  let mut request = Kling3p0Request::text_to_video(prompt, aspect_ratio, resolution, duration);
  request.sound = builder.generate_audio.take().unwrap_or(true);

  Ok(finish(HiggsfieldVideoPlan::Request(HiggsfieldVideoRequest::Kling3p0(request)), references, false))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::api::router_aspect_ratio::RouterAspectRatio;
  use crate::api::router_video_model::RouterVideoModel;
  use crate::client::request_mismatch_mitigation_strategy::RequestMismatchMitigationStrategy;
  use crate::generate::generate_video::providers::higgsfield::common_test_helpers::{base_builder, unwrap_draft, unwrap_request, with_reference_images, with_start_frame};
  use higgsfield_client::types::video_aspect_ratio::KlingAspectRatio;

  fn built(builder: GenerateVideoRequestBuilder) -> Kling3p0Request {
    match unwrap_request(build_higgsfield_kling_3p0_standard(builder).expect("build")) {
      HiggsfieldVideoRequest::Kling3p0(request) => request,
      other => panic!("wrong model: {other:?}"),
    }
  }

  #[test]
  fn standard_is_always_720p() {
    let mut b = base_builder(RouterVideoModel::Kling3p0Standard);
    assert_eq!(built(b.clone()).resolution, Kling3p0Resolution::P720);
    b.resolution = Some(RouterResolution::FourK);
    assert_eq!(built(b.clone()).resolution, Kling3p0Resolution::P720);
    b.request_mismatch_mitigation_strategy = RequestMismatchMitigationStrategy::ErrorOut;
    assert!(build_higgsfield_kling_3p0_standard(b).is_err());
  }

  #[test]
  fn options_flow_through() {
    let mut b = base_builder(RouterVideoModel::Kling3p0Standard);
    b.aspect_ratio = Some(RouterAspectRatio::TallThreeByFour);
    b.duration_seconds = Some(12);
    b.generate_audio = Some(false);
    let request = built(b);
    assert_eq!(request.aspect_ratio, KlingAspectRatio::Portrait9x16);
    assert_eq!(request.duration.seconds(), 12);
    assert!(!request.sound);
    assert!(request.enhance_prompt);
  }

  #[test]
  fn keyframes_draft_without_ip_check_and_reference_images_are_dropped() {
    let draft = unwrap_draft(build_higgsfield_kling_3p0_standard(with_start_frame(base_builder(RouterVideoModel::Kling3p0Standard))).expect("build"));
    assert!(!draft.ip_check);
    assert!(draft.unhandled_request_state.unwrap().start_frame.is_some());
    // Reference images aren't a Kling input; with nothing else attached the
    // request goes straight out.
    let result = build_higgsfield_kling_3p0_standard(with_reference_images(base_builder(RouterVideoModel::Kling3p0Standard), 2)).expect("build");
    assert!(matches!(result, VideoGenerationDraftOrRequest::Request(_)));
  }
}
