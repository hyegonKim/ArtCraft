use artcraft_client::enums::common::generation::common_model_type::CommonModelType;
use core_types::enums::generation_source::GenerationSource;
use errors::AnyhowResult;
use higgsfield_client::endpoints::jobs::job_status::JobStatusResponse;
use log::{error, info, warn};
use sqlite_database::queries::task::Task;
use sqlite_identifiers::enums::task_media_file_class::TaskMediaFileClass;
use sqlite_identifiers::enums::task_model_type::TaskModelType;
use sqlite_identifiers::enums::task_type::TaskType;
use tauri::AppHandle;

use crate::services::storyteller::state::storyteller_credential_manager::StorytellerCredentialManager;
use crate::state::app_preferences::app_preferences_manager::AppPreferencesManager;
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::state::database::task_database::TaskDatabase;
use crate::threads::task_completion::complete_task_with_local_files::{complete_task_with_local_files, CompleteTaskArgs};
use crate::threads::task_completion::upload_results_to_artcraft::CompletionPrompt;
use crate::services::asset_uploads::asset_upload_ledger::{AccountUploadCache, AssetUploadLedger};
use crate::state::database::local_files_database::LocalFilesDatabase;
use crate::utils::download::download_url_to_temp_dir::download_url_to_temp_dir;
use core_types::identifiers::credential_id::CredentialId;
use file_hashing::hash_file_blake3;
use router::api::asset_upload_cache::{AssetUploadCache, CachedAssetUpload};
use std::path::Path;
use tauri::Manager;

/// Download filename slug when the task has no model type recorded.
const HIGGSFIELD_FALLBACK_MODEL_SLUG: &str = "higgsfield";

/// A Higgsfield job set finished: fetch every finished job's file, then hand
/// them to the shared completion routine.
pub async fn handle_higgsfield_complete(
  app_handle: &AppHandle,
  app_data_root: &AppDataRoot,
  app_preferences: &AppPreferencesManager,
  task_database: &TaskDatabase,
  storyteller_creds_manager: &StorytellerCredentialManager,
  credential_id: &str,
  task: &Task,
  jobs: &[JobStatusResponse],
) {
  info!("[HiggsfieldComplete] Handling completed task {} ({} job(s))", task.id.as_str(), jobs.len());

  let result = handle_higgsfield_complete_inner(
    app_handle,
    app_data_root,
    app_preferences,
    task_database,
    storyteller_creds_manager,
    credential_id,
    task,
    jobs,
  ).await;

  if let Err(err) = result {
    error!("[HiggsfieldComplete] Failed to handle task {}: {:?}", task.id.as_str(), err);
  }
}

async fn handle_higgsfield_complete_inner(
  app_handle: &AppHandle,
  app_data_root: &AppDataRoot,
  app_preferences: &AppPreferencesManager,
  task_database: &TaskDatabase,
  // Uploads now go to the backup account (see `services::backup`), not the session.
  _storyteller_creds_manager: &StorytellerCredentialManager,
  credential_id: &str,
  task: &Task,
  jobs: &[JobStatusResponse],
) -> AnyhowResult<()> {
  // Higgsfield already holds these files: remember each one under the
  // account that generated it, so using the download as a reference later
  // passes the job id instead of uploading (and IP-checking) it again.
  let maybe_ledger = app_handle.try_state::<LocalFilesDatabase>()
      .map(|database| AssetUploadLedger::new(database.inner().clone()))
      .map(|ledger| ledger.for_higgsfield_credential(&CredentialId::from_trusted(credential_id)));

  // NB: `NamedTempFile`s delete themselves on drop, so keep them alive until
  // the completion routine has copied and uploaded them.
  let mut temp_files = Vec::with_capacity(jobs.len());
  for (index, job) in jobs.iter().enumerate() {
    let url = job.result_url().expect("caller passes only jobs with a result URL");
    info!("[HiggsfieldComplete] Downloading result {} of {} for task {} ...", index + 1, jobs.len(), task.id.as_str());
    let temp_file = download_url_to_temp_dir(url, app_data_root).await?;
    if let Some(ledger) = &maybe_ledger {
      record_generation_result(ledger, job, temp_file.path()).await;
    }
    temp_files.push(temp_file);
  }
  let local_files = temp_files.iter()
      .map(|file| file.path().to_path_buf())
      .collect::<Vec<_>>();

  let media_class = media_class_for(task.task_type);
  let maybe_prompt = jobs.iter()
      .filter_map(|job| job.params.prompt.as_deref())
      .map(str::trim)
      .find(|prompt| !prompt.is_empty())
      .map(str::to_string);

  // Attribute a prompt when storyteller-web knows the model; the
  // Higgsfield-only models have no prompt type there.
  let prompt = match task.model_type.and_then(common_model_type_for) {
    Some(model_type) => CompletionPrompt::Create { model_type, maybe_prompt },
    None => CompletionPrompt::None,
  };


  complete_task_with_local_files(CompleteTaskArgs {
    app_handle,
    app_data_root,
    app_preferences,
    task_database,
    task,
    generation_provider: GenerationSource::Higgsfield,
    media_class,
    prompt,
    fallback_model_slug: HIGGSFIELD_FALLBACK_MODEL_SLUG,
    local_files: &local_files,
  }).await?;

  Ok(())
}

/// Ledger a downloaded result as content Higgsfield generated (job id +
/// result URL). Best-effort: a hashing or database problem only costs a
/// future re-upload.
async fn record_generation_result(ledger: &AccountUploadCache, job: &JobStatusResponse, path: &Path) {
  let Some(url) = job.result_url() else { return };
  let (hash, size) = match (hash_file_blake3(path), std::fs::metadata(path)) {
    (Ok(hash), Ok(metadata)) => (hash, metadata.len()),
    (Err(err), _) | (_, Err(err)) => {
      warn!("[HiggsfieldComplete] Could not hash downloaded result of job {}: {}", job.id, err);
      return;
    }
  };
  ledger.record_upload(&hash, size, &CachedAssetUpload::generation_result(job.id.as_str(), url)).await;
}

fn media_class_for(task_type: TaskType) -> TaskMediaFileClass {
  match task_type {
    TaskType::VideoGeneration => TaskMediaFileClass::Video,
    TaskType::ImageGeneration => TaskMediaFileClass::Image,
    // Higgsfield only produces images and videos; anything else is a
    // mis-recorded task, delivered as an image so it isn't lost.
    TaskType::AudioGeneration | TaskType::MeshGeneration | TaskType::SplatGeneration => TaskMediaFileClass::Image,
  }
}

/// The storyteller-web prompt type for the models Higgsfield shares with it.
fn common_model_type_for(model_type: TaskModelType) -> Option<CommonModelType> {
  match model_type {
    TaskModelType::NanoBananaPro => Some(CommonModelType::NanoBananaPro),
    TaskModelType::NanoBanana2 => Some(CommonModelType::NanoBanana2),
    TaskModelType::GptImage2 => Some(CommonModelType::GptImage2),
    TaskModelType::Seedream5p0Pro => Some(CommonModelType::Seedream5p0Pro),
    TaskModelType::Seedream5Lite => Some(CommonModelType::Seedream5Lite),
    TaskModelType::Seedream4p5 => Some(CommonModelType::Seedream4p5),
    TaskModelType::Seedance2p0 => Some(CommonModelType::Seedance2p0),
    TaskModelType::Seedance2p0Mini => Some(CommonModelType::Seedance2p0Mini),
    TaskModelType::Kling3p0Standard => Some(CommonModelType::Kling3p0Standard),
    TaskModelType::Kling3p0Pro => Some(CommonModelType::Kling3p0Pro),
    TaskModelType::GrokImagineVideo1p5 => Some(CommonModelType::GrokImagineVideo1p5),
    _ => None,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn media_class_follows_task_type() {
    assert_eq!(media_class_for(TaskType::ImageGeneration), TaskMediaFileClass::Image);
    assert_eq!(media_class_for(TaskType::VideoGeneration), TaskMediaFileClass::Video);
  }

  #[test]
  fn higgsfield_only_models_have_no_prompt_type() {
    assert!(common_model_type_for(TaskModelType::Seedance2p5).is_none());
    assert!(common_model_type_for(TaskModelType::MinimaxH3).is_none());
    assert!(common_model_type_for(TaskModelType::NanoBanana2Lite).is_none());
    assert_eq!(common_model_type_for(TaskModelType::NanoBananaPro), Some(CommonModelType::NanoBananaPro));
  }
}
