//! Image generation on the user's own Higgsfield account, via the router's
//! first-party Higgsfield provider.

use core_types::enums::generation_source::GenerationSource;
use log::info;
use router::api::image_list_ref::ImageListRef;
use router::api::router_provider::RouterProvider;
use router::client::generation_mode_mismatch_strategy::GenerationModeMismatchStrategy;
use router::client::request_mismatch_mitigation_strategy::RequestMismatchMitigationStrategy;
use router::generate::generate_image::generate_image_request_builder::GenerateImageRequestBuilder;
use sqlite_identifiers::enums::task_type::TaskType;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;

use crate::commands::generate::common::higgsfield_generation::{
  higgsfield_media_url_map, higgsfield_router_client, join_higgsfield_job_ids, send_higgsfield_image_request,
};
use crate::commands::generate::common::media_source_conversion::collect_source_tokens;
use crate::commands::generate::common::tauri_media_source::TauriMediaSource;
use crate::commands::generate::generate_error::GenerateError;
use crate::commands::generate::generate_image::tauri_generate_image_request::TauriGenerateImageRequest;
use crate::commands::generate::generate_image::utils::convert_enums_to_router::{convert_aspect_ratio, convert_quality, convert_resolution};
use crate::commands::generate::task_enqueue_success::TaskEnqueueSuccess;
use crate::commands::utils::api_adapters::models::image::tauri_image_model_to_generation_model::tauri_image_model_to_generation_model;
use crate::commands::utils::api_adapters::models::image::tauri_image_model_to_router_model::tauri_image_model_to_router_model;
use crate::credentials::auth_credential::AuthCredential;
use tauri::AppHandle;
use crate::services::asset_uploads::asset_upload_ledger::AssetUploadLedger;

/// Enqueue via the router's first-party Higgsfield provider.
///
/// Reference images (canvas, scene, and the prompt box's three-way image
/// sources) go to the router; local files and raw bytes upload straight to
/// Higgsfield, while ArtCraft media tokens (cloud-library picks) resolve to
/// CDN URLs through the map built here before the router downloads and
/// re-uploads them. The returned task carries every job id of the
/// Higgsfield job set so the poller can follow a batch.
pub async fn enqueue_via_higgsfield(
  maybe_app: Option<&AppHandle>,
  request: &TauriGenerateImageRequest,
  credential: &AuthCredential,
  ledger: &AssetUploadLedger,
) -> Result<TaskEnqueueSuccess, GenerateError> {
  let tauri_model = request.model.ok_or(GenerateError::no_model_specified())?;

  let router_model = tauri_image_model_to_router_model(tauri_model).ok_or(
    GenerateError::NotYetImplemented(format!(
      "Model {:?} is not supported for a Higgsfield account",
      tauri_model
    )),
  )?;

  let sources = collect_reference_sources(request);
  let media_url_map = higgsfield_media_url_map(&collect_source_tokens(&sources)).await?;
  let image_inputs = (!sources.is_empty()).then(|| {
    ImageListRef::Sources(sources.into_iter().map(TauriMediaSource::into_image_ref).collect())
  });

  let router_request = GenerateImageRequestBuilder {
    model: router_model,
    provider: RouterProvider::Higgsfield,
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

  let client = higgsfield_router_client(credential)?;
  // References this account already uploaded are reused by media id.
  let upload_cache = ledger.for_higgsfield_credential(&credential.id);
  let response = send_higgsfield_image_request(maybe_app, credential, router_request, &client, &media_url_map, &upload_cache).await?;

  let payload = response
      .get_higgsfield_payload()
      .ok_or(GenerateError::ResponseHadNoJobTokens)?;

  info!(
    "Higgsfield image generation enqueued: job_set={} jobs={:?}",
    payload.job_set_id, payload.job_ids,
  );

  Ok(TaskEnqueueSuccess {
    task_type: TaskType::ImageGeneration,
    model: Some(tauri_image_model_to_generation_model(tauri_model)),
    // NB: the task uses the bare `Higgsfield` provider (what the polling
    // thread queries), not the `HiggsfieldCookies` credential service.
    provider: GenerationSource::Higgsfield,
    provider_job_id: Some(join_higgsfield_job_ids(&payload.job_ids)),
    is_batch_generation: payload.job_ids.len() > 1,
    maybe_queue_status_url: None,
    maybe_queue_response_url: None,
    maybe_prompt_token: None,
  })
}

/// Reference images in priority order: canvas first, then scene, then the
/// un-semantic references (three-way sources). (Raw canvas / scene bytes
/// aren't accepted on this path — they'd need an ArtCraft session to become
/// tokens.)
fn collect_reference_sources(request: &TauriGenerateImageRequest) -> Vec<TauriMediaSource> {
  let mut sources: Vec<TauriMediaSource> = Vec::new();
  if let Some(canvas_token) = &request.canvas_image_media_token {
    sources.push(TauriMediaSource::MediaFileToken { token: canvas_token.clone() });
  }
  if let Some(scene_token) = &request.scene_image_media_token {
    sources.push(TauriMediaSource::MediaFileToken { token: scene_token.clone() });
  }
  sources.extend(request.reference_media_sources().unwrap_or_default());
  sources
}

/// Live smoke tests that hit the REAL Higgsfield API with the REAL stored
/// credential and SPEND HIGGSFIELD CREDITS. `#[ignore]`; run explicitly:
///   SQLX_OFFLINE=true cargo test -p artcraftx live_higgsfield_image -- --ignored --nocapture
#[cfg(test)]
mod live_higgsfield_image_tests {
  use artcraft_client::enums::common::generation::common_aspect_ratio::CommonAspectRatio as EnumsAspectRatio;
  use artcraft_client::enums::common::generation::common_quality::CommonQuality as EnumsQuality;
  use sqlite_identifiers::ids::media_file_token::MediaFileToken;

  use crate::commands::generate::generate_image::tauri_image_model::TauriImageModel;
  use crate::state::data_dir::app_data_root::AppDataRoot;
  use crate::state::database::local_files_database::LocalFilesDatabase;

  use super::*;

  async fn ledger() -> AssetUploadLedger {
    let app_data_root = AppDataRoot::create_default().expect("app data root");
    AssetUploadLedger::new(LocalFilesDatabase::connect(&app_data_root).await.expect("local files db"))
  }

  /// A production ArtCraft media file (see `test_data::web::image_media_tokens`).
  const JUNO_AT_LAKE_MEDIA_TOKEN: &str = "m_m1bz02z1kkzanxy6rb4vk1kvq9de9g";

  fn higgsfield_credential() -> AuthCredential {
    let app_data_root = AppDataRoot::create_default().expect("app data root");
    app_data_root
        .credentials_dir()
        .load_credentials()
        .expect("load credentials")
        .into_iter()
        .find(|c| c.service == GenerationSource::HiggsfieldCookies)
        .expect("no `higgsfield_cookies` credential on disk; log into Higgsfield via the app first")
  }

  #[tokio::test]
  #[ignore] // live: spends Higgsfield credits
  async fn live_higgsfield_image_text_to_image() {
    let credential = higgsfield_credential();
    let request = TauriGenerateImageRequest {
      credential_id: Some(credential.id.as_str().to_string()),
      model: Some(TauriImageModel::NanoBanana2Lite),
      prompt: Some("a shiba inu doing a kickflip on a skateboard, golden hour".to_string()),
      aspect_ratio: Some(EnumsAspectRatio::WideSixteenByNine),
      quality: Some(EnumsQuality::Low),
      batch_size: Some(1),
      ..Default::default()
    };

    let success = enqueue_via_higgsfield(None, &request, &credential, &ledger().await).await.expect("enqueue should succeed");
    println!("[live] Higgsfield image enqueued: provider_job_id={:?}", success.provider_job_id);
    assert_eq!(success.provider, GenerationSource::Higgsfield);
    assert!(success.provider_job_id.is_some());
  }

  #[tokio::test]
  #[ignore] // live: spends Higgsfield credits and uploads a reference image
  async fn live_higgsfield_image_edit_with_reference() {
    let credential = higgsfield_credential();
    let request = TauriGenerateImageRequest {
      credential_id: Some(credential.id.as_str().to_string()),
      model: Some(TauriImageModel::NanoBanana2),
      prompt: Some("the same scene at night under the northern lights".to_string()),
      image_media_tokens: Some(vec![MediaFileToken::new_from_str(JUNO_AT_LAKE_MEDIA_TOKEN)]),
      batch_size: Some(1),
      ..Default::default()
    };

    let success = enqueue_via_higgsfield(None, &request, &credential, &ledger().await).await.expect("enqueue should succeed");
    println!("[live] Higgsfield image edit enqueued: provider_job_id={:?}", success.provider_job_id);
    assert!(success.provider_job_id.is_some());
  }
}
