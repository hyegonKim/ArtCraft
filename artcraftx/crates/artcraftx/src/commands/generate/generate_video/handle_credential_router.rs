use log::info;

use crate::commands::utils::api_adapters::models::video::tauri_video_model_to_generation_model::tauri_video_model_to_generation_model;
use crate::commands::utils::api_adapters::models::video::tauri_video_model_to_router_model::tauri_video_model_to_router_model;
use crate::commands::generate::generate_error::GenerateError;
use crate::commands::generate::task_enqueue_success::TaskEnqueueSuccess;
use crate::commands::generate::common::generation_credential::{credential_not_usable, resolve_generation_credential, storyteller_creds_from_credential};
use crate::commands::generate::generate_video::artcraft::handle_artcraft_video_via_router::handle_artcraft_video_via_router;
use crate::commands::generate::generate_video::higgsfield::handle_higgsfield_video_via_router::handle_higgsfield_video_via_router;
use crate::commands::generate::generate_video::request::TauriGenerateVideoRequest;
use crate::utils::services::artcraft_api_host::maybe_artcraft_api_host_for_service;
use crate::credentials::auth_credential::AuthCredential;
use core_types::enums::generation_source::GenerationSource;
use crate::services::asset_uploads::asset_upload_ledger::AssetUploadLedger;
use crate::state::data_dir::app_data_root::AppDataRoot;
use tauri::AppHandle;

/// Credential-driven video generation: resolve the stored credential named
/// by the request's `credential_id`, then route to that credential's service
/// via the router. The router owns upload + dispatch mechanics per provider.
pub async fn handle_credential_router(
  request: &TauriGenerateVideoRequest,
  // Optional so headless callers (live tests) can skip UI notices.
  maybe_app: Option<&AppHandle>,
  app_data_root: &AppDataRoot,
  ledger: &AssetUploadLedger,
) -> Result<TaskEnqueueSuccess, GenerateError> {
  let credential = resolve_generation_credential(
    request.credential_id.as_deref(),
    app_data_root,
  )?;

  info!(
    "handle_credential_router (video): credential={} service={}",
    credential.id.as_str(),
    credential.service,
  );

  match credential.service {
    GenerationSource::Artcraft
    | GenerationSource::ArtcraftLocal
    | GenerationSource::ArtcraftCookies => {
      handle_artcraft_credential(request, &credential, ledger).await
    }
    GenerationSource::HiggsfieldCookies
    | GenerationSource::Higgsfield => {
      // Reference media is uploaded (with Higgsfield's 15-30s IP/likeness
      // scan) or reused from the ledger; the notices for either come from
      // the router's upload observer, so nothing is announced up front.
      handle_higgsfield_video_via_router(maybe_app, request, &credential, ledger).await
    }
    other => Err(credential_not_usable(
      &credential,
      &format!("accounts for service {} can't generate videos yet", other),
    )),
  }
}

/// Generate via the router's Artcraft provider, authenticating with the
/// stored session cookies. Video requests carry media tokens only, which
/// Artcraft accepts directly — no pre-upload step is needed.
async fn handle_artcraft_credential(
  request: &TauriGenerateVideoRequest,
  credential: &AuthCredential,
  ledger: &AssetUploadLedger,
) -> Result<TaskEnqueueSuccess, GenerateError> {
  let tauri_model = request.model.ok_or(GenerateError::no_model_specified())?;

  let router_model = tauri_video_model_to_router_model(tauri_model);
  let generation_model = tauri_video_model_to_generation_model(tauri_model);

  let api_host = maybe_artcraft_api_host_for_service(credential.service)
      .expect("caller only routes ArtCraft services here");

  let creds = storyteller_creds_from_credential(credential)?;

  handle_artcraft_video_via_router(
    request,
    &api_host,
    &creds,
    router_model,
    generation_model,
    ledger,
  ).await
}

/// Live smoke test that hits the REAL production API with the REAL stored
/// credential and SPENDS CREDITS. `#[ignore]`; run explicitly:
///   SQLX_OFFLINE=true cargo test -p artcraftx live_generation -- --ignored --nocapture
#[cfg(test)]
mod live_generation_tests {
  use artcraft_client::endpoints::media_files::upload_image_media_file_from_bytes::{upload_image_media_file_from_bytes, ImageType, UploadImageBytesArgs};
  use artcraft_client::utils::api_host::ApiHost;
  use router::api::router_resolution::RouterResolution;

  use crate::commands::generate::generate_video::request::TauriVideoModel;

  use super::*;

  #[tokio::test]
  #[ignore] // live: spends credits on api.storyteller.ai
  async fn live_generation_video_seedance_lite_480p() {
    let app_data_root = AppDataRoot::create_default().expect("app data root");
    let credential_id = app_data_root
        .credentials_dir()
        .load_credentials()
        .expect("load credentials")
        .into_iter()
        .find(|c| c.service == GenerationSource::Artcraft)
        .expect("no `artcraft` (production) credential on disk")
        .id
        .as_str()
        .to_string();

    // Seedance is image-to-video: upload a small start frame first, using
    // the same stored credential the generation will use.
    let credential = app_data_root
        .credentials_dir()
        .find_credential_by_id(&credential_id)
        .expect("lookup")
        .expect("credential exists");
    let creds = storyteller_creds_from_credential(&credential).expect("cookie creds");

    let mut png_bytes: Vec<u8> = Vec::new();
    let start_frame = image::RgbaImage::from_fn(512, 512, |x, y| {
      image::Rgba([(x / 2) as u8, (y / 2) as u8, 200, 255])
    });
    image::DynamicImage::ImageRgba8(start_frame)
        .write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
        .expect("encode png");

    let upload = upload_image_media_file_from_bytes(UploadImageBytesArgs {
      api_host: &ApiHost::Storyteller,
      maybe_creds: Some(&creds),
      image_bytes: png_bytes,
      image_type: ImageType::Png,
      is_intermediate_system_file: true,
      maybe_generation_provider: None,
    }).await.expect("start frame upload");
    println!("[live] start frame uploaded: {}", upload.media_file_token.as_str());

    let request = TauriGenerateVideoRequest {
      credential_id: Some(credential_id),
      model: Some(TauriVideoModel::Seedance10Lite),
      prompt: Some("gentle camera pan across a colorful gradient".to_string()),
      start_frame_image_media_token: Some(upload.media_file_token.clone()),
      image_media_token: Some(upload.media_file_token),
      resolution: Some(RouterResolution::FourEightyP),
      duration_seconds: Some(5),
      video_batch_count: Some(1),
      ..Default::default()
    };

    let ledger = AssetUploadLedger::new(crate::state::database::local_files_database::LocalFilesDatabase::connect(&app_data_root).await.expect("local files db"));
    let result = handle_credential_router(&request, None, &app_data_root, &ledger).await;
    let success = result.expect("seedance lite enqueue should succeed");
    println!("[live] seedance 1.0 lite enqueued: job_id={:?}", success.provider_job_id);
    assert!(success.provider_job_id.is_some());
  }
}
