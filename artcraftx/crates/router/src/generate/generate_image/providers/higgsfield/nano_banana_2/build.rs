//! Nano Banana 2 on Higgsfield: 11 aspect ratios (incl. Auto), 1K / 2K / 4K
//! (no 0.5K tier), 1–4 images, up to 6 reference images.

use higgsfield_client::endpoints::generate::image::nano_banana_2::{NanoBanana2Request, NanoBanana2Resolution};

use crate::api::router_resolution::RouterResolution;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::generate::generate_image::generate_image_request_builder::GenerateImageRequestBuilder;
use crate::generate::generate_image::image_generation_draft_or_request::ImageGenerationDraftOrRequest;
use crate::generate::generate_image::providers::higgsfield::common::{
  check_reference_limit, finish, image_input_count, plan_batch_size, plan_nano_banana_aspect_ratio, require_prompt, snap_resolution,
};
use crate::generate::generate_image::providers::higgsfield::image_request::HiggsfieldImageRequest;

pub const MAX_REFERENCE_IMAGES: usize = 6;

const RESOLUTION_TIERS: &[(RouterResolution, NanoBanana2Resolution)] = &[
  (RouterResolution::OneK, NanoBanana2Resolution::OneK),
  (RouterResolution::TwoK, NanoBanana2Resolution::TwoK),
  (RouterResolution::FourK, NanoBanana2Resolution::FourK),
];

pub fn build_higgsfield_nano_banana_2(mut builder: GenerateImageRequestBuilder) -> Result<ImageGenerationDraftOrRequest, ArtcraftRouterError> {
  let strategy = builder.request_mismatch_mitigation_strategy;
  let prompt = require_prompt(builder.prompt.take())?;
  let image_inputs = builder.image_inputs.take();
  let reference_count = image_input_count(image_inputs.as_ref());
  check_reference_limit(reference_count, MAX_REFERENCE_IMAGES, "Nano Banana 2")?;

  let aspect_ratio = plan_nano_banana_aspect_ratio(builder.aspect_ratio.take(), reference_count > 0);
  let resolution = snap_resolution(builder.resolution.take(), RESOLUTION_TIERS, NanoBanana2Resolution::OneK, strategy)?;
  let batch_size = plan_batch_size(builder.image_batch_count.take(), strategy)?;

  let mut request = NanoBanana2Request::text_to_image(prompt, aspect_ratio, resolution);
  request.batch_size = batch_size;
  Ok(finish(HiggsfieldImageRequest::NanoBanana2(request), image_inputs))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::api::router_image_model::RouterImageModel;
  use crate::client::request_mismatch_mitigation_strategy::RequestMismatchMitigationStrategy;
  use crate::generate::generate_image::providers::higgsfield::common_test_helpers::{base_builder, unwrap_request, with_references};

  fn built(builder: GenerateImageRequestBuilder) -> NanoBanana2Request {
    match unwrap_request(build_higgsfield_nano_banana_2(builder).expect("build")) {
      HiggsfieldImageRequest::NanoBanana2(request) => request,
      other => panic!("wrong model: {other:?}"),
    }
  }

  #[test]
  fn half_k_is_not_offered_and_rounds_to_one_k() {
    let mut b = base_builder(RouterImageModel::NanoBanana2);
    b.resolution = Some(RouterResolution::HalfK);
    assert_eq!(built(b.clone()).resolution, NanoBanana2Resolution::OneK);
    b.request_mismatch_mitigation_strategy = RequestMismatchMitigationStrategy::PayLessDowngrade;
    assert_eq!(built(b.clone()).resolution, NanoBanana2Resolution::OneK);
    b.request_mismatch_mitigation_strategy = RequestMismatchMitigationStrategy::ErrorOut;
    assert!(build_higgsfield_nano_banana_2(b).is_err());
  }

  #[test]
  fn supported_tiers_pass_through() {
    let mut b = base_builder(RouterImageModel::NanoBanana2);
    b.resolution = Some(RouterResolution::TwoK);
    assert_eq!(built(b).resolution, NanoBanana2Resolution::TwoK);
  }

  #[test]
  fn reference_limit_is_six() {
    assert!(build_higgsfield_nano_banana_2(with_references(base_builder(RouterImageModel::NanoBanana2), 6)).is_ok());
    assert!(build_higgsfield_nano_banana_2(with_references(base_builder(RouterImageModel::NanoBanana2), 7)).is_err());
  }
}
