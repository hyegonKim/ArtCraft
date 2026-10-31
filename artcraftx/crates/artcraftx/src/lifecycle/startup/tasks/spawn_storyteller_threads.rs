use crate::state::app_preferences::app_preferences_manager::AppPreferencesManager;
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::state::runtime::artcraft_platform_info::ArtcraftPlatformInfo;
use crate::state::usage_tracker::artcraft_usage_tracker::ArtcraftUsageTracker;
use crate::state::database::task_database::TaskDatabase;
use crate::services::storyteller::state::storyteller_credential_manager::StorytellerCredentialManager;
use crate::services::storyteller::threads::storyteller_activity_thread::storyteller_activity_thread;
use crate::services::storyteller::threads::storyteller_task_polling_thread::storyteller_task_polling_thread::storyteller_task_polling_thread;
use errors::AnyhowResult;
use tauri::AppHandle;

pub fn spawn_storyteller_threads(
  app: &AppHandle,
  app_data_root: &AppDataRoot,
  app_preferences: &AppPreferencesManager,
  artcraft_usage_tracker: &ArtcraftUsageTracker,
  artcraft_platform_info: &ArtcraftPlatformInfo,
  task_database: &TaskDatabase,
  storyteller_creds_manager: &StorytellerCredentialManager,
) -> AnyhowResult<()> {
  
  tauri::async_runtime::spawn(storyteller_task_polling_thread(
    app.clone(),
    app_data_root.clone(),
    app_preferences.clone(),
    task_database.clone(),
    storyteller_creds_manager.clone(),
  ));

  tauri::async_runtime::spawn(storyteller_activity_thread(
    artcraft_platform_info.clone(),
    artcraft_usage_tracker.clone(),
    storyteller_creds_manager.clone(),
  ));
  
  Ok(())
}
