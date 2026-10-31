//! Nano Banana 2 Lite on Higgsfield: 11 aspect ratios (incl. Auto), a fixed
//! 1K with a High / Minimal quality ("thinking") menu, 1–4 images, up to 6
//! reference images. The router's Low / Medium / High quality maps onto the
//! two tiers; a requested resolution is ignored (there is no menu).

use higgsfield_client::endpoints::generate::image::nano_banana_2_lite::{NanoBanana2LiteQuality, NanoBanana2LiteRequest};
use log::warn;

use crate::api::router_quality::RouterQuality;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::generate::generate_image::generate_image_request_builder::GenerateImageRequestBuilder;
use crate::generate::generate_image::image_generation_draft_or_request::ImageGenerationDraftOrRequest;
use crate::generate::generate_image::providers::higgsfield::common::{
  check_reference_limit, finish, image_input_count, plan_batch_size, plan_nano_banana_aspect_ratio, require_prompt,
};
use crate::generate::generate_image::providers::higgsfield::image_request::HiggsfieldImageRequest;

pub const MAX_REFERENCE_IMAGES: usize = 6;

pub fn build_higgsfield_nano_banana_2_lite(mut builder: GenerateImageRequestBuilder) -> Result<ImageGenerationDraftOrRequest, ArtcraftRouterError> {
  let strategy = builder.request_mismatch_mitigation_strategy;
  let prompt = require_prompt(builder.prompt.take())?;
  let image_inputs = builder.image_inputs.take();
  let reference_count = image_input_count(image_inputs.as_ref());
  check_reference_limit(reference_count, MAX_REFERENCE_IMAGES, "Nano Banana 2 Lite")?;

  if let Some(resolution) = builder.resolution.take() {
    warn!("Nano Banana 2 Lite always renders at 1K on Higgsfield; ignoring requested {resolution:?}");
  }

  let aspect_ratio = plan_nano_banana_aspect_ratio(builder.aspect_ratio.take(), reference_count > 0);
  let quality = plan_quality(builder.quality.take());
  let batch_size = plan_batch_size(builder.image_batch_count.take(), strategy)?;

  let mut request = NanoBanana2LiteRequest::text_to_image(prompt, aspect_ratio, quality);
  request.batch_size = batch_size;
  Ok(finish(HiggsfieldImageRequest::NanoBanana2Lite(request), image_inputs))
}

/// High stays High; anything lower is the "Minimal" tier.
fn plan_quality(quality: Option<RouterQuality>) -> NanoBanana2LiteQuality {
  match quality {
    None | Some(RouterQuality::High) => NanoBanana2LiteQuality::High,
    Some(RouterQuality::Medium) | Some(RouterQuality::Low) => NanoBanana2LiteQuality::Minimal,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::api::router_image_model::RouterImageModel;
  use crate::api::router_resolution::RouterResolution;
  use crate::generate::generate_image::providers::higgsfield::common_test_helpers::{base_builder, unwrap_request};

  fn built(builder: GenerateImageRequestBuilder) -> NanoBanana2LiteRequest {
    match unwrap_request(build_higgsfield_nano_banana_2_lite(builder).expect("build")) {
      HiggsfieldImageRequest::NanoBanana2Lite(request) => request,
      other => panic!("wrong model: {other:?}"),
    }
  }

  #[test]
  fn quality_maps_onto_the_two_tiers() {
    assert_eq!(plan_quality(None), NanoBanana2LiteQuality::High);
    assert_eq!(plan_quality(Some(RouterQuality::High)), NanoBanana2LiteQuality::High);
    assert_eq!(plan_quality(Some(RouterQuality::Medium)), NanoBanana2LiteQuality::Minimal);
    assert_eq!(plan_quality(Some(RouterQuality::Low)), NanoBanana2LiteQuality::Minimal);
  }

  #[test]
  fn resolution_is_ignored_rather_than_rejected() {
    let mut b = base_builder(RouterImageModel::NanoBanana2Lite);
    b.resolution = Some(RouterResolution::FourK);
    b.quality = Some(RouterQuality::Low);
    let request = built(b);
    assert_eq!(request.quality, NanoBanana2LiteQuality::Minimal);
  }
}
