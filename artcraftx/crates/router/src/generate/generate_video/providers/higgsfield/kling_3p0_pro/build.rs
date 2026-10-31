//! Kling 3.0 Pro on Higgsfield: the "pro" (1080p) and "4K" modes of the
//! Kling 3.0 endpoint — 16:9 / 9:16 / 1:1, 3–15s, one clip, start and end
//! frames only.

use higgsfield_client::endpoints::generate::video::kling_3p0::Kling3p0Resolution;

use crate::api::router_resolution::RouterResolution;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::generate::generate_video::generate_video_request_builder::GenerateVideoRequestBuilder;
use crate::generate::generate_video::providers::higgsfield::kling_3p0_standard::build::build_kling_3p0;
use crate::generate::generate_video::video_generation_draft_or_request::VideoGenerationDraftOrRequest;

const MODEL: &str = "Kling 3.0 Pro";

const RESOLUTION_TIERS: &[(RouterResolution, Kling3p0Resolution)] = &[
  (RouterResolution::TenEightyP, Kling3p0Resolution::P1080),
  (RouterResolution::FourK, Kling3p0Resolution::FourK),
];

pub fn build_higgsfield_kling_3p0_pro(builder: GenerateVideoRequestBuilder) -> Result<VideoGenerationDraftOrRequest, ArtcraftRouterError> {
  build_kling_3p0(builder, MODEL, RESOLUTION_TIERS, Kling3p0Resolution::P1080)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::api::router_video_model::RouterVideoModel;
  use crate::client::request_mismatch_mitigation_strategy::RequestMismatchMitigationStrategy;
  use crate::generate::generate_video::providers::higgsfield::common_test_helpers::{base_builder, unwrap_request};
  use crate::generate::generate_video::providers::higgsfield::video_request::HiggsfieldVideoRequest;
  use higgsfield_client::endpoints::generate::video::kling_3p0::Kling3p0Request;
  use higgsfield_client::types::video_mode::VideoMode;

  fn built(builder: GenerateVideoRequestBuilder) -> Kling3p0Request {
    match unwrap_request(build_higgsfield_kling_3p0_pro(builder).expect("build")) {
      HiggsfieldVideoRequest::Kling3p0(request) => request,
      other => panic!("wrong model: {other:?}"),
    }
  }

  #[test]
  fn pro_is_1080p_or_4k() {
    let mut b = base_builder(RouterVideoModel::Kling3p0Pro);
    assert_eq!(built(b.clone()).resolution, Kling3p0Resolution::P1080);
    b.resolution = Some(RouterResolution::FourK);
    assert_eq!(built(b.clone()).resolution, Kling3p0Resolution::FourK);
    assert_eq!(built(b.clone()).resolution.to_video_mode(), VideoMode::FourK);
    // 720p is the Standard tier; Pro upgrades it to 1080p.
    b.resolution = Some(RouterResolution::SevenTwentyP);
    assert_eq!(built(b.clone()).resolution, Kling3p0Resolution::P1080);
    b.request_mismatch_mitigation_strategy = RequestMismatchMitigationStrategy::PayLessDowngrade;
    assert_eq!(built(b.clone()).resolution, Kling3p0Resolution::P1080);
    b.request_mismatch_mitigation_strategy = RequestMismatchMitigationStrategy::ErrorOut;
    assert!(build_higgsfield_kling_3p0_pro(b).is_err());
  }
}
