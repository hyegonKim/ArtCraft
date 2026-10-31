use crate::credentials::login_website::LoginWebsite;
use crate::windows::login_window::open_login_window::open_login_window;
use crate::state::data_dir::app_data_root::AppDataRoot;
use log::{error, info};
use tauri::{AppHandle, State};

/// Open a web-login window for a site (e.g. the user clicked "Artcraft").
///
/// Opens a fresh, cookie-cleared webview and drives the login flow; a
/// background thread captures the resulting cookies into the credentials
/// directory once the user signs in.
///
/// `credential_id` targets an existing credential to refresh in place (a
/// re-login after the session expired); it is created anew only if that
/// file is gone by the time login completes. Without it, the login upserts
/// the service's managed credential as before.
#[tauri::command]
pub async fn open_web_login_command(
  app: AppHandle,
  app_data_root: State<'_, AppDataRoot>,
  website: LoginWebsite,
  credential_id: Option<String>,
) -> Result<(), String> {
  info!("open_web_login_command called for website: {} (target credential: {:?})", website, credential_id);

  open_login_window(&app, &app_data_root, website, credential_id)
      .await
      .map_err(|err| {
        error!("Error opening {} login window: {:?}", website, err);
        format!("Error opening {} login window: {}", website, err)
      })
}
