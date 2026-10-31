use crate::services::midjourney::utils::download_midjourney_image::download_midjourney_image;
use crate::state::app_preferences::app_preferences_manager::AppPreferencesManager;
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::state::database::task_database::TaskDatabase;
use crate::threads::task_completion::complete_task_with_local_files::{complete_task_with_local_files, CompleteTaskArgs};
use crate::threads::task_completion::upload_results_to_artcraft::CompletionPrompt;
use artcraft_client::enums::common::generation::common_model_type::CommonModelType;
use core_types::enums::generation_source::GenerationSource;
use errors::AnyhowResult;
use midjourney_client::utils::image_downloader_client::ImageDownloaderClient;
use sqlite_database::queries::task::Task;
use sqlite_identifiers::enums::task_media_file_class::TaskMediaFileClass;
use tauri::AppHandle;

/// How many images a Midjourney job produces (the 2x2 grid).
const MIDJOURNEY_GRID_SIZE: u8 = 4;

/// Download filename slug when the task has no model type recorded.
const MIDJOURNEY_FALLBACK_MODEL_SLUG: &str = "midjourney";

pub struct FinalizeMidjourneyJobArgs<'a> {
  pub app_handle: &'a AppHandle,
  pub app_data_root: &'a AppDataRoot,
  pub app_preferences: &'a AppPreferencesManager,
  pub task_database: &'a TaskDatabase,
  pub image_downloader: &'a ImageDownloaderClient,
  pub midjourney_job_id: &'a str,
  pub local_task: &'a Task,
  pub model_type: CommonModelType,
  pub maybe_full_command: Option<String>,
}

/// The completion routine for a Midjourney job: download the four
/// full-resolution images (from the deterministic CDN URL), then hand them to
/// the shared completion routine.
///
/// Both the websocket fast path and the long-polling fallback call this. It is
/// safe to call from both concurrently: the shared routine finalizes via
/// `update_successful_task_status_with_metadata`, which no-ops (returns false)
/// once a task is already complete — so whichever path gets there first wins.
///
/// Returns `Ok(true)` if this call finalized the task, `Ok(false)` if it was
/// already finalized by someone else.
pub async fn finalize_midjourney_job(args: FinalizeMidjourneyJobArgs<'_>) -> AnyhowResult<bool> {
  let FinalizeMidjourneyJobArgs {
    app_handle,
    app_data_root,
    app_preferences,
    task_database,
    image_downloader,
    midjourney_job_id,
    local_task,
    model_type,
    maybe_full_command,
  } = args;

  let mut local_files = Vec::with_capacity(MIDJOURNEY_GRID_SIZE as usize);
  for index in 0..MIDJOURNEY_GRID_SIZE {
    local_files.push(download_midjourney_image(image_downloader, midjourney_job_id, index, app_data_root).await?);
  }

  complete_task_with_local_files(CompleteTaskArgs {
    app_handle,
    app_data_root,
    app_preferences,
    task_database,
    task: local_task,
    generation_provider: GenerationSource::Midjourney,
    media_class: TaskMediaFileClass::Image,
    prompt: CompletionPrompt::Create {
      model_type,
      maybe_prompt: maybe_full_command,
    },
    fallback_model_slug: MIDJOURNEY_FALLBACK_MODEL_SLUG,
    local_files: &local_files,
  }).await
}
