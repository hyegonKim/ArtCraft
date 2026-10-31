use crate::services::storyteller::state::storyteller_credential_manager::StorytellerCredentialManager;
use crate::state::app_preferences::app_preferences_manager::AppPreferencesManager;
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::state::data_dir::subdirectory::trait_data_subdir::DataSubdir;
use crate::state::database::task_database::TaskDatabase;
use crate::threads::task_completion::complete_task_with_local_files::{complete_task_with_local_files, CompleteTaskArgs};
use crate::threads::task_completion::upload_results_to_artcraft::CompletionPrompt;
use core_types::enums::generation_source::GenerationSource;
use errors::AnyhowResult;
use fal_client::polling::poll_job_response::poll_job_response::PollJobResponse;
use fal_client::polling::poll_job_response::success_case_extractors::PollResponseExtractedContents;
use log::{error, info, warn};
use sqlite_database::queries::task::Task;
use sqlite_identifiers::enums::task_media_file_class::TaskMediaFileClass;
use sqlite_identifiers::ids::prompt_token::PromptToken;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tauri::AppHandle;
use url_utils::download_extension::extract_download_extension_from_url::extract_download_extension_from_url_str;
use uuid_utils::uuid::generate_random_uuid;

/// Download filename slug when the task has no model type recorded.
const FAL_FALLBACK_MODEL_SLUG: &str = "fal";

/// A FAL job finished: fetch its files, then hand them to the shared
/// completion routine.
pub async fn handle_fal_complete(
  app_handle: &AppHandle,
  app_data_root: &AppDataRoot,
  app_preferences: &AppPreferencesManager,
  task_database: &TaskDatabase,
  storyteller_creds_manager: &StorytellerCredentialManager,
  task: &Task,
  job_response: PollJobResponse,
) {
  info!("[FalComplete] Handling completed task {}", task.id.as_str());

  let result = handle_fal_complete_inner(
    app_handle,
    app_data_root,
    app_preferences,
    task_database,
    storyteller_creds_manager,
    task,
    job_response,
  ).await;

  if let Err(err) = result {
    error!("[FalComplete] Failed to handle task {}: {:?}", task.id.as_str(), err);
  }
}

async fn handle_fal_complete_inner(
  app_handle: &AppHandle,
  app_data_root: &AppDataRoot,
  app_preferences: &AppPreferencesManager,
  task_database: &TaskDatabase,
  // Uploads now go to the backup account (see `services::backup`), not the session.
  _storyteller_creds_manager: &StorytellerCredentialManager,
  task: &Task,
  job_response: PollJobResponse,
) -> AnyhowResult<()> {
  let (urls, media_class) = collect_media_urls(&job_response.extracted_contents);

  if urls.is_empty() {
    warn!("[FalComplete] Task {} completed but no downloadable media found in response", task.id.as_str());
    return Ok(());
  }

  let mut local_files = Vec::with_capacity(urls.len());
  for (index, url) in urls.iter().enumerate() {
    info!("[FalComplete] Downloading result {} from: {}", index, url);
    local_files.push(download_file(url, app_data_root, index).await?);
  }

  // The prompt was created at enqueue time.
  let prompt = match task.prompt_token.as_deref() {
    Some(token) => CompletionPrompt::Existing(PromptToken::new_from_str(token)),
    None => CompletionPrompt::None,
  };


  complete_task_with_local_files(CompleteTaskArgs {
    app_handle,
    app_data_root,
    app_preferences,
    task_database,
    task,
    generation_provider: GenerationSource::Fal,
    media_class,
    prompt,
    fallback_model_slug: FAL_FALLBACK_MODEL_SLUG,
    local_files: &local_files,
  }).await?;

  Ok(())
}

// ── Helpers ──

/// Collect downloadable media URLs from the extracted response contents.
fn collect_media_urls(extracted: &Option<PollResponseExtractedContents>) -> (Vec<String>, TaskMediaFileClass) {
  let Some(extracted) = extracted else {
    return (vec![], TaskMediaFileClass::Image);
  };

  // Images (batch)
  if let Some(images) = &extracted.images {
    let urls: Vec<String> = images.iter()
      .filter_map(|img| img.url.clone())
      .collect();
    if !urls.is_empty() {
      return (urls, TaskMediaFileClass::Image);
    }
  }

  // Single image (e.g. background removal)
  if let Some(url) = extracted.image.as_ref().and_then(|image| image.url.clone()) {
    return (vec![url], TaskMediaFileClass::Image);
  }

  // Video
  if let Some(url) = extracted.video.as_ref().and_then(|video| video.url.clone()) {
    return (vec![url], TaskMediaFileClass::Video);
  }

  // 3D model (GLB)
  if let Some(url) = extracted.model_glb.as_ref().and_then(|glb| glb.url.clone()) {
    return (vec![url], TaskMediaFileClass::Mesh);
  }

  (vec![], TaskMediaFileClass::Image)
}

async fn download_file(url: &str, app_data_root: &AppDataRoot, index: usize) -> AnyhowResult<PathBuf> {
  let response = reqwest::get(url).await?;
  let bytes = response.bytes().await?;

  let extension = extract_download_extension_from_url_str(url)
    .map(|ext| ext.as_extension_without_period())
    .unwrap_or("bin");

  let tempdir = app_data_root.temp_dir().path();
  let filename = format!("fal_{}_{}.{}", generate_random_uuid(), index, extension);
  let download_path = tempdir.join(filename);

  let mut file = File::create(&download_path)?;
  file.write_all(&bytes)?;

  Ok(download_path)
}
