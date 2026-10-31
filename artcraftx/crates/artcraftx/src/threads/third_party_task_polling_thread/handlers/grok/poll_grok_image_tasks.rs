use crate::services::grok::state::grok_websockets::GrokWebsockets;
use crate::services::storyteller::state::storyteller_credential_manager::StorytellerCredentialManager;
use crate::state::app_preferences::app_preferences_manager::AppPreferencesManager;
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::state::database::task_database::TaskDatabase;
use crate::threads::third_party_task_polling_thread::handlers::grok::grok_image_collector::GrokImageCollector;
use crate::threads::third_party_task_polling_thread::handlers::grok::handle_grok_image_complete::handle_grok_image_complete;
use crate::threads::third_party_task_polling_thread::handlers::grok::handle_grok_image_failure::handle_grok_image_failure;
use grok_consumer_client::endpoint_bindings::generate_image_websocket::grok_generate_image_websocket::{
  ImageWebsocketEvent, DEFAULT_IMAGE_COUNT,
};
use log::{debug, error, info, warn};
use sqlite_database::queries::task::Task;
use std::collections::HashMap;
use std::time::Duration;
use tauri::AppHandle;

/// How long each iteration listens on each socket for new frames.
const SOCKET_POLL_BUDGET: Duration = Duration::from_secs(2);

/// A prompt with at least one image but fewer than expected is treated as
/// done once nothing more arrives for this long.
const PARTIAL_RESULT_IDLE_TIMEOUT: Duration = Duration::from_secs(20);

/// A prompt that produced nothing at all within this window is failed.
const NO_RESULT_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// One polling pass over the pending Grok image tasks: read every account's
/// imagine websocket, route finished images / error frames to their tasks by
/// request id, and complete or fail tasks that are done.
pub async fn poll_grok_image_tasks(
  app_handle: &AppHandle,
  app_data_root: &AppDataRoot,
  app_preferences: &AppPreferencesManager,
  task_database: &TaskDatabase,
  storyteller_creds_manager: &StorytellerCredentialManager,
  grok_websockets: &GrokWebsockets,
  collector: &mut GrokImageCollector,
  grok_tasks: &[&Task],
) {
  // provider_job_id is the websocket request id the enqueue returned.
  let tasks_by_request_id: HashMap<&str, &Task> = grok_tasks.iter()
      .filter_map(|task| task.provider_job_id.as_deref().map(|id| (id, *task)))
      .collect();

  let live_request_ids: Vec<&str> = tasks_by_request_id.keys().copied().collect();
  collector.retain_only(&live_request_ids);
  for request_id in &live_request_ids {
    collector.track(request_id);
  }

  let websockets = match grok_websockets.all() {
    Ok(websockets) => websockets,
    Err(err) => {
      error!("[GrokPolling] Could not read Grok websockets: {:?}", err);
      return;
    }
  };

  if websockets.is_empty() {
    // Pending tasks but no socket (e.g. app restarted): they can only time out.
    debug!("[GrokPolling] {} pending Grok task(s) but no live websocket", grok_tasks.len());
  }

  // 1) Drain every socket and route events to tasks.
  for (credential_id, websocket) in &websockets {
    let events = match websocket.poll_events(SOCKET_POLL_BUDGET).await {
      Ok(events) => events,
      Err(err) => {
        warn!("[GrokPolling] Websocket for credential {} failed ({}); reconnecting", credential_id.as_str(), err);
        if let Err(err) = websocket.reconnect().await {
          error!("[GrokPolling] Could not reconnect Grok websocket for credential {}: {}", credential_id.as_str(), err);
        }
        continue;
      }
    };

    for event in events {
      match event {
        ImageWebsocketEvent::Completed(image) => {
          let request_id = image.request_id.to_string();
          if collector.push_image(image) {
            info!("[GrokPolling] Image arrived for request {}", request_id);
          } else {
            debug!("[GrokPolling] Ignoring image for untracked request {}", request_id);
          }
        }
        ImageWebsocketEvent::Failed { request_id, message, err_code, raw_frame } => {
          let reason = match (&message, &err_code) {
            (Some(message), _) => format!("Grok rejected the prompt: {message}"),
            (None, Some(code)) => format!("Grok rejected the prompt (code {code})"),
            (None, None) => "Grok rejected the prompt".to_string(),
          };
          warn!("[GrokPolling] Error frame (request {:?}): {}", request_id.as_ref().map(|id| id.to_string()), raw_frame);

          let Some(request_id) = request_id.map(|id| id.to_string()) else {
            warn!("[GrokPolling] Error frame names no request id; can't attribute it to a task");
            continue;
          };
          if let Some(task) = tasks_by_request_id.get(request_id.as_str()) {
            collector.remove(&request_id);
            handle_grok_image_failure(app_handle, task_database, task, &reason).await;
          }
        }
      }
    }
  }

  // 2) Complete / fail tasks that are done.
  for (request_id, task) in &tasks_by_request_id {
    let Some(pending) = collector.get(request_id) else {
      continue; // Just failed above.
    };

    if pending.is_complete(DEFAULT_IMAGE_COUNT, PARTIAL_RESULT_IDLE_TIMEOUT) {
      let pending = collector.remove(request_id).expect("checked above");
      handle_grok_image_complete(
        app_handle,
        app_data_root,
        app_preferences,
        task_database,
        storyteller_creds_manager,
        task,
        &pending.images,
      ).await;
    } else if pending.is_timed_out(NO_RESULT_TIMEOUT) {
      collector.remove(request_id);
      handle_grok_image_failure(
        app_handle,
        task_database,
        task,
        "Grok did not return any images in time",
      ).await;
    }
  }
}
