//! Live statsig harvester: open a real Grok WebView, inject the observing
//! harness, and collect every `x-statsig-id` the page emits.
//!
//! Compiled only with `--features webview-harvest` (pulls in wry/tao). A native
//! event loop must own its thread, so this drives the loop with `run_return`
//! and returns the captures once the observation window closes — usable from
//! both the `harvest_statsig` binary (main thread) and an `#[ignore]` test
//! (`any_thread`).

use crate::browser_context::DEFAULT_USER_AGENT;
use crate::mint_harness::MINT_HARNESS_SCRIPT;
use crate::statsig_cache_file::{decode_statsig, CapturedStatsig};
use errors::AnyhowResult;
use log::{debug, info, warn};
use serde::Deserialize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::platform::run_return::EventLoopExtRunReturn;
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

const GROK_URL: &str = "https://grok.com/";
const DEFAULT_CAPTURE_SECS: u64 = 90;

/// How the harvester should drive the WebView.
#[derive(Clone, Debug)]
pub struct HarvestConfig {
  /// Where the WebView starts. Defaults to grok.com.
  pub start_url: String,

  /// User-Agent for the WebView. Must match the UA that obtained `cf_clearance`
  /// and the one the HTTP client uses. Defaults to the crate's Firefox UA.
  pub user_agent: Option<String>,

  /// How long to keep observing before returning, in seconds.
  pub capture_secs: u64,

  /// Show the window so a human can log in / start a chat (which is what emits
  /// the `POST /conversations/new` signature).
  pub visible: bool,

  /// Allow building the event loop off the main thread. Required to call this
  /// from a `cargo test` worker; leave `false` for the binary (`main`).
  pub any_thread: bool,
}

impl Default for HarvestConfig {
  fn default() -> Self {
    Self {
      start_url: GROK_URL.to_string(),
      user_agent: Some(DEFAULT_USER_AGENT.to_string()),
      capture_secs: DEFAULT_CAPTURE_SECS,
      visible: true,
      any_thread: false,
    }
  }
}

/// What the injected harness posts back for each observed request.
#[derive(Deserialize)]
struct HarnessReport {
  method: String,
  path: String,
  #[serde(rename = "statsigId")]
  statsig_id: String,
  #[serde(rename = "capturedAt")]
  captured_at_ms: i64,
}

/// Open the WebView, observe for `config.capture_secs`, and return every
/// signature captured (one raw entry per observation; callers dedupe).
pub fn harvest_statsigs(config: HarvestConfig) -> AnyhowResult<Vec<CapturedStatsig>> {
  let captures: Arc<Mutex<Vec<CapturedStatsig>>> = Arc::new(Mutex::new(Vec::new()));

  let mut event_loop = {
    let mut builder = EventLoopBuilder::new();
    // Only Unix/Windows can create an event loop off the main thread; on macOS
    // it must be the main thread, so `any_thread` is a no-op there (use the
    // `harvest_statsig` binary, whose `main` already is the main thread).
    #[cfg(all(unix, not(target_os = "macos")))]
    {
      use tao::platform::unix::EventLoopBuilderExtUnix;
      builder.with_any_thread(config.any_thread);
    }
    #[cfg(target_os = "windows")]
    {
      use tao::platform::windows::EventLoopBuilderExtWindows;
      builder.with_any_thread(config.any_thread);
    }
    #[cfg(target_os = "macos")]
    let _ = config.any_thread;
    builder.build()
  };

  let window = WindowBuilder::new()
      .with_title("Grok statsig harvester")
      .with_visible(config.visible)
      .build(&event_loop)?;

  let sink = captures.clone();
  let ipc_handler = move |request: wry::http::Request<String>| {
    match serde_json::from_str::<HarnessReport>(request.body()) {
      Ok(report) => {
        let captured_at_unix = report.captured_at_ms / 1000;
        match decode_statsig(&report.statsig_id, &report.method, &report.path, captured_at_unix) {
          Ok(entry) => {
            info!("captured statsig: {} {}", entry.method, entry.path);
            sink.lock().expect("harvest sink poisoned").push(entry);
          }
          Err(err) => debug!("ignoring unparseable statsig report: {err}"),
        }
      }
      Err(err) => debug!("ignoring non-report IPC message: {err}"),
    }
  };

  let mut builder = WebViewBuilder::new()
      .with_url(&config.start_url)
      .with_initialization_script(MINT_HARNESS_SCRIPT)
      .with_ipc_handler(ipc_handler);
  if let Some(user_agent) = &config.user_agent {
    builder = builder.with_user_agent(user_agent);
  }
  let _webview = builder.build(&window)?;

  info!("harvesting statsigs from {} for {}s…", config.start_url, config.capture_secs);
  let deadline = Instant::now() + Duration::from_secs(config.capture_secs);

  event_loop.run_return(move |event, _target, control_flow| {
    *control_flow = ControlFlow::WaitUntil(deadline);

    if let Event::WindowEvent { event: WindowEvent::CloseRequested, .. } = event {
      *control_flow = ControlFlow::Exit;
    }
    if Instant::now() >= deadline {
      *control_flow = ControlFlow::Exit;
    }
  });

  let result = captures.lock().expect("harvest sink poisoned").clone();
  if result.is_empty() {
    warn!("no statsigs captured — did the page load and emit any signed requests?");
  }
  Ok(result)
}

#[cfg(test)]
mod tests {
  use super::*;

  // Tool 1 (live): open a real Grok WebView, harvest, and print. Manual: log in
  // and start a chat in the window to emit a POST /conversations/new signature.
  // Run with:
  //   cargo test -p grok_consumer_statsig --features webview-harvest \
  //     harvest_and_print -- --ignored --nocapture
  #[test]
  #[ignore]
  fn harvest_and_print() {
    let captures = harvest_statsigs(HarvestConfig {
      capture_secs: 60,
      any_thread: true,
      ..Default::default()
    }).expect("harvest failed");

    println!("\ncaptured {} statsig(s):", captures.len());
    for entry in &captures {
      println!("\n{} {}", entry.method, entry.path);
      println!("  x-statsig-id : {}", entry.statsig_id);
      println!("  seed (b64)   : {}", entry.seed_b64);
      println!("  signed_at    : {} (unix)", entry.signed_at_unix);
      println!("  digest[..16] : {}", entry.digest_hex);
    }
  }
}
