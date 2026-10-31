//! GPT Image 2 on Higgsfield: 9 aspect ratios (incl. Auto; no 5:4 / 4:5,
//! no "1:1 HD"), Low / Medium / High quality, 1K / 2K / 4K (no 3K), 1–4
//! images, up to 6 reference images.

use higgsfield_client::endpoints::generate::image::gpt_image_2::{GptImage2Quality, GptImage2Request, GptImage2Resolution};

use crate::api::router_quality::RouterQuality;
use crate::api::router_resolution::RouterResolution;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::generate::generate_image::generate_image_request_builder::GenerateImageRequestBuilder;
use crate::generate::generate_image::image_generation_draft_or_request::ImageGenerationDraftOrRequest;
use crate::generate::generate_image::providers::higgsfield::common::{
  check_reference_limit, finish, image_input_count, plan_batch_size, plan_gpt_image_2_aspect_ratio, require_prompt, snap_resolution,
};
use crate::generate::generate_image::providers::higgsfield::image_request::HiggsfieldImageRequest;

pub const MAX_REFERENCE_IMAGES: usize = 6;

const RESOLUTION_TIERS: &[(RouterResolution, GptImage2Resolution)] = &[
  (RouterResolution::OneK, GptImage2Resolution::OneK),
  (RouterResolution::TwoK, GptImage2Resolution::TwoK),
  (RouterResolution::FourK, GptImage2Resolution::FourK),
];

pub fn build_higgsfield_gpt_image_2(mut builder: GenerateImageRequestBuilder) -> Result<ImageGenerationDraftOrRequest, ArtcraftRouterError> {
  let strategy = builder.request_mismatch_mitigation_strategy;
  let prompt = require_prompt(builder.prompt.take())?;
  let image_inputs = builder.image_inputs.take();
  let reference_count = image_input_count(image_inputs.as_ref());
  check_reference_limit(reference_count, MAX_REFERENCE_IMAGES, "GPT Image 2")?;

  let aspect_ratio = plan_gpt_image_2_aspect_ratio(builder.aspect_ratio.take(), reference_count > 0);
  let quality = plan_quality(builder.quality.take());
  let resolution = snap_resolution(builder.resolution.take(), RESOLUTION_TIERS, GptImage2Resolution::OneK, strategy)?;
  let batch_size = plan_batch_size(builder.image_batch_count.take(), strategy)?;

  let mut request = GptImage2Request::text_to_image(prompt, aspect_ratio, quality, resolution);
  request.batch_size = batch_size;
  Ok(finish(HiggsfieldImageRequest::GptImage2(request), image_inputs))
}

/// The three tiers line up; unset means the web app's default (Medium).
fn plan_quality(quality: Option<RouterQuality>) -> GptImage2Quality {
  match quality {
    None | Some(RouterQuality::Medium) => GptImage2Quality::Medium,
    Some(RouterQuality::High) => GptImage2Quality::High,
    Some(RouterQuality::Low) => GptImage2Quality::Low,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::api::router_aspect_ratio::RouterAspectRatio;
  use crate::api::router_image_model::RouterImageModel;
  use crate::client::request_mismatch_mitigation_strategy::RequestMismatchMitigationStrategy;
  use crate::generate::generate_image::providers::higgsfield::common_test_helpers::{base_builder, unwrap_request};
  use higgsfield_client::endpoints::generate::image::gpt_image_2::GptImage2AspectRatio;

  fn built(builder: GenerateImageRequestBuilder) -> GptImage2Request {
    match unwrap_request(build_higgsfield_gpt_image_2(builder).expect("build")) {
      HiggsfieldImageRequest::GptImage2(request) => request,
      other => panic!("wrong model: {other:?}"),
    }
  }

  #[test]
  fn three_k_snaps_up_or_down() {
    let mut b = base_builder(RouterImageModel::GptImage2);
    b.resolution = Some(RouterResolution::ThreeK);
    assert_eq!(built(b.clone()).resolution, GptImage2Resolution::FourK);
    b.request_mismatch_mitigation_strategy = RequestMismatchMitigationStrategy::PayLessDowngrade;
    assert_eq!(built(b).resolution, GptImage2Resolution::TwoK);
  }

  #[test]
  fn quality_and_missing_ratios() {
    let mut b = base_builder(RouterImageModel::GptImage2);
    b.quality = Some(RouterQuality::Low);
    b.aspect_ratio = Some(RouterAspectRatio::SquareHd);
    let request = built(b);
    assert_eq!(request.quality, GptImage2Quality::Low);
    assert_eq!(request.aspect_ratio, GptImage2AspectRatio::Square1x1);
    assert_eq!(plan_quality(None), GptImage2Quality::Medium);
  }
}
