use crate::events::basic_sendable_event_trait::BasicSendableEvent;
use crate::events::generation_events::generation_complete_event::GenerationCompleteEvent;
use crate::error::artcraftx_error::ArtcraftXError;
use crate::state::app_preferences::app_preferences_manager::AppPreferencesManager;
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::state::database::task_database::TaskDatabase;
use crate::state::thumbnails::thumbnail_work_queue::ThumbnailWorkQueue;
use crate::state::app_preferences::settings::preferred_download_filename::{model_slug_from_model_type_str, DownloadFilenameParts};
use chrono::Local;
use crate::utils::download::download_url_to_download_dir_via_temp::download_url_to_download_dir_via_temp;
use crate::utils::download::record_task_download_locations::record_task_download_locations;
use std::path::PathBuf;
use crate::utils::enum_conversion::generation_source::to_generation_service_provider;
use crate::utils::enum_conversion::task_type::to_generation_action;
use super::events::maybe_handle_frontend_caller_notification::maybe_handle_frontend_caller_notification;
use artcraft_client::api_defs::jobs::get_job_status::JobStatusPayload;
use artcraft_client::api_defs::media_file::list_media_files_by_job::JobMediaFileInfo;
use artcraft_client::api_defs::utils::media_links_to_thumbnail_template::media_links_to_thumbnail_template;
use artcraft_client::credentials::storyteller_credential_set::StorytellerCredentialSet;
use artcraft_client::endpoints::media_files::list_media_files_by_job::list_media_files_by_job;
use artcraft_client::enums::by_table::generic_inference_jobs::inference_category::InferenceCategory;
use artcraft_client::enums::by_table::media_files::media_file_class::MediaFileClass;
use artcraft_client::utils::api_host::ApiHost;
use errors::AnyhowResult;
use log::error;
use log::info;
use log::warn;
use sqlite_database::queries::task::Task;
use sqlite_database::queries::update::update_successful_task_status_with_metadata::{update_successful_task_status_with_metadata, UpdateSuccessfulTaskArgs};
use sqlite_identifiers::enums::task_media_file_class::TaskMediaFileClass;
use sqlite_identifiers::ids::media_file_token::MediaFileToken;
use tauri::AppHandle;

pub async fn handle_successful_job(
  app_handle: &AppHandle,
  app_data_root: &AppDataRoot,
  app_preferences: &AppPreferencesManager,
  creds: Option<&StorytellerCredentialSet>,
  job: &JobStatusPayload,
  task: &Task,
  task_database: &TaskDatabase,
) -> AnyhowResult<()> {
  // A job can produce MULTIPLE files (e.g. a 4-image batch). List them all so
  // every file is delivered, not just the job's primary result entity.
  let media_files = list_all_job_media_files(creds, job).await;

  // Primary media file: the first listed file, falling back to the job's
  // single result entity (some generation paths don't populate the
  // media-file -> source-job linkage yet).
  let maybe_primary_media_file_token = media_files.first()
      .map(|file| file.token.clone())
      .or_else(|| {
        job.maybe_result
            .as_ref()
            .map(|result| MediaFileToken::new_from_str(&result.entity_token))
      });

  let updated = update_successful_task_status_with_metadata(UpdateSuccessfulTaskArgs {
    db: task_database.get_connection(),
    task_id: &task.id,
    maybe_batch_token: media_files.first()
        .and_then(|file| file.maybe_batch_token.as_ref()),
    maybe_primary_media_file_token: maybe_primary_media_file_token.as_ref(),
    maybe_primary_media_file_class: get_media_file_class(job, media_files.first()),
    maybe_primary_media_file_thumbnail_url_template: get_thumbnail_template(job, media_files.first()),
    maybe_primary_media_file_cdn_url: media_files.first()
        .map(|file| file.media_links.cdn_url.as_str())
        .or_else(|| {
          job.maybe_result
              .as_ref()
              .map(|result| result.media_links.cdn_url.as_str())
        }),
  }).await?;

  if !updated {
    return Ok(()); // If anything breaks with queries, don't spam events.
  }

  let downloaded = download_all_files(app_data_root, app_preferences, job, task, &media_files).await;
  record_task_download_locations(task_database, &task.id, &downloaded).await;
  ThumbnailWorkQueue::enqueue_for_app(app_handle, &downloaded);

  send_additional_success_events(app_handle, job, task, &media_files).await;

  let service = to_generation_service_provider(task.provider);
  let action = to_generation_action(task.task_type);

  let event = GenerationCompleteEvent {
    action: Some(action),
    service,
    model: None, // TODO
  };

  if let Err(err) = event.send(app_handle) {
    error!("Failed to send GenerationCompleteEvent: {:?}", err); // Fail open
  }

  Ok(())
}

/// Download every file the job produced into the user's configured download
/// directory (temp dir first, then moved into place), named per the user's
/// preferred filename convention. Fails open per file so one bad download
/// doesn't lose the rest. Returns the files' paths (in job order), including
/// ones that were already there from an earlier run.
async fn download_all_files(
  app_data_root: &AppDataRoot,
  app_preferences: &AppPreferencesManager,
  job: &JobStatusPayload,
  task: &Task,
  media_files: &[JobMediaFileInfo],
) -> Vec<PathBuf> {
  let mut downloaded = Vec::new();
  let app_prefs = match app_preferences.get() {
    Ok(prefs) => prefs,
    Err(err) => {
      error!("Can't read app preferences; skipping downloads for job {}: {:?}", job.job_token.as_str(), err);
      return downloaded;
    }
  };

  // Every listed file; fall back to the job's single result entity when the
  // media-file listing is empty.
  let cdn_urls = if media_files.is_empty() {
    job.maybe_result
        .as_ref()
        .map(|result| result.media_links.cdn_url.clone())
        .into_iter()
        .collect::<Vec<_>>()
  } else {
    media_files.iter()
        .map(|file| file.media_links.cdn_url.clone())
        .collect::<Vec<_>>()
  };

  let model_slug = task.model_type
      .as_ref()
      .map(|model_type| model_slug_from_model_type_str(model_type.to_str()))
      .unwrap_or_else(|| "artcraft".to_string());

  let download_time = Local::now();

  for (index, cdn_url) in cdn_urls.iter().enumerate() {
    let extension = cdn_url.path()
        .rsplit('.')
        .next()
        .unwrap_or("bin")
        .to_string();

    let filename = app_prefs.downloads.preferred_download_filename.build_filename(&DownloadFilenameParts {
      model_slug: &model_slug,
      download_time,
      maybe_batch_index: (cdn_urls.len() > 1).then(|| index + 1),
      extension: &extension,
    });

    match download_url_to_download_dir_via_temp(cdn_url, Some(&filename), app_data_root, &app_prefs).await {
      Ok(path) => {
        info!("Downloaded job {} file to {:?}", job.job_token.as_str(), path);
        downloaded.push(path);
      }
      Err(ArtcraftXError::CannotDownloadFilePathAlreadyExists { path }) => {
        info!("Job {} file already downloaded: {:?}", job.job_token.as_str(), path);
        downloaded.push(path);
      }
      Err(err) => {
        error!("Failed to download job {} file {}: {:?}", job.job_token.as_str(), cdn_url, err);
      }
    }
  }
  downloaded
}

/// Fetch every media file the job produced. Fails open with an empty list
/// (callers fall back to the job's single result entity).
async fn list_all_job_media_files(
  creds: Option<&StorytellerCredentialSet>,
  job: &JobStatusPayload,
) -> Vec<JobMediaFileInfo> {
  let creds = match creds {
    Some(creds) => creds,
    None => {
      warn!("No credentials; can't list media files for job {}", job.job_token.as_str());
      return Vec::new();
    }
  };

  match list_media_files_by_job(&ApiHost::Storyteller, creds, &job.job_token).await {
    Ok(response) => response.media_files,
    Err(err) => {
      warn!("Failed to list media files for job {}: {:?}", job.job_token.as_str(), err);
      Vec::new()
    }
  }
}

async fn send_additional_success_events(
  app_handle: &AppHandle,
  job: &JobStatusPayload,
  task: &Task,
  media_files: &[JobMediaFileInfo],
) {
  info!("Attempting to dispatch events for completed Storyteller job: {:?}", task);

  let result = maybe_handle_frontend_caller_notification(
    app_handle,
    task,
    job,
    media_files,
  ).await;

  if let Err(err) = result {
    error!("Failed to send generation complete event: {:?}", err);
  }
}

fn get_thumbnail_template<'a>(
  job: &'a JobStatusPayload,
  maybe_primary_file: Option<&'a JobMediaFileInfo>,
) -> Option<&'a str> {
  if let Some(file) = maybe_primary_file {
    if let Some(template) = media_links_to_thumbnail_template(&file.media_links) {
      return Some(template);
    }
  }

  let links = match job.maybe_result.as_ref() {
    None => return None,
    Some(result) => &result.media_links,
  };

  media_links_to_thumbnail_template(links)
}

fn get_media_file_class(
  job: &JobStatusPayload,
  maybe_primary_file: Option<&JobMediaFileInfo>,
) -> Option<TaskMediaFileClass> {
  // The listed media file's class is authoritative when we have it.
  if let Some(file) = maybe_primary_file {
    match file.media_class {
      MediaFileClass::Audio => return Some(TaskMediaFileClass::Audio),
      MediaFileClass::Image => return Some(TaskMediaFileClass::Image),
      MediaFileClass::Video => return Some(TaskMediaFileClass::Video),
      MediaFileClass::Mesh => return Some(TaskMediaFileClass::Mesh),
      MediaFileClass::Splat => return Some(TaskMediaFileClass::Splat),
      _ => {} // Fall-through (Unknown / legacy Dimensional / Project): infer below.
    }
  }

  match job.request.inference_category {
    InferenceCategory::BackgroundRemoval => return Some(TaskMediaFileClass::Image),
    InferenceCategory::ImageGeneration => return Some(TaskMediaFileClass::Image),
    InferenceCategory::VideoGeneration => return Some(TaskMediaFileClass::Video),
    InferenceCategory::ObjectGeneration => return Some(TaskMediaFileClass::Mesh),
    _ => {}, // Fall-through
  }

  let url = match (maybe_primary_file, job.maybe_result.as_ref()) {
    (Some(file), _) => file.media_links.cdn_url.as_str(),
    (None, Some(result)) => result.media_links.cdn_url.as_str(),
    (None, None) => return None,
  };

  if url.ends_with("jpg")
      || url.ends_with("jpeg")
      || url.ends_with("png")
  {
    return Some(TaskMediaFileClass::Image);
  }

  if url.ends_with("mp4")
      || url.ends_with("webm")
  {
    return Some(TaskMediaFileClass::Video);
  }

  if url.ends_with("glb") {
    return Some(TaskMediaFileClass::Mesh);
  }

  if url.ends_with("ply") || url.ends_with("spz") {
    return Some(TaskMediaFileClass::Splat);
  }

  if url.ends_with("wav")
      || url.ends_with("mp3")
  {
    return Some(TaskMediaFileClass::Audio);
  }

  None
}
