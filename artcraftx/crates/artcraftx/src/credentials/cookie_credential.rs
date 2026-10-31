use crate::credentials::cookie_credential_grok_extra_pieces::CookieCredentialGrokExtraPieces;
use chrono::{DateTime, Utc};
use cookie_store_wrapper::cookie_store::CookieStore;
use serde_derive::{Deserialize, Serialize};

/// A browser-cookie secret for a website integration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CookieCredential {
  /// Last time the cookies were rewritten (e.g. refreshed by the app).
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub updated_at: Option<DateTime<Utc>>,

  /// Last time a request with these cookies failed (e.g. session expired).
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub failed_at: Option<DateTime<Utc>>,

  /// Last time a request with these cookies succeeded.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub succeeded_at: Option<DateTime<Utc>>,

  /// Set when the provider told us this session is dead (signed out,
  /// expired, or revoked) and only a fresh browser login can fix it. While
  /// set, the app makes no API calls with these cookies at all: generation
  /// fails fast with a "log in again" message and background polling skips
  /// the account. Cleared by a successful re-login (or pasting new cookies).
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub relogin_required_since: Option<DateTime<Utc>>,

  /// The User-Agent of the browser that captured these cookies. Bot-protection
  /// cookies (Cloudflare `cf_clearance`, DataDome `datadome`) are bound to
  /// it, so HTTP clients replaying the cookies must present the same string.
  /// Absent for hand-written files and captures from before this was
  /// recorded; clients then fall back to their pinned per-site UA.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub user_agent: Option<String>,

  /// Grok-only extras: the reusable statsig material and its refresh schedule.
  /// NB: a table, so it must stay after all scalar fields (before `cookies`).
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub grok_data: Option<CookieCredentialGrokExtraPieces>,

  /// The stored cookies with their RFC 6265 attributes.
  /// NB: Serialized as an array of tables, so this field must stay LAST for
  /// the TOML credential files to remain valid.
  pub cookies: CookieStore,
}

impl CookieCredential {
  pub fn new(cookies: CookieStore) -> Self {
    Self {
      updated_at: None,
      failed_at: None,
      succeeded_at: None,
      relogin_required_since: None,
      user_agent: None,
      grok_data: None,
      cookies,
    }
  }

  /// Whether the session is known-dead and the user has to log in again
  /// before these cookies are worth using.
  pub fn needs_relogin(&self) -> bool {
    self.relogin_required_since.is_some()
  }

  /// Record that the provider rejected the session. Keeps the earliest
  /// timestamp if already marked. Returns whether this call changed anything.
  pub fn mark_relogin_required(&mut self, now: DateTime<Utc>) -> bool {
    self.failed_at = Some(now);
    if self.relogin_required_since.is_some() {
      return false;
    }
    self.relogin_required_since = Some(now);
    true
  }

  /// The stored cookies rendered as a `Cookie:` request-header string.
  pub fn cookie_header(&self) -> String {
    self.cookies.to_cookie_string()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use reqwest::Url;

  fn credential() -> CookieCredential {
    let origin = Url::parse("https://higgsfield.ai/").unwrap();
    CookieCredential::new(CookieStore::from_cookie_header("__client=abc", &origin))
  }

  #[test]
  fn marking_sets_the_timestamp_once_and_reports_the_transition() {
    let mut cookie = credential();
    assert!(!cookie.needs_relogin());

    let first = Utc::now();
    assert!(cookie.mark_relogin_required(first), "first mark is a transition");
    assert!(cookie.needs_relogin());
    assert_eq!(cookie.relogin_required_since, Some(first));
    assert_eq!(cookie.failed_at, Some(first));

    let later = first + chrono::Duration::minutes(5);
    assert!(!cookie.mark_relogin_required(later), "already marked");
    assert_eq!(cookie.relogin_required_since, Some(first), "keeps the earliest mark");
    assert_eq!(cookie.failed_at, Some(later), "but records the latest failure");
  }

  #[test]
  fn mark_round_trips_through_toml_and_is_absent_when_unset() {
    let mut cookie = credential();
    let unmarked = toml::to_string(&cookie).unwrap();
    assert!(!unmarked.contains("relogin_required_since"));

    cookie.mark_relogin_required(Utc::now());
    let marked = toml::to_string(&cookie).unwrap();
    assert!(marked.contains("relogin_required_since"));
    let back: CookieCredential = toml::from_str(&marked).unwrap();
    assert_eq!(back.relogin_required_since, cookie.relogin_required_since);
  }
}
