use browser_emulation::browser_profile::BrowserProfile;

/// The cookie Cloudflare issues once a client passes a challenge. It is
/// bound to the client's User-Agent and IP, and validated against the TLS /
/// HTTP fingerprint, for a site-configured lifetime (30 minutes by default,
/// often extended to hours or days).
pub const CF_CLEARANCE_COOKIE: &str = "cf_clearance";

/// Whether a cookie header carries a clearance at all. Without one, a
/// challenge is a normal first contact rather than a broken replay.
pub fn cookie_header_has_clearance(cookie_header: &str) -> bool {
  cookie_header
      .split(';')
      .filter_map(|pair| pair.trim().split_once('='))
      .any(|(name, value)| name.trim() == CF_CLEARANCE_COOKIE && !value.trim().is_empty())
}

/// The browser profile an HTTP client must use to replay cookies captured
/// by another client (typically a login webview): the exact User-Agent that
/// earned the `cf_clearance`, on the fingerprint of the same browser family.
///
/// When the capturing UA wasn't recorded, fall back to the profile the
/// integration pins on both sides (e.g. `MIDJOURNEY_USER_AGENT`).
pub fn profile_for_captured_user_agent(
  maybe_captured_user_agent: Option<&str>,
  fallback: BrowserProfile,
) -> BrowserProfile {
  match maybe_captured_user_agent.map(str::trim).filter(|ua| !ua.is_empty()) {
    Some(user_agent) => BrowserProfile::matching_user_agent(user_agent),
    None => fallback,
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn detects_clearance_cookie() {
    assert!(cookie_header_has_clearance("a=1; cf_clearance=abc.def-ghi; b=2"));
    assert!(!cookie_header_has_clearance("a=1; b=2"));
    assert!(!cookie_header_has_clearance("cf_clearance="));
  }

  #[test]
  fn captured_ua_wins_over_fallback() {
    let ua = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.6 Safari/605.1.15";
    let profile = profile_for_captured_user_agent(Some(ua), BrowserProfile::Chrome145);
    assert_eq!(profile.maybe_user_agent_override(), Some(ua));

    let profile = profile_for_captured_user_agent(None, BrowserProfile::Chrome145);
    assert_eq!(profile, BrowserProfile::Chrome145);

    let profile = profile_for_captured_user_agent(Some("  "), BrowserProfile::Chrome145);
    assert_eq!(profile, BrowserProfile::Chrome145);
  }
}
