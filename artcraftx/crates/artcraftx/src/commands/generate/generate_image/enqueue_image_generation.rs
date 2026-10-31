use artcraft_client::utils::api_host::ApiHost;
use chrono::Utc;
use core_types::enums::generation_source::GenerationSource;
use log::{error, info, warn};
use midjourney_client::recipes::get_user_info::{get_user_info, GetUserInfoArgs};
use router::api::image_list_ref::ImageListRef;
use router::api::router_image_model::RouterImageModel;
use router::api::router_provider::RouterProvider;
use router::client::generation_mode_mismatch_strategy::GenerationModeMismatchStrategy;
use router::client::request_mismatch_mitigation_strategy::RequestMismatchMitigationStrategy;
use router::client::router_artcraft_client::RouterArtcraftClient;
use router::client::router_client::RouterClient;
use router::client::router_fal_client::RouterFalClient;
use router::client::router_grok_client::RouterGrokClient;
use router::client::router_midjourney_client::RouterMidjourneyClient;
use router::generate::generate_image::generate_image_request_builder::GenerateImageRequestBuilder;
use router::generate::generate_image::generate_image_response::GenerateImageResponse;
use router::generate::generate_image::image_generation_draft_or_request::ImageGenerationDraftOrRequest;
use router::errors::artcraft_router_error::ArtcraftRouterError;
use router::generate::generate_image::image_generation_request::ImageGenerationRequest;
use grok_consumer_client::endpoint_bindings::generate_image_websocket::grok_generate_image_websocket::GrokImageWebsocket;
use sqlite_identifiers::enums::task_type::TaskType;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;

use crate::commands::generate::common::generation_credential::{
  credential_not_usable, resolve_generation_credential, storyteller_creds_from_credential,
};
use crate::commands::generate::common::media_source_conversion::{
  image_sources_to_fal_urls, sources_to_artcraft_tokens, ArtcraftMediaKind,
};
use crate::commands::generate::common::tauri_media_source::validate_sources;
use crate::commands::generate::generate_error::{BadInputReason, GenerateError};
use crate::commands::generate::generate_image::higgsfield::enqueue_via_higgsfield::enqueue_via_higgsfield;
use crate::commands::generate::generate_image::tauri_generate_image_request::TauriGenerateImageRequest;
use crate::commands::generate::generate_image::tauri_image_model::TauriImageModel;
use crate::commands::generate::generate_image::utils::convert_enums_to_router::{
  convert_aspect_ratio, convert_quality, convert_resolution,
};
use crate::commands::generate::generate_image::utils::map_media_files_to_urls::map_media_file_tokens_to_cdn_urls;
use crate::commands::generate::generate_image::utils::parse_semantic_media_files::{
  parse_semantic_media_files, SemanticMediaFiles,
};
use crate::commands::generate::task_enqueue_success::TaskEnqueueSuccess;
use crate::commands::utils::api_adapters::models::image::tauri_image_model_to_generation_model::tauri_image_model_to_generation_model;
use crate::commands::utils::api_adapters::models::image::tauri_image_model_to_router_model::tauri_image_model_to_router_model;
use crate::credentials::auth_credential::AuthCredential;
use crate::credentials::find_service_credentials::find_first_credential_for_service;
use crate::services::asset_uploads::asset_upload_ledger::AssetUploadLedger;
use crate::services::grok::state::grok_websockets::GrokWebsockets;
use crate::services::grok::util::refresh_grok_statsig::{refresh_grok_statsig_blocking, GrokStatsigRefreshOutcome};
use crate::services::midjourney::state::midjourney_live_session::MidjourneyLiveSession;
use crate::services::midjourney::utils::extract_midjourney_user_id_from_cookies::extract_midjourney_user_id_from_cookie_header;
use crate::services::midjourney::utils::midjourney_browser_profile::midjourney_browser_profile;
use crate::state::data_dir::app_data_root::AppDataRoot;
use tauri::AppHandle;
use crate::utils::services::artcraft_api_host::maybe_artcraft_api_host_for_service;

/// Credential-driven image generation: resolve the stored credential named by
/// the request's `credential_id`, then invoke the router for that credential's
/// service. This is the single enqueue entry point — there is no separate
/// per-provider "handler" layer.
pub async fn enqueue_image_generation(
  request: &TauriGenerateImageRequest,
  app: &AppHandle,
  app_data_root: &AppDataRoot,
  mj_session: &MidjourneyLiveSession,
  grok_websockets: &GrokWebsockets,
  ledger: &AssetUploadLedger,
) -> Result<TaskEnqueueSuccess, GenerateError> {
  // Reject unusable media sources (missing local files, empty bytes) before
  // any provider work.
  validate_sources(request.reference_media_sources().iter().flatten())?;

  let credential = resolve_generation_credential(request.credential_id.as_deref(), app_data_root)?;
  let credential = maybe_reroute_grok_model_to_grok_credential(request, credential, app_data_root)?;

  info!(
    "enqueue_image_generation: credential={} service={}",
    credential.id.as_str(),
    credential.service,
  );

  match credential.service {
    GenerationSource::Artcraft
    | GenerationSource::ArtcraftLocal
    | GenerationSource::ArtcraftCookies => enqueue_via_artcraft(request, &credential, ledger).await,

    GenerationSource::FalApi => {
      let api_key = credential.api_key().ok_or_else(|| {
        credential_not_usable(&credential, "the FAL credential has no API key")
      })?;
      enqueue_via_fal(request, &api_key.api_key, app_data_root, ledger).await
    }

    GenerationSource::MidjourneyCookies | GenerationSource::Midjourney => {
      enqueue_via_midjourney(request, &credential, mj_session).await
    }

    GenerationSource::GrokCookies | GenerationSource::Grok => {
      enqueue_via_grok(request, app, app_data_root, &credential, grok_websockets).await
    }

    GenerationSource::HiggsfieldCookies | GenerationSource::Higgsfield => {
      enqueue_via_higgsfield(Some(app), request, &credential, ledger).await
    }

    other => Err(credential_not_usable(
      &credential,
      &format!("accounts for service {} can't generate images yet", other),
    )),
  }
}

// ── Artcraft ──

/// Generate via the router's Artcraft provider, authenticating with the stored
/// session cookies. Raw canvas/scene/mask bytes are uploaded to ArtCraft first
/// (with the same credential) so the router request only carries media tokens.
async fn enqueue_via_artcraft(
  request: &TauriGenerateImageRequest,
  credential: &AuthCredential,
  ledger: &AssetUploadLedger,
) -> Result<TaskEnqueueSuccess, GenerateError> {
  let tauri_model = request.model.ok_or(GenerateError::no_model_specified())?;

  let router_model = tauri_image_model_to_router_model(tauri_model).ok_or(
    GenerateError::NotYetImplemented(format!(
      "Model {:?} can't be generated with this account yet",
      tauri_model
    )),
  )?;

  let api_host = maybe_artcraft_api_host_for_service(credential.service)
      .expect("caller only routes ArtCraft services here");

  let creds = storyteller_creds_from_credential(credential)?;

  // Upload any raw image bytes before the generate request. Local-file /
  // bytes references also upload here: ArtCraft's API is token-native and
  // the media must reach ArtCraft anyway.
  let semantic_media_files = parse_semantic_media_files(request, &creds, &api_host).await?;
  let reference_tokens = sources_to_artcraft_tokens(
    request.reference_media_sources(), ArtcraftMediaKind::Image, Some(&creds), &api_host, ledger,
  ).await?;
  let image_inputs = collect_artcraft_image_inputs(&semantic_media_files, reference_tokens);

  let router_request = GenerateImageRequestBuilder {
    model: router_model,
    provider: RouterProvider::Artcraft,
    prompt: request.prompt.clone(),
    image_inputs,
    resolution: request.resolution.map(convert_resolution),
    aspect_ratio: request.aspect_ratio.map(convert_aspect_ratio),
    quality: request.quality.map(convert_quality),
    image_batch_count: request.batch_size.map(|n| n as u16),
    horizontal_angle: request.adjust_horizontal_angle,
    vertical_angle: request.adjust_vertical_angle,
    zoom: request.adjust_zoom,
    request_mismatch_mitigation_strategy: RequestMismatchMitigationStrategy::PayMoreUpgrade,
    generation_mode_mismatch_strategy: Some(GenerationModeMismatchStrategy::GenerateAnyway),
    idempotency_token: None,
  };

  let client = RouterClient::Artcraft(RouterArtcraftClient::new(api_host, creds));
  let generation_request = build_direct_request(router_request, "Artcraft")?;

  let response = generation_request.send_request(&client).await.map_err(|err| {
    warn!("Artcraft router image generation failed: {:?}", err);
    GenerateError::from(err)
  })?;

  let payload = response
      .get_artcraft_payload()
      .ok_or(GenerateError::ResponseHadNoJobTokens)?;

  Ok(TaskEnqueueSuccess {
    task_type: TaskType::ImageGeneration,
    model: Some(tauri_image_model_to_generation_model(tauri_model)),
    provider: GenerationSource::Artcraft,
    provider_job_id: Some(payload.inference_job_token.to_string()),
    is_batch_generation: request.batch_size.unwrap_or(1) > 1,
    maybe_queue_status_url: None,
    maybe_queue_response_url: None,
    maybe_prompt_token: None,
  })
}

/// Reference images for the Artcraft router request: canvas first, then scene,
/// then the un-semantic reference images (already resolved to tokens).
fn collect_artcraft_image_inputs(
  semantic_media_files: &SemanticMediaFiles,
  reference_tokens: Option<Vec<MediaFileToken>>,
) -> Option<ImageListRef> {
  let mut tokens: Vec<MediaFileToken> = Vec::new();

  if let Some(canvas_token) = &semantic_media_files.canvas_image_media_token {
    tokens.push(canvas_token.clone());
  }
  if let Some(scene_token) = &semantic_media_files.scene_image_media_token {
    tokens.push(scene_token.clone());
  }
  tokens.extend(reference_tokens.unwrap_or_default());

  if tokens.is_empty() {
    None
  } else {
    Some(ImageListRef::MediaFileTokens(tokens))
  }
}

// ── Fal ──

/// Generate via the router's FAL provider using a stored FAL API key. FAL only
/// accepts URLs, so media tokens resolve to CDN URLs — and local files / raw
/// bytes upload to ArtCraft first to get one (FAL is the ONLY non-ArtCraft
/// provider allowed to route local media through the ArtCraft cloud).
async fn enqueue_via_fal(
  request: &TauriGenerateImageRequest,
  api_key: &str,
  app_data_root: &AppDataRoot,
  ledger: &AssetUploadLedger,
) -> Result<TaskEnqueueSuccess, GenerateError> {
  let tauri_model = request.model.ok_or(GenerateError::no_model_specified())?;

  let router_model = tauri_image_model_to_router_model(tauri_model).ok_or(
    GenerateError::NotYetImplemented(format!(
      "Model {:?} is not supported via the FAL router path",
      tauri_model
    )),
  )?;

  let image_inputs = resolve_fal_image_inputs(request, &ApiHost::Storyteller, app_data_root, ledger).await?;

  let router_request = GenerateImageRequestBuilder {
    model: router_model,
    provider: RouterProvider::Fal,
    prompt: request.prompt.clone(),
    image_inputs,
    resolution: request.resolution.map(convert_resolution),
    aspect_ratio: request.aspect_ratio.map(convert_aspect_ratio),
    quality: request.quality.map(convert_quality),
    image_batch_count: request.batch_size.map(|n| n as u16),
    horizontal_angle: request.adjust_horizontal_angle,
    vertical_angle: request.adjust_vertical_angle,
    zoom: request.adjust_zoom,
    request_mismatch_mitigation_strategy: RequestMismatchMitigationStrategy::PayMoreUpgrade,
    generation_mode_mismatch_strategy: Some(GenerationModeMismatchStrategy::GenerateAnyway),
    idempotency_token: None,
  };

  let client = RouterClient::Fal(RouterFalClient::new_polling_only_from_raw_key(api_key));
  let generation_request = build_direct_request(router_request, "FAL")?;

  let response = generation_request.send_request(&client).await.map_err(|err| {
    warn!("FAL image generation failed: {:?}", err);
    GenerateError::from(err)
  })?;

  build_fal_task_enqueue_success(tauri_model, response, request.batch_size.unwrap_or(1) > 1)
}

async fn resolve_fal_image_inputs(
  request: &TauriGenerateImageRequest,
  api_host: &ApiHost,
  app_data_root: &AppDataRoot,
  ledger: &AssetUploadLedger,
) -> Result<Option<ImageListRef>, GenerateError> {
  let mut canvas_and_scene_tokens: Vec<MediaFileToken> = Vec::new();

  if let Some(canvas_token) = &request.canvas_image_media_token {
    canvas_and_scene_tokens.push(canvas_token.clone());
  }
  if let Some(scene_token) = &request.scene_image_media_token {
    canvas_and_scene_tokens.push(scene_token.clone());
  }

  let mut urls = map_media_file_tokens_to_cdn_urls(&canvas_and_scene_tokens, api_host).await?;

  // Local files / raw bytes upload to ArtCraft, authenticated with any
  // stored ArtCraft account (anonymous otherwise).
  let maybe_artcraft_credential = find_first_credential_for_service(app_data_root, GenerationSource::ArtcraftCookies)
      .or_else(|| find_first_credential_for_service(app_data_root, GenerationSource::Artcraft));
  let maybe_creds = maybe_artcraft_credential.as_ref()
      .and_then(|credential| storyteller_creds_from_credential(credential).ok());
  let reference_urls = image_sources_to_fal_urls(
    request.reference_media_sources(), maybe_creds.as_ref(), api_host, ledger,
  ).await?;
  urls.extend(reference_urls.unwrap_or_default());

  if urls.is_empty() {
    return Ok(None);
  }
  Ok(Some(ImageListRef::Urls(urls)))
}

fn build_fal_task_enqueue_success(
  tauri_model: TauriImageModel,
  response: GenerateImageResponse,
  is_batch_generation: bool,
) -> Result<TaskEnqueueSuccess, GenerateError> {
  let fal_payload = response
      .get_fal_payload()
      .ok_or(GenerateError::ResponseHadNoJobTokens)?;

  let provider_job_id = fal_payload
      .request_id
      .or(fal_payload.gateway_request_id)
      .ok_or(GenerateError::ResponseHadNoJobTokens)?;

  Ok(TaskEnqueueSuccess {
    task_type: TaskType::ImageGeneration,
    model: Some(tauri_image_model_to_generation_model(tauri_model)),
    provider: GenerationSource::Fal,
    provider_job_id: Some(provider_job_id),
    is_batch_generation,
    maybe_queue_status_url: fal_payload.maybe_status_url,
    maybe_queue_response_url: fal_payload.maybe_response_url,
    maybe_prompt_token: None,
  })
}

// ── Midjourney (first-party, cookie-session) ──

/// Generate via the router's first-party Midjourney provider, authenticating
/// with the stored session cookies. The `user_id` (needed to form the
/// submit channel) is taken from the in-memory session, resolving it once via
/// the index page when absent.
async fn enqueue_via_midjourney(
  request: &TauriGenerateImageRequest,
  credential: &AuthCredential,
  mj_session: &MidjourneyLiveSession,
) -> Result<TaskEnqueueSuccess, GenerateError> {
  let tauri_model = request.model.ok_or(GenerateError::no_model_specified())?;

  let router_model = tauri_image_model_to_router_model(tauri_model).ok_or(
    GenerateError::NotYetImplemented(format!(
      "Model {:?} is not supported for a Midjourney account",
      tauri_model
    )),
  )?;

  let cookie = credential.cookies().ok_or_else(|| {
    credential_not_usable(credential, "the Midjourney credential has no cookies")
  })?;
  let cookie_header = cookie.cookie_header();

  let browser = midjourney_browser_profile();

  info!(
    "Midjourney enqueue: cookie_header_len={}, has_auth_i={}, has_auth_r={}, has_cf_clearance={}, browser={}",
    cookie_header.len(),
    cookie_header.contains("__Host-Midjourney.AuthUserTokenV3_i"),
    cookie_header.contains("__Host-Midjourney.AuthUserTokenV3_r"),
    cookie_header.contains("cf_clearance"),
    browser.label(),
  );

  // Resolve the Midjourney user id (needed for `singleplayer_{user_id}`).
  // Prefer the live session, then the auth cookie's JWT (no network), and only
  // fall back to the Cloudflare-gated index page as a last resort.
  let user_id = match mj_session.user_id() {
    Some(user_id) => user_id,
    None => match extract_midjourney_user_id_from_cookie_header(&cookie_header) {
      Some(user_id) => {
        info!("Resolved Midjourney user id from auth cookie JWT: {}", user_id.as_str());
        mj_session.set_identity(user_id.clone(), None);
        user_id
      }
      None => {
        warn!("Could not read Midjourney user id from the auth cookie; falling back to the index page.");
        let info = get_user_info(GetUserInfoArgs {
          cookie_header: &cookie_header,
          hostname: None,
          browser: Some(browser.clone()),
        })
        .await
        .map_err(|err| {
          warn!("Could not read Midjourney user info from index page: {:?}", err);
          credential_not_usable(
            credential,
            "could not read your Midjourney account info; the session may have expired",
          )
        })?;

        let user_id = info.user_id.ok_or_else(|| {
          credential_not_usable(credential, "Midjourney did not return a user id")
        })?;
        mj_session.set_identity(user_id.clone(), info.websocket_token);
        user_id
      }
    },
  };

  let router_request = GenerateImageRequestBuilder {
    model: router_model,
    provider: RouterProvider::Midjourney,
    prompt: request.prompt.clone(),
    image_inputs: None,
    resolution: request.resolution.map(convert_resolution),
    aspect_ratio: request.aspect_ratio.map(convert_aspect_ratio),
    quality: request.quality.map(convert_quality),
    image_batch_count: request.batch_size.map(|n| n as u16),
    horizontal_angle: request.adjust_horizontal_angle,
    vertical_angle: request.adjust_vertical_angle,
    zoom: request.adjust_zoom,
    request_mismatch_mitigation_strategy: RequestMismatchMitigationStrategy::PayMoreUpgrade,
    generation_mode_mismatch_strategy: Some(GenerationModeMismatchStrategy::GenerateAnyway),
    idempotency_token: None,
  };

  let client = RouterClient::Midjourney(RouterMidjourneyClient::new(
    cookie_header,
    user_id,
    browser,
  ));
  let generation_request = build_direct_request(router_request, "Midjourney")?;

  let response = generation_request.send_request(&client).await.map_err(|err| {
    warn!("Midjourney image generation failed: {:?}", err);
    GenerateError::from(err)
  })?;

  let payload = response
      .get_midjourney_payload()
      .ok_or(GenerateError::ResponseHadNoJobTokens)?;

  info!("Midjourney image generation enqueued: job_id={}", payload.job_id);

  // Stash the prompt so the (metadata-less) websocket completion path can
  // still attribute the created Storyteller prompt.
  if let Some(prompt) = &request.prompt {
    mj_session.record_pending_prompt(payload.job_id.clone(), prompt.clone());
  }

  Ok(TaskEnqueueSuccess {
    task_type: TaskType::ImageGeneration,
    model: Some(tauri_image_model_to_generation_model(tauri_model)),
    // NB: the task uses the bare `Midjourney` provider (what the completion
    // pollers query), not the `MidjourneyCookies` credential service.
    provider: GenerationSource::Midjourney,
    provider_job_id: Some(payload.job_id),
    // NB: Midjourney always produces a 2x2 grid.
    is_batch_generation: true,
    maybe_queue_status_url: None,
    maybe_queue_response_url: None,
    maybe_prompt_token: None,
  })
}

// ── Grok Imagine (first-party, cookie-session) ──

/// Grok Imagine models only run on the user's own Grok session. If the picker
/// still has another account selected, quietly use the first stored Grok
/// account instead; without one, ask the user to log into Grok.
fn maybe_reroute_grok_model_to_grok_credential(
  request: &TauriGenerateImageRequest,
  credential: AuthCredential,
  app_data_root: &AppDataRoot,
) -> Result<AuthCredential, GenerateError> {
  if !is_grok_imagine_model(request.model) || is_grok_service(credential.service) {
    return Ok(credential);
  }

  match find_first_credential_for_service(app_data_root, GenerationSource::GrokCookies) {
    Some(grok_credential) => {
      info!(
        "Grok Imagine selected with a {} account; using Grok credential {} instead",
        credential.service,
        grok_credential.id.as_str(),
      );
      Ok(grok_credential)
    }
    None => {
      warn!("Grok Imagine selected but no Grok credential is stored");
      Err(GenerateError::needs_grok_credentials())
    }
  }
}

fn is_grok_imagine_model(model: Option<TauriImageModel>) -> bool {
  matches!(model, Some(TauriImageModel::GrokImage | TauriImageModel::GrokImagineImageQuality))
}

fn is_grok_service(service: GenerationSource) -> bool {
  matches!(service, GenerationSource::GrokCookies | GenerationSource::Grok)
}

/// Enqueue via the router's first-party Grok Imagine provider.
///
/// Sends the prompt on the account's live imagine websocket (shared via
/// [`GrokWebsockets`]) and returns Grok's request id as the task's
/// `provider_job_id`; the third-party task polling thread collects the images
/// off the same socket later.
///
/// Statsig policy: use the credential file's latest statsig as long as it
/// isn't expired (refresh first if it is). If the enqueue fails, warn, force a
/// refresh (and reconnect the socket), and retry once; if that also fails the
/// error goes back to the frontend as a flashed message.
async fn enqueue_via_grok(
  request: &TauriGenerateImageRequest,
  app: &AppHandle,
  app_data_root: &AppDataRoot,
  credential: &AuthCredential,
  grok_websockets: &GrokWebsockets,
) -> Result<TaskEnqueueSuccess, GenerateError> {
  let tauri_model = request.model.ok_or(GenerateError::no_model_specified())?;

  let router_model = match tauri_model {
    TauriImageModel::GrokImage => RouterImageModel::GrokImagineImage,
    TauriImageModel::GrokImagineImageQuality => RouterImageModel::GrokImagineImageQuality,
    other => {
      return Err(GenerateError::NotYetImplemented(format!(
        "Model {:?} is not supported for a Grok account",
        other
      )));
    }
  };

  let cookie_header = credential
      .cookies()
      .map(|cookie| cookie.cookie_header())
      .ok_or_else(|| credential_not_usable(credential, "the Grok credential has no cookies"))?;

  let prompt = request.prompt.clone().unwrap_or_default().trim().to_string();
  if prompt.is_empty() {
    return Err(GenerateError::BadInput(BadInputReason::WrongImageArguments(
      "Grok Imagine needs a text prompt".to_string(),
    )));
  }

  let router_request = GenerateImageRequestBuilder {
    model: router_model,
    provider: RouterProvider::Grok,
    prompt: Some(prompt),
    image_inputs: None,
    resolution: request.resolution.map(convert_resolution),
    aspect_ratio: request.aspect_ratio.map(convert_aspect_ratio),
    quality: request.quality.map(convert_quality),
    image_batch_count: request.batch_size.map(|n| n as u16),
    horizontal_angle: request.adjust_horizontal_angle,
    vertical_angle: request.adjust_vertical_angle,
    zoom: request.adjust_zoom,
    request_mismatch_mitigation_strategy: RequestMismatchMitigationStrategy::PayMoreUpgrade,
    generation_mode_mismatch_strategy: Some(GenerationModeMismatchStrategy::GenerateAnyway),
    idempotency_token: None,
  };

  let generation_request = build_direct_request(router_request, "Grok")?;

  let websocket = grok_websockets
      .get_or_connect(&credential.id, &cookie_header)
      .await
      .map_err(|err| {
        error!("Could not open the Grok image websocket for credential {}: {:?}", credential.id.as_str(), err);
        GenerateError::ProviderRejected(format!("Couldn't connect to Grok: {}", err))
      })?;

  // Attempt 1: stored statsig if fresh, else refresh first.
  ensure_grok_statsig_fresh(app, app_data_root, credential).await;

  let first_error = match dispatch_grok_once(app_data_root, credential, &cookie_header, &websocket, &generation_request, 1).await {
    Ok(request_id) => return Ok(grok_enqueue_success(tauri_model, request_id)),
    Err(err) => err,
  };

  // Attempt 2: the failure may be a stale statsig/session — force a refresh,
  // reconnect the socket, and retry once.
  warn!(
    "Grok enqueue attempt 1 failed ({}); checking whether a new statsig/secret is needed and refreshing.",
    first_error,
  );
  match refresh_grok_statsig_blocking(app, app_data_root, true).await {
    Ok(outcome) => info!("Grok statsig refresh outcome: {:?}", outcome),
    Err(err) => warn!("Grok statsig refresh failed: {:?}", err),
  }
  if let Err(err) = websocket.reconnect().await {
    warn!("Grok image websocket reconnect failed before retry: {}", err);
  }

  match dispatch_grok_once(app_data_root, credential, &cookie_header, &websocket, &generation_request, 2).await {
    Ok(request_id) => Ok(grok_enqueue_success(tauri_model, request_id)),
    Err(second_error) => {
      error!(
        "Grok enqueue failed after statsig refresh + retry (attempt 1: {}; attempt 2: {})",
        first_error, second_error,
      );
      Err(GenerateError::ProviderRejected(format!(
        "Grok image generation couldn't be started, even after refreshing your Grok session: {}",
        second_error,
      )))
    }
  }
}

/// Use the credential file's latest statsig as long as it isn't expired; if it
/// is, fire off a refresh first and wait for it.
async fn ensure_grok_statsig_fresh(app: &AppHandle, app_data_root: &AppDataRoot, credential: &AuthCredential) {
  if fresh_grok_statsig(credential).is_some() {
    info!("Grok enqueue: using the stored statsig (not expired).");
    return;
  }

  warn!("Grok enqueue: stored statsig is missing or expired; refreshing before dispatch.");
  match refresh_grok_statsig_blocking(app, app_data_root, false).await {
    Ok(GrokStatsigRefreshOutcome::Refreshed) => info!("Grok enqueue: statsig refreshed."),
    Ok(outcome) => warn!("Grok enqueue: statsig refresh outcome {:?}; dispatching anyway.", outcome),
    Err(err) => warn!("Grok enqueue: statsig refresh failed ({:?}); dispatching anyway.", err),
  }
}

/// One send through the router, with the statsig re-read from disk so a
/// refresh between attempts is picked up. Returns Grok's request id.
async fn dispatch_grok_once(
  app_data_root: &AppDataRoot,
  credential: &AuthCredential,
  cookie_header: &str,
  websocket: &GrokImageWebsocket,
  generation_request: &ImageGenerationRequest,
  attempt: u32,
) -> Result<String, String> {
  // Re-read the credential file: a refresh may have just updated the statsig.
  let maybe_statsig = app_data_root
      .credentials_dir()
      .find_credential_by_id(credential.id.as_str())
      .ok()
      .flatten()
      .as_ref()
      .and_then(fresh_grok_statsig);

  info!(
    "Grok enqueue: dispatch attempt {} (credential={}, cookie_header_len={}, has_statsig={})",
    attempt, credential.id.as_str(), cookie_header.len(), maybe_statsig.is_some(),
  );

  let client = RouterClient::Grok(RouterGrokClient::new(cookie_header.to_string(), maybe_statsig, websocket.clone()));

  let response = generation_request.send_request(&client).await.map_err(|err| {
    warn!("Grok enqueue: router dispatch attempt {} failed: {:?}", attempt, err);
    match &err {
      ArtcraftRouterError::Provider(provider_error) => provider_error.to_string(),
      other => format!("{other:?}"),
    }
  })?;

  let payload = response
      .get_grok_payload()
      .ok_or_else(|| "the router returned a non-Grok response".to_string())?;

  info!("Grok enqueue: prompt sent, request_id={}", payload.request_id);
  Ok(payload.request_id)
}

/// The credential file's latest statsig, if it hasn't expired.
fn fresh_grok_statsig(credential: &AuthCredential) -> Option<String> {
  credential
      .cookies()?
      .grok_data
      .as_ref()?
      .latest_statsig_if_fresh(Utc::now())
      .map(|statsig| statsig.to_string())
}

fn grok_enqueue_success(tauri_model: TauriImageModel, request_id: String) -> TaskEnqueueSuccess {
  TaskEnqueueSuccess {
    task_type: TaskType::ImageGeneration,
    model: Some(tauri_image_model_to_generation_model(tauri_model)),
    // NB: the task uses the bare `Grok` provider (what the polling thread
    // queries), not the `GrokCookies` credential service.
    provider: GenerationSource::Grok,
    provider_job_id: Some(request_id),
    // NB: Grok's imagine websocket returns two images per prompt.
    is_batch_generation: true,
    maybe_queue_status_url: None,
    maybe_queue_response_url: None,
    maybe_prompt_token: None,
  }
}

// ── Shared ──

/// Build a directly-sendable request, rejecting the draft phase (none of these
/// providers use it on the text/token path).
fn build_direct_request(
  router_request: GenerateImageRequestBuilder,
  provider_label: &str,
) -> Result<ImageGenerationRequest, GenerateError> {
  match router_request.build2() {
    Ok(ImageGenerationDraftOrRequest::Request(generation_request)) => Ok(generation_request),
    Ok(ImageGenerationDraftOrRequest::Draft(draft)) => {
      warn!("{} build unexpectedly produced a draft: {:?}", provider_label, draft);
      Err(GenerateError::NotYetImplemented(format!(
        "{} requests should not require a draft phase",
        provider_label
      )))
    }
    Err(err) => {
      warn!("Could not build {} router request: {:?}", provider_label, err);
      Err(err.into())
    }
  }
}
