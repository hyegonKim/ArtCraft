use crate::commands::generate::common::media_source_conversion::{
  maybe_source_to_artcraft_token, sources_to_artcraft_tokens, ArtcraftMediaKind,
};
use crate::commands::generate::generate_error::GenerateError;
use crate::services::asset_uploads::asset_upload_ledger::AssetUploadLedger;
use crate::commands::generate::generate_video::request::TauriGenerateVideoRequest;
use crate::commands::generate::task_enqueue_success::TaskEnqueueSuccess;
use crate::events::generation_events::common::GenerationModel;
use artcraft_client::credentials::storyteller_credential_set::StorytellerCredentialSet;
use artcraft_client::utils::api_host::ApiHost;
use router::api::audio_list_ref::AudioListRef;
use router::api::character_list_ref::CharacterListRef;
use router::api::router_video_model::RouterVideoModel;
use router::api::image_list_ref::ImageListRef;
use router::api::image_ref::ImageRef;
use router::api::router_provider::RouterProvider;
use router::api::video_list_ref::VideoListRef;
use router::client::request_mismatch_mitigation_strategy::RequestMismatchMitigationStrategy;
use router::client::router_artcraft_client::RouterArtcraftClient;
use router::client::router_client::RouterClient;
use router::generate::generate_video::generate_video_request_builder::GenerateVideoRequestBuilder;
use router::generate::generate_video::generate_video_response::GenerateVideoResponse;
use router::generate::generate_video::video_generation_draft_or_request::VideoGenerationDraftOrRequest;
use core_types::enums::generation_source::GenerationSource;
use sqlite_identifiers::enums::task_type::TaskType;
use log::{error, info};

pub async fn handle_artcraft_video_via_router(
  request: &TauriGenerateVideoRequest,
  api_host: &ApiHost,
  creds: &StorytellerCredentialSet,
  model: RouterVideoModel,
  generation_model: GenerationModel,
  ledger: &AssetUploadLedger,
) -> Result<TaskEnqueueSuccess, GenerateError> {
  let client = RouterClient::Artcraft(RouterArtcraftClient::new(
    api_host.clone(),
    creds.clone(),
  ));

  // ArtCraft's API is token-native: local files and bytes upload to
  // ArtCraft here, at generate time (the media must reach ArtCraft anyway).
  let media = request.media_sources();
  let start_frame = maybe_source_to_artcraft_token(media.start_frame, ArtcraftMediaKind::Image, Some(creds), api_host, ledger)
      .await?.map(ImageRef::MediaFileToken);
  let end_frame = maybe_source_to_artcraft_token(media.end_frame, ArtcraftMediaKind::Image, Some(creds), api_host, ledger)
      .await?.map(ImageRef::MediaFileToken);

  let reference_images = sources_to_artcraft_tokens(media.reference_images, ArtcraftMediaKind::Image, Some(creds), api_host, ledger)
      .await?.map(ImageListRef::MediaFileTokens);
  let reference_videos = sources_to_artcraft_tokens(media.reference_videos, ArtcraftMediaKind::Video, Some(creds), api_host, ledger)
      .await?.map(VideoListRef::MediaFileTokens);
  let reference_audio = sources_to_artcraft_tokens(media.reference_audios, ArtcraftMediaKind::Audio, Some(creds), api_host, ledger)
      .await?.map(AudioListRef::MediaFileTokens);

  let reference_character_tokens = request.reference_character_tokens.clone().map(CharacterListRef::CharacterTokens);

  let router_request = GenerateVideoRequestBuilder {
    model,
    provider: RouterProvider::Artcraft,
    prompt: request.prompt.clone(),
    start_frame,
    end_frame,
    reference_images,
    reference_videos,
    reference_audio,
    reference_character_tokens,
    resolution: request.resolution,
    aspect_ratio: request.aspect_ratio,
    bitrate: None,
    duration_seconds: request.duration_seconds,
    video_batch_count: request.video_batch_count,
    generate_audio: request.generate_audio,
    request_mismatch_mitigation_strategy: RequestMismatchMitigationStrategy::PayMoreUpgrade,
    idempotency_token: None,
    negative_prompt: None,
  };

  info!("Building request for artcraft_router (v2 pipeline)...");
  let response = generate_via_v2(router_request, &client).await?;

  let job_id = response.get_artcraft_payload()
    .map(|p| p.inference_job_token.to_string())
    .ok_or(GenerateError::ResponseHadNoJobTokens)?;

  info!(
    "Router video generation enqueued: inference_job_token={}, response={:?}",
    job_id,
    response,
  );

  Ok(TaskEnqueueSuccess {
    task_type: TaskType::VideoGeneration,
    model: Some(generation_model),
    provider: GenerationSource::Artcraft,
    provider_job_id: Some(job_id),
    is_batch_generation: request.video_batch_count.unwrap_or(1) > 1,
    maybe_queue_status_url: None,
    maybe_prompt_token: None,
    maybe_queue_response_url: None,
  })
}

/// V2 pipeline: build2 → send_request (Artcraft skips draft phase).
async fn generate_via_v2(
  router_request: GenerateVideoRequestBuilder,
  client: &RouterClient,
) -> Result<GenerateVideoResponse, GenerateError> {
  let draft_or_request = router_request.build2()?;

  let request = match draft_or_request {
    VideoGenerationDraftOrRequest::Request(r) => r,
    VideoGenerationDraftOrRequest::Draft(_) => {
      error!("Unexpected Draft variant for Artcraft provider");
      return Err(GenerateError::NotYetImplemented("Artcraft provider should not produce a draft request".to_string()));
    }
  };

  let response = request.send_request(client).await.map_err(|err| {
    error!("V2 failed to enqueue: {:?}", err);
    GenerateError::from(err)
  })?;

  info!("V2 successfully enqueued.");
  Ok(response)
}
