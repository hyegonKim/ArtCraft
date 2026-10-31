use crate::commands::credentials::add_web_credential_command::{save_web_credential, WebCredentialSave};
use crate::credentials::login_website::LoginWebsite;
use crate::events::basic_sendable_event_trait::BasicSendableEvent;
use crate::events::functional_events::refresh_account_state_event::RefreshAccountStateEvent;
use crate::windows::login_window::utils::extract_login_window_cookies::extract_login_window_cookies;
use crate::windows::login_window::utils::extract_user_info_from_cookies::extract_user_info_from_cookies;
use crate::windows::login_window::utils::grok_statsig_capture::read_captured_statsig;
use crate::windows::login_window::login_window_trait::LoginWindowSite;
use crate::windows::login_window::logins::login_site_for;
use crate::windows::login_window::open_login_window::login_window_name;
use crate::state::data_dir::app_data_root::AppDataRoot;
use errors::AnyhowResult;
use log::{error, info};
use std::time::Duration;
use tauri::{AppHandle, Manager, WebviewWindow};

/// How often we poll the webview to see whether login has completed.
const POLL_INTERVAL: Duration = Duration::from_millis(2_000);

/// Path fragments that mean "this is still a login / auth page", so we don't
/// treat it as the logged-in destination. Matched as substrings, so
/// `/login`, `/login/`, `/auth/sign-in`, etc. all count.
const LOGIN_PATH_FRAGMENTS: &[&str] = &["login", "signin", "sign-in"];

/// Watch a login webview until the user finishes signing in, then persist the
/// captured cookies (and any detected identity) and close the window.
///
/// Exits when the window is gone (user closed it) or once cookies are saved.
pub async fn login_window_thread(
  app: AppHandle,
  app_data_root: AppDataRoot,
  website: LoginWebsite,
  maybe_target_credential_id: Option<String>,
) {
  let site = login_site_for(website);
  let window_name = login_window_name(website);

  loop {
    let login_webview_window = match app.get_webview_window(&window_name) {
      Some(webview) => webview,
      None => {
        info!("Exit {} login thread (window gone).", website);
        return; // NB: Only exit if we don't have the webview.
      }
    };

    let result = check_login_window(
      &app,
      &login_webview_window,
      &app_data_root,
      website,
      site.as_ref(),
      maybe_target_credential_id.as_deref(),
    );

    match result {
      Err(err) => {
        error!("Error checking {} login window: {:?}", website, err);
      }
      Ok(false) => {} // Continue iteration and try again...
      Ok(true) => {
        info!("Saved {} cookies from login window. Closing.", website);
        if let Err(err) = login_webview_window.close() {
          error!("Error closing {} login window: {:?}", website, err);
        }
        return;
      }
    }

    tokio::time::sleep(POLL_INTERVAL).await;
  }
}

/// Returns `true` once cookies have been captured and saved (time to exit).
///
/// Stateless by design: login is "done" when the webview is on a recognized
/// destination host, not on a login/auth page, and the session signal is
/// present. No cross-poll flags to get wrong.
fn check_login_window(
  app_handle: &AppHandle,
  webview_window: &WebviewWindow,
  app_data_root: &AppDataRoot,
  website: LoginWebsite,
  site: &dyn LoginWindowSite,
  maybe_target_credential_id: Option<&str>,
) -> AnyhowResult<bool> {
  let url = webview_window.url()?;
  let hostname = url.host_str().unwrap_or_default().to_string();
  let path = url.path().to_ascii_lowercase();

  // Still on an SSO / identity host mid auth-flow: keep waiting.
  if site.auth_flow_hostnames().contains(&hostname.as_str()) {
    return Ok(false);
  }

  // Capture only once we're on a logged-in destination page — a recognized
  // host and not the login page itself. Substring path matching keeps this
  // robust to www / trailing-slash / query-string redirects on the login URL.
  let at_destination = site.destination_hostnames().contains(&hostname.as_str());
  let on_login_page = LOGIN_PATH_FRAGMENTS
      .iter()
      .any(|fragment| path.contains(fragment));

  if !at_destination || on_login_page {
    return Ok(false);
  }

  let cookie_store = extract_login_window_cookies(webview_window, &site.cookie_urls())?;

  let session_cookie_names = site.session_cookie_names();
  let cookie_length = cookie_store.calculate_approx_cookie_character_length();

  // When a site declares its session cookie name(s), their presence is the
  // definitive "logged in" signal — cookies are cleared when the window opens,
  // so a known session cookie can only appear after the user authenticates
  // here. (Login pages set few, small cookies, so the size/count thresholds
  // miss real logins; those are only a fallback for sites whose session cookie
  // names we don't know.)
  let has_captured_session = if session_cookie_names.is_empty() {
    cookie_store.len() >= site.min_cookie_count()
        && cookie_length >= site.min_cookie_char_length()
  } else {
    session_cookie_names.iter().any(|name| cookie_store.has_cookie(name))
  };

  let cookie_names = cookie_store.cookie_names().join(", ");
  info!(
    "{} login check: host `{}`, path `{}`, cookies={} [{}], length={}, has_session={}",
    website, hostname, path, cookie_store.len(), cookie_names, cookie_length, has_captured_session,
  );

  if !has_captured_session {
    return Ok(false);
  }

  // The cookies are the credential — everything below is best-effort and must
  // never prevent the save. Both are plain `Option`s (no blocking, no `?`), so
  // a missing identity or statsig just means those fields stay empty.

  // 1) Try to grab the account's extra information.
  let maybe_user_info = extract_user_info_from_cookies(&cookie_store);
  if let Some(user_info) = &maybe_user_info {
    info!("Detected {} login identity: {:?}", website, user_info);
  }

  // 2) Try to grab the statsig prerequisites the capture harness stashed (Grok
  // only; other sites don't install it, so this is `None`).
  let maybe_statsig = read_captured_statsig(webview_window);
  if maybe_statsig.is_some() {
    info!("Captured {} statsig material (seed)", website);
  }

  // 3) Save the credential file to the credentials directory.
  let credential = save_web_credential(
    app_data_root.credentials_dir(),
    WebCredentialSave {
      service: site.credential_service(),
      cookies: cookie_store,
      maybe_user_info,
      maybe_statsig,
      maybe_user_agent: site.user_agent().map(str::to_string),
      maybe_target_credential_id: maybe_target_credential_id.map(str::to_string),
    },
  )?;

  info!("Saved {} credential to {:?}.", website, credential.source_path);

  let event = RefreshAccountStateEvent {
    provider: website.generation_provider(),
  };
  event.send_infallible(app_handle);

  Ok(true)
}
