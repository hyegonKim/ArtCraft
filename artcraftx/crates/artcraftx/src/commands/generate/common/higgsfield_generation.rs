//! Shared pieces of the first-party (cookie-session) Higgsfield enqueue path
//! for images and videos: the router client for a stored credential, the
//! media-token → CDN URL map for cloud-library references (local files and
//! bytes go straight to Higgsfield without touching ArtCraft), and running a
//! built request through the draft phase.

use std::collections::HashMap;

use artcraft_client::utils::api_host::ApiHost;
use log::{error, info, warn};
use router::client::router_client::RouterClient;
use router::client::router_higgsfield_client::RouterHiggsfieldClient;
use router::errors::artcraft_router_error::ArtcraftRouterError;
use router::errors::provider_error::ProviderError;
use router::generate::generate_image::generate_image_request_builder::GenerateImageRequestBuilder;
use router::generate::generate_image::generate_image_response::GenerateImageResponse;
use router::generate::generate_image::image_generation_draft_context::ImageGenerationDraftContext;
use router::generate::generate_image::image_generation_draft::ImageGenerationDraftRequest;
use router::generate::generate_image::image_generation_draft_or_request::ImageGenerationDraftOrRequest;
use router::generate::generate_image::image_generation_request::ImageGenerationRequest;
use router::generate::generate_video::generate_video_request_builder::GenerateVideoRequestBuilder;
use router::generate::generate_video::generate_video_response::GenerateVideoResponse;
use router::generate::generate_video::video_generation_draft_context::VideoGenerationDraftContext;
use router::generate::generate_video::video_generation_draft::VideoGenerationDraftRequest;
use router::generate::generate_video::video_generation_draft_or_request::VideoGenerationDraftOrRequest;
use router::generate::generate_video::video_generation_request::VideoGenerationRequest;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;
use tauri::AppHandle;

use crate::commands::generate::generate_error::{CredentialProblemReason, GenerateError};
use crate::commands::generate::generate_image::utils::map_media_files_to_urls::map_media_file_tokens_to_cdn_urls;
use crate::credentials::auth_credential::AuthCredential;
use crate::services::asset_uploads::asset_upload_ledger::AccountUploadCache;
use crate::services::higgsfield::higgsfield_upload_notices::HiggsfieldUploadNotices;
use crate::services::higgsfield::higgsfield_session_from_credential::higgsfield_session_from_credential;

/// Batch job ids are stored on the task as one comma-separated
/// `provider_job_id`, so the poller can follow every job of the set.
pub const HIGGSFIELD_JOB_ID_SEPARATOR: char = ',';

/// The router client for a stored Higgsfield credential. A credential whose
/// session Higgsfield already rejected fails here, before any request, until
/// the user logs in again.
pub fn higgsfield_router_client(credential: &AuthCredential) -> Result<RouterClient, GenerateError> {
  if credential.needs_relogin() {
    info!("Higgsfield credential {} is marked as needing a re-login; not calling the API", credential.id);
    return Err(session_expired(credential));
  }
  let session = higgsfield_session_from_credential(credential)?;
  Ok(RouterClient::Higgsfield(RouterHiggsfieldClient::new(session)))
}

/// The error for a credential whose session is dead.
pub fn session_expired(credential: &AuthCredential) -> GenerateError {
  GenerateError::CredentialProblem(CredentialProblemReason::SessionExpired {
    credential_id: credential.id.to_string(),
    service: credential.service,
  })
}

/// Convert a router error from a Higgsfield call.
///
/// - A refusal the user can act on (reference media failing the IP /
///   likeness check) becomes a [`GenerateError::ProviderRejected`] carrying
///   the explanation, which the commands show verbatim.
/// - A dead session (`needs_browser_reauth()`) marks the credential file so
///   nothing else retries it, and becomes a session-expired credential
///   problem (which the frontend turns into a "log in again" prompt).
/// - Anything else is a plain provider failure.
fn higgsfield_error_to_generate_error(credential: &AuthCredential, err: ArtcraftRouterError) -> GenerateError {
  let ArtcraftRouterError::Provider(ProviderError::Higgsfield(higgsfield_err)) = &err else {
    return GenerateError::from(err);
  };
  if let Some(message) = higgsfield_err.user_facing_rejection() {
    info!("Higgsfield rejected the request for a user-actionable reason: {}", higgsfield_err);
    return GenerateError::ProviderRejected(message);
  }
  if !higgsfield_err.needs_browser_reauth() {
    return GenerateError::from(err);
  }
  warn!("Higgsfield rejected the session of credential {}; marking it as needing a re-login: {:?}", credential.id, err);
  let mut marked = credential.clone();
  if let Err(save_err) = marked.mark_relogin_required() {
    error!("Could not mark credential {} as needing a re-login: {}", credential.id, save_err);
  }
  session_expired(credential)
}

/// Resolve reference media tokens (cloud-library picks only) to their
/// ArtCraft CDN URLs. Higgsfield can't fetch ArtCraft media itself, so the
/// router downloads each URL and re-uploads the bytes as Higgsfield
/// reference media. Empty when every reference is a local file or bytes —
/// then ArtCraft is never contacted at all.
pub async fn higgsfield_media_url_map(tokens: &[MediaFileToken]) -> Result<HashMap<MediaFileToken, String>, GenerateError> {
  if tokens.is_empty() {
    return Ok(HashMap::new());
  }
  let urls = map_media_file_tokens_to_cdn_urls(tokens, &ApiHost::Storyteller).await?;
  Ok(tokens.iter().cloned().zip(urls).collect())
}

/// Build, finalize (uploading references) and send an image request.
///
/// References the account already uploaded are reused from the ledger. If
/// Higgsfield then rejects the job because a reused media id is no good
/// (deleted or flagged since), those ledger entries are dropped and the
/// request is finalized and sent once more with fresh uploads.
pub async fn send_higgsfield_image_request(
  maybe_app: Option<&AppHandle>,
  credential: &AuthCredential,
  builder: GenerateImageRequestBuilder,
  client: &RouterClient,
  media_url_map: &HashMap<MediaFileToken, String>,
  upload_cache: &AccountUploadCache,
) -> Result<GenerateImageResponse, GenerateError> {
  let draft = match builder.build2().map_err(|err| {
    warn!("Could not build Higgsfield image request: {:?}", err);
    GenerateError::from(err)
  })? {
    ImageGenerationDraftOrRequest::Request(request) => {
      return request.send_request(client).await
          .map_err(|err| higgsfield_error_to_generate_error(credential, err));
    }
    ImageGenerationDraftOrRequest::Draft(draft) => draft,
  };

  info!("Higgsfield image request has references; uploading them first");
  // Reuse flashes a notice; a real upload keeps a progress toast up until
  // `notices` drops at the end of this function (retry included).
  let notices = HiggsfieldUploadNotices::new(maybe_app);
  let request = finalize_image_draft(credential, draft.clone(), client, media_url_map, upload_cache, &notices).await?;
  match request.send_request(client).await {
    Ok(response) => Ok(response),
    Err(err) if should_retry_with_fresh_uploads(&err, upload_cache) => {
      let forgotten = upload_cache.forget_reused().await;
      warn!("Higgsfield rejected reused reference media ({:?}); forgot {} ledger entries and uploading afresh", err, forgotten);
      let request = finalize_image_draft(credential, draft, client, media_url_map, upload_cache, &notices).await?;
      request.send_request(client).await
          .map_err(|err| higgsfield_error_to_generate_error(credential, err))
    }
    Err(err) => Err(higgsfield_error_to_generate_error(credential, err)),
  }
}

async fn finalize_image_draft(
  credential: &AuthCredential,
  draft: ImageGenerationDraftRequest,
  client: &RouterClient,
  media_url_map: &HashMap<MediaFileToken, String>,
  upload_cache: &AccountUploadCache,
  notices: &HiggsfieldUploadNotices,
) -> Result<ImageGenerationRequest, GenerateError> {
  let context = ImageGenerationDraftContext {
    client: Some(client),
    media_file_to_artcraft_url_map: Some(media_url_map),
    asset_upload_cache: Some(upload_cache),
    asset_upload_observer: Some(notices),
  };
  draft.finalize(context).await.map_err(|err| {
    warn!("Could not upload references to Higgsfield: {:?}", err);
    higgsfield_error_to_generate_error(credential, err)
  })
}

/// Build, finalize (uploading keyframes and references) and send a video
/// request. Reused uploads that Higgsfield rejects at enqueue time are
/// forgotten and uploaded afresh, once — see [`send_higgsfield_image_request`].
pub async fn send_higgsfield_video_request(
  maybe_app: Option<&AppHandle>,
  credential: &AuthCredential,
  builder: GenerateVideoRequestBuilder,
  client: &RouterClient,
  media_url_map: &HashMap<MediaFileToken, String>,
  upload_cache: &AccountUploadCache,
) -> Result<GenerateVideoResponse, GenerateError> {
  let draft = match builder.build2().map_err(|err| {
    warn!("Could not build Higgsfield video request: {:?}", err);
    GenerateError::from(err)
  })? {
    VideoGenerationDraftOrRequest::Request(request) => {
      return request.send_request(client).await
          .map_err(|err| higgsfield_error_to_generate_error(credential, err));
    }
    VideoGenerationDraftOrRequest::Draft(draft) => draft,
  };

  info!("Higgsfield video request has media; uploading it first");
  // Reuse flashes a notice; a real upload keeps a progress toast up until
  // `notices` drops at the end of this function (retry included).
  let notices = HiggsfieldUploadNotices::new(maybe_app);
  let request = finalize_video_draft(credential, draft.clone(), client, media_url_map, upload_cache, &notices).await?;
  match request.send_request(client).await {
    Ok(response) => Ok(response),
    Err(err) if should_retry_with_fresh_uploads(&err, upload_cache) => {
      let forgotten = upload_cache.forget_reused().await;
      warn!("Higgsfield rejected reused reference media ({:?}); forgot {} ledger entries and uploading afresh", err, forgotten);
      let request = finalize_video_draft(credential, draft, client, media_url_map, upload_cache, &notices).await?;
      request.send_request(client).await
          .map_err(|err| higgsfield_error_to_generate_error(credential, err))
    }
    Err(err) => Err(higgsfield_error_to_generate_error(credential, err)),
  }
}

async fn finalize_video_draft(
  credential: &AuthCredential,
  draft: VideoGenerationDraftRequest,
  client: &RouterClient,
  media_url_map: &HashMap<MediaFileToken, String>,
  upload_cache: &AccountUploadCache,
  notices: &HiggsfieldUploadNotices,
) -> Result<VideoGenerationRequest, GenerateError> {
  let context = VideoGenerationDraftContext {
    client: Some(client),
    media_file_to_artcraft_url_map: Some(media_url_map),
    character_token_to_kinovi_id_map: None,
    asset_upload_cache: Some(upload_cache),
    asset_upload_observer: Some(notices),
  };
  draft.finalize(context).await.map_err(|err| {
    warn!("Could not upload media to Higgsfield: {:?}", err);
    higgsfield_error_to_generate_error(credential, err)
  })
}

/// Retry with fresh uploads only when Higgsfield rejected a media input and
/// this request actually reused something from the ledger; otherwise the
/// media was uploaded just now and re-uploading would change nothing.
fn should_retry_with_fresh_uploads(err: &ArtcraftRouterError, upload_cache: &AccountUploadCache) -> bool {
  let media_rejected = matches!(
    err,
    ArtcraftRouterError::Provider(ProviderError::Higgsfield(higgsfield_err)) if higgsfield_err.is_media_input_rejected()
  );
  media_rejected && upload_cache.reused_count() > 0
}

/// One `provider_job_id` for a Higgsfield job set.
pub fn join_higgsfield_job_ids(job_ids: &[String]) -> String {
  job_ids.join(&HIGGSFIELD_JOB_ID_SEPARATOR.to_string())
}

/// The job ids stored on a task by [`join_higgsfield_job_ids`].
pub fn split_higgsfield_job_ids(provider_job_id: &str) -> Vec<String> {
  provider_job_id
      .split(HIGGSFIELD_JOB_ID_SEPARATOR)
      .map(str::trim)
      .filter(|id| !id.is_empty())
      .map(str::to_string)
      .collect()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::credentials::auth_credential::CredentialSecret;
  use crate::credentials::cookie_credential::CookieCredential;
  use cookie_store_wrapper::cookie_store::CookieStore;
  use core_types::enums::generation_source::GenerationSource;
  use core_types::identifiers::credential_id::CredentialId;
  use higgsfield_client::error::higgsfield_client_error::HiggsfieldClientError;
  use higgsfield_client::error::higgsfield_error::HiggsfieldError;
  use higgsfield_client::types::ids::MediaId;
  use reqwest::Url;

  #[test]
  fn protected_content_becomes_a_provider_rejection_with_the_explanation() {
    let err = ArtcraftRouterError::Provider(ProviderError::Higgsfield(
      HiggsfieldError::Client(HiggsfieldClientError::MediaProtectedContent { media_id: MediaId::new("m1") }),
    ));
    match higgsfield_error_to_generate_error(&unsaved_credential(), err) {
      GenerateError::ProviderRejected(message) => assert!(message.contains("intellectual property"), "{message}"),
      other => panic!("expected ProviderRejected, got {other:?}"),
    }
  }

  #[test]
  fn other_higgsfield_errors_stay_provider_failures() {
    let err = ArtcraftRouterError::Provider(ProviderError::Higgsfield(
      HiggsfieldError::Client(HiggsfieldClientError::UploadSourceDomainBlocked {
        source_url: "https://cdn.example.com/a.png".to_string(),
        domain: "cdn.example.com",
      }),
    ));
    assert!(matches!(higgsfield_error_to_generate_error(&unsaved_credential(), err), GenerateError::ProviderFailure(_)));
  }

  fn unsaved_credential() -> AuthCredential {
    let origin = Url::parse("https://higgsfield.ai/").unwrap();
    AuthCredential {
      id: CredentialId::generate(),
      service: GenerationSource::HiggsfieldCookies,
      name: None,
      secret: CredentialSecret::Cookies(CookieCredential::new(CookieStore::from_cookie_header("__client=abc", &origin))),
      user_info: None,
      source_path: std::env::temp_dir().join("never_written_higgsfield_cookies.toml"),
    }
  }

  #[test]
  fn job_ids_round_trip() {
    let ids = vec!["job_a".to_string(), "job_b".to_string()];
    let joined = join_higgsfield_job_ids(&ids);
    assert_eq!(joined, "job_a,job_b");
    assert_eq!(split_higgsfield_job_ids(&joined), ids);
    assert_eq!(split_higgsfield_job_ids("solo"), vec!["solo".to_string()]);
    assert!(split_higgsfield_job_ids(" , ").is_empty());
  }
}
