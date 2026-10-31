use crate::services::grok::util::refresh_grok_statsig::refresh_grok_statsig_if_stale;
use crate::state::data_dir::app_data_root::AppDataRoot;
use log::info;
use tauri::{AppHandle, State};

/// Refresh the Grok statsig material if it's missing or stale.
///
/// Best-effort and non-blocking: it spawns the refresh (which opens a hidden
/// grok.com webview to re-capture) and returns immediately, so callers — the
/// credentials page on mount, say — never wait on it. A no-op when there's no
/// Grok credential or the material is still fresh.
#[tauri::command]
pub async fn refresh_grok_statsig_command(
  app: AppHandle,
  app_data_root: State<'_, AppDataRoot>,
) -> Result<(), String> {
  info!("refresh_grok_statsig_command called");
  // Returns as soon as the (hidden) window is built and the poll loop spawned;
  // the actual capture happens in the background. A single-flight guard inside
  // makes repeat/concurrent calls (e.g. a double-mounting page) safe.
  refresh_grok_statsig_if_stale(&app, app_data_root.inner()).await;
  Ok(())
}
