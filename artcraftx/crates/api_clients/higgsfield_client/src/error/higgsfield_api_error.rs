use cloudflare_errors::cloudflare_error::CloudflareError;
use datadome_errors::datadome_error::DataDomeError;
use std::error::Error;
use std::fmt::{Display, Formatter};

/// Errors that came back from Higgsfield's gateway, or from the network
/// while talking to it.
///
/// Every server-response variant carries the raw HTTP body so logs and
/// alerts keep the upstream context, not just our classification.
#[derive(Debug)]
pub enum HiggsfieldApiError {
  /// 400 — the request body was rejected (unknown field, bad value, ...).
  BadRequest {
    /// The message parsed from the body, or the raw body if unparseable.
    reason: String,
    raw_http_body: String,
  },

  /// 401 — the bearer token is missing, expired, or revoked. Clerk tokens
  /// only live about a minute; mint a new one and retry.
  Unauthorized {
    raw_http_body: String,
  },

  /// 402, or a 4xx whose body says the wallet can't cover the job.
  InsufficientCredits {
    reason: String,
    raw_http_body: String,
  },

  /// 403 — the session is valid but this action isn't allowed (blocked or
  /// suspended account, feature not on the plan, ...).
  Forbidden {
    reason: String,
    raw_http_body: String,
  },

  /// Cloudflare's edge answered instead of the API: a bot challenge, a WAF
  /// block, a rate limit, or an origin failure. See
  /// [`CloudflareError`] for which, and `cloudflare_mitigation` for what to
  /// do about it.
  Cloudflare(CloudflareError),

  /// DataDome's bot protection answered instead of the API: a CAPTCHA,
  /// an interstitial, or a hard block. The session (cookies,
  /// `x-datadome-clientid`, User-Agent) needs refreshing from a real
  /// browser. See `datadome_mitigation`.
  DataDome(DataDomeError),

  /// 404 — e.g. an unknown job id.
  NotFound {
    raw_http_body: String,
  },

  /// Clerk reports no active session for these cookies: the user signed
  /// out, or the session expired. Log in again to capture new cookies.
  NoActiveSession {
    raw_http_body: String,
  },

  /// 422 — the body was well-formed JSON but failed validation.
  UnprocessableEntity {
    reason: String,
    raw_http_body: String,
  },

  /// 429 — slow down.
  RateLimited {
    raw_http_body: String,
  },

  /// The prompt or input was rejected by content moderation.
  ContentModerated {
    reason: String,
    raw_http_body: String,
  },

  /// 5xx.
  ServerError {
    status_code: u16,
    raw_http_body: String,
  },

  /// Any other non-2xx status.
  UnknownHttpFailure {
    status_code: u16,
    raw_http_body: String,
  },

  /// The response body didn't match the expected schema. Carries the raw
  /// body so the drift is visible in logs.
  Deserialization {
    source: serde_json::Error,
    raw_http_body: String,
  },

  /// The request timed out.
  Timeout(String),

  /// A network-level failure (DNS, connect, reset, TLS, ...).
  Network(String),
}

impl Error for HiggsfieldApiError {}

impl Display for HiggsfieldApiError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::BadRequest { reason, raw_http_body } =>
        write!(f, "Bad request: {}{}", reason, fmt_raw_body(raw_http_body)),
      Self::Unauthorized { raw_http_body } =>
        write!(f, "Unauthorized (bearer token missing, expired, or revoked){}", fmt_raw_body(raw_http_body)),
      Self::InsufficientCredits { reason, raw_http_body } =>
        write!(f, "Insufficient credits: {}{}", reason, fmt_raw_body(raw_http_body)),
      Self::Forbidden { reason, raw_http_body } =>
        write!(f, "Forbidden: {}{}", reason, fmt_raw_body(raw_http_body)),
      Self::Cloudflare(err) => write!(f, "{}", err),
      Self::DataDome(err) => write!(f, "{}", err),
      Self::NotFound { raw_http_body } =>
        write!(f, "Not found{}", fmt_raw_body(raw_http_body)),
      Self::NoActiveSession { raw_http_body } =>
        write!(f, "No active Clerk session (signed out or expired; log in again){}", fmt_raw_body(raw_http_body)),
      Self::UnprocessableEntity { reason, raw_http_body } =>
        write!(f, "Unprocessable entity: {}{}", reason, fmt_raw_body(raw_http_body)),
      Self::RateLimited { raw_http_body } =>
        write!(f, "Rate limited{}", fmt_raw_body(raw_http_body)),
      Self::ContentModerated { reason, raw_http_body } =>
        write!(f, "Content moderated: {}{}", reason, fmt_raw_body(raw_http_body)),
      Self::ServerError { status_code, raw_http_body } =>
        write!(f, "Server error (status {}){}", status_code, fmt_raw_body(raw_http_body)),
      Self::UnknownHttpFailure { status_code, raw_http_body } =>
        write!(f, "Unknown HTTP failure (status {}){}", status_code, fmt_raw_body(raw_http_body)),
      Self::Deserialization { source, raw_http_body } =>
        write!(f, "Deserialization error: {} | raw response body ({} bytes): {}", source, raw_http_body.len(), raw_http_body),
      Self::Timeout(msg) => write!(f, "Timeout: {}", msg),
      Self::Network(msg) => write!(f, "Network error: {}", msg),
    }
  }
}

impl HiggsfieldApiError {
  /// Whether retrying the same request later is reasonable.
  pub fn is_retryable(&self) -> bool {
    match self {
      Self::RateLimited { .. } | Self::ServerError { .. } | Self::Timeout(_) | Self::Network(_) => true,
      Self::Cloudflare(err) => err.is_retryable(),
      _ => false,
    }
  }

  /// Bot protection (Cloudflare challenge / DataDome) refused this client:
  /// the only fix is a fresh browser login that re-earns the protection
  /// cookies. Distinct from an expired bearer token, which the session
  /// re-mints on its own.
  pub fn needs_browser_reauth(&self) -> bool {
    match self {
      Self::Cloudflare(err) => err.is_challenge(),
      Self::DataDome(err) => err.is_challenge() || matches!(err, DataDomeError::Unclassified { .. }),
      Self::NoActiveSession { .. } => true,
      _ => false,
    }
  }

  /// Build a [`HiggsfieldApiError::Deserialization`] that captures the raw
  /// body, and log the whole payload immediately so it's never lost even if
  /// the error is later swallowed.
  pub fn deserialization(source: serde_json::Error, raw_http_body: &str) -> Self {
    log::warn!(
      "Higgsfield response failed to deserialize: {} | raw body ({} bytes): {}",
      source,
      raw_http_body.len(),
      raw_http_body,
    );
    Self::Deserialization {
      source,
      raw_http_body: raw_http_body.to_string(),
    }
  }

  /// Map a transport-level `wreq` failure (sending, or reading the body) to
  /// the timeout / network split.
  pub(crate) fn from_transport_error(error: wreq::Error) -> Self {
    if error.is_timeout() {
      Self::Timeout(error.to_string())
    } else {
      Self::Network(error.to_string())
    }
  }
}

/// Append the raw HTTP body to a `Display` message when one is present.
fn fmt_raw_body(raw_http_body: &str) -> String {
  if raw_http_body.is_empty() {
    String::new()
  } else {
    format!(" [raw http body: {}]", raw_http_body)
  }
}
