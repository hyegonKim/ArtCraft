//! Seedream 5.0 Pro on Higgsfield: 8 aspect ratios (no Auto), 1K / 1.5K /
//! 2K, 1–4 images. The router has no 1.5K tier, so it offers 1K and 2K.

use higgsfield_client::endpoints::generate::image::seedream_5p0_pro::{Seedream5p0ProRequest, Seedream5p0ProResolution};
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

pub const MAX_REFERENCE_IMAGES: usize = 14;

const RESOLUTION_TIERS: &[(RouterResolution, Seedream5p0ProResolution)] = &[
  (RouterResolution::OneK, Seedream5p0ProResolution::OneK),
  (RouterResolution::TwoK, Seedream5p0ProResolution::TwoK),
];

pub fn build_higgsfield_seedream_5p0_pro(mut builder: GenerateImageRequestBuilder) -> Result<ImageGenerationDraftOrRequest, ArtcraftRouterError> {
  let strategy = builder.request_mismatch_mitigation_strategy;
  let prompt = require_prompt(builder.prompt.take())?;
  let image_inputs = builder.image_inputs.take();
  check_reference_limit(image_input_count(image_inputs.as_ref()), MAX_REFERENCE_IMAGES, "Seedream 5.0 Pro")?;

  let requested_aspect_ratio = builder.aspect_ratio.take();
  let aspect_ratio = plan_seedream_aspect_ratio(requested_aspect_ratio, SeedreamAspectRatio::Square1x1);
  let requested_resolution = builder.resolution.take().or_else(|| seedream_auto_resolution_hint(requested_aspect_ratio));
  let resolution = snap_resolution(requested_resolution, RESOLUTION_TIERS, Seedream5p0ProResolution::OneK, strategy)?;
  let batch_size = plan_batch_size(builder.image_batch_count.take(), strategy)?;

  let mut request = Seedream5p0ProRequest::text_to_image(prompt, aspect_ratio, resolution);
  request.batch_size = batch_size;
  Ok(finish(HiggsfieldImageRequest::Seedream5p0Pro(request), image_inputs))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::api::router_aspect_ratio::RouterAspectRatio;
  use crate::api::router_image_model::RouterImageModel;
  use crate::client::request_mismatch_mitigation_strategy::RequestMismatchMitigationStrategy;
  use crate::generate::generate_image::providers::higgsfield::common_test_helpers::{base_builder, unwrap_request};
  use higgsfield_client::types::image_batch_size::ImageBatchSize;

  fn built(builder: GenerateImageRequestBuilder) -> Seedream5p0ProRequest {
    match unwrap_request(build_higgsfield_seedream_5p0_pro(builder).expect("build")) {
      HiggsfieldImageRequest::Seedream5p0Pro(request) => request,
      other => panic!("wrong model: {other:?}"),
    }
  }

  #[test]
  fn defaults_to_square_at_one_k() {
    let request = built(base_builder(RouterImageModel::Seedream5p0Pro));
    assert_eq!(request.aspect_ratio, SeedreamAspectRatio::Square1x1);
    assert_eq!(request.resolution, Seedream5p0ProResolution::OneK);
  }

  #[test]
  fn four_k_downgrades_to_two_k_even_when_upgrading() {
    let mut b = base_builder(RouterImageModel::Seedream5p0Pro);
    b.resolution = Some(RouterResolution::FourK);
    assert_eq!(built(b.clone()).resolution, Seedream5p0ProResolution::TwoK);
    b.request_mismatch_mitigation_strategy = RequestMismatchMitigationStrategy::ErrorOut;
    assert!(build_higgsfield_seedream_5p0_pro(b).is_err());
  }

  #[test]
  fn artcraft_batches_of_eight_clamp_to_four() {
    let mut b = base_builder(RouterImageModel::Seedream5p0Pro);
    b.image_batch_count = Some(8);
    assert_eq!(built(b).batch_size, ImageBatchSize::Four);
  }

  #[test]
  fn auto_ratio_falls_back_to_square() {
    let mut b = base_builder(RouterImageModel::Seedream5p0Pro);
    b.aspect_ratio = Some(RouterAspectRatio::Auto);
    assert_eq!(built(b).aspect_ratio, SeedreamAspectRatio::Square1x1);
  }
}
