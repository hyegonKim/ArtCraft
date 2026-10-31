//! `harvest_statsig` — tool 2.
//!
//! Opens a real Grok WebView, observes the `x-statsig-id` signatures the page
//! emits, and writes them (with every decoded piece) to `statsig.toml`.
//!
//! ```sh
//! cargo run -p grok_consumer_statsig --features webview-harvest --bin harvest_statsig -- [out.toml] [seconds]
//! ```
//!
//! Log in and start a chat in the window to emit a `POST /conversations/new`
//! signature (the one video generation needs); other endpoints are captured
//! automatically as the page loads.

use errors::AnyhowResult;
use grok_consumer_statsig::harvest_via_webview::{harvest_statsigs, HarvestConfig};
use grok_consumer_statsig::statsig_cache_file::StatsigCacheFile;

const DEFAULT_OUTPUT: &str = "statsig.toml";

fn main() -> AnyhowResult<()> {
  env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

  let mut args = std::env::args().skip(1);
  let output_path = args.next().unwrap_or_else(|| DEFAULT_OUTPUT.to_string());
  let capture_secs = args.next().and_then(|s| s.parse().ok()).unwrap_or(90);

  let captures = harvest_statsigs(HarvestConfig {
    capture_secs,
    ..Default::default()
  })?;

  let now = chrono::Utc::now().timestamp();
  let mut file = StatsigCacheFile::new(now);
  for entry in captures {
    file.upsert(entry);
  }
  file.save(&output_path)?;

  println!("wrote {} signature(s) to {output_path}", file.captured.len());
  for entry in &file.captured {
    println!("  {} {}", entry.method, entry.path);
  }
  Ok(())
}
