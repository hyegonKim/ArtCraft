use crate::api::image_list_ref::ImageListRef;
use crate::api::router_aspect_ratio::RouterAspectRatio;
use crate::errors::artcraft_router_error::ArtcraftRouterError;
use crate::generate::generate_image::generate_image_request_builder::GenerateImageRequestBuilder;
use crate::generate::generate_image::image_generation_draft_or_request::ImageGenerationDraftOrRequest;
use crate::generate::generate_image::image_generation_request::ImageGenerationRequest;
use crate::generate::generate_image::providers::grok::grok_imagine_image::request::GrokImagineImageRequestState;

/// Build a first-party Grok Imagine image request. `enable_pro` selects quality
/// ("pro") mode over fast ("speed"); the two `RouterImageModel`s map here.
pub fn build_grok_imagine_image(
  builder: GenerateImageRequestBuilder,
  enable_pro: bool,
) -> Result<ImageGenerationDraftOrRequest, ArtcraftRouterError> {
  // The imagine websocket path is text-to-image only for now.
  if has_image_inputs(builder.image_inputs.as_ref()) {
    return Err(ArtcraftRouterError::UnsupportedProviderAndModelForNewApi(
      "Grok Imagine does not support image references yet".to_string(),
    ));
  }

  let prompt = builder.prompt.unwrap_or_default().trim().to_string();
  let aspect_ratio = builder.aspect_ratio.unwrap_or(RouterAspectRatio::Square);

  Ok(ImageGenerationDraftOrRequest::Request(
    ImageGenerationRequest::GrokImagineImage(GrokImagineImageRequestState {
      prompt,
      aspect_ratio,
      enable_pro,
    }),
  ))
}

fn has_image_inputs(image_inputs: Option<&ImageListRef>) -> bool {
  image_inputs.is_some_and(|refs| !refs.is_empty())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::api::router_image_model::RouterImageModel;
  use crate::api::router_provider::RouterProvider;
  use crate::client::generation_mode_mismatch_strategy::GenerationModeMismatchStrategy;
  use crate::client::request_mismatch_mitigation_strategy::RequestMismatchMitigationStrategy;

  fn base_builder() -> GenerateImageRequestBuilder {
    GenerateImageRequestBuilder {
      model: RouterImageModel::GrokImagineImage,
      provider: RouterProvider::Grok,
      prompt: Some("  a red kite  ".to_string()),
      image_inputs: None,
      resolution: None,
      aspect_ratio: Some(RouterAspectRatio::WideSixteenByNine),
      quality: None,
      image_batch_count: None,
      horizontal_angle: None,
      vertical_angle: None,
      zoom: None,
      request_mismatch_mitigation_strategy: RequestMismatchMitigationStrategy::PayMoreUpgrade,
      generation_mode_mismatch_strategy: Some(GenerationModeMismatchStrategy::GenerateAnyway),
      idempotency_token: None,
    }
  }

  fn built(builder: GenerateImageRequestBuilder, enable_pro: bool) -> GrokImagineImageRequestState {
    match build_grok_imagine_image(builder, enable_pro).expect("build") {
      ImageGenerationDraftOrRequest::Request(ImageGenerationRequest::GrokImagineImage(r)) => r,
      _ => panic!("expected a direct Grok Imagine request"),
    }
  }

  #[test]
  fn trims_prompt_and_carries_aspect_and_mode() {
    let state = built(base_builder(), true);
    assert_eq!(state.prompt, "a red kite");
    assert_eq!(state.aspect_ratio, RouterAspectRatio::WideSixteenByNine);
    assert!(state.enable_pro);
  }

  #[test]
  fn defaults_aspect_to_square() {
    let builder = GenerateImageRequestBuilder { aspect_ratio: None, ..base_builder() };
    assert_eq!(built(builder, false).aspect_ratio, RouterAspectRatio::Square);
  }

  #[test]
  fn image_inputs_are_rejected() {
    let builder = GenerateImageRequestBuilder {
      image_inputs: Some(ImageListRef::Urls(vec!["https://example.com/x.png".to_string()])),
      ..base_builder()
    };
    assert!(build_grok_imagine_image(builder, false).is_err());
  }
}
