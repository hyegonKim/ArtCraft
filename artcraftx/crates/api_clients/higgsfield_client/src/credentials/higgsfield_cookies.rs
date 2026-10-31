use crate::error::higgsfield_client_error::HiggsfieldClientError;

/// Clerk's client cookie: the long-lived credential that mints session JWTs.
/// It's an HttpOnly cookie on the Clerk frontend-API host
/// (`clerk.higgsfield.ai`), which the login webview capture keeps because it
/// keeps every cookie under `higgsfield.ai`.
pub const CLERK_CLIENT_COOKIE: &str = "__client";

/// The current session JWT as a cookie on `higgsfield.ai`. Short-lived, but
/// useful as a seed: even expired it carries the session id.
pub const CLERK_SESSION_COOKIE: &str = "__session";

/// A browser session's cookies for higgsfield.ai (all hosts under it), as
/// one `cookie` header value: `name=value; name2=value2`.
///
/// This is the long-lived credential. The API gateway itself wants a Clerk
/// JWT, which these cookies can mint — see
/// [`HiggsfieldSession`](crate::session::higgsfield_session::HiggsfieldSession).
#[derive(Clone, PartialEq, Eq)]
pub struct HiggsfieldCookies {
  cookie_header: String,
}

impl HiggsfieldCookies {
  /// From a `cookie` header value. Whitespace is trimmed.
  pub fn from_cookie_header(cookie_header: impl Into<String>) -> Self {
    Self { cookie_header: cookie_header.into().trim().to_string() }
  }

  pub fn as_header_value(&self) -> &str {
    &self.cookie_header
  }

  /// The value of one cookie, if present.
  pub fn cookie_value(&self, name: &str) -> Option<&str> {
    self.cookie_header
        .split(';')
        .map(str::trim)
        .filter_map(|pair| pair.split_once('='))
        .find(|(cookie_name, _)| cookie_name.trim() == name)
        .map(|(_, value)| value.trim())
  }

  /// Whether the Clerk client cookie is present — without it no token can
  /// be minted.
  pub fn has_clerk_client_cookie(&self) -> bool {
    self.cookie_value(CLERK_CLIENT_COOKIE).is_some_and(|value| !value.is_empty())
  }

  /// The `__session` JWT, if the capture included one.
  pub fn maybe_session_jwt(&self) -> Option<&str> {
    self.cookie_value(CLERK_SESSION_COOKIE).filter(|value| !value.is_empty())
  }

  pub(crate) fn validate(&self) -> Result<(), HiggsfieldClientError> {
    if self.cookie_header.is_empty() {
      return Err(HiggsfieldClientError::MissingCookies);
    }
    if self.cookie_header.chars().any(|c| c.is_control()) {
      return Err(HiggsfieldClientError::InvalidCookies);
    }
    if !self.has_clerk_client_cookie() {
      return Err(HiggsfieldClientError::MissingClerkClientCookie);
    }
    Ok(())
  }
}

// Debug is redacted so the cookies can't end up in a log line.
impl std::fmt::Debug for HiggsfieldCookies {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let names = self.cookie_header
        .split(';')
        .filter_map(|pair| pair.trim().split_once('='))
        .map(|(name, _)| name.trim())
        .collect::<Vec<_>>();
    f.debug_struct("HiggsfieldCookies")
        .field("cookie_names", &names)
        .field("values", &"<redacted>")
        .finish()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const HEADER: &str = "__client_uat=1788147196; __client=eyJhbGciOi.client.sig; __session=eyJhbGciOi.session.sig; ph_id=abc";

  #[test]
  fn parses_cookie_values() {
    let cookies = HiggsfieldCookies::from_cookie_header(HEADER);
    assert_eq!(cookies.cookie_value("__client"), Some("eyJhbGciOi.client.sig"));
    assert_eq!(cookies.cookie_value("ph_id"), Some("abc"));
    assert_eq!(cookies.cookie_value("missing"), None);
    assert!(cookies.has_clerk_client_cookie());
    assert_eq!(cookies.maybe_session_jwt(), Some("eyJhbGciOi.session.sig"));
    assert!(cookies.validate().is_ok());
  }

  #[test]
  fn missing_client_cookie_is_rejected() {
    let cookies = HiggsfieldCookies::from_cookie_header("__session=abc");
    assert!(!cookies.has_clerk_client_cookie());
    assert!(matches!(cookies.validate(), Err(HiggsfieldClientError::MissingClerkClientCookie)));
  }

  #[test]
  fn empty_is_rejected() {
    assert!(matches!(
      HiggsfieldCookies::from_cookie_header("  ").validate(),
      Err(HiggsfieldClientError::MissingCookies),
    ));
  }

  #[test]
  fn debug_lists_names_only() {
    let debug = format!("{:?}", HiggsfieldCookies::from_cookie_header(HEADER));
    assert!(debug.contains("__client"));
    assert!(!debug.contains("client.sig"));
    assert!(debug.contains("<redacted>"));
  }
}
