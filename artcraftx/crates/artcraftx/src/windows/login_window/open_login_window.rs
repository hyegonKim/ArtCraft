use crate::credentials::login_website::LoginWebsite;
use crate::windows::login_window::login_journey::NavigationAction;
use crate::windows::login_window::login_window_thread::login_window_thread;
use crate::windows::login_window::logins::login_site_for;
use crate::state::data_dir::app_data_root::AppDataRoot;
use crate::utils::window::clear_all_webview_cookies::clear_all_webview_cookies;
use anyhow::anyhow;
use errors::AnyhowResult;
use reqwest::Url;
use std::time::Duration;
use tauri::{AppHandle, Manager, Runtime, Webview, WebviewUrl, WebviewWindowBuilder};

/// Brief pause between navigations so each page settles (and to dodge
/// Cloudflare interstitials that trip on instant redirects).
const NAVIGATION_DELAY: Duration = Duration::from_millis(100);

/// How long we wait for a page to arrive before injecting a discovery script.
const PAGE_ARRIVAL_TIMEOUT: Duration = Duration::from_secs(15);

/// How often we poll the webview URL while waiting for a page to arrive.
const PAGE_ARRIVAL_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// How many times a discovery script is re-injected. A document swap between
/// injection and page load wipes the script, so we inject repeatedly (the
/// script is idempotent) until the page navigates or attempts run out.
const SCRIPT_INJECTION_ATTEMPTS: u32 = 5;

/// Pause between discovery-script injections.
const SCRIPT_INJECTION_INTERVAL: Duration = Duration::from_secs(1);

/// Open a fresh, cookie-cleared login window for a website and start watching
/// it. The site's [`crate::windows::login_window::login_journey::LoginJourney`]
/// drives the webview to the login screen, then a background thread captures
/// cookies once the user finishes. See [`login_window_thread`].
pub async fn open_login_window(
  app: &AppHandle,
  app_data_root: &AppDataRoot,
  website: LoginWebsite,
  maybe_target_credential_id: Option<String>,
) -> AnyhowResult<()> {
  let window_name = login_window_name(website);
  if app.get_window(&window_name).is_some() {
    return Err(anyhow!("{} login window already open", website));
  }

  let site = login_site_for(website);
  let plan = site.journey().plan();

  // The window itself opens on the journey's first URL, so the first action
  // must be a navigation (a script has no page to run against yet).
  let first_url = match plan.first() {
    Some(NavigationAction::Navigate(url)) => url.clone(),
    Some(NavigationAction::RunScript(_)) => {
      return Err(anyhow!("{} login journey must start with a navigation", website));
    }
    None => return Err(anyhow!("{} login journey is empty", website)),
  };

  let start_url = WebviewUrl::External(first_url);
  let mut builder = WebviewWindowBuilder::new(app, &window_name, start_url)
      .title(site.window_title())
      .center()
      .resizable(true)
      .visible(true)
      .closable(true)
      .min_inner_size(200.0, 800.0)
      .focused(true)
      .devtools(true);

  // Some sites (e.g. Midjourney, whose Google sign-in otherwise forces a
  // passkey step-up that the default WKWebView can't satisfy) need a
  // mainstream desktop User-Agent. It must match the UA the site's HTTP client
  // later uses, since Cloudflare's cf_clearance is UA-bound.
  if let Some(user_agent) = site.user_agent() {
    builder = builder.user_agent(user_agent);
  }

  // An optional init script that runs before page scripts on every load (Grok
  // uses it to install the passive statsig-capture harness).
  if let Some(script) = site.initialization_script() {
    builder = builder.initialization_script(&script);
  }

  let window = builder.build()?;

  let webview = window.get_webview(&window_name)
      .ok_or_else(|| anyhow!("no webview found"))?;

  // Start every login from a clean slate — no stale cookies/state.
  clear_all_webview_cookies(&webview)?;

  run_navigation_actions(&webview, &plan[1..]).await?;

  let app_handle = app.clone();
  let app_data_root = app_data_root.clone();

  let _ = tauri::async_runtime::spawn(async move {
    login_window_thread(app_handle, app_data_root, website, maybe_target_credential_id).await;
  });

  Ok(())
}

/// The unique Tauri window label for a website's login window.
pub fn login_window_name(website: LoginWebsite) -> String {
  format!("{}_login_window", website.to_str())
}

/// Execute the remainder of a journey plan against an open webview.
async fn run_navigation_actions<R: Runtime>(
  webview: &Webview<R>,
  actions: &[NavigationAction],
) -> AnyhowResult<()> {
  // The most recent page we deliberately navigated to; discovery scripts run
  // against this page, so we wait for it to arrive before injecting.
  let mut current_page: Option<Url> = None;

  for action in actions {
    match action {
      NavigationAction::Navigate(url) => {
        tokio::time::sleep(NAVIGATION_DELAY).await;
        webview.navigate(url.clone())?;
        current_page = Some(url.clone());
      }
      NavigationAction::RunScript(script) => {
        let page = current_page.as_ref()
            .ok_or_else(|| anyhow!("script action requires a preceding navigation"))?;
        wait_for_page_arrival(webview, page).await;
        inject_script_until_navigation(webview, script).await?;
      }
    }
  }
  Ok(())
}

/// Wait (bounded) until the webview lands on the host we navigated to, so a
/// discovery script runs against the intended page rather than its
/// predecessor. Falls through on timeout — the script also self-waits.
async fn wait_for_page_arrival<R: Runtime>(webview: &Webview<R>, page: &Url) {
  let deadline = tokio::time::Instant::now() + PAGE_ARRIVAL_TIMEOUT;
  let expected_host = normalized_host(page);

  while tokio::time::Instant::now() < deadline {
    if let Ok(url) = webview.url() {
      if normalized_host(&url) == expected_host {
        return;
      }
    }
    tokio::time::sleep(PAGE_ARRIVAL_POLL_INTERVAL).await;
  }
}

/// Inject an idempotent discovery script, re-injecting a few times in case a
/// document swap wiped it, and stopping early once the page navigates away
/// (i.e. the script did its job or the user moved on).
async fn inject_script_until_navigation<R: Runtime>(
  webview: &Webview<R>,
  script: &str,
) -> AnyhowResult<()> {
  let starting_url = webview.url()?;

  for _ in 0..SCRIPT_INJECTION_ATTEMPTS {
    webview.eval(script)?;
    tokio::time::sleep(SCRIPT_INJECTION_INTERVAL).await;
    if webview.url()? != starting_url {
      return Ok(());
    }
  }

  // Not navigated yet — the last injected script keeps polling in-page, and
  // the monitor thread takes over from here.
  Ok(())
}

/// Hostname with any `www.` prefix stripped, so redirects between the bare
/// and `www.` forms of a site still count as arrival.
fn normalized_host(url: &Url) -> String {
  url.host_str()
      .unwrap_or_default()
      .trim_start_matches("www.")
      .to_string()
}
