//! Test-only loader for the browser-captured Grok secrets kept off to the
//! side in the (gitignored) `external/credentials/grok` directory.
//!
//! Live-wire tests load these to talk to the real website. The files hold a
//! single real session's cookies and header secrets; regenerate them when
//! they expire.

use crate::credentials::grok_cookies::GrokCookies;
use crate::credentials::grok_request_headers::GrokRequestHeaders;
use errors::AnyhowResult;
use serde::Deserialize;
use std::fs::read_to_string;

/// Repo-root-relative directory holding the captured secrets. Resolved from
/// this crate's manifest dir (`crates/api_clients/grok_consumer_client`), so
/// tests work regardless of the shell's working directory.
const SECRETS_DIR: &str = concat!(
  env!("CARGO_MANIFEST_DIR"),
  "/../../../external/credentials/grok",
);

/// A loaded browser session: cookies plus the captured request headers.
pub struct GrokTestSecrets {
  pub cookies: GrokCookies,
  pub headers: GrokRequestHeaders,
}

/// TOML shape of `external/credentials/grok/headers.toml`.
#[derive(Deserialize)]
struct HeadersToml {
  statsig_id: Option<String>,
  xai_request_id: Option<String>,
  traceparent: Option<String>,
  sentry_trace: Option<String>,
}

/// Load the captured cookies and header secrets.
///
/// Errors (with a pointer to the directory) if the files are missing — that
/// means the secrets haven't been captured yet, which is expected on a fresh
/// checkout since they're gitignored.
pub fn load_grok_test_secrets() -> AnyhowResult<GrokTestSecrets> {
  let cookies_path = format!("{SECRETS_DIR}/cookies.txt");
  let headers_path = format!("{SECRETS_DIR}/headers.toml");

  let cookies_raw = read_to_string(&cookies_path).map_err(|err| {
    anyhow::anyhow!("Could not read Grok test cookies at {cookies_path}: {err}")
  })?;
  let cookies = GrokCookies::new(cookies_raw.trim().to_string());

  let headers_raw = read_to_string(&headers_path).map_err(|err| {
    anyhow::anyhow!("Could not read Grok test headers at {headers_path}: {err}")
  })?;
  let parsed: HeadersToml = toml::from_str(&headers_raw).map_err(|err| {
    anyhow::anyhow!("Could not parse Grok test headers at {headers_path}: {err}")
  })?;

  let headers = GrokRequestHeaders {
    statsig_id: parsed.statsig_id,
    xai_request_id: parsed.xai_request_id,
    traceparent: parsed.traceparent,
    sentry_trace: parsed.sentry_trace,
  };

  Ok(GrokTestSecrets { cookies, headers })
}
