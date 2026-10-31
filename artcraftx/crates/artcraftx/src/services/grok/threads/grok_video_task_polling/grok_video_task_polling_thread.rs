use crate::database::task_database_pending_statuses::TASK_DATABASE_PENDING_STATUSES;
use crate::services::grok::state::grok_credential_manager::GrokCredentialManager;
use crate::services::grok::util::get_or_upgrade_grok_full_credentials::get_or_update_grok_full_credentials;
use crate::services::storyteller::state::storyteller_credential_manager::StorytellerCredentialManager;
use crate::state::app_preferences::app_preferences_manager::AppPreferencesManager;
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::state::data_dir::subdirectory::trait_data_subdir::DataSubdir;
use crate::state::database::task_database::TaskDatabase;
use crate::threads::task_completion::complete_task_with_local_files::{complete_task_with_local_files, CompleteTaskArgs};
use crate::threads::task_completion::upload_results_to_artcraft::CompletionPrompt;
use artcraft_client::enums::common::generation::common_model_type::CommonModelType;
use core_types::enums::generation_source::GenerationSource;
use errors::AnyhowResult;
use grok_consumer_client::credentials::grok_full_credentials::GrokFullCredentials;
use grok_consumer_client::endpoint_bindings::old_bindings::download_video_file::grok_download_video::GrokDownloadVideo;
use grok_consumer_client::endpoint_bindings::old_bindings::media_posts::list_media_posts::grok_list_media_posts::{GrokMediaPostListRequest, VideoData};
use log::{error, info};
use sqlite_database::queries::read::list_tasks_by_provider_and_status::{list_tasks_by_provider_and_status, ListTasksByProviderAndStatusArgs, TaskList};
use sqlite_database::queries::task::Task;
use sqlite_identifiers::enums::task_media_file_class::TaskMediaFileClass;
use std::collections::HashMap;
use std::time::Duration;
use tauri::AppHandle;

/// Download filename slug when the task has no model type recorded.
const GROK_VIDEO_FALLBACK_MODEL_SLUG: &str = "grok_video";

pub async fn grok_video_task_polling_thread(
  app_handle: AppHandle,
  app_data_root: AppDataRoot,
  app_preferences: AppPreferencesManager,
  task_database: TaskDatabase,
  creds: GrokCredentialManager,
  storyteller_creds_manager: StorytellerCredentialManager,
) -> ! {
  loop {
    let res = polling_loop(
      &app_handle,
      &app_data_root,
      &app_preferences,
      &task_database,
      &creds,
      &storyteller_creds_manager,
    ).await;
    if let Err(err) = res {
      error!("An error occurred: {:?}", err);
    }
    // NB: Only sleep if an error occurs.
    tokio::time::sleep(std::time::Duration::from_millis(30_000)).await;
  }
}

async fn polling_loop(
  app_handle: &AppHandle,
  app_data_root: &AppDataRoot,
  app_preferences: &AppPreferencesManager,
  task_database: &TaskDatabase,
  grok_creds: &GrokCredentialManager,
  // Uploads now go to the backup account (see `services::backup`), not the session.
  _storyteller_creds_manager: &StorytellerCredentialManager,
) -> AnyhowResult<()> {
  loop {
    if !grok_creds.do_task_polling()? {
      tokio::time::sleep(std::time::Duration::from_millis(10_000)).await;
      continue;
    }


    let grok_full_creds = match get_or_update_grok_full_credentials(&grok_creds).await {
      Ok(creds) => creds,
      Err(err) => {
        info!("No full grok credentials: {:?}", err);
        tokio::time::sleep(std::time::Duration::from_millis(30_000)).await;
        continue;
      }
    };

    let local_tasks = list_tasks_by_provider_and_status(ListTasksByProviderAndStatusArgs {
      db: task_database.get_connection(),
      provider: GenerationSource::Grok,
      task_statuses: &TASK_DATABASE_PENDING_STATUSES,
    }).await?;

    poll_grok_tasks(
      app_handle,
      app_data_root,
      app_preferences,
      task_database,
      &grok_full_creds,
      local_tasks,
    ).await?;

    tokio::time::sleep(std::time::Duration::from_millis(2_000)).await;
  }
}

async fn poll_grok_tasks(
  app_handle: &AppHandle,
  app_data_root: &AppDataRoot,
  app_preferences: &AppPreferencesManager,
  task_database: &TaskDatabase,
  grok_full_creds: &GrokFullCredentials,
  local_tasks: TaskList,
) -> AnyhowResult<()> {
  let local_tasks = local_tasks.tasks;

  if local_tasks.is_empty() {
    return Ok(())
  }
  
  info!("Grok tasks waiting: {:?}", local_tasks.len());

  // Map of Grok Post ID to Local Task.
  let local_tasks_by_grok_post_id = local_tasks.iter()
      .filter_map(|task| {
        if let Some(provider_job_id) = &task.provider_job_id {
          Some((provider_job_id.clone(), task.clone()))
        } else {
          None
        }
      })
      .collect::<HashMap<String, Task>>();

  let list_media_request = GrokMediaPostListRequest {
    cookie: grok_full_creds.cookies.as_str(),
    cursor: None,
    request_timeout: Some(Duration::from_millis(20_000)),
  };

  let list_result  = list_media_request.send().await?;

  let grok_posts = list_result.posts;

  let grok_video_posts_by_id = {
    let mut hash = HashMap::new();
    for post in grok_posts.iter() {
      if let Some(video_data) = &post.video_data {
        hash.insert(post.post_id.to_string(), video_data.clone());
      }
    }
    hash
  };

  for (grok_post_id, local_task) in local_tasks_by_grok_post_id.iter() {
    // TODO: Copy prompt from this.
    let grok_video_data = match grok_video_posts_by_id.get(grok_post_id) {
      Some(video_data) => video_data,
      None => continue,
    };

    let result = complete_grok_video(
      app_handle,
      app_data_root,
      app_preferences,
      task_database,
      grok_full_creds,
      grok_post_id,
      local_task,
      grok_video_data,
    ).await;

    if let Err(err) = result {
      error!("Failed to complete Grok video task {}: {:?}", local_task.id.as_str(), err);
    }

    tokio::time::sleep(std::time::Duration::from_millis(2_000)).await;
  }

  tokio::time::sleep(std::time::Duration::from_millis(60_000)).await;

  Ok(())
}

/// Download the finished video to the temp dir, then hand it to the shared
/// completion routine.
async fn complete_grok_video(
  app_handle: &AppHandle,
  app_data_root: &AppDataRoot,
  app_preferences: &AppPreferencesManager,
  task_database: &TaskDatabase,
  grok_full_creds: &GrokFullCredentials,
  grok_post_id: &str,
  local_task: &Task,
  grok_video_post: &VideoData,
) -> AnyhowResult<bool> {
  info!("Downloading generated Grok video ...");

  let download_path = app_data_root.temp_dir().path().join(format!("{}.mp4", grok_post_id));

  let download = GrokDownloadVideo {
    cookies: grok_full_creds.cookies.as_str(),
    user_id: grok_full_creds.get_user_id_ref(),
    file_id: &grok_video_post.file_id,
    request_timeout: None,
  };

  download.download_to_path(&download_path).await?;

  complete_task_with_local_files(CompleteTaskArgs {
    app_handle,
    app_data_root,
    app_preferences,
    task_database,
    task: local_task,
    generation_provider: GenerationSource::Grok,
    media_class: TaskMediaFileClass::Video,
    prompt: CompletionPrompt::Create {
      model_type: CommonModelType::GrokVideo,
      maybe_prompt: grok_video_post.prompt.clone(),
    },
    fallback_model_slug: GROK_VIDEO_FALLBACK_MODEL_SLUG,
    local_files: &[download_path],
  }).await
}
