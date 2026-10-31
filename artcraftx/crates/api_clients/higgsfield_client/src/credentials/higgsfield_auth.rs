use crate::error::higgsfield_client_error::HiggsfieldClientError;

/// Everything needed to act as a logged-in higgsfield.ai session.
///
/// The gateway authenticates with a Clerk-issued JWT sent as
/// `authorization: Bearer <token>`. These tokens are short-lived (about a
/// minute in captured sessions) — the web app mints a fresh one from the
/// Clerk session cookie before each burst of requests, so callers should
/// expect to refresh `bearer_token` regularly.
///
/// The cookie header and DataDome client id are optional extras captured
/// from the same browser session; sending them makes requests look more like
/// the web app's and can matter for bot detection.
#[derive(Clone)]
pub struct HiggsfieldAuth {
  /// The Clerk session JWT (without the `Bearer ` prefix).
  pub bearer_token: String,

  /// The browser's `cookie` header for higgsfield.ai, if available.
  pub maybe_cookie_header: Option<String>,

  /// The `x-datadome-clientid` header value from the browser session, if
  /// available.
  pub maybe_datadome_client_id: Option<String>,

  /// The User-Agent of the browser that captured the cookies. Bot-protection
  /// cookies (`cf_clearance`, `datadome`) are bound to it, so requests
  /// replaying them present the same string. When `None`, the client's
  /// pinned default is used.
  pub maybe_user_agent: Option<String>,
}

impl HiggsfieldAuth {
  pub fn new(bearer_token: impl Into<String>) -> Self {
    Self {
      bearer_token: bearer_token.into().trim().to_string(),
      maybe_cookie_header: None,
      maybe_datadome_client_id: None,
      maybe_user_agent: None,
    }
  }

  pub fn with_cookie_header(mut self, cookie_header: impl Into<String>) -> Self {
    self.maybe_cookie_header = Some(cookie_header.into().trim().to_string());
    self
  }

  pub fn with_datadome_client_id(mut self, datadome_client_id: impl Into<String>) -> Self {
    self.maybe_datadome_client_id = Some(datadome_client_id.into().trim().to_string());
    self
  }

  pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
    self.maybe_user_agent = Some(user_agent.into().trim().to_string());
    self
  }

  /// Cheap pre-flight checks so an obviously bad session fails before any
  /// HTTP round trip.
  pub(crate) fn validate(&self) -> Result<(), HiggsfieldClientError> {
    if self.bearer_token.is_empty() {
      return Err(HiggsfieldClientError::MissingBearerToken);
    }
    if self.bearer_token.chars().any(|c| c.is_control() || c.is_whitespace()) {
      return Err(HiggsfieldClientError::InvalidBearerToken);
    }
    Ok(())
  }

  pub(crate) fn bearer_header_value(&self) -> String {
    format!("Bearer {}", self.bearer_token)
  }
}

// Debug is redacted so the session can't end up in a log line via an
// accidental `{:?}` on an enclosing args struct.
impl std::fmt::Debug for HiggsfieldAuth {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("HiggsfieldAuth")
        .field("bearer_token", &"<redacted>")
        .field("maybe_cookie_header", &self.maybe_cookie_header.as_ref().map(|_| "<redacted>"))
        .field("maybe_datadome_client_id", &self.maybe_datadome_client_id.as_ref().map(|_| "<redacted>"))
        .field("maybe_user_agent", &self.maybe_user_agent)
        .finish()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn new_trims_token() {
    let auth = HiggsfieldAuth::new("  abc.def.ghi\n");
    assert_eq!(auth.bearer_token, "abc.def.ghi");
    assert_eq!(auth.bearer_header_value(), "Bearer abc.def.ghi");
  }

  #[test]
  fn empty_token_is_rejected() {
    assert!(matches!(
      HiggsfieldAuth::new("   ").validate(),
      Err(HiggsfieldClientError::MissingBearerToken),
    ));
  }

  #[test]
  fn token_with_whitespace_is_rejected() {
    assert!(matches!(
      HiggsfieldAuth::new("abc def").validate(),
      Err(HiggsfieldClientError::InvalidBearerToken),
    ));
  }

  #[test]
  fn debug_is_redacted() {
    let auth = HiggsfieldAuth::new("super-secret-token")
        .with_cookie_header("__session=secret-cookie")
        .with_datadome_client_id("secret-datadome");
    let debug = format!("{:?}", auth);
    assert!(!debug.contains("super-secret-token"));
    assert!(!debug.contains("secret-cookie"));
    assert!(!debug.contains("secret-datadome"));
    assert!(debug.contains("<redacted>"));
  }
}
