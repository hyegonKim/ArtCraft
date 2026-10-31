//! Grok statsig refresh.
//!
//! When a Grok credential exists but its statsig (the browser-minted
//! `x-statsig-id` plus its seed material) is missing or stale, open a *hidden*
//! grok.com webview so the passive capture harness records a fresh signature
//! the page makes on load, then persist it onto the credential file.
//!
//! Two entry points:
//!
//! - [`refresh_grok_statsig_if_stale`] — fire-and-forget. Used by the login
//!   flow and the credentials page; returns as soon as the hidden window is
//!   built and the poll loop is spawned.
//! - [`refresh_grok_statsig_blocking`] — awaits the whole capture. Used by the
//!   generation workers right before (or after a failed) enqueue, so the retry
//!   can pick up the fresh statsig.
//!
//! Relies on the shared WebKit session (the cookies the login window captured),
//! so the hidden window loads logged-in without re-authenticating.
//!
//! Concurrency: a process-wide single-flight guard ensures only one refresh
//! runs at a time — the credentials page mounts can fire the command twice (and
//! two concurrent same-label window builds deadlock the UI). The blocking
//! variant waits for an in-flight refresh instead of starting a second one.

use crate::credentials::auth_credential::CredentialSecret;
use crate::credentials::cookie_credential_grok_extra_pieces::{
  grok_statsig_needs_refresh, CookieCredentialGrokExtraPieces, GrokStatsigCapture,
};
use crate::credentials::find_service_credentials::find_first_credential_for_service;
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::windows::login_window::utils::grok_statsig_capture::{grok_statsig_init_script, read_captured_statsig};
use chrono::Utc;
use core_types::enums::generation_source::GenerationSource;
use errors::{anyhow, AnyhowResult};
use log::{info, warn};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const REFRESH_WINDOW_NAME: &str = "grok_statsig_refresh";
const GROK_URL: &str = "https://grok.com/";

/// Let the page load before reading cookies, then poll gently up to a timeout.
const INITIAL_LOAD_DELAY: Duration = Duration::from_secs(3);
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(20);
const CAPTURE_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// How long a blocking caller waits for another in-flight refresh to finish
/// before giving up. Comfortably above a full capture cycle.
const WAIT_FOR_IN_FLIGHT_TIMEOUT: Duration = Duration::from_secs(45);
const WAIT_FOR_IN_FLIGHT_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Single-flight guard: only one refresh (and thus one refresh window) at a time.
static REFRESH_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// Resets the single-flight flag on drop, so a panic or early return can never
/// wedge future refreshes.
struct RefreshGuard;

impl RefreshGuard {
  /// Acquire the guard, or `None` if a refresh is already running.
  fn acquire() -> Option<Self> {
    REFRESH_IN_PROGRESS
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .ok()
        .map(|_| RefreshGuard)
  }

  fn is_held() -> bool {
    REFRESH_IN_PROGRESS.load(Ordering::SeqCst)
  }
}

impl Drop for RefreshGuard {
  fn drop(&mut self) {
    REFRESH_IN_PROGRESS.store(false, Ordering::SeqCst);
  }
}

/// Outcome of a blocking refresh.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GrokStatsigRefreshOutcome {
  /// A fresh statsig was captured and saved to the credential file.
  Refreshed,
  /// Nothing to do: no Grok credential, or the stored statsig is still fresh
  /// (possibly because a concurrent refresh just finished).
  StillFresh,
  /// The hidden window loaded but no signed request was observed in time.
  NothingCaptured,
}

/// Refresh the Grok statsig if missing/stale. Returns immediately after
/// kicking off a background poll; safe to call on login, credentials-page
/// mount, etc. Non-blocking and best-effort.
pub async fn refresh_grok_statsig_if_stale(app: &AppHandle, app_data_root: &AppDataRoot) {
  if !stored_statsig_is_stale(app_data_root) {
    return; // Not logged in, or still fresh.
  }

  let Some(guard) = RefreshGuard::acquire() else {
    return; // A refresh is already running.
  };

  if let Err(err) = start_background_refresh(app, app_data_root, guard) {
    warn!("Grok statsig refresh could not start: {err:?}");
    // `guard` was moved into start_background_refresh only on success; on
    // error it has already been dropped, releasing the flag.
  }
}

/// Refresh the Grok statsig and wait for the result.
///
/// `force` re-captures even when the stored statsig still looks fresh — used
/// after Grok rejects a request, since the freshness window is a heuristic.
/// If another refresh is in flight, waits for it and reports its result via
/// the credential file's freshness rather than starting a second window.
pub async fn refresh_grok_statsig_blocking(
  app: &AppHandle,
  app_data_root: &AppDataRoot,
  force: bool,
) -> AnyhowResult<GrokStatsigRefreshOutcome> {
  if find_first_credential_for_service(app_data_root, GenerationSource::GrokCookies).is_none() {
    return Ok(GrokStatsigRefreshOutcome::StillFresh); // Not logged in — nothing to refresh.
  }

  if !force && !stored_statsig_is_stale(app_data_root) {
    return Ok(GrokStatsigRefreshOutcome::StillFresh);
  }

  let guard = match RefreshGuard::acquire() {
    Some(guard) => guard,
    None => {
      info!("Grok statsig refresh already in flight; waiting for it to finish.");
      wait_for_in_flight_refresh().await?;
      if !stored_statsig_is_stale(app_data_root) {
        return Ok(GrokStatsigRefreshOutcome::StillFresh);
      }
      // The other refresh finished without producing fresh material; run our own.
      RefreshGuard::acquire()
          .ok_or_else(|| anyhow!("another Grok statsig refresh started before we could"))?
    }
  };
  let _guard = guard; // held until this function returns (or panics)

  info!("Refreshing Grok statsig (blocking, force={force}) ...");
  build_refresh_window(app)?;
  let captured = poll_and_persist(app, app_data_root).await;
  close_refresh_window(app);

  Ok(if captured {
    GrokStatsigRefreshOutcome::Refreshed
  } else {
    GrokStatsigRefreshOutcome::NothingCaptured
  })
}

/// Whether the stored Grok credential's statsig needs re-capturing. `false`
/// when there is no Grok credential at all.
fn stored_statsig_is_stale(app_data_root: &AppDataRoot) -> bool {
  let Some(credential) = find_first_credential_for_service(app_data_root, GenerationSource::GrokCookies) else {
    return false;
  };
  let grok_data = credential.cookies().and_then(|cookie| cookie.grok_data.as_ref());
  grok_statsig_needs_refresh(grok_data, Utc::now())
}

/// Build the hidden window (in the caller's context) and spawn the poll loop.
/// Ownership of `guard` transfers to the spawned task so the flag stays held
/// until the whole refresh finishes.
fn start_background_refresh(app: &AppHandle, app_data_root: &AppDataRoot, guard: RefreshGuard) -> AnyhowResult<()> {
  build_refresh_window(app)?;

  info!("Grok statsig missing/stale — refreshing in a hidden window.");

  let app = app.clone();
  let app_data_root = app_data_root.clone();
  tauri::async_runtime::spawn(async move {
    let _guard = guard; // released on task completion (or panic)
    poll_and_persist(&app, &app_data_root).await;
    close_refresh_window(&app);
  });

  Ok(())
}

async fn wait_for_in_flight_refresh() -> AnyhowResult<()> {
  let deadline = tokio::time::Instant::now() + WAIT_FOR_IN_FLIGHT_TIMEOUT;
  while RefreshGuard::is_held() {
    if tokio::time::Instant::now() >= deadline {
      return Err(anyhow!("timed out waiting for the in-flight Grok statsig refresh"));
    }
    tokio::time::sleep(WAIT_FOR_IN_FLIGHT_POLL_INTERVAL).await;
  }
  Ok(())
}

fn build_refresh_window(app: &AppHandle) -> AnyhowResult<()> {
  if app.get_window(REFRESH_WINDOW_NAME).is_some() {
    return Err(anyhow!("refresh window already exists"));
  }

  WebviewWindowBuilder::new(app, REFRESH_WINDOW_NAME, WebviewUrl::External(GROK_URL.parse()?))
      .title("Refreshing Grok…")
      .visible(false)
      .initialization_script(&grok_statsig_init_script())
      .build()?;

  Ok(())
}

fn close_refresh_window(app: &AppHandle) {
  if let Some(window) = app.get_webview_window(REFRESH_WINDOW_NAME) {
    let _ = window.close();
  }
}

/// Poll the capture cookie the harness stashes until it appears or we time out,
/// then persist the capture. Returns whether something was captured and saved.
async fn poll_and_persist(app: &AppHandle, app_data_root: &AppDataRoot) -> bool {
  tokio::time::sleep(INITIAL_LOAD_DELAY).await;

  let deadline = tokio::time::Instant::now() + CAPTURE_TIMEOUT;
  loop {
    let Some(window) = app.get_webview_window(REFRESH_WINDOW_NAME) else {
      warn!("Grok statsig refresh: window closed before a capture was observed.");
      return false;
    };

    if let Some(capture) = read_captured_statsig(&window) {
      return match save_refreshed_capture(app_data_root, capture) {
        Ok(()) => {
          info!("Refreshed Grok statsig and saved it to the credential file.");
          true
        }
        Err(err) => {
          warn!("Grok statsig refresh: could not save capture: {err:?}");
          false
        }
      };
    }

    if tokio::time::Instant::now() >= deadline {
      warn!("Grok statsig refresh: no signed request observed within {CAPTURE_TIMEOUT:?}.");
      return false;
    }
    tokio::time::sleep(CAPTURE_POLL_INTERVAL).await;
  }
}

/// Persist a fresh capture onto the existing Grok credential, updating only
/// `grok_data` (cookies untouched).
fn save_refreshed_capture(app_data_root: &AppDataRoot, capture: GrokStatsigCapture) -> AnyhowResult<()> {
  let mut credential = find_first_credential_for_service(app_data_root, GenerationSource::GrokCookies)
      .ok_or_else(|| anyhow!("Grok credential vanished mid-refresh"))?;

  let CredentialSecret::Cookies(cookie) = &mut credential.secret else {
    return Err(anyhow!("Grok credential is not a cookie credential"));
  };
  cookie.grok_data = Some(CookieCredentialGrokExtraPieces::fresh(capture, Utc::now()));

  credential.save()?;
  Ok(())
}
