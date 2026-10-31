//! Seedream 5 Lite on Higgsfield: 8 aspect ratios (no Auto), a 2K / 3K / 4K
//! menu, 1–4 images. ArtCraft's baked-in `auto_2k` / `auto_3k` ratios become
//! a 16:9 request at that resolution.

use higgsfield_client::endpoints::generate::image::seedream_5p0_lite::{Seedream5p0LiteRequest, Seedream5p0LiteResolution};
use higgsfield_client::types::seedream_aspect_ratio::SeedreamAspectRatio;

use crate::api::router_resolution::RouterResolution;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::generate::generate_image::generate_image_request_builder::GenerateImageRequestBuilder;
use crate::generate::generate_image::image_generation_draft_or_request::ImageGenerationDraftOrRequest;
use crate::generate::generate_image::providers::higgsfield::common::{
  check_reference_limit, finish, image_input_count, plan_batch_size, plan_seedream_aspect_ratio, require_prompt,
  seedream_auto_resolution_hint, snap_resolution,
};
use crate::generate::generate_image::providers::higgsfield::image_request::HiggsfieldImageRequest;

pub const MAX_REFERENCE_IMAGES: usize = 6;

const RESOLUTION_TIERS: &[(RouterResolution, Seedream5p0LiteResolution)] = &[
  (RouterResolution::TwoK, Seedream5p0LiteResolution::TwoK),
  (RouterResolution::ThreeK, Seedream5p0LiteResolution::ThreeK),
  (RouterResolution::FourK, Seedream5p0LiteResolution::FourK),
];

pub fn build_higgsfield_seedream_5p0_lite(mut builder: GenerateImageRequestBuilder) -> Result<ImageGenerationDraftOrRequest, ArtcraftRouterError> {
  let strategy = builder.request_mismatch_mitigation_strategy;
  let prompt = require_prompt(builder.prompt.take())?;
  let image_inputs = builder.image_inputs.take();
  check_reference_limit(image_input_count(image_inputs.as_ref()), MAX_REFERENCE_IMAGES, "Seedream 5 Lite")?;

  let requested_aspect_ratio = builder.aspect_ratio.take();
  let aspect_ratio = plan_seedream_aspect_ratio(requested_aspect_ratio, SeedreamAspectRatio::Landscape16x9);
  let requested_resolution = builder.resolution.take().or_else(|| seedream_auto_resolution_hint(requested_aspect_ratio));
  let resolution = snap_resolution(requested_resolution, RESOLUTION_TIERS, Seedream5p0LiteResolution::TwoK, strategy)?;
  let batch_size = plan_batch_size(builder.image_batch_count.take(), strategy)?;

  let mut request = Seedream5p0LiteRequest::text_to_image(prompt, aspect_ratio, resolution);
  request.batch_size = batch_size;
  Ok(finish(HiggsfieldImageRequest::Seedream5p0Lite(request), image_inputs))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::api::router_aspect_ratio::RouterAspectRatio;
  use crate::api::router_image_model::RouterImageModel;
  use crate::client::request_mismatch_mitigation_strategy::RequestMismatchMitigationStrategy;
  use crate::generate::generate_image::providers::higgsfield::common_test_helpers::{base_builder, unwrap_request};

  fn built(builder: GenerateImageRequestBuilder) -> Seedream5p0LiteRequest {
    match unwrap_request(build_higgsfield_seedream_5p0_lite(builder).expect("build")) {
      HiggsfieldImageRequest::Seedream5p0Lite(request) => request,
      other => panic!("wrong model: {other:?}"),
    }
  }

  #[test]
  fn auto_3k_becomes_16x9_at_3k() {
    let mut b = base_builder(RouterImageModel::Seedream5Lite);
    b.aspect_ratio = Some(RouterAspectRatio::Auto3k);
    let request = built(b);
    assert_eq!(request.aspect_ratio, SeedreamAspectRatio::Landscape16x9);
    assert_eq!(request.resolution, Seedream5p0LiteResolution::ThreeK);
  }

  #[test]
  fn an_explicit_resolution_beats_the_auto_hint() {
    let mut b = base_builder(RouterImageModel::Seedream5Lite);
    b.aspect_ratio = Some(RouterAspectRatio::Auto2k);
    b.resolution = Some(RouterResolution::FourK);
    assert_eq!(built(b).resolution, Seedream5p0LiteResolution::FourK);
  }

  #[test]
  fn one_k_upgrades_to_two_k() {
    let mut b = base_builder(RouterImageModel::Seedream5Lite);
    b.resolution = Some(RouterResolution::OneK);
    assert_eq!(built(b.clone()).resolution, Seedream5p0LiteResolution::TwoK);
    b.request_mismatch_mitigation_strategy = RequestMismatchMitigationStrategy::PayLessDowngrade;
    // Nothing below 2K exists; the bottom tier is still 2K.
    assert_eq!(built(b).resolution, Seedream5p0LiteResolution::TwoK);
  }

  #[test]
  fn square_hd_is_square() {
    let mut b = base_builder(RouterImageModel::Seedream5Lite);
    b.aspect_ratio = Some(RouterAspectRatio::SquareHd);
    assert_eq!(built(b).aspect_ratio, SeedreamAspectRatio::Square1x1);
  }
}
