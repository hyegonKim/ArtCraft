use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::threads::main_window_thread::main_window_thread::main_window_thread;
use errors::AnyhowResult;
use tauri::AppHandle;

pub fn spawn_main_window_thread(
  app: &AppHandle,
  root: &AppDataRoot,
) -> AnyhowResult<()> {

  tauri::async_runtime::spawn(main_window_thread(
    app.clone(),
    root.clone(),
  ));

  Ok(())
}
