//! Nano Banana Pro on Higgsfield: 11 aspect ratios (incl. Auto), 1K / 2K /
//! 4K, 1–4 images, up to 4 reference images.

use higgsfield_client::endpoints::generate::image::nano_banana_pro::{NanoBananaProRequest, NanoBananaProResolution};

use crate::api::router_resolution::RouterResolution;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::generate::generate_image::generate_image_request_builder::GenerateImageRequestBuilder;
use crate::generate::generate_image::image_generation_draft_or_request::ImageGenerationDraftOrRequest;
use crate::generate::generate_image::providers::higgsfield::common::{
  check_reference_limit, finish, image_input_count, plan_batch_size, plan_nano_banana_aspect_ratio, require_prompt, snap_resolution,
};
use crate::generate::generate_image::providers::higgsfield::image_request::HiggsfieldImageRequest;

pub const MAX_REFERENCE_IMAGES: usize = 4;

const RESOLUTION_TIERS: &[(RouterResolution, NanoBananaProResolution)] = &[
  (RouterResolution::OneK, NanoBananaProResolution::OneK),
  (RouterResolution::TwoK, NanoBananaProResolution::TwoK),
  (RouterResolution::FourK, NanoBananaProResolution::FourK),
];

pub fn build_higgsfield_nano_banana_pro(mut builder: GenerateImageRequestBuilder) -> Result<ImageGenerationDraftOrRequest, ArtcraftRouterError> {
  let strategy = builder.request_mismatch_mitigation_strategy;
  let prompt = require_prompt(builder.prompt.take())?;
  let image_inputs = builder.image_inputs.take();
  let reference_count = image_input_count(image_inputs.as_ref());
  check_reference_limit(reference_count, MAX_REFERENCE_IMAGES, "Nano Banana Pro")?;

  let aspect_ratio = plan_nano_banana_aspect_ratio(builder.aspect_ratio.take(), reference_count > 0);
  let resolution = snap_resolution(builder.resolution.take(), RESOLUTION_TIERS, NanoBananaProResolution::OneK, strategy)?;
  let batch_size = plan_batch_size(builder.image_batch_count.take(), strategy)?;

  let mut request = NanoBananaProRequest::text_to_image(prompt, aspect_ratio, resolution);
  request.batch_size = batch_size;
  Ok(finish(HiggsfieldImageRequest::NanoBananaPro(request), image_inputs))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::api::router_aspect_ratio::RouterAspectRatio;
  use crate::api::router_image_model::RouterImageModel;
  use crate::client::request_mismatch_mitigation_strategy::RequestMismatchMitigationStrategy;
  use crate::generate::generate_image::providers::higgsfield::common_test_helpers::{base_builder, unwrap_draft, unwrap_request, with_references};
  use higgsfield_client::types::image_batch_size::ImageBatchSize;
  use higgsfield_client::types::nano_banana_aspect_ratio::NanoBananaAspectRatio;

  fn builder() -> GenerateImageRequestBuilder {
    base_builder(RouterImageModel::NanoBananaPro)
  }

  fn built(builder: GenerateImageRequestBuilder) -> NanoBananaProRequest {
    match unwrap_request(build_higgsfield_nano_banana_pro(builder).expect("build")) {
      HiggsfieldImageRequest::NanoBananaPro(request) => request,
      other => panic!("wrong model: {other:?}"),
    }
  }

  #[test]
  fn defaults_are_the_web_apps() {
    let request = built(builder());
    assert_eq!(request.prompt, "a shiba inu doing a kickflip");
    assert_eq!(request.aspect_ratio, NanoBananaAspectRatio::Landscape16x9);
    assert_eq!(request.resolution, NanoBananaProResolution::OneK);
    assert_eq!(request.batch_size, ImageBatchSize::One);
    assert!(request.reference_images.is_empty());
  }

  #[test]
  fn options_flow_through() {
    let mut b = builder();
    b.aspect_ratio = Some(RouterAspectRatio::TallThreeByFour);
    b.resolution = Some(RouterResolution::FourK);
    b.image_batch_count = Some(3);
    let request = built(b);
    assert_eq!(request.aspect_ratio, NanoBananaAspectRatio::Portrait3x4);
    assert_eq!(request.resolution, NanoBananaProResolution::FourK);
    assert_eq!(request.batch_size, ImageBatchSize::Three);
  }

  #[test]
  fn unsupported_resolutions_snap_per_strategy() {
    let mut b = builder();
    b.resolution = Some(RouterResolution::ThreeK);
    assert_eq!(built(b.clone()).resolution, NanoBananaProResolution::FourK);
    b.request_mismatch_mitigation_strategy = RequestMismatchMitigationStrategy::PayLessDowngrade;
    assert_eq!(built(b.clone()).resolution, NanoBananaProResolution::TwoK);
    b.request_mismatch_mitigation_strategy = RequestMismatchMitigationStrategy::ErrorOut;
    assert!(build_higgsfield_nano_banana_pro(b).is_err());
    // 0.5K (a Nano Banana 2 tier on ArtCraft) rounds up to 1K.
    let mut b = builder();
    b.resolution = Some(RouterResolution::HalfK);
    assert_eq!(built(b).resolution, NanoBananaProResolution::OneK);
  }

  #[test]
  fn references_make_a_draft_and_default_to_auto_aspect() {
    let draft = unwrap_draft(build_higgsfield_nano_banana_pro(with_references(builder(), 2)).expect("build"));
    assert_eq!(draft.reference_image_count(), 2);
    match draft.request {
      HiggsfieldImageRequest::NanoBananaPro(request) => assert_eq!(request.aspect_ratio, NanoBananaAspectRatio::Auto),
      other => panic!("wrong model: {other:?}"),
    }
  }

  #[test]
  fn too_many_references_are_rejected_before_upload() {
    assert!(build_higgsfield_nano_banana_pro(with_references(builder(), MAX_REFERENCE_IMAGES + 1)).is_err());
    assert!(build_higgsfield_nano_banana_pro(with_references(builder(), MAX_REFERENCE_IMAGES)).is_ok());
  }

  #[test]
  fn empty_prompt_is_rejected() {
    let mut b = builder();
    b.prompt = Some("  ".to_string());
    assert!(matches!(build_higgsfield_nano_banana_pro(b), Err(ArtcraftRouterError::InvalidInput(_))));
  }
}
