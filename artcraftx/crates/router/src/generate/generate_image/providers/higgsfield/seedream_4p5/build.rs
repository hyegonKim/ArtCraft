//! Seedream 4.5 on Higgsfield: 8 aspect ratios (no Auto), a 2K / 4K menu,
//! 1–4 images. ArtCraft's baked-in `auto_2k` / `auto_4k` ratios become a
//! 16:9 request at that resolution.

use higgsfield_client::endpoints::generate::image::seedream_4p5::{Seedream4p5Request, Seedream4p5Resolution};
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

const RESOLUTION_TIERS: &[(RouterResolution, Seedream4p5Resolution)] = &[
  (RouterResolution::TwoK, Seedream4p5Resolution::TwoK),
  (RouterResolution::FourK, Seedream4p5Resolution::FourK),
];

pub fn build_higgsfield_seedream_4p5(mut builder: GenerateImageRequestBuilder) -> Result<ImageGenerationDraftOrRequest, ArtcraftRouterError> {
  let strategy = builder.request_mismatch_mitigation_strategy;
  let prompt = require_prompt(builder.prompt.take())?;
  let image_inputs = builder.image_inputs.take();
  check_reference_limit(image_input_count(image_inputs.as_ref()), MAX_REFERENCE_IMAGES, "Seedream 4.5")?;

  let requested_aspect_ratio = builder.aspect_ratio.take();
  let aspect_ratio = plan_seedream_aspect_ratio(requested_aspect_ratio, SeedreamAspectRatio::Landscape16x9);
  let requested_resolution = builder.resolution.take().or_else(|| seedream_auto_resolution_hint(requested_aspect_ratio));
  let resolution = snap_resolution(requested_resolution, RESOLUTION_TIERS, Seedream4p5Resolution::TwoK, strategy)?;
  let batch_size = plan_batch_size(builder.image_batch_count.take(), strategy)?;

  let mut request = Seedream4p5Request::text_to_image(prompt, aspect_ratio, resolution);
  request.batch_size = batch_size;
  Ok(finish(HiggsfieldImageRequest::Seedream4p5(request), image_inputs))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::api::router_aspect_ratio::RouterAspectRatio;
  use crate::api::router_image_model::RouterImageModel;
  use crate::client::request_mismatch_mitigation_strategy::RequestMismatchMitigationStrategy;
  use crate::generate::generate_image::providers::higgsfield::common_test_helpers::{base_builder, unwrap_request};

  fn built(builder: GenerateImageRequestBuilder) -> Seedream4p5Request {
    match unwrap_request(build_higgsfield_seedream_4p5(builder).expect("build")) {
      HiggsfieldImageRequest::Seedream4p5(request) => request,
      other => panic!("wrong model: {other:?}"),
    }
  }

  #[test]
  fn three_k_sits_between_the_two_tiers() {
    let mut b = base_builder(RouterImageModel::Seedream4p5);
    b.resolution = Some(RouterResolution::ThreeK);
    assert_eq!(built(b.clone()).resolution, Seedream4p5Resolution::FourK);
    b.request_mismatch_mitigation_strategy = RequestMismatchMitigationStrategy::PayLessDowngrade;
    assert_eq!(built(b).resolution, Seedream4p5Resolution::TwoK);
  }

  #[test]
  fn auto_4k_becomes_16x9_at_4k() {
    let mut b = base_builder(RouterImageModel::Seedream4p5);
    b.aspect_ratio = Some(RouterAspectRatio::Auto4k);
    let request = built(b);
    assert_eq!(request.aspect_ratio, SeedreamAspectRatio::Landscape16x9);
    assert_eq!(request.resolution, Seedream4p5Resolution::FourK);
  }
}
