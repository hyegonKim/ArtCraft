use core_types::enums::generation_source::GenerationSource;
use sqlite_identifiers::enums::task_type::TaskType;
use log::{info, warn};
use router::api::router_provider::RouterProvider;
use router::client::request_mismatch_mitigation_strategy::RequestMismatchMitigationStrategy;
use router::client::router_artcraft_client::RouterArtcraftClient;
use router::client::router_client::RouterClient;
use router::generate::generate_audio::generate_audio_request_builder::GenerateAudioRequestBuilder;
use router::generate::generate_audio::audio_generation_draft_or_request::AudioGenerationDraftOrRequest;
use router::api::audio_list_ref::AudioListRef;
use router::api::image_list_ref::ImageListRef;

use crate::commands::utils::api_adapters::models::audio::tauri_audio_model_to_generation_model::tauri_audio_model_to_generation_model;
use crate::commands::utils::api_adapters::models::audio::tauri_audio_model_to_router_model::tauri_audio_model_to_router_model;
use crate::commands::generate::generate_error::GenerateError;
use crate::services::asset_uploads::asset_upload_ledger::AssetUploadLedger;
use crate::commands::generate::task_enqueue_success::TaskEnqueueSuccess;
use crate::commands::generate::common::generation_credential::{credential_not_usable, resolve_generation_credential, storyteller_creds_from_credential};
use crate::commands::generate::common::media_source_conversion::{sources_to_artcraft_tokens, ArtcraftMediaKind};
use crate::commands::generate::common::tauri_media_source::validate_sources;
use crate::commands::generate::generate_audio::request::TauriGenerateAudioRequest;
use crate::utils::services::artcraft_api_host::maybe_artcraft_api_host_for_service;
use crate::credentials::auth_credential::AuthCredential;
use crate::state::data_dir::app_data_root::AppDataRoot;

/// Credential-driven audio generation: resolve the stored credential named
/// by the request's `credential_id`, then route to that credential's service
/// via the router. The router owns upload + dispatch mechanics per provider.
pub async fn handle_credential_router(
  request: &TauriGenerateAudioRequest,
  app_data_root: &AppDataRoot,
  ledger: &AssetUploadLedger,
) -> Result<TaskEnqueueSuccess, GenerateError> {
  // Reject unusable media sources (missing local files, empty bytes) before
  // any provider work.
  validate_sources(request.audio_media_sources().iter().flatten())?;
  validate_sources(request.image_media_sources().iter().flatten())?;

  let credential = resolve_generation_credential(
    request.credential_id.as_deref(),
    app_data_root,
  )?;

  info!(
    "handle_credential_router (audio): credential={} service={}",
    credential.id.as_str(),
    credential.service,
  );

  match credential.service {
    GenerationSource::Artcraft
    | GenerationSource::ArtcraftLocal
    | GenerationSource::ArtcraftCookies => {
      handle_artcraft_credential(request, &credential, ledger).await
    }
    other => Err(credential_not_usable(
      &credential,
      &format!("accounts for service {} can't generate audio yet", other),
    )),
  }
}

/// Generate via the router's Artcraft provider, authenticating with the
/// stored session cookies. Artcraft's API is token-native; local files and
/// bytes upload to ArtCraft here, at generate time.
async fn handle_artcraft_credential(
  request: &TauriGenerateAudioRequest,
  credential: &AuthCredential,
  ledger: &AssetUploadLedger,
) -> Result<TaskEnqueueSuccess, GenerateError> {
  let tauri_model = request.model.ok_or(GenerateError::no_model_specified())?;

  let router_model = tauri_audio_model_to_router_model(tauri_model);
  let generation_model = tauri_audio_model_to_generation_model(tauri_model);

  let api_host = maybe_artcraft_api_host_for_service(credential.service)
      .expect("caller only routes ArtCraft services here");

  let creds = storyteller_creds_from_credential(credential)?;

  let audio_references = sources_to_artcraft_tokens(
    request.audio_media_sources(), ArtcraftMediaKind::Audio, Some(&creds), &api_host, ledger,
  ).await?.map(AudioListRef::MediaFileTokens);
  let image_references = sources_to_artcraft_tokens(
    request.image_media_sources(), ArtcraftMediaKind::Image, Some(&creds), &api_host, ledger,
  ).await?.map(ImageListRef::MediaFileTokens);

  let client = RouterClient::Artcraft(RouterArtcraftClient::new(
    api_host,
    creds,
  ));

  let router_request = GenerateAudioRequestBuilder {
    model: router_model,
    provider: RouterProvider::Artcraft,
    prompt: request.prompt.clone(),
    style_prompt: request.style_prompt.clone(),
    audio_references,
    image_references,
    keep_lyrics: request.keep_lyrics,
    is_instrumental: request.is_instrumental,
    is_loopable: request.is_loopable,
    bpm: request.bpm,
    musical_key: request.musical_key,
    sample_rate_hz: request.sample_rate_hz,
    speed: request.speed,
    volume: request.volume,
    pitch: request.pitch,
    request_mismatch_mitigation_strategy: RequestMismatchMitigationStrategy::PayMoreUpgrade,
    idempotency_token: None,
  };

  info!("Building Artcraft audio generation plan: model={:?}", router_model);

  let generation_request = match router_request.build2() {
    Ok(AudioGenerationDraftOrRequest::Request(generation_request)) => generation_request,
    Ok(AudioGenerationDraftOrRequest::Draft(draft)) => {
      warn!("Artcraft build unexpectedly produced a draft: {:?}", draft);
      return Err(GenerateError::NotYetImplemented(
        "Artcraft requests should not require a draft phase".to_string(),
      ));
    }
    Err(err) => {
      warn!("Could not build Artcraft router request: {:?}", err);
      return Err(err.into());
    }
  };

  let response = generation_request.send_request(&client).await
      .map_err(|err| {
        warn!("Artcraft router audio generation failed: {:?}", err);
        GenerateError::from(err)
      })?;

  let payload = response.get_artcraft_payload()
      .ok_or(GenerateError::ResponseHadNoJobTokens)?;

  info!(
    "Router audio generation enqueued: inference_job_token={}, all_tokens={:?}, response={:?}",
    payload.inference_job_token.as_str(),
    payload.all_inference_job_tokens,
    response,
  );

  Ok(TaskEnqueueSuccess {
    task_type: TaskType::AudioGeneration,
    model: Some(generation_model),
    provider: GenerationSource::Artcraft,
    provider_job_id: Some(payload.inference_job_token.to_string()),
    is_batch_generation: false,
    maybe_queue_status_url: None,
    maybe_queue_response_url: None,
    maybe_prompt_token: None,
  })
}
