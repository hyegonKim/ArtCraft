use crate::lifecycle::startup::tasks::bootstrap_local_files_database::bootstrap_local_files_database;
use crate::lifecycle::startup::tasks::bootstrap_task_database::bootstrap_task_database;
use crate::lifecycle::startup::tasks::initially_size_and_position_windows::initially_size_and_position_windows;
use crate::lifecycle::startup::tasks::set_app_log_level::set_app_log_level;
use crate::lifecycle::startup::tasks::spawn_discord_presence_thread::spawn_discord_presence_thread;
use crate::lifecycle::startup::tasks::spawn_main_window_thread::spawn_main_window_thread;
use crate::lifecycle::startup::tasks::spawn_storyteller_threads::spawn_storyteller_threads;
use crate::lifecycle::startup::tasks::spawn_thumbnail_worker_thread::spawn_thumbnail_worker_thread;
use crate::state::runtime::artcraft_platform_info::ArtcraftPlatformInfo;
use crate::state::usage_tracker::artcraft_usage_tracker::ArtcraftUsageTracker;
use crate::state::app_preferences::app_preferences_manager::AppPreferencesManager;
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::services::grok::state::grok_credential_manager::GrokCredentialManager;
use crate::services::grok::state::grok_websockets::GrokWebsockets;
use crate::services::grok::threads::grok_video_task_polling::grok_video_task_polling_thread::grok_video_task_polling_thread;
use crate::services::midjourney::state::midjourney_live_session::MidjourneyLiveSession;
use crate::services::midjourney::threads::midjourney_long_polling_thread::midjourney_long_polling_thread;
use crate::services::midjourney::threads::midjourney_websocket_thread::midjourney_websocket_thread;
use crate::services::storyteller::state::storyteller_credential_manager::StorytellerCredentialManager;
use crate::threads::third_party_task_polling_thread::third_party_task_polling_thread::third_party_task_polling_thread;
use errors::AnyhowResult;
use tauri::AppHandle;

pub async fn handle_tauri_startup(
  app: AppHandle,
  root: AppDataRoot,
  app_preferences: AppPreferencesManager,
  artcraft_platform_info: ArtcraftPlatformInfo,
  artcraft_usage_tracker: ArtcraftUsageTracker,
  storyteller_creds_manager: StorytellerCredentialManager,
  midjourney_live_session: MidjourneyLiveSession,
  grok_creds_manager: GrokCredentialManager,
  grok_websockets: GrokWebsockets,
) -> AnyhowResult<()> {

  set_app_log_level(
    &app,
    &root,
  )?;

  let task_database =
      bootstrap_task_database(&app, &root).await?;

  let local_files_database =
      bootstrap_local_files_database(&app, &root).await?;

  spawn_thumbnail_worker_thread(
    &app,
    &root,
    &task_database,
    &local_files_database,
  )?;

  spawn_main_window_thread(
    &app,
    &root,
  )?;

  spawn_storyteller_threads(
    &app,
    &root,
    &app_preferences,
    &artcraft_usage_tracker,
    &artcraft_platform_info,
    &task_database,
    &storyteller_creds_manager,
  )?;

  tauri::async_runtime::spawn(grok_video_task_polling_thread(
    app.clone(),
    root.clone(),
    app_preferences.clone(),
    task_database.clone(),
    grok_creds_manager.clone(),
    storyteller_creds_manager.clone(),
  ));

  tauri::async_runtime::spawn(midjourney_websocket_thread(
    app.clone(),
    root.clone(),
    app_preferences.clone(),
    task_database.clone(),
    midjourney_live_session.clone(),
    storyteller_creds_manager.clone(),
  ));

  tauri::async_runtime::spawn(midjourney_long_polling_thread(
    app.clone(),
    root.clone(),
    app_preferences.clone(),
    task_database.clone(),
    midjourney_live_session.clone(),
    storyteller_creds_manager.clone(),
  ));

  tauri::async_runtime::spawn(third_party_task_polling_thread(
    app.clone(),
    root.clone(),
    app_preferences.clone(),
    task_database.clone(),
    storyteller_creds_manager.clone(),
    grok_websockets.clone(),
  ));

  spawn_discord_presence_thread()?;

  initially_size_and_position_windows(&app, &root);

  Ok(())
}
