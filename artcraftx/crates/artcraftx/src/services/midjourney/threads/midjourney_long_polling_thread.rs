use crate::credentials::find_service_credentials::find_first_credential_for_service;
use crate::services::midjourney::completion::finalize_midjourney_job::{
  finalize_midjourney_job, FinalizeMidjourneyJobArgs,
};
use crate::services::midjourney::state::midjourney_live_session::MidjourneyLiveSession;
use crate::services::midjourney::utils::extract_midjourney_user_id_from_cookies::extract_midjourney_user_id_from_cookie_header;
use crate::services::midjourney::utils::midjourney_browser_profile::midjourney_browser_profile;
use crate::services::storyteller::state::storyteller_credential_manager::StorytellerCredentialManager;
use crate::state::app_preferences::app_preferences_manager::AppPreferencesManager;
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::state::database::task_database::TaskDatabase;
use crate::database::task_database_pending_statuses::TASK_DATABASE_PENDING_STATUSES;
use artcraft_client::enums::common::generation::common_model_type::CommonModelType;
use core_types::enums::generation_source::GenerationSource;
use errors::AnyhowResult;
use log::{error, warn};
use midjourney_client::credentials::midjourney_user_id::MidjourneyUserId;
use midjourney_client::endpoints::imagine::{imagine, ImagineArgs, ImagineItem, ImagineRequest, MidjourneyJobType};
use midjourney_client::recipes::get_user_info::{get_user_info, GetUserInfoArgs};
use midjourney_client::utils::image_downloader_client::ImageDownloaderClient;
use sqlite_database::queries::read::list_tasks_by_provider_and_status::{
  list_tasks_by_provider_and_status, ListTasksByProviderAndStatusArgs, TaskList,
};
use sqlite_database::queries::task::Task;
use std::collections::HashMap;
use tauri::AppHandle;

const SLEEP_NO_SESSION_MS: u64 = 30_000;
const SLEEP_BETWEEN_TASKS_MS: u64 = 2_000;
const SLEEP_AFTER_BATCH_MS: u64 = 60_000;
const SLEEP_ON_ERROR_MS: u64 = 30_000;

/// Fallback completion thread for first-party Midjourney jobs: it reconciles
/// pending `Midjourney` tasks against the account's `imagine` feed and finalizes
/// any that are ready. The websocket thread is the fast path; this catches
/// anything it missed (socket down, job not streamed). Both call the shared
/// `finalize_midjourney_job`, which dedups via the task-status update.
pub async fn midjourney_long_polling_thread(
  app_handle: AppHandle,
  app_data_root: AppDataRoot,
  app_preferences: AppPreferencesManager,
  task_database: TaskDatabase,
  mj_session: MidjourneyLiveSession,
  storyteller_creds_manager: StorytellerCredentialManager,
) -> ! {
  loop {
    let res = polling_loop(
      &app_handle,
      &app_data_root,
      &app_preferences,
      &task_database,
      &mj_session,
      &storyteller_creds_manager,
    ).await;
    if let Err(err) = res {
      error!("Midjourney polling error: {:?}", err);
    }
    // NB: Only reached if the loop errors out.
    tokio::time::sleep(std::time::Duration::from_millis(SLEEP_ON_ERROR_MS)).await;
  }
}

async fn polling_loop(
  app_handle: &AppHandle,
  app_data_root: &AppDataRoot,
  app_preferences: &AppPreferencesManager,
  task_database: &TaskDatabase,
  mj_session: &MidjourneyLiveSession,
  // Uploads now go to the backup account (see `services::backup`), not the session.
  _storyteller_creds_manager: &StorytellerCredentialManager,
) -> AnyhowResult<()> {
  loop {
    // Cookie credential comes from the unified TOML store.
    let cookie_header = match midjourney_cookie_header(app_data_root) {
      Some(header) => header,
      None => {
        tokio::time::sleep(std::time::Duration::from_millis(SLEEP_NO_SESSION_MS)).await;
        continue;
      }
    };


    let user_id = match resolve_user_id(mj_session, &cookie_header).await {
      Some(user_id) => user_id,
      None => {
        tokio::time::sleep(std::time::Duration::from_millis(SLEEP_NO_SESSION_MS)).await;
        continue;
      }
    };

    let local_tasks = list_tasks_by_provider_and_status(ListTasksByProviderAndStatusArgs {
      db: task_database.get_connection(),
      provider: GenerationSource::Midjourney,
      task_statuses: &TASK_DATABASE_PENDING_STATUSES,
    }).await?;

    poll_midjourney_tasks(
      app_handle,
      app_data_root,
      app_preferences,
      task_database,
      mj_session,
      &cookie_header,
      &user_id,
      local_tasks,
    ).await?;

    tokio::time::sleep(std::time::Duration::from_millis(SLEEP_BETWEEN_TASKS_MS)).await;
  }
}

async fn poll_midjourney_tasks(
  app_handle: &AppHandle,
  app_data_root: &AppDataRoot,
  app_preferences: &AppPreferencesManager,
  task_database: &TaskDatabase,
  mj_session: &MidjourneyLiveSession,
  cookie_header: &str,
  mj_user_id: &MidjourneyUserId,
  local_tasks: TaskList,
) -> AnyhowResult<()> {
  let local_tasks = local_tasks.tasks;
  if local_tasks.is_empty() {
    return Ok(());
  }

  let local_tasks_by_job_id = local_tasks
      .iter()
      .filter_map(|task| task.provider_job_id.clone().map(|job_id| (job_id, task.clone())))
      .collect::<HashMap<String, Task>>();

  let midjourney_result = imagine(ImagineArgs {
    request: ImagineRequest { user_id: mj_user_id, page_size: None },
    cookie_header,
    hostname: None,
    browser: Some(midjourney_browser_profile()),
  }).await?;

  let midjourney_items_by_id = midjourney_result
      .items
      .iter()
      .filter_map(|item| item.id.clone().map(|id| (id, item.clone())))
      .collect::<HashMap<String, ImagineItem>>();

  let image_downloader = ImageDownloaderClient::create(Some(midjourney_browser_profile()))?;

  for (job_id, local_task) in local_tasks_by_job_id.iter() {
    let Some(item) = midjourney_items_by_id.get(job_id) else {
      continue; // Not ready (or not present) in the feed yet.
    };

    // Prefer the feed's command; fall back to the prompt we stashed at enqueue.
    let maybe_full_command = item
        .full_command
        .clone()
        .or_else(|| mj_session.take_pending_prompt(job_id));

    let result = finalize_midjourney_job(FinalizeMidjourneyJobArgs {
      app_handle,
      app_data_root,
      app_preferences,
      task_database,
      image_downloader: &image_downloader,
      midjourney_job_id: job_id,
      local_task,
      model_type: job_type_to_model_type(job_id, item),
      maybe_full_command,
    }).await;

    if let Err(err) = result {
      error!("Failed to finalize Midjourney job {}: {:?}", job_id, err);
    }

    tokio::time::sleep(std::time::Duration::from_millis(SLEEP_BETWEEN_TASKS_MS)).await;
  }

  tokio::time::sleep(std::time::Duration::from_millis(SLEEP_AFTER_BATCH_MS)).await;
  Ok(())
}

// ── Helpers ──

/// The cookie header for the stored `MidjourneyCookies` credential (TOML store).
pub(crate) fn midjourney_cookie_header(app_data_root: &AppDataRoot) -> Option<String> {
  let credential = find_first_credential_for_service(app_data_root, GenerationSource::MidjourneyCookies)?;
  let cookie = credential.cookies()?;
  Some(cookie.cookie_header())
}

/// The Midjourney user id, from the live session or resolved once via the
/// index page (and cached).
pub(crate) async fn resolve_user_id(
  mj_session: &MidjourneyLiveSession,
  cookie_header: &str,
) -> Option<MidjourneyUserId> {
  if let Some(user_id) = mj_session.user_id() {
    return Some(user_id);
  }

  // Prefer the auth-cookie JWT (no network, not Cloudflare-gated).
  if let Some(user_id) = extract_midjourney_user_id_from_cookie_header(cookie_header) {
    mj_session.set_identity(user_id.clone(), None);
    return Some(user_id);
  }

  // Fall back to the index page (needed for the websocket token anyway).
  let info = get_user_info(GetUserInfoArgs {
    cookie_header,
    hostname: None,
    browser: Some(midjourney_browser_profile()),
  }).await;

  match info {
    Ok(info) => {
      let user_id = info.user_id?;
      mj_session.set_identity(user_id.clone(), info.websocket_token);
      Some(user_id)
    }
    Err(err) => {
      warn!("Could not resolve Midjourney user info: {:?}", err);
      None
    }
  }
}

fn job_type_to_model_type(job_id: &str, item: &ImagineItem) -> CommonModelType {
  match item.job_type {
    Some(MidjourneyJobType::V6Diffusion) => CommonModelType::MidjourneyV6,
    Some(MidjourneyJobType::V6p1Diffusion) => CommonModelType::MidjourneyV6p1,
    Some(MidjourneyJobType::V6p1RawDiffusion) => CommonModelType::MidjourneyV6p1Raw,
    Some(MidjourneyJobType::V7Diffusion) => CommonModelType::MidjourneyV7,
    Some(MidjourneyJobType::V7RawDiffusion) => CommonModelType::MidjourneyV7Raw,
    Some(MidjourneyJobType::V7DraftDiffusion) => CommonModelType::MidjourneyV7Draft,
    Some(MidjourneyJobType::V7DraftRawDiffusion) => CommonModelType::MidjourneyV7DraftRaw,
    Some(MidjourneyJobType::Other(ref other)) => {
      warn!("Unknown Midjourney job type (job {}): {}", job_id, other);
      CommonModelType::Midjourney
    }
    _ => CommonModelType::Midjourney,
  }
}
