use artcraft_client::api_defs::prompts::create_prompt::CreatePromptRequest;
use artcraft_client::api_defs::utils::media_links_to_thumbnail_template::media_links_to_thumbnail_template;
use artcraft_client::credentials::storyteller_credential_set::StorytellerCredentialSet;
use artcraft_client::endpoints::media_files::get_media_file::get_media_file;
use artcraft_client::endpoints::media_files::legacy_upload_media_file_from_file::{
  legacy_upload_media_file_from_file, LegacyUploadMediaFileFromFileArgs,
};
use artcraft_client::endpoints::media_files::upload_image_media_file_from_file::{
  upload_image_media_file_from_file, UploadImageFromFileArgs,
};
use artcraft_client::endpoints::media_files::upload_video_media_file_from_file::{
  upload_video_media_file_from_file, UploadVideoFromFileArgs,
};
use artcraft_client::endpoints::prompts::create_prompt::create_prompt;
use artcraft_client::enums::common::generation::common_model_type::CommonModelType;
use artcraft_client::error::api_error::ApiError;
use artcraft_client::error::storyteller_error::StorytellerError;
use artcraft_client::utils::api_host::ApiHost;
use core_types::enums::generation_source::GenerationSource;
use errors::AnyhowResult;
use log::{error, info, warn};
use reqwest::Url;
use sqlite_database::queries::task::Task;
use sqlite_identifiers::enums::task_media_file_class::TaskMediaFileClass;
use sqlite_identifiers::ids::batch_generation_token::BatchGenerationToken;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;
use sqlite_identifiers::ids::prompt_token::PromptToken;
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid_utils::uuid::generate_random_uuid;
use crate::services::asset_uploads::asset_upload_ledger::AssetUploadLedger;
use crate::utils::enum_conversion::artcraft_api_generation_provider::artcraft_api_generation_provider;

const MAX_UPLOAD_RETRIES: u32 = 5;
const INITIAL_RETRY_DELAY_SECS: u64 = 10;
const MAX_RETRY_DELAY_SECS: u64 = 60;

/// How the uploads get associated with a Storyteller prompt record.
pub enum CompletionPrompt {
  /// Reuse a prompt created at enqueue time (eg. the task's `prompt_token`).
  Existing(PromptToken),

  /// Create the prompt now from what the provider reported.
  Create {
    model_type: CommonModelType,
    maybe_prompt: Option<String>,
  },

  /// Don't associate a prompt.
  None,
}

/// What came back from uploading a task's results.
pub struct UploadedResults {
  /// The first uploaded file.
  pub primary_media_file_token: MediaFileToken,

  /// Set when more than one file was uploaded.
  pub maybe_batch_token: Option<BatchGenerationToken>,

  pub maybe_cdn_url: Option<Url>,
  pub maybe_thumbnail_url_template: Option<String>,
}

/// Upload every result file to ArtCraft (in order; the first becomes the
/// primary), retrying on rate limits, then look up the primary file's CDN and
/// thumbnail URLs (failing open). With a ledger, a file this account already
/// uploaded (and ArtCraft still has) is reused by token instead.
pub async fn upload_results_to_artcraft(
  api_host: &ApiHost,
  creds: &StorytellerCredentialSet,
  task: &Task,
  generation_provider: GenerationSource,
  media_class: TaskMediaFileClass,
  prompt: CompletionPrompt,
  local_files: &[PathBuf],
  maybe_ledger: Option<&AssetUploadLedger>,
) -> AnyhowResult<UploadedResults> {
  let task_id = task.id.as_str();
  let maybe_upload_cache = maybe_ledger.and_then(|ledger| ledger.for_artcraft_session(Some(creds)));

  let maybe_prompt_token = resolve_prompt_token(api_host, creds, task, generation_provider, prompt).await;

  // TODO: Move this from clientside to the backend.
  //  The first upload should produce a batch token that we can reuse.
  let maybe_batch_token = (local_files.len() > 1).then(BatchGenerationToken::generate);

  let mut maybe_primary_media_file_token: Option<MediaFileToken> = None;

  for (index, local_file) in local_files.iter().enumerate() {
    info!("[TaskCompletion] Uploading result {} of {} for task {} ...", index + 1, local_files.len(), task_id);

    let upload = || upload_with_retry(
      api_host,
      creds,
      local_file,
      generation_provider,
      media_class,
      maybe_prompt_token.as_ref(),
      maybe_batch_token.as_ref(),
    );
    let media_token = match &maybe_upload_cache {
      Some(cache) => cache.artcraft_token_for_file(api_host, local_file, upload).await?,
      None => upload().await?,
    };

    info!("[TaskCompletion] Uploaded task {} result as {:?}", task_id, media_token);

    if maybe_primary_media_file_token.is_none() {
      maybe_primary_media_file_token = Some(media_token);
    }
  }

  let Some(primary_media_file_token) = maybe_primary_media_file_token else {
    anyhow::bail!("No files were uploaded for task {}", task_id);
  };

  let mut maybe_cdn_url = None;
  let mut maybe_thumbnail_url_template = None;

  match get_media_file(api_host, &primary_media_file_token).await {
    Ok(response) => {
      maybe_cdn_url = Some(response.media_file.media_links.cdn_url.clone());
      maybe_thumbnail_url_template = media_links_to_thumbnail_template(&response.media_file.media_links)
          .map(|s| s.to_string());
    }
    Err(err) => {
      error!("[TaskCompletion] Failed to look up media file after upload: {:?} (failing open)", err);
    }
  }

  Ok(UploadedResults {
    primary_media_file_token,
    maybe_batch_token,
    maybe_cdn_url,
    maybe_thumbnail_url_template,
  })
}

// ── Helpers ──

/// Turn the caller's prompt instruction into a token. Creating a prompt fails
/// open: the uploads are still worth keeping without one.
async fn resolve_prompt_token(
  api_host: &ApiHost,
  creds: &StorytellerCredentialSet,
  task: &Task,
  generation_provider: GenerationSource,
  prompt: CompletionPrompt,
) -> Option<PromptToken> {
  let task_id = task.id.as_str();

  match prompt {
    CompletionPrompt::Existing(token) => {
      info!("[TaskCompletion] Using prompt {:?} from task {}", token, task_id);
      Some(token)
    }
    CompletionPrompt::None => {
      warn!("[TaskCompletion] Task {} has no prompt; uploading without prompt association", task_id);
      None
    }
    CompletionPrompt::Create { model_type, maybe_prompt } => {
      // Only providers the API's enum knows; others (Higgsfield) go unspecified
      // rather than failing the request with `unknown variant`.
      let request = CreatePromptRequest {
        uuid_idempotency_token: generate_random_uuid(),
        positive_prompt: maybe_prompt,
        negative_prompt: None,
        model_type: Some(model_type),
        generation_provider: artcraft_api_generation_provider(generation_provider),
        maybe_generation_mode: None,
        maybe_aspect_ratio: None,
        maybe_resolution: None,
        maybe_batch_count: None,
        maybe_generate_audio: None,
        maybe_duration_seconds: None,
      };

      match create_prompt(api_host, Some(creds), request).await {
        Ok(response) => {
          info!("[TaskCompletion] Created prompt {:?} for task {}", response.prompt_token, task_id);
          Some(response.prompt_token)
        }
        Err(err) => {
          error!("[TaskCompletion] Failed to create prompt for task {}: {:?} (uploading without one)", task_id, err);
          None
        }
      }
    }
  }
}

async fn upload_with_retry(
  api_host: &ApiHost,
  creds: &StorytellerCredentialSet,
  path: &Path,
  generation_provider: GenerationSource,
  media_class: TaskMediaFileClass,
  maybe_prompt_token: Option<&PromptToken>,
  maybe_batch_token: Option<&BatchGenerationToken>,
) -> AnyhowResult<MediaFileToken> {
  let mut retry_delay_secs = INITIAL_RETRY_DELAY_SECS;

  for attempt in 1..=MAX_UPLOAD_RETRIES {
    let result = try_upload(api_host, creds, path, generation_provider, media_class, maybe_prompt_token, maybe_batch_token).await;

    match result {
      Ok(token) => return Ok(token),
      Err(StorytellerError::Api(ApiError::TooManyRequests(_))) if attempt < MAX_UPLOAD_RETRIES => {
        warn!(
          "[TaskCompletion] Upload rate-limited (429), retrying in {}s (attempt {}/{})",
          retry_delay_secs, attempt, MAX_UPLOAD_RETRIES,
        );
        tokio::time::sleep(Duration::from_secs(retry_delay_secs)).await;
        retry_delay_secs = (retry_delay_secs * 2).min(MAX_RETRY_DELAY_SECS);
      }
      Err(err) => return Err(err.into()),
    }
  }

  unreachable!("loop returns on the final attempt")
}

async fn try_upload(
  api_host: &ApiHost,
  creds: &StorytellerCredentialSet,
  path: &Path,
  generation_provider: GenerationSource,
  media_class: TaskMediaFileClass,
  maybe_prompt_token: Option<&PromptToken>,
  maybe_batch_token: Option<&BatchGenerationToken>,
) -> Result<MediaFileToken, StorytellerError> {
  // See `resolve_prompt_token`: providers the API doesn't know go unspecified.
  let maybe_generation_provider = artcraft_api_generation_provider(generation_provider);
  let media_token = match media_class {
    TaskMediaFileClass::Video => {
      let result = upload_video_media_file_from_file(UploadVideoFromFileArgs {
        api_host,
        maybe_creds: Some(creds),
        path,
        maybe_prompt_token,
        maybe_generation_provider,
      }).await?;
      result.media_file_token
    }
    TaskMediaFileClass::Splat | TaskMediaFileClass::Mesh => {
      let result = legacy_upload_media_file_from_file(LegacyUploadMediaFileFromFileArgs {
        api_host,
        maybe_creds: Some(creds),
        path,
        maybe_generation_provider,
      }).await?;
      result.media_file_token
    }
    _ => {
      let result = upload_image_media_file_from_file(UploadImageFromFileArgs {
        api_host,
        maybe_creds: Some(creds),
        path,
        is_intermediate_system_file: false,
        maybe_prompt_token,
        maybe_batch_token,
        maybe_generation_provider,
      }).await?;
      result.media_file_token
    }
  };

  Ok(media_token)
}
