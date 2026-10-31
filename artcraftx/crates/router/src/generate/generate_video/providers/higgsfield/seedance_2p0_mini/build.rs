//! Seedance 2.0 Mini on Higgsfield: Auto / 21:9 / 16:9 / 4:3 / 1:1 / 3:4 /
//! 9:16, 480p / 720p, 4–15s, 1–4 clips, keyframes plus image / video /
//! audio references. No bitrate menu.

use higgsfield_client::endpoints::generate::video::seedance_2p0_mini::{Seedance2p0MiniRequest, Seedance2p0MiniResolution};

use crate::api::router_resolution::RouterResolution;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::generate::generate_video::generate_video_request_builder::GenerateVideoRequestBuilder;
use crate::generate::generate_video::providers::higgsfield::common::{
  finish, plan_batch_size, plan_duration, plan_seedance_aspect_ratio, require_prompt, snap_resolution, warn_ignored,
  HiggsfieldVideoReferences,
};
use crate::generate::generate_video::providers::higgsfield::draft::HiggsfieldVideoPlan;
use crate::generate::generate_video::providers::higgsfield::video_request::HiggsfieldVideoRequest;
use crate::generate::generate_video::video_generation_draft_or_request::VideoGenerationDraftOrRequest;

const MODEL: &str = "Seedance 2.0 Mini";
pub const MAX_REFERENCE_IMAGES: usize = 9;
pub const MAX_REFERENCE_VIDEOS: usize = 3;
pub const MAX_REFERENCE_AUDIO: usize = 3;
const DEFAULT_DURATION_SECONDS: u32 = 5;

const RESOLUTION_TIERS: &[(RouterResolution, Seedance2p0MiniResolution)] = &[
  (RouterResolution::FourEightyP, Seedance2p0MiniResolution::P480),
  (RouterResolution::SevenTwentyP, Seedance2p0MiniResolution::P720),
];

pub fn build_higgsfield_seedance_2p0_mini(mut builder: GenerateVideoRequestBuilder) -> Result<VideoGenerationDraftOrRequest, ArtcraftRouterError> {
  let strategy = builder.request_mismatch_mitigation_strategy;
  let prompt = require_prompt(builder.prompt.take())?;
  warn_ignored(MODEL, "negative prompt", builder.negative_prompt.take());
  warn_ignored(MODEL, "bitrate", builder.bitrate.take());

  let mut references = HiggsfieldVideoReferences::take_from(&mut builder, MODEL);
  references.retain_roles(Seedance2p0MiniRequest::MEDIA_ROLES, MODEL);
  references.check_limits(MAX_REFERENCE_IMAGES, MAX_REFERENCE_VIDEOS, MAX_REFERENCE_AUDIO, MODEL)?;

  let aspect_ratio = plan_seedance_aspect_ratio(builder.aspect_ratio.take(), true);
  let resolution = snap_resolution(builder.resolution.take(), RESOLUTION_TIERS, Seedance2p0MiniResolution::P720, strategy)?;
  let duration = plan_duration(builder.duration_seconds.take(), Seedance2p0MiniRequest::DURATION, DEFAULT_DURATION_SECONDS, strategy)?;

  let mut request = Seedance2p0MiniRequest::text_to_video(prompt, aspect_ratio, resolution, duration);
  request.batch_size = plan_batch_size(builder.video_batch_count.take(), strategy)?;
  request.generate_audio = builder.generate_audio.take().unwrap_or(true);

  Ok(finish(HiggsfieldVideoPlan::Request(HiggsfieldVideoRequest::Seedance2p0Mini(request)), references, true))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::api::router_video_model::RouterVideoModel;
  use crate::client::request_mismatch_mitigation_strategy::RequestMismatchMitigationStrategy;
  use crate::generate::generate_video::providers::higgsfield::common_test_helpers::{base_builder, unwrap_request};

  fn built(builder: GenerateVideoRequestBuilder) -> Seedance2p0MiniRequest {
    match unwrap_request(build_higgsfield_seedance_2p0_mini(builder).expect("build")) {
      HiggsfieldVideoRequest::Seedance2p0Mini(request) => request,
      other => panic!("wrong model: {other:?}"),
    }
  }

  #[test]
  fn tops_out_at_720p() {
    let mut b = base_builder(RouterVideoModel::Seedance2p0Mini);
    b.resolution = Some(RouterResolution::FourK);
    assert_eq!(built(b.clone()).resolution, Seedance2p0MiniResolution::P720);
    b.request_mismatch_mitigation_strategy = RequestMismatchMitigationStrategy::ErrorOut;
    assert!(build_higgsfield_seedance_2p0_mini(b).is_err());
  }

  #[test]
  fn defaults() {
    let request = built(base_builder(RouterVideoModel::Seedance2p0Mini));
    assert_eq!(request.resolution, Seedance2p0MiniResolution::P720);
    assert_eq!(request.duration.seconds(), 5);
    assert!(request.generate_audio);
  }
}
