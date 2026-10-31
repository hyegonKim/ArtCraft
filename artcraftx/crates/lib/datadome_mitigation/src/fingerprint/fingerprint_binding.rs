use browser_emulation::browser_profile::BrowserProfile;

/// The browser profile an HTTP client must use to replay a DataDome-profiled
/// session: the exact User-Agent that earned the `datadome` cookie, on the
/// fingerprint of the same browser family. DataDome scores the UA, TLS, and
/// HTTP/2 fingerprints together, so a cookie earned by WebKit and replayed
/// by a Chrome fingerprint is exactly the mismatch that gets challenged.
///
/// When the capturing UA wasn't recorded, fall back to the profile the
/// integration pins on both sides.
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
  fn captured_ua_wins_over_fallback() {
    let ua = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.6 Safari/605.1.15";
    assert_eq!(profile_for_captured_user_agent(Some(ua), BrowserProfile::Chrome145).maybe_user_agent_override(), Some(ua));
    assert_eq!(profile_for_captured_user_agent(None, BrowserProfile::Chrome145), BrowserProfile::Chrome145);
  }
}
