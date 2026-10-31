use log::Level;
use std::error::Error;
use std::fmt::{Display, Formatter};

/// A response that came from Cloudflare's edge instead of the origin.
///
/// Two families, which need opposite reactions:
/// - **Access decisions** (challenge, block, rate limit): the *request* is
///   the problem — retrying the same thing just repeats the answer. See
///   [`Self::is_access_denied`].
/// - **Origin failures** (502/504/52x): the *site* is the problem — wait
///   and retry. See [`Self::is_origin_failure`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudflareError {
  /// 301 from Cloudflare (e.g. an always-HTTPS or www redirect the client
  /// didn't follow).
  /// TODO: This needs to include "location" and "set-cookie" headers.
  MovedPermanently301,

  /// A bot-management challenge (managed challenge / JS challenge / Turnstile
  /// interstitial — "Just a moment..."). Only a real browser can pass it; the
  /// fix is a fresh `cf_clearance` from a browser presenting the SAME
  /// User-Agent and IP the replaying client uses.
  ChallengeInterstitial403,

  /// A hard WAF / firewall block (error 1020 "Access denied" and friends).
  /// The site's rules rejected this client outright; a challenge won't be
  /// offered.
  AccessDenied1020,

  /// Cloudflare's rate limiting (429 from the edge, error 1015).
  RateLimited429,

  /// Cloudflare returned a 502 Bad Gateway response.
  BadGateway502,

  /// 503 from the edge: origin overloaded, or "I'm Under Attack" mode.
  ServiceUnavailable503,

  /// Cloudflare could not form a connection to the backend server.
  GatewayTimeout504,

  /// 52x origin errors other than the ones above (520 unknown, 521 down,
  /// 522 connection timed out, 523 unreachable, 525/526 TLS).
  OriginError5xx(u16),

  /// Cloudflare formed a TCP connection to the backend server, but no payload was delivered before timeout
  TimeoutOccurred524,
}

impl Error for CloudflareError {}

impl Display for CloudflareError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::MovedPermanently301 => {
        write!(f, "Cloudflare Moved Permanently (301)")
      }
      Self::ChallengeInterstitial403 => {
        write!(f, "Cloudflare Challenge Interstitial (403); Cloudflare wants to verify the request with a CAPTCHA challenge.")
      }
      Self::AccessDenied1020 => {
        write!(f, "Cloudflare Access Denied (error 1020); the site's firewall rules blocked this client.")
      }
      Self::RateLimited429 => {
        write!(f, "Cloudflare Rate Limited (429 / error 1015); slow down before retrying.")
      }
      Self::BadGateway502 => {
        write!(f, "Cloudflare Bad Gateway (502); This is likely a backend server issue.")
      }
      Self::ServiceUnavailable503 => {
        write!(f, "Cloudflare Service Unavailable (503); the origin is overloaded or in Under Attack mode.")
      }
      Self::GatewayTimeout504 => {
        write!(f, "Cloudflare Gateway Timeout (504); This is likely a backend server issue.")
      }
      Self::OriginError5xx(status) => {
        write!(f, "Cloudflare origin error ({}); Cloudflare could not get a usable response from the backend server.", status)
      }
      Self::TimeoutOccurred524 => {
        write!(f, "Cloudflare Timeout (524); This is likely a backend server issue. Cloudflare connected, but did not receive a response from the server in time.")
      }
    }
  }
}

impl CloudflareError {
  /// Cloudflare refused this client (challenge, block, rate limit). Retrying
  /// unchanged won't help; see `cloudflare_mitigation` for what will.
  pub fn is_access_denied(&self) -> bool {
    matches!(
      self,
      Self::ChallengeInterstitial403 | Self::AccessDenied1020 | Self::RateLimited429,
    )
  }

  /// A bot-management challenge specifically — passable only by a browser.
  pub fn is_challenge(&self) -> bool {
    matches!(self, Self::ChallengeInterstitial403)
  }

  /// The origin behind Cloudflare is failing; the request itself was fine.
  pub fn is_origin_failure(&self) -> bool {
    matches!(
      self,
      Self::BadGateway502
        | Self::ServiceUnavailable503
        | Self::GatewayTimeout504
        | Self::OriginError5xx(_)
        | Self::TimeoutOccurred524,
    )
  }

  /// Whether waiting and retrying the same request is reasonable.
  pub fn is_retryable(&self) -> bool {
    self.is_origin_failure() || matches!(self, Self::RateLimited429)
  }

  /// The level a client should log this at. Origin failures are the site's
  /// problem (info — expected noise); challenges and blocks mean our
  /// fingerprint or session is off and need a human eventually (warn);
  /// redirects we didn't expect are a client bug (error).
  pub fn log_level(&self) -> Level {
    match self {
      Self::ChallengeInterstitial403 | Self::AccessDenied1020 | Self::RateLimited429 => Level::Warn,
      Self::MovedPermanently301 => Level::Error,
      _ => Level::Info,
    }
  }

  /// Log at [`Self::log_level`] with a caller-supplied context (which host,
  /// which endpoint). One place to keep the wording consistent.
  pub fn log(&self, context: &str) {
    log::log!(self.log_level(), "[cloudflare] {}: {}", context, self);
  }
}
