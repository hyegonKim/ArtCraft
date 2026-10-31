//! MiniMax H3 on Higgsfield: a fixed 2K output that follows its references
//! (no aspect-ratio or resolution controls), 5–15s, one clip, keyframes
//! plus image / video / audio references.

use higgsfield_client::endpoints::generate::video::minimax_h3::MinimaxH3Request;

use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::generate::generate_video::generate_video_request_builder::GenerateVideoRequestBuilder;
use crate::generate::generate_video::providers::higgsfield::common::{
  finish, plan_duration, plan_single_video, require_prompt, warn_ignored, HiggsfieldVideoReferences,
};
use crate::generate::generate_video::providers::higgsfield::draft::HiggsfieldVideoPlan;
use crate::generate::generate_video::providers::higgsfield::video_request::HiggsfieldVideoRequest;
use crate::generate::generate_video::video_generation_draft_or_request::VideoGenerationDraftOrRequest;

const MODEL: &str = "MiniMax H3";
pub const MAX_REFERENCE_IMAGES: usize = 9;
pub const MAX_REFERENCE_VIDEOS: usize = 3;
pub const MAX_REFERENCE_AUDIO: usize = 3;
const DEFAULT_DURATION_SECONDS: u32 = 5;

pub fn build_higgsfield_minimax_h3(mut builder: GenerateVideoRequestBuilder) -> Result<VideoGenerationDraftOrRequest, ArtcraftRouterError> {
  let strategy = builder.request_mismatch_mitigation_strategy;
  let prompt = require_prompt(builder.prompt.take())?;
  warn_ignored(MODEL, "negative prompt", builder.negative_prompt.take());
  warn_ignored(MODEL, "aspect ratio", builder.aspect_ratio.take());
  warn_ignored(MODEL, "resolution", builder.resolution.take());
  warn_ignored(MODEL, "bitrate", builder.bitrate.take());
  warn_ignored(MODEL, "audio toggle", builder.generate_audio.take());
  plan_single_video(builder.video_batch_count.take(), strategy, MODEL)?;

  let mut references = HiggsfieldVideoReferences::take_from(&mut builder, MODEL);
  references.retain_roles(MinimaxH3Request::MEDIA_ROLES, MODEL);
  references.check_limits(MAX_REFERENCE_IMAGES, MAX_REFERENCE_VIDEOS, MAX_REFERENCE_AUDIO, MODEL)?;

  let duration = plan_duration(builder.duration_seconds.take(), MinimaxH3Request::DURATION, DEFAULT_DURATION_SECONDS, strategy)?;
  let request = MinimaxH3Request::text_to_video(prompt, duration);

  Ok(finish(HiggsfieldVideoPlan::Request(HiggsfieldVideoRequest::MinimaxH3(request)), references, false))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::api::router_resolution::RouterResolution;
  use crate::api::router_video_model::RouterVideoModel;
  use crate::client::request_mismatch_mitigation_strategy::RequestMismatchMitigationStrategy;
  use crate::generate::generate_video::providers::higgsfield::common_test_helpers::{base_builder, unwrap_request};

  fn built(builder: GenerateVideoRequestBuilder) -> MinimaxH3Request {
    match unwrap_request(build_higgsfield_minimax_h3(builder).expect("build")) {
      HiggsfieldVideoRequest::MinimaxH3(request) => request,
      other => panic!("wrong model: {other:?}"),
    }
  }

  #[test]
  fn resolution_and_aspect_are_ignored_and_durations_clamp() {
    let mut b = base_builder(RouterVideoModel::MinimaxH3);
    b.resolution = Some(RouterResolution::FourK);
    b.duration_seconds = Some(3);
    assert_eq!(built(b).duration.seconds(), 5);
  }

  #[test]
  fn batches_are_single() {
    let mut b = base_builder(RouterVideoModel::MinimaxH3);
    b.video_batch_count = Some(3);
    assert!(build_higgsfield_minimax_h3(b.clone()).is_ok());
    b.request_mismatch_mitigation_strategy = RequestMismatchMitigationStrategy::ErrorOut;
    assert!(build_higgsfield_minimax_h3(b).is_err());
  }
}
