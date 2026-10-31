use crate::services::storyteller::state::storyteller_credential_manager::StorytellerCredentialManager;
use crate::state::app_preferences::app_preferences_manager::AppPreferencesManager;
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::state::database::task_database::TaskDatabase;
use crate::threads::task_completion::complete_task_with_local_files::{complete_task_with_local_files, CompleteTaskArgs};
use crate::threads::task_completion::upload_results_to_artcraft::CompletionPrompt;
use crate::utils::download::download_url_to_temp_dir::download_url_to_temp_dir;
use artcraft_client::enums::common::generation::common_model_type::CommonModelType;
use core_types::enums::generation_source::GenerationSource;
use errors::AnyhowResult;
use grok_consumer_client::endpoint_bindings::generate_image_websocket::grok_generate_image_websocket::CompletedImage;
use log::{error, info};
use sqlite_database::queries::task::Task;
use sqlite_identifiers::enums::task_media_file_class::TaskMediaFileClass;
use tauri::AppHandle;

/// Download filename slug when the task has no model type recorded.
const GROK_IMAGE_FALLBACK_MODEL_SLUG: &str = "grok_imagine";

/// A Grok image task finished: fetch the images, then hand them to the shared
/// completion routine.
pub async fn handle_grok_image_complete(
  app_handle: &AppHandle,
  app_data_root: &AppDataRoot,
  app_preferences: &AppPreferencesManager,
  task_database: &TaskDatabase,
  storyteller_creds_manager: &StorytellerCredentialManager,
  task: &Task,
  images: &[CompletedImage],
) {
  info!("[GrokComplete] Handling completed task {} ({} image(s))", task.id.as_str(), images.len());

  let result = handle_grok_image_complete_inner(
    app_handle,
    app_data_root,
    app_preferences,
    task_database,
    storyteller_creds_manager,
    task,
    images,
  ).await;

  if let Err(err) = result {
    error!("[GrokComplete] Failed to handle task {}: {:?}", task.id.as_str(), err);
  }
}

async fn handle_grok_image_complete_inner(
  app_handle: &AppHandle,
  app_data_root: &AppDataRoot,
  app_preferences: &AppPreferencesManager,
  task_database: &TaskDatabase,
  // Uploads now go to the backup account (see `services::backup`), not the session.
  _storyteller_creds_manager: &StorytellerCredentialManager,
  task: &Task,
  images: &[CompletedImage],
) -> AnyhowResult<()> {
  // NB: `NamedTempFile`s delete themselves on drop, so keep them alive until
  // the completion routine has copied and uploaded them.
  let mut temp_files = Vec::with_capacity(images.len());
  for (index, image) in images.iter().enumerate() {
    info!("[GrokComplete] Downloading image {} of {} for task {} ...", index + 1, images.len(), task.id.as_str());
    temp_files.push(download_url_to_temp_dir(&image.url, app_data_root).await?);
  }
  let local_files = temp_files.iter()
      .map(|file| file.path().to_path_buf())
      .collect::<Vec<_>>();

  let maybe_prompt = images.iter()
      .map(|image| image.user_prompt.trim())
      .find(|prompt| !prompt.is_empty())
      .map(|prompt| prompt.to_string());


  complete_task_with_local_files(CompleteTaskArgs {
    app_handle,
    app_data_root,
    app_preferences,
    task_database,
    task,
    generation_provider: GenerationSource::Grok,
    media_class: TaskMediaFileClass::Image,
    prompt: CompletionPrompt::Create {
      model_type: CommonModelType::GrokImagineImage,
      maybe_prompt,
    },
    fallback_model_slug: GROK_IMAGE_FALLBACK_MODEL_SLUG,
    local_files: &local_files,
  }).await?;

  Ok(())
}
