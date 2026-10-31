//! Test-only helpers shared by the per-model `build.rs` tests.

use crate::api::image_list_ref::ImageListRef;
use crate::api::image_ref::ImageRef;
use crate::api::router_provider::RouterProvider;
use crate::api::router_video_model::RouterVideoModel;
use crate::api::video_list_ref::VideoListRef;
use crate::client::request_mismatch_mitigation_strategy::RequestMismatchMitigationStrategy;
use crate::generate::generate_video::generate_video_request_builder::GenerateVideoRequestBuilder;
use crate::generate::generate_video::providers::higgsfield::draft::{HiggsfieldVideoDraftState, HiggsfieldVideoPlan};
use crate::generate::generate_video::providers::higgsfield::video_request::HiggsfieldVideoRequest;
use crate::generate::generate_video::video_generation_draft::VideoGenerationDraftRequest;
use crate::generate::generate_video::video_generation_draft_or_request::VideoGenerationDraftOrRequest;
use crate::generate::generate_video::video_generation_request::VideoGenerationRequest;

pub fn base_builder(model: RouterVideoModel) -> GenerateVideoRequestBuilder {
  GenerateVideoRequestBuilder {
    model,
    provider: RouterProvider::Higgsfield,
    prompt: Some("a shiba inu surfing a big wave".to_string()),
    request_mismatch_mitigation_strategy: RequestMismatchMitigationStrategy::PayMoreUpgrade,
    ..Default::default()
  }
}

pub fn with_start_frame(mut builder: GenerateVideoRequestBuilder) -> GenerateVideoRequestBuilder {
  builder.start_frame = Some(ImageRef::Url("https://cdn.example.com/start.png".to_string()));
  builder
}

pub fn with_reference_images(mut builder: GenerateVideoRequestBuilder, count: usize) -> GenerateVideoRequestBuilder {
  let urls = (0..count).map(|i| format!("https://cdn.example.com/ref_{i}.png")).collect();
  builder.reference_images = Some(ImageListRef::Urls(urls));
  builder
}

pub fn with_reference_videos(mut builder: GenerateVideoRequestBuilder, count: usize) -> GenerateVideoRequestBuilder {
  let urls = (0..count).map(|i| format!("https://cdn.example.com/clip_{i}.mp4")).collect();
  builder.reference_videos = Some(VideoListRef::Urls(urls));
  builder
}

/// The planned request out of a direct (no media) build.
pub fn unwrap_request(result: VideoGenerationDraftOrRequest) -> HiggsfieldVideoRequest {
  match result {
    VideoGenerationDraftOrRequest::Request(VideoGenerationRequest::HiggsfieldVideo(state)) => state.request,
    other => panic!("expected a direct Higgsfield request, got {other:?}"),
  }
}

/// The draft out of a build with media.
pub fn unwrap_draft(result: VideoGenerationDraftOrRequest) -> HiggsfieldVideoDraftState {
  match result {
    VideoGenerationDraftOrRequest::Draft(VideoGenerationDraftRequest::HiggsfieldVideo(draft)) => draft,
    other => panic!("expected a Higgsfield draft, got {other:?}"),
  }
}

/// The planned request inside a draft (media not yet attached).
pub fn unwrap_draft_request(result: VideoGenerationDraftOrRequest) -> (HiggsfieldVideoRequest, HiggsfieldVideoDraftState) {
  let draft = unwrap_draft(result);
  match &draft.plan {
    HiggsfieldVideoPlan::Request(request) => (request.clone(), draft),
    other => panic!("expected a planned request, got {other:?}"),
  }
}
