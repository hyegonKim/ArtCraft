//! Seedance 2.5 on Higgsfield: 21:9 / 16:9 / 4:3 / 1:1 / 3:4 / 9:16 (no
//! Auto), 480p / 720p / 1080p, 4–30s, 1–4 clips, keyframes plus image /
//! video / audio references.

use higgsfield_client::endpoints::generate::video::seedance_2p5::{Seedance2p5Request, Seedance2p5Resolution};

use crate::api::router_resolution::RouterResolution;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::generate::generate_video::generate_video_request_builder::GenerateVideoRequestBuilder;
use crate::generate::generate_video::providers::higgsfield::common::{
  finish, plan_batch_size, plan_bitrate, plan_duration, plan_seedance_aspect_ratio, require_prompt, snap_resolution,
  warn_ignored, HiggsfieldVideoReferences,
};
use crate::generate::generate_video::providers::higgsfield::draft::HiggsfieldVideoPlan;
use crate::generate::generate_video::providers::higgsfield::video_request::HiggsfieldVideoRequest;
use crate::generate::generate_video::video_generation_draft_or_request::VideoGenerationDraftOrRequest;

const MODEL: &str = "Seedance 2.5";
pub const MAX_REFERENCE_IMAGES: usize = 9;
pub const MAX_REFERENCE_VIDEOS: usize = 3;
pub const MAX_REFERENCE_AUDIO: usize = 3;
const DEFAULT_DURATION_SECONDS: u32 = 5;

const RESOLUTION_TIERS: &[(RouterResolution, Seedance2p5Resolution)] = &[
  (RouterResolution::FourEightyP, Seedance2p5Resolution::P480),
  (RouterResolution::SevenTwentyP, Seedance2p5Resolution::P720),
  (RouterResolution::TenEightyP, Seedance2p5Resolution::P1080),
];

pub fn build_higgsfield_seedance_2p5(mut builder: GenerateVideoRequestBuilder) -> Result<VideoGenerationDraftOrRequest, ArtcraftRouterError> {
  let strategy = builder.request_mismatch_mitigation_strategy;
  let prompt = require_prompt(builder.prompt.take())?;
  warn_ignored(MODEL, "negative prompt", builder.negative_prompt.take());

  let mut references = HiggsfieldVideoReferences::take_from(&mut builder, MODEL);
  references.retain_roles(Seedance2p5Request::MEDIA_ROLES, MODEL);
  references.check_limits(MAX_REFERENCE_IMAGES, MAX_REFERENCE_VIDEOS, MAX_REFERENCE_AUDIO, MODEL)?;

  let aspect_ratio = plan_seedance_aspect_ratio(builder.aspect_ratio.take(), false);
  let resolution = snap_resolution(builder.resolution.take(), RESOLUTION_TIERS, Seedance2p5Resolution::P720, strategy)?;
  let duration = plan_duration(builder.duration_seconds.take(), Seedance2p5Request::DURATION, DEFAULT_DURATION_SECONDS, strategy)?;

  let mut request = Seedance2p5Request::text_to_video(prompt, aspect_ratio, resolution, duration);
  request.batch_size = plan_batch_size(builder.video_batch_count.take(), strategy)?;
  request.generate_audio = builder.generate_audio.take().unwrap_or(true);
  request.bitrate_mode = plan_bitrate(builder.bitrate.take());

  Ok(finish(HiggsfieldVideoPlan::Request(HiggsfieldVideoRequest::Seedance2p5(request)), references, true))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::api::router_aspect_ratio::RouterAspectRatio;
  use crate::api::router_bitrate::RouterBitrate;
  use crate::api::router_video_model::RouterVideoModel;
  use crate::client::request_mismatch_mitigation_strategy::RequestMismatchMitigationStrategy;
  use crate::generate::generate_video::providers::higgsfield::common_test_helpers::{base_builder, unwrap_draft_request, unwrap_request, with_reference_images, with_start_frame};
  use higgsfield_client::types::image_batch_size::ImageBatchSize;
  use higgsfield_client::types::video_aspect_ratio::SeedanceVideoAspectRatio;
  use higgsfield_client::types::video_bitrate_mode::VideoBitrateMode;

  fn builder() -> GenerateVideoRequestBuilder {
    base_builder(RouterVideoModel::Seedance2p5)
  }

  fn built(builder: GenerateVideoRequestBuilder) -> Seedance2p5Request {
    match unwrap_request(build_higgsfield_seedance_2p5(builder).expect("build")) {
      HiggsfieldVideoRequest::Seedance2p5(request) => request,
      other => panic!("wrong model: {other:?}"),
    }
  }

  #[test]
  fn defaults_are_the_web_apps() {
    let request = built(builder());
    assert_eq!(request.aspect_ratio, SeedanceVideoAspectRatio::Landscape16x9);
    assert_eq!(request.resolution, Seedance2p5Resolution::P720);
    assert_eq!(request.duration.seconds(), 5);
    assert_eq!(request.batch_size, ImageBatchSize::One);
    assert!(request.generate_audio);
    assert_eq!(request.bitrate_mode, VideoBitrateMode::High);
    assert!(request.medias.is_empty());
  }

  #[test]
  fn options_flow_through() {
    let mut b = builder();
    b.aspect_ratio = Some(RouterAspectRatio::TallNineBySixteen);
    b.resolution = Some(RouterResolution::TenEightyP);
    b.duration_seconds = Some(30);
    b.video_batch_count = Some(2);
    b.generate_audio = Some(false);
    b.bitrate = Some(RouterBitrate::Normal);
    let request = built(b);
    assert_eq!(request.aspect_ratio, SeedanceVideoAspectRatio::Portrait9x16);
    assert_eq!(request.resolution, Seedance2p5Resolution::P1080);
    assert_eq!(request.duration.seconds(), 30);
    assert_eq!(request.batch_size, ImageBatchSize::Two);
    assert!(!request.generate_audio);
    assert_eq!(request.bitrate_mode, VideoBitrateMode::Standard);
  }

  #[test]
  fn four_k_is_not_offered_and_snaps_to_1080p() {
    let mut b = builder();
    b.resolution = Some(RouterResolution::FourK);
    assert_eq!(built(b.clone()).resolution, Seedance2p5Resolution::P1080);
    b.request_mismatch_mitigation_strategy = RequestMismatchMitigationStrategy::ErrorOut;
    assert!(build_higgsfield_seedance_2p5(b).is_err());
  }

  #[test]
  fn durations_clamp_to_4_to_30() {
    let mut b = builder();
    b.duration_seconds = Some(45);
    assert_eq!(built(b.clone()).duration.seconds(), 30);
    b.duration_seconds = Some(2);
    assert_eq!(built(b).duration.seconds(), 4);
  }

  #[test]
  fn auto_aspect_becomes_16x9_because_2p5_has_no_auto() {
    let mut b = builder();
    b.aspect_ratio = Some(RouterAspectRatio::Auto);
    assert_eq!(built(b).aspect_ratio, SeedanceVideoAspectRatio::Landscape16x9);
  }

  #[test]
  fn media_makes_a_draft_that_runs_the_ip_check() {
    let (request, draft) = unwrap_draft_request(build_higgsfield_seedance_2p5(with_start_frame(builder())).expect("build"));
    assert!(draft.ip_check);
    assert!(request.medias().is_empty(), "media is attached at finalize");
    assert!(draft.unhandled_request_state.as_ref().unwrap().start_frame.is_some());
  }

  #[test]
  fn too_many_references_are_rejected_before_upload() {
    assert!(build_higgsfield_seedance_2p5(with_reference_images(builder(), MAX_REFERENCE_IMAGES + 1)).is_err());
    assert!(build_higgsfield_seedance_2p5(with_reference_images(builder(), MAX_REFERENCE_IMAGES)).is_ok());
  }
}
