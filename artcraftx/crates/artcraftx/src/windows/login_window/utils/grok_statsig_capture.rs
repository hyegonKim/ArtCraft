//! Passive `x-statsig-id` capture during the Grok login.
//!
//! An initialization script installs `grok_consumer_statsig`'s observing
//! harness on the login page from first load, stashing each captured
//! `x-statsig-id` into a cookie — the only Rust-visible channel out of an
//! external webview in the login flow. After login the freshest capture is
//! decoded into storable pieces (seed, key, timestamp, …) via the library.
//!
//! This is deliberately passive: it records whatever signed request the page
//! makes on its own (no generation is triggered), so the stored statsig is a
//! session sample, not necessarily the video endpoint's.

use crate::credentials::cookie_credential_grok_extra_pieces::GrokStatsigCapture;
use chrono::{DateTime, Utc};
use grok_consumer_statsig::statsig_cache_file::decode_statsig;
use grok_consumer_statsig::{StatsigMaterial, MINT_HARNESS_SCRIPT};
use tauri::WebviewWindow;

/// Cookie the capture harness stashes the latest statsig report into. Skipped
/// when persisting login cookies (see `extract_login_window_cookies`).
pub const STATSIG_CAPTURE_COOKIE: &str = "__artcraft_statsig_capture";

/// The Grok login webview's initialization script: define a cookie sink, then
/// install the observing harness. Runs before page scripts on every load, so it
/// catches the page's signed requests from the first one.
///
/// The harness reports `{method, path, statsigId, capturedAt}` JSON to
/// `window.__grokStatsigReport`; the sink flattens it into a cookie-safe
/// `method|path|statsigId|capturedAt` string (base64 statsig ids never contain
/// `|`), which [`read_captured_statsig`] reads back.
pub fn grok_statsig_init_script() -> String {
  // Guard to grok.com only: the harness must never run on the accounts.x.ai
  // sign-in page, where wrapping fetch/XHR could interfere with the login flow.
  format!(
    r#"if (location.hostname.indexOf("grok.com") !== -1) {{
  (function () {{
    window.__grokStatsigReport = function (json) {{
      try {{
        var o = JSON.parse(json);
        var packed = [o.method, o.path, o.statsigId, o.capturedAt].join("|");
        document.cookie = "{cookie}=" + packed + "; path=/; max-age=3600";
      }} catch (e) {{ /* never break the page */ }}
    }};
  }})();
  {harness}
}}"#,
    cookie = STATSIG_CAPTURE_COOKIE,
    harness = MINT_HARNESS_SCRIPT,
  )
}

/// Read the freshest statsig the harness captured: the signed `x-statsig-id`
/// itself (what requests send) plus the reusable [`StatsigMaterial`] (the seed)
/// decoded from it, if any.
///
/// The captured signature yields the seed (decodable) but not the genuine-hex
/// fingerprint (SHA-locked), so the material is the seed alone — see
/// [`StatsigMaterial::generate_statsig`].
pub fn read_captured_statsig(webview: &WebviewWindow) -> Option<GrokStatsigCapture> {
  let packed = webview
      .cookies()
      .ok()?
      .into_iter()
      .find(|cookie| cookie.name() == STATSIG_CAPTURE_COOKIE)
      .map(|cookie| cookie.value().to_string())?;

  parse_packed_statsig(&packed)
}

/// Parse the `method|path|statsigId|capturedAtMs` cookie payload into the
/// signed header and its reusable seed material.
fn parse_packed_statsig(packed: &str) -> Option<GrokStatsigCapture> {
  let mut parts = packed.splitn(4, '|');
  let method = parts.next()?;
  let path = parts.next()?;
  let statsig_id = parts.next()?;
  let captured_at_ms: i64 = parts.next()?.parse().ok()?;

  let captured = decode_statsig(statsig_id, method, path, captured_at_ms / 1000).ok()?;
  let captured_at: DateTime<Utc> = DateTime::from_timestamp_millis(captured_at_ms)?;

  Some(GrokStatsigCapture {
    material: StatsigMaterial::from_seed_b64(captured.seed_b64),
    statsig_id: statsig_id.to_string(),
    method: method.trim().to_ascii_uppercase(),
    path: path.trim().to_string(),
    captured_at,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  // A real browser-captured statsig for POST /conversations/new.
  const CAPTURE: &str =
    "XbHvyYh7hmNc+8sVAGnrpcuHN/MK+mgN63cmnBljOJaoCKu6zTPVw6C7HVv1wculKCNmYVum+L1h0IjFmtyZD/C53M+ZXg";

  #[test]
  fn init_script_installs_sink_and_harness() {
    let script = grok_statsig_init_script();
    // Guarded to grok.com so it never touches the accounts.x.ai sign-in page.
    assert!(script.contains("grok.com"));
    assert!(script.contains("__grokStatsigReport"));
    assert!(script.contains(STATSIG_CAPTURE_COOKIE));
    // The library harness is embedded.
    assert!(script.contains("x-statsig-id"));
  }

  #[test]
  fn parses_a_packed_capture_into_seed_material() {
    let packed = format!("POST|/rest/app-chat/conversations/new|{CAPTURE}|1787535100000");
    let capture = parse_packed_statsig(&packed).expect("should decode");
    // The reusable piece is the decoded seed (48 bytes -> 64 base64 chars).
    assert_eq!(capture.material.seed_b64.len(), 64);
    // The signed header itself is kept verbatim, with the request it signed.
    assert_eq!(capture.statsig_id, CAPTURE);
    assert_eq!(capture.method, "POST");
    assert_eq!(capture.path, "/rest/app-chat/conversations/new");
    assert_eq!(capture.captured_at.timestamp_millis(), 1787535100000);
  }

  #[test]
  fn rejects_malformed_payloads() {
    assert!(parse_packed_statsig("garbage").is_none());
    assert!(parse_packed_statsig("POST|/p|not-base64!!|123").is_none());
  }
}
