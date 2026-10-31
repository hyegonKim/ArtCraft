use crate::commands::utils::response::shorthand::ResponseOrErrorMessage;
use crate::commands::utils::response::success_response_wrapper::SerializeMarker;
use crate::services::local_files::thumbnail_service::generated_thumbnail_paths;
use crate::state::database::local_files_database::LocalFilesDatabase;
use crate::state::database::task_database::TaskDatabase;
use local_files_database::queries::get_local_files_by_paths::get_local_files_by_paths;
use chrono::{DateTime, Utc};
use core_types::enums::generation_source::GenerationSource;
use sqlite_identifiers::enums::task_model_type::TaskModelType;
use sqlite_identifiers::enums::task_status::TaskStatus;
use sqlite_identifiers::enums::task_type::TaskType;
use errors::AnyhowResult;
use log::{debug, error, warn};
use serde_derive::Serialize;
use sqlite_database::queries::read::list_tasks_for_frontend::list_tasks_for_frontend;
use tauri::{AppHandle, State};
use sqlite_identifiers::enums::task_failure_type::TaskFailureType;
use sqlite_identifiers::enums::task_media_file_class::TaskMediaFileClass;
use sqlite_identifiers::ids::batch_generation_token::BatchGenerationToken;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;
use sqlite_identifiers::ids::task_id::TaskId;

#[derive(Serialize)]
pub struct GetTaskQueueCommandResponse {
  tasks: Vec<TaskQueueItem>,
}

#[derive(Serialize)]
pub struct TaskQueueItem {
  pub id: TaskId,

  pub task_status: TaskStatus,
  pub task_type: TaskType,
  pub model_type: Option<TaskModelType>,

  pub provider: Option<GenerationSource>,
  pub provider_job_id: Option<String>,

  /// Whether the job produces more than one file.
  pub is_batch_generation: bool,

  /// If the item is done, these will be filled out.
  pub completed_item: Option<CompletedItemData>,

  /// If the item failed, this will be filled out.
  pub failure_reason: Option<FailedItemData>,

  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
  pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub struct CompletedItemData {
  /// Present only when the result was uploaded to ArtCraft (requires a
  /// logged-in session). Local-only completions have download locations but
  /// no cloud media file.
  pub primary_media_file: Option<MediaFileData>,

  /// The type of file(s) generated.
  pub media_file_class: Option<TaskMediaFileClass>,

  /// If generated in a batch, we probably have a batch token we can query.
  pub maybe_batch_token: Option<BatchGenerationToken>,

  /// If the results were downloaded: the directory they were saved into and
  /// the first (or only) file. Absolute paths recorded at completion time.
  pub maybe_download_directory: Option<String>,
  pub maybe_first_downloaded_file: Option<String>,

  /// Thumbnails of the first downloaded file, once the thumbnail worker has
  /// produced them (absolute cache paths; render via the asset protocol).
  /// Until then the frontend hears about them via
  /// `local_thumbnail_ready_event`.
  pub maybe_local_thumbnail_path: Option<String>,
  pub maybe_local_animated_preview_path: Option<String>,
}

#[derive(Serialize)]
pub struct MediaFileData {
  pub token: MediaFileToken,
  pub cdn_url: String,
  pub maybe_thumbnail_url_template: Option<String>,

  // NB: The frontend wants this.
  pub created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct FailedItemData {
  pub failure_type: TaskFailureType,
  pub failure_message: Option<String>,
}

impl SerializeMarker for GetTaskQueueCommandResponse {}

#[tauri::command]
pub async fn get_task_queue_command(
  _app: AppHandle,
  task_database: State<'_, TaskDatabase>,
  local_files_database: State<'_, LocalFilesDatabase>,
) -> ResponseOrErrorMessage<GetTaskQueueCommandResponse> {

  // NB: This is debug because it spams the logs.
  debug!("get_task_queue_command called");

  let result = handle_request(
    &task_database,
    &local_files_database,
  ).await;

  let tasks = match result {
    Ok(items) => items,
    Err(err) => {
      error!("get_task_queue_command failed: {:?}", err);
      return Err("get_task_queue_command failed".into())
    }
  };

  Ok(GetTaskQueueCommandResponse{
    tasks,
  }.into())
}

pub async fn handle_request(
  task_database: &TaskDatabase,
  local_files_database: &LocalFilesDatabase,
) -> AnyhowResult<Vec<TaskQueueItem>> {

  let tasks = list_tasks_for_frontend(task_database.get_connection())
      .await?;

  let mut transformed_tasks = Vec::with_capacity(tasks.tasks.len());

  for task in tasks.tasks.into_iter() {
    let mut completed_item = None;
    let mut failure_reason = None;

    if task.status == TaskStatus::CompleteSuccess {
      // The cloud media file only exists when the result was uploaded to
      // ArtCraft; a local-only completion still gets a completed item so
      // the frontend can render (and thumbnail) the downloaded files.
      let primary_media_file = task.on_complete_primary_media_file_token
          .zip(task.on_complete_primary_media_file_cdn_url)
          .map(|(primary_media_file_token, media_file_url)| MediaFileData {
            token: primary_media_file_token,
            cdn_url: media_file_url,
            maybe_thumbnail_url_template: task.on_complete_primary_media_file_thumbnail_url_template.clone(),
            // NB: This isn't the exact completion date. Also, fallback to now if missing.
            created_at: task.completed_at.unwrap_or_else(Utc::now),
          });

      if primary_media_file.is_none() && task.on_complete_first_file_location.is_none() {
        warn!("Task {} is marked complete but has neither a cloud media file nor a local download.", task.id);
      }

      completed_item = Some(CompletedItemData {
        primary_media_file,
        media_file_class: task.on_complete_primary_media_file_class,
        maybe_batch_token: task.on_complete_batch_token,
        maybe_download_directory: task.on_complete_directory_location.clone(),
        maybe_first_downloaded_file: task.on_complete_first_file_location.clone(),
        maybe_local_thumbnail_path: None,
        maybe_local_animated_preview_path: None,
      });
    } else {
      // If either failure field is present, fill out the failure report.
      if let Some(failure_message) = task.on_failure_message.as_deref() {
        failure_reason = Some(FailedItemData {
          failure_type: task.on_failure_type.unwrap_or(TaskFailureType::Unknown),
          failure_message: Some(failure_message.to_string()),
        });
      } else if let Some(failure_type) = task.on_failure_type {
        failure_reason = Some(FailedItemData {
          failure_type,
          failure_message: None,
        });
      }
    }

    transformed_tasks.push(TaskQueueItem {
      id: task.id,
      task_status: task.status,
      task_type: task.task_type,
      model_type: task.model_type,
      provider: task.provider,
      provider_job_id: task.provider_job_id,
      is_batch_generation: task.is_batch_generation,
      created_at: task.created_at,
      updated_at: task.updated_at,
      completed_at: task.completed_at,
      completed_item,
      failure_reason,
    })
  }

  attach_local_thumbnails(local_files_database, &mut transformed_tasks).await;

  Ok(transformed_tasks)
}

/// Fill in the worker-generated thumbnails for every downloaded result, in
/// one batch lookup. Fails open: a lookup error just leaves them unset.
async fn attach_local_thumbnails(local_files_database: &LocalFilesDatabase, tasks: &mut [TaskQueueItem]) {
  let file_paths: Vec<String> = tasks.iter()
      .filter_map(|task| task.completed_item.as_ref())
      .filter_map(|item| item.maybe_first_downloaded_file.clone())
      .collect();
  if file_paths.is_empty() {
    return;
  }

  let records = match get_local_files_by_paths(local_files_database.get_connection(), &file_paths).await {
    Ok(records) => records,
    Err(err) => {
      warn!("Could not look up local thumbnails for the task queue: {}", err);
      return;
    }
  };

  for item in tasks.iter_mut().filter_map(|task| task.completed_item.as_mut()) {
    let Some(record) = item.maybe_first_downloaded_file.as_ref().and_then(|path| records.get(path)) else {
      continue;
    };
    if let Some((thumbnail_path, maybe_animated_preview_path)) = generated_thumbnail_paths(record) {
      item.maybe_local_thumbnail_path = Some(thumbnail_path);
      item.maybe_local_animated_preview_path = maybe_animated_preview_path;
    }
  }
}
