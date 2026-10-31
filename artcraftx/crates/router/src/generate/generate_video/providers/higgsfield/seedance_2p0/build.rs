//! Seedance 2.0 on Higgsfield: Auto / 21:9 / 16:9 / 4:3 / 1:1 / 3:4 / 9:16,
//! 480p / 720p / 1080p / 4K, 4–15s, 1–4 clips, keyframes plus image /
//! video / audio references.

use higgsfield_client::endpoints::generate::video::seedance_2p0::{Seedance2p0Request, Seedance2p0Resolution};

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

const MODEL: &str = "Seedance 2.0";
pub const MAX_REFERENCE_IMAGES: usize = 9;
pub const MAX_REFERENCE_VIDEOS: usize = 3;
pub const MAX_REFERENCE_AUDIO: usize = 3;
const DEFAULT_DURATION_SECONDS: u32 = 5;

const RESOLUTION_TIERS: &[(RouterResolution, Seedance2p0Resolution)] = &[
  (RouterResolution::FourEightyP, Seedance2p0Resolution::P480),
  (RouterResolution::SevenTwentyP, Seedance2p0Resolution::P720),
  (RouterResolution::TenEightyP, Seedance2p0Resolution::P1080),
  (RouterResolution::FourK, Seedance2p0Resolution::FourK),
];

pub fn build_higgsfield_seedance_2p0(mut builder: GenerateVideoRequestBuilder) -> Result<VideoGenerationDraftOrRequest, ArtcraftRouterError> {
  let strategy = builder.request_mismatch_mitigation_strategy;
  let prompt = require_prompt(builder.prompt.take())?;
  warn_ignored(MODEL, "negative prompt", builder.negative_prompt.take());

  let mut references = HiggsfieldVideoReferences::take_from(&mut builder, MODEL);
  references.retain_roles(Seedance2p0Request::MEDIA_ROLES, MODEL);
  references.check_limits(MAX_REFERENCE_IMAGES, MAX_REFERENCE_VIDEOS, MAX_REFERENCE_AUDIO, MODEL)?;

  let aspect_ratio = plan_seedance_aspect_ratio(builder.aspect_ratio.take(), true);
  let resolution = snap_resolution(builder.resolution.take(), RESOLUTION_TIERS, Seedance2p0Resolution::P720, strategy)?;
  let duration = plan_duration(builder.duration_seconds.take(), Seedance2p0Request::DURATION, DEFAULT_DURATION_SECONDS, strategy)?;

  let mut request = Seedance2p0Request::text_to_video(prompt, aspect_ratio, resolution, duration);
  request.batch_size = plan_batch_size(builder.video_batch_count.take(), strategy)?;
  request.generate_audio = builder.generate_audio.take().unwrap_or(true);
  request.bitrate_mode = plan_bitrate(builder.bitrate.take());

  Ok(finish(HiggsfieldVideoPlan::Request(HiggsfieldVideoRequest::Seedance2p0(request)), references, true))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::api::router_aspect_ratio::RouterAspectRatio;
  use crate::api::router_video_model::RouterVideoModel;
  use crate::client::request_mismatch_mitigation_strategy::RequestMismatchMitigationStrategy;
  use crate::generate::generate_video::providers::higgsfield::common_test_helpers::{base_builder, unwrap_request};
  use higgsfield_client::types::video_aspect_ratio::SeedanceVideoAspectRatio;

  fn built(builder: GenerateVideoRequestBuilder) -> Seedance2p0Request {
    match unwrap_request(build_higgsfield_seedance_2p0(builder).expect("build")) {
      HiggsfieldVideoRequest::Seedance2p0(request) => request,
      other => panic!("wrong model: {other:?}"),
    }
  }

  #[test]
  fn four_k_and_auto_are_offered() {
    let mut b = base_builder(RouterVideoModel::Seedance2p0);
    b.resolution = Some(RouterResolution::FourK);
    b.aspect_ratio = Some(RouterAspectRatio::Auto);
    let request = built(b);
    assert_eq!(request.resolution, Seedance2p0Resolution::FourK);
    assert_eq!(request.aspect_ratio, SeedanceVideoAspectRatio::Auto);
  }

  #[test]
  fn image_tiers_snap_into_the_video_ladder() {
    let mut b = base_builder(RouterVideoModel::Seedance2p0);
    // 1K sits between 720p and 1080p.
    b.resolution = Some(RouterResolution::OneK);
    assert_eq!(built(b.clone()).resolution, Seedance2p0Resolution::P1080);
    b.request_mismatch_mitigation_strategy = RequestMismatchMitigationStrategy::PayLessDowngrade;
    assert_eq!(built(b.clone()).resolution, Seedance2p0Resolution::P720);
    // 2K sits between 1080p and 4K.
    b.resolution = Some(RouterResolution::TwoK);
    b.request_mismatch_mitigation_strategy = RequestMismatchMitigationStrategy::PayMoreUpgrade;
    assert_eq!(built(b).resolution, Seedance2p0Resolution::FourK);
  }

  #[test]
  fn durations_clamp_to_4_to_15() {
    let mut b = base_builder(RouterVideoModel::Seedance2p0);
    b.duration_seconds = Some(30);
    assert_eq!(built(b).duration.seconds(), 15);
  }
}
