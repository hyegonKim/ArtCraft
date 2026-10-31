use crate::services::midjourney::completion::finalize_midjourney_job::{
  finalize_midjourney_job, FinalizeMidjourneyJobArgs,
};
use crate::services::midjourney::state::midjourney_live_session::MidjourneyLiveSession;
use crate::services::midjourney::threads::midjourney_long_polling_thread::{
  midjourney_cookie_header, resolve_user_id,
};
use crate::services::midjourney::utils::midjourney_browser_profile::midjourney_browser_profile;
use crate::services::storyteller::state::storyteller_credential_manager::StorytellerCredentialManager;
use crate::state::app_preferences::app_preferences_manager::AppPreferencesManager;
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::state::database::task_database::TaskDatabase;
use crate::database::task_database_pending_statuses::TASK_DATABASE_PENDING_STATUSES;
use artcraft_client::enums::common::generation::common_model_type::CommonModelType;
use core_types::enums::generation_source::GenerationSource;
use errors::AnyhowResult;
use log::{error, info, warn};
use midjourney_client::client::websocket::midjourney_websocket::MidjourneyWebSocket;
use midjourney_client::client::websocket::midjourney_ws_event::MidjourneyWsEvent;
use midjourney_client::client::websocket::open_midjourney_websocket::{
  open_midjourney_websocket, OpenMidjourneyWebSocketRequest,
};
use midjourney_client::utils::image_downloader_client::ImageDownloaderClient;
use sqlite_database::queries::read::list_tasks_by_provider_and_status::{
  list_tasks_by_provider_and_status, ListTasksByProviderAndStatusArgs,
};
use sqlite_database::queries::task::Task;
use std::collections::{HashMap, HashSet};
use tauri::AppHandle;
use tokio::sync::broadcast::error::RecvError;

const SLEEP_NOT_READY_MS: u64 = 15_000;
const SLEEP_AFTER_SOCKET_DEATH_MS: u64 = 5_000;

/// How often (while the socket is up) to rescan the tasks DB for newly
/// enqueued Midjourney jobs and subscribe to them.
const RESCAN_INTERVAL_MS: u64 = 3_000;

/// The fast completion path: keep a Midjourney websocket open, subscribe to
/// every pending job, and finalize as soon as its `completed` frame arrives.
/// The long-polling thread is the fallback for anything this misses.
pub async fn midjourney_websocket_thread(
  app_handle: AppHandle,
  app_data_root: AppDataRoot,
  app_preferences: AppPreferencesManager,
  task_database: TaskDatabase,
  mj_session: MidjourneyLiveSession,
  storyteller_creds_manager: StorytellerCredentialManager,
) -> ! {
  loop {
    let res = run_once(
      &app_handle,
      &app_data_root,
      &app_preferences,
      &task_database,
      &mj_session,
      &storyteller_creds_manager,
    ).await;

    // The socket closed (or never opened). Drop the handle so we re-open next
    // pass, and let the long-poller cover the gap.
    mj_session.clear_websocket();

    if let Err(err) = res {
      warn!("Midjourney websocket session ended: {:?}", err);
    }
    tokio::time::sleep(std::time::Duration::from_millis(SLEEP_AFTER_SOCKET_DEATH_MS)).await;
  }
}

async fn run_once(
  app_handle: &AppHandle,
  app_data_root: &AppDataRoot,
  app_preferences: &AppPreferencesManager,
  task_database: &TaskDatabase,
  mj_session: &MidjourneyLiveSession,
  // Uploads now go to the backup account (see `services::backup`), not the session.
  _storyteller_creds_manager: &StorytellerCredentialManager,
) -> AnyhowResult<()> {
  let cookie_header = match midjourney_cookie_header(app_data_root) {
    Some(header) => header,
    None => return sleep_not_ready().await,
  };


  // Resolves and caches user_id + websocket_token in the session.
  let user_id = match resolve_user_id(mj_session, &cookie_header).await {
    Some(user_id) => user_id,
    None => return sleep_not_ready().await,
  };

  let websocket_token = match mj_session.websocket_token() {
    Some(token) => token,
    None => {
      // No token available (index page didn't expose one); leave it to the poller.
      return sleep_not_ready().await;
    }
  };

  let websocket = match mj_session.connected_websocket() {
    Some(ws) => ws,
    None => {
      let ws = open_midjourney_websocket(OpenMidjourneyWebSocketRequest {
        websocket_token: &websocket_token,
        user_id,
        hostname: None,
        browser: Some(midjourney_browser_profile()),
      }).await?;
      mj_session.set_websocket(ws.clone());
      info!("Midjourney websocket opened for fast completion.");
      ws
    }
  };

  drive_websocket(
    app_handle,
    app_data_root,
    app_preferences,
    task_database,
    mj_session,
    websocket,
  ).await
}

/// Subscribe to pending jobs and finalize them as `completed` frames arrive,
/// returning when the socket closes.
async fn drive_websocket(
  app_handle: &AppHandle,
  app_data_root: &AppDataRoot,
  app_preferences: &AppPreferencesManager,
  task_database: &TaskDatabase,
  mj_session: &MidjourneyLiveSession,
  websocket: MidjourneyWebSocket,
) -> AnyhowResult<()> {
  let image_downloader = ImageDownloaderClient::create(Some(midjourney_browser_profile()))?;
  let mut events = websocket.events();
  let mut subscribed: HashSet<String> = HashSet::new();
  let mut pending_tasks_by_job_id: HashMap<String, Task> = HashMap::new();

  // Subscribe to whatever is already pending before we start waiting.
  refresh_subscriptions(task_database, &websocket, &mut subscribed, &mut pending_tasks_by_job_id).await?;

  let mut rescan = tokio::time::interval(std::time::Duration::from_millis(RESCAN_INTERVAL_MS));

  loop {
    tokio::select! {
      _ = rescan.tick() => {
        if !websocket.is_connected() {
          return Ok(());
        }
        if let Err(err) = refresh_subscriptions(
          task_database, &websocket, &mut subscribed, &mut pending_tasks_by_job_id,
        ).await {
          warn!("Failed to refresh Midjourney subscriptions: {:?}", err);
        }
      }

      received = events.recv() => {
        match received {
          Ok(event) => {
            if let MidjourneyWsEvent::Completed { job_id, .. } = event.as_ref() {
              let Some(local_task) = pending_tasks_by_job_id.get(job_id).cloned() else {
                continue; // Not one of ours (or already handled).
              };

              let maybe_full_command = mj_session.take_pending_prompt(job_id);

              let result = finalize_midjourney_job(FinalizeMidjourneyJobArgs {
                app_handle,
                app_data_root,
                app_preferences,
                task_database,
                image_downloader: &image_downloader,
                midjourney_job_id: job_id,
                local_task: &local_task,
                // NB: the `completed` frame carries no model detail; the
                // poller sets a precise model when it finalizes instead.
                model_type: CommonModelType::Midjourney,
                maybe_full_command,
              }).await;

              match result {
                Ok(true) => info!("Finalized Midjourney job {} via websocket.", job_id),
                Ok(false) => {} // Already finalized elsewhere.
                Err(err) => error!("Websocket finalize failed for job {}: {:?}", job_id, err),
              }

              pending_tasks_by_job_id.remove(job_id);
            }
          }
          Err(RecvError::Lagged(skipped)) => {
            warn!("Midjourney websocket consumer lagged, skipped {} events", skipped);
          }
          Err(RecvError::Closed) => {
            return Ok(()); // Socket closed; outer loop re-opens.
          }
        }
      }
    }
  }
}

/// List currently-pending Midjourney tasks, subscribe to any we haven't yet,
/// and refresh the job-id → task lookup used to finalize completions.
async fn refresh_subscriptions(
  task_database: &TaskDatabase,
  websocket: &MidjourneyWebSocket,
  subscribed: &mut HashSet<String>,
  pending_tasks_by_job_id: &mut HashMap<String, Task>,
) -> AnyhowResult<()> {
  let pending = list_tasks_by_provider_and_status(ListTasksByProviderAndStatusArgs {
    db: task_database.get_connection(),
    provider: GenerationSource::Midjourney,
    task_statuses: &TASK_DATABASE_PENDING_STATUSES,
  }).await?;

  pending_tasks_by_job_id.clear();
  for task in pending.tasks {
    let Some(job_id) = task.provider_job_id.clone() else {
      continue;
    };
    pending_tasks_by_job_id.insert(job_id.clone(), task);

    if subscribed.insert(job_id.clone()) {
      if let Err(err) = websocket.subscribe_to_job(&job_id) {
        warn!("Failed to subscribe to Midjourney job {}: {:?}", job_id, err);
        subscribed.remove(&job_id);
      }
    }
  }

  Ok(())
}

async fn sleep_not_ready() -> AnyhowResult<()> {
  tokio::time::sleep(std::time::Duration::from_millis(SLEEP_NOT_READY_MS)).await;
  Ok(())
}
