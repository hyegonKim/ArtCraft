use crate::state::runtime::app_startup_time::AppStartupTime;
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::state::window::main_window_position::MainWindowPosition;
use crate::state::window::main_window_size::MainWindowSize;
use crate::threads::main_window_thread::persist_window_position_task::persist_window_position_task;
use crate::threads::main_window_thread::persist_window_resize_task::persist_window_resize_task;
use crate::windows::main_window::constants::MAIN_WINDOW_NAME;
use errors::AnyhowResult;
use log::{error, info, warn};
use memory_store::clone_slot::CloneSlot;
use tauri::{AppHandle, Manager, Window};

pub async fn main_window_thread(
  app: AppHandle,
  app_data_root: AppDataRoot,
) -> ! {
  // TODO: Move these into some kind of dependency injection framework
  let window_size_slot: CloneSlot<MainWindowSize> = CloneSlot::empty();
  let window_pos_slot: CloneSlot<MainWindowPosition> = CloneSlot::empty();
  let _app_startup_time = AppStartupTime::new();

  // Emit debugging information to the logs (do not remove this!)
  info!("git commit id: {:?}", build_metadata::git_commit_id()
    .unwrap_or_else(|| "unknown"));
  info!("git commit timestamp: {:?}", build_metadata::git_commit_timestamp()
    .map(|t| t.to_string())
    .unwrap_or_else(|| "unknown".to_string()));
  info!("build timestamp: {:?}", build_metadata::build_timestamp()
    .to_string());
  
  // debug_try_clear_all_webview_data(&app);

  loop {
    for (window_name, window) in app.windows() {
      if window_name == MAIN_WINDOW_NAME {
        let result = handle_main_window(
          &window,
          &app_data_root,
          &window_size_slot,
          &window_pos_slot,
        ).await;
        if let Err(err) = result {
          error!("Error handling main window: {:?}", err);
        }
      }
    }
    tokio::time::sleep(std::time::Duration::from_millis(1_000)).await;
  }
}

pub async fn handle_main_window(
  window: &Window,
  app_data_root: &AppDataRoot,
  window_size_slot: &CloneSlot<MainWindowSize>,
  window_pos_slot: &CloneSlot<MainWindowPosition>,
) -> AnyhowResult<()> {
  loop {
    log_errors(persist_window_resize_task(window, app_data_root, window_size_slot).await);
    log_errors(persist_window_position_task(window, app_data_root, window_pos_slot).await);
    tokio::time::sleep(std::time::Duration::from_millis(1_000)).await;
  }
}

pub fn log_errors<T>(result: AnyhowResult<T>) {
  if let Err(err) = result {
    error!("Error persisting window size: {:?}", err);
  }
}

fn debug_try_clear_all_webview_data(app: &AppHandle) {
  warn!("[!!!] THIS IS ONLY FOR DEBUGGING PURPOSES: Attempting to clear webview data...");
  
  for (name, webview) in app.webviews() {
    if let Err(err) = webview.clear_all_browsing_data() {
      error!("Failed to clear cookies for '{}' window webview: {:?}", name, err);
    } else {
      warn!("Successfully cleared cookies for '{}' window webview", name);
    }
  }
}
