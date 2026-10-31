use browser_emulation::browser_profile::BrowserProfile;
use cloudflare_mitigation::clearance::clearance_binding::profile_for_captured_user_agent;

/// The User-Agent to use for BOTH the Higgsfield login webview and every
/// Higgsfield HTTP call when no captured UA is recorded.
///
/// It must be identical in both places: Cloudflare's `cf_clearance` and
/// DataDome's `datadome` cookies (captured by the webview) are bound to the
/// exact User-Agent, and both check it against the TLS/HTTP fingerprint. A
/// mainstream desktop Safari UA on a Safari fingerprint matches the macOS
/// WKWebView that does the capturing.
pub const HIGGSFIELD_USER_AGENT: &str =
  "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.6 Safari/605.1.15";

/// The browser profile for Higgsfield traffic: the captured User-Agent when
/// the credential recorded one, else [`HIGGSFIELD_USER_AGENT`] — always on a
/// fingerprint matching the UA's browser family.
pub fn higgsfield_browser_profile(maybe_captured_user_agent: Option<&str>) -> BrowserProfile {
  profile_for_captured_user_agent(
    maybe_captured_user_agent,
    BrowserProfile::safari_macos_with_user_agent(HIGGSFIELD_USER_AGENT),
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn default_is_safari_with_pinned_ua() {
    let profile = higgsfield_browser_profile(None);
    assert_eq!(profile.maybe_user_agent_override(), Some(HIGGSFIELD_USER_AGENT));
    assert!(profile.label().contains("Safari18"), "{}", profile.label());
  }

  #[test]
  fn captured_ua_is_honored_on_its_own_family() {
    let chrome = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36";
    let profile = higgsfield_browser_profile(Some(chrome));
    assert_eq!(profile.maybe_user_agent_override(), Some(chrome));
    assert!(profile.label().contains("Chrome145"), "{}", profile.label());
  }
}
