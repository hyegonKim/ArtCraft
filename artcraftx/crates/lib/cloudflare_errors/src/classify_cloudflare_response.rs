use crate::cloudflare_error::CloudflareError;
use crate::cloudflare_response_signals::CloudflareResponseSignals;

/// Body markers of Cloudflare's challenge pages (managed challenge, JS
/// challenge, Turnstile interstitial). Any one is conclusive.
const CHALLENGE_BODY_MARKERS: &[&str] = &[
  "challenge-platform",
  "challenge-error-text",
  "cType: 'managed'",
  "Just a moment...",
  "_cf_chl_opt",
  "cf-chl-",
  "cf_chl_",
  "Enable JavaScript and cookies to continue",
  "Checking if the site connection is secure",
  "Verify you are human",
];

/// Body markers of a hard WAF block page.
const ACCESS_DENIED_BODY_MARKERS: &[&str] = &[
  "error code: 1020",
  "Error 1020",
  "Access denied",
  "Sorry, you have been blocked",
  "Attention Required! | Cloudflare",
];

/// Body markers of Cloudflare's rate-limit page.
const RATE_LIMIT_BODY_MARKERS: &[&str] = &[
  "error code: 1015",
  "Error 1015",
  "You are being rate limited",
];

/// Body markers that a page came from Cloudflare at all (used only to avoid
/// mislabeling an origin's own 5xx as an edge error).
const CLOUDFLARE_BODY_MARKERS: &[&str] = &[
  "cloudflare",
  "Cloudflare",
  "cf-error-details",
  "Ray ID",
];

/// Decide whether a response is a Cloudflare edge response and which kind.
///
/// Returns `None` for 2xx, for responses that clearly came from the origin,
/// and for anything we can't attribute to Cloudflare. Header signals
/// ([`CloudflareResponseSignals::maybe_cf_mitigated`] especially) take
/// precedence over body heuristics.
pub fn classify_cloudflare_response(signals: &CloudflareResponseSignals<'_>) -> Option<CloudflareError> {
  let status = signals.status_code;
  let body = signals.body;

  if (200..300).contains(&status) {
    return None;
  }

  // `cf-mitigated: challenge` is Cloudflare telling us outright.
  if signals.maybe_cf_mitigated.is_some_and(|value| value.eq_ignore_ascii_case("challenge")) {
    return Some(CloudflareError::ChallengeInterstitial403);
  }

  if contains_any(body, CHALLENGE_BODY_MARKERS) {
    return Some(CloudflareError::ChallengeInterstitial403);
  }

  if status == 403 && contains_any(body, ACCESS_DENIED_BODY_MARKERS) && looks_like_cloudflare(signals) {
    return Some(CloudflareError::AccessDenied1020);
  }

  if status == 429 && (contains_any(body, RATE_LIMIT_BODY_MARKERS) || signals.headers_say_cloudflare() == Some(true)) {
    return Some(CloudflareError::RateLimited429);
  }

  // Everything below is only attributable to Cloudflare when the response
  // says so — an origin's own 502 is not a Cloudflare error.
  if !looks_like_cloudflare(signals) {
    return None;
  }

  match status {
    301 => Some(CloudflareError::MovedPermanently301), // TODO: Include location header.
    502 => Some(CloudflareError::BadGateway502),
    503 => Some(CloudflareError::ServiceUnavailable503),
    504 => Some(CloudflareError::GatewayTimeout504),
    524 => Some(CloudflareError::TimeoutOccurred524),
    520..=523 | 525 | 526 | 527 | 530 => Some(CloudflareError::OriginError5xx(status)),
    _ => {
      if contains_any(body, &["errorcode_504", "Gateway time-out", "Error code 504"]) {
        Some(CloudflareError::GatewayTimeout504)
      } else {
        None
      }
    }
  }
}

fn looks_like_cloudflare(signals: &CloudflareResponseSignals<'_>) -> bool {
  match signals.headers_say_cloudflare() {
    Some(answer) => answer,
    None => contains_any(signals.body, CLOUDFLARE_BODY_MARKERS),
  }
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
  needles.iter().any(|needle| haystack.contains(needle))
}

#[cfg(test)]
mod tests {
  use super::*;

  const CHALLENGE_PAGE: &str = r#"<!DOCTYPE html><html lang="en-US"><head><title>Just a moment...</title></head><body><div id="challenge-error-text">Enable JavaScript and cookies to continue</div><script>window._cf_chl_opt={cvId: '3'};</script></body></html>"#;
  const BLOCK_PAGE: &str = r#"<html><head><title>Attention Required! | Cloudflare</title></head><body><h1>Sorry, you have been blocked</h1><p>error code: 1020</p><span class="cf-footer-item">Cloudflare Ray ID: 8abc</span></body></html>"#;
  const RATE_LIMIT_PAGE: &str = r#"<html><body><h1>Error 1015</h1><p>You are being rate limited</p><p>Cloudflare Ray ID: 8abc</p></body></html>"#;
  const ORIGIN_502_PAGE: &str = r#"<html><head><title>502: Bad gateway</title></head><body><div id="cf-error-details"><h1>Bad gateway</h1><p>Error code 502</p><p>Cloudflare Ray ID: 8abc</p></div></body></html>"#;

  fn classify(status: u16, body: &str) -> Option<CloudflareError> {
    classify_cloudflare_response(&CloudflareResponseSignals::new(status, body))
  }

  #[test]
  fn success_is_never_cloudflare() {
    assert_eq!(classify(200, CHALLENGE_PAGE), None);
  }

  #[test]
  fn challenge_page_by_body() {
    assert_eq!(classify(403, CHALLENGE_PAGE), Some(CloudflareError::ChallengeInterstitial403));
    // Under-attack mode serves the challenge as a 503.
    assert_eq!(classify(503, CHALLENGE_PAGE), Some(CloudflareError::ChallengeInterstitial403));
  }

  #[test]
  fn challenge_by_cf_mitigated_header_even_with_opaque_body() {
    let signals = CloudflareResponseSignals::new(403, "").with_cf_mitigated(Some("challenge"));
    assert_eq!(classify_cloudflare_response(&signals), Some(CloudflareError::ChallengeInterstitial403));
  }

  #[test]
  fn access_denied_1020() {
    assert_eq!(classify(403, BLOCK_PAGE), Some(CloudflareError::AccessDenied1020));
  }

  #[test]
  fn rate_limited_1015() {
    assert_eq!(classify(429, RATE_LIMIT_PAGE), Some(CloudflareError::RateLimited429));
    // A 429 with a plain body counts when headers prove the edge answered.
    let signals = CloudflareResponseSignals::new(429, "slow down").with_server_header(Some("cloudflare"));
    assert_eq!(classify_cloudflare_response(&signals), Some(CloudflareError::RateLimited429));
  }

  #[test]
  fn origin_failures_need_cloudflare_attribution() {
    assert_eq!(classify(502, ORIGIN_502_PAGE), Some(CloudflareError::BadGateway502));
    // An origin's own 502 (no Cloudflare markers) is not ours to label.
    assert_eq!(classify(502, r#"{"error":"upstream failed"}"#), None);
    // ...unless the headers say the edge produced it.
    let signals = CloudflareResponseSignals::new(504, "").with_cf_ray(Some("8abc-SJC")).with_server_header(Some("cloudflare"));
    assert_eq!(classify_cloudflare_response(&signals), Some(CloudflareError::GatewayTimeout504));
  }

  #[test]
  fn fifty_two_x_family() {
    assert_eq!(classify(524, "cloudflare"), Some(CloudflareError::TimeoutOccurred524));
    assert_eq!(classify(522, "cloudflare"), Some(CloudflareError::OriginError5xx(522)));
    assert_eq!(classify(503, "cloudflare"), Some(CloudflareError::ServiceUnavailable503));
  }

  #[test]
  fn an_api_403_from_the_origin_is_not_cloudflare() {
    // A JSON 403 from the app itself, even with a cf-ray (it passed through).
    let signals = CloudflareResponseSignals::new(403, r#"{"detail":"Forbidden"}"#).with_cf_ray(Some("8abc-SJC"));
    assert_eq!(classify_cloudflare_response(&signals), None);
  }

  #[test]
  fn semantics() {
    assert!(CloudflareError::ChallengeInterstitial403.is_access_denied());
    assert!(CloudflareError::ChallengeInterstitial403.is_challenge());
    assert!(!CloudflareError::ChallengeInterstitial403.is_retryable());
    assert!(CloudflareError::RateLimited429.is_access_denied());
    assert!(CloudflareError::RateLimited429.is_retryable());
    assert!(CloudflareError::BadGateway502.is_origin_failure());
    assert!(CloudflareError::BadGateway502.is_retryable());
    assert_eq!(CloudflareError::ChallengeInterstitial403.log_level(), log::Level::Warn);
    assert_eq!(CloudflareError::BadGateway502.log_level(), log::Level::Info);
  }
}
