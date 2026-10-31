//! Test-only helpers shared by the per-model `build.rs` tests.

use crate::api::image_list_ref::ImageListRef;
use crate::api::router_image_model::RouterImageModel;
use crate::api::router_provider::RouterProvider;
use crate::client::request_mismatch_mitigation_strategy::RequestMismatchMitigationStrategy;
use crate::generate::generate_image::generate_image_request_builder::GenerateImageRequestBuilder;
use crate::generate::generate_image::image_generation_draft::ImageGenerationDraftRequest;
use crate::generate::generate_image::image_generation_draft_or_request::ImageGenerationDraftOrRequest;
use crate::generate::generate_image::image_generation_request::ImageGenerationRequest;
use crate::generate::generate_image::providers::higgsfield::draft::HiggsfieldImageDraftState;
use crate::generate::generate_image::providers::higgsfield::image_request::HiggsfieldImageRequest;

pub fn base_builder(model: RouterImageModel) -> GenerateImageRequestBuilder {
  GenerateImageRequestBuilder {
    model,
    provider: RouterProvider::Higgsfield,
    prompt: Some("a shiba inu doing a kickflip".to_string()),
    image_inputs: None,
    resolution: None,
    aspect_ratio: None,
    quality: None,
    image_batch_count: None,
    horizontal_angle: None,
    vertical_angle: None,
    zoom: None,
    request_mismatch_mitigation_strategy: RequestMismatchMitigationStrategy::PayMoreUpgrade,
    generation_mode_mismatch_strategy: None,
    idempotency_token: None,
  }
}

pub fn with_references(mut builder: GenerateImageRequestBuilder, count: usize) -> GenerateImageRequestBuilder {
  let urls = (0..count).map(|i| format!("https://cdn.example.com/ref_{i}.png")).collect();
  builder.image_inputs = Some(ImageListRef::Urls(urls));
  builder
}

/// The planned request out of a direct (no references) build.
pub fn unwrap_request(result: ImageGenerationDraftOrRequest) -> HiggsfieldImageRequest {
  match result {
    ImageGenerationDraftOrRequest::Request(ImageGenerationRequest::HiggsfieldImage(state)) => state.request,
    other => panic!("expected a direct Higgsfield request, got {other:?}"),
  }
}

/// The draft out of a build with references.
pub fn unwrap_draft(result: ImageGenerationDraftOrRequest) -> HiggsfieldImageDraftState {
  match result {
    ImageGenerationDraftOrRequest::Draft(ImageGenerationDraftRequest::HiggsfieldImage(draft)) => draft,
    other => panic!("expected a Higgsfield draft, got {other:?}"),
  }
}
