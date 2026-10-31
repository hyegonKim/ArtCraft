use crate::error::higgsfield_api_error::HiggsfieldApiError;
use crate::error::higgsfield_client_error::HiggsfieldClientError;
use std::error::Error;
use std::fmt::{Display, Formatter};

/// Top-level error for the Higgsfield client: every failure any endpoint can
/// produce, split by where it happened.
#[derive(Debug)]
pub enum HiggsfieldError {
  /// Failed on our side, before or independent of a server response.
  Client(HiggsfieldClientError),

  /// The server (or the network in between) produced the failure.
  Api(HiggsfieldApiError),
}

impl Error for HiggsfieldError {}

impl Display for HiggsfieldError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Client(err) => write!(f, "Higgsfield client error: {}", err),
      Self::Api(err) => write!(f, "Higgsfield API error: {}", err),
    }
  }
}

impl HiggsfieldError {
  /// Whether retrying the same request later is reasonable (transient
  /// network trouble, rate limiting, or a server-side failure).
  pub fn is_retryable(&self) -> bool {
    match self {
      Self::Client(_) => false,
      Self::Api(err) => err.is_retryable(),
    }
  }

  /// The bearer token was rejected (expired or revoked). The session can fix
  /// this itself by minting a new one — see `HiggsfieldSession::with_auth`.
  pub fn is_token_rejected(&self) -> bool {
    matches!(self, Self::Api(HiggsfieldApiError::Unauthorized { .. }))
  }

  /// Bot protection or a dead Clerk session: only a human in a browser can
  /// fix this (a fresh login re-earns the cookies).
  pub fn needs_browser_reauth(&self) -> bool {
    match self {
      Self::Client(_) => false,
      Self::Api(err) => err.needs_browser_reauth(),
    }
  }

  /// A plain-language explanation when Higgsfield refused the request for a
  /// reason the user can act on (rather than a fault to retry or report):
  /// reference media failing the intellectual-property / likeness check, or
  /// that check not finishing. `None` for everything else.
  pub fn user_facing_rejection(&self) -> Option<String> {
    match self {
      Self::Client(HiggsfieldClientError::MediaProtectedContent { .. }) => Some(
        "Higgsfield's intellectual property check rejected one of your reference images or videos: \
         it appears to contain a recognizable person or copyrighted content, so Higgsfield won't use it. \
         Please choose different reference media.".to_string(),
      ),
      Self::Client(HiggsfieldClientError::MediaIpCheckTimedOut { .. }) => Some(
        "Higgsfield's intellectual property check on your reference media didn't finish in time. \
         Please try again in a moment.".to_string(),
      ),
      _ => None,
    }
  }

  /// The server refused a job because a referenced media input is unknown
  /// or unusable (`404 "Media input not found"` and its 400/422 cousins).
  /// Happens when a previously uploaded reference has since been deleted or
  /// flagged: forget the cached upload and upload afresh.
  pub fn is_media_input_rejected(&self) -> bool {
    let text = match self {
      Self::Api(HiggsfieldApiError::NotFound { raw_http_body }) => raw_http_body,
      Self::Api(HiggsfieldApiError::BadRequest { reason, .. }) => reason,
      Self::Api(HiggsfieldApiError::UnprocessableEntity { reason, .. }) => reason,
      _ => return false,
    };
    let text = text.to_ascii_lowercase();
    text.contains("media") && (text.contains("not found") || text.contains("input"))
  }

  /// Whether the session itself is the problem (expired token, dead session,
  /// bot protection, unusable cookies) — some form of re-authentication is
  /// needed before retrying. Union of [`Self::is_token_rejected`],
  /// [`Self::needs_browser_reauth`], and client-side credential problems.
  pub fn is_auth_failure(&self) -> bool {
    if self.is_token_rejected() || self.needs_browser_reauth() {
      return true;
    }
    match self {
      Self::Client(err) => matches!(
        err,
        HiggsfieldClientError::MissingBearerToken
          | HiggsfieldClientError::InvalidBearerToken
          | HiggsfieldClientError::InvalidSessionToken(_)
          | HiggsfieldClientError::MissingCookies
          | HiggsfieldClientError::InvalidCookies
          | HiggsfieldClientError::MissingClerkClientCookie,
      ),
      Self::Api(_) => false,
    }
  }
}

impl From<HiggsfieldClientError> for HiggsfieldError {
  fn from(error: HiggsfieldClientError) -> Self {
    Self::Client(error)
  }
}

impl From<HiggsfieldApiError> for HiggsfieldError {
  fn from(error: HiggsfieldApiError) -> Self {
    Self::Api(error)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::types::ids::MediaId;
  use std::time::Duration;

  #[test]
  fn ip_check_outcomes_have_user_facing_messages() {
    let protected = HiggsfieldError::Client(HiggsfieldClientError::MediaProtectedContent { media_id: MediaId::new("m1") });
    let message = protected.user_facing_rejection().expect("protected content is user-facing");
    assert!(message.contains("intellectual property"));
    assert!(message.contains("different reference media"));

    let timed_out = HiggsfieldError::Client(HiggsfieldClientError::MediaIpCheckTimedOut { media_id: MediaId::new("m1"), waited: Duration::from_secs(30) });
    assert!(timed_out.user_facing_rejection().unwrap().contains("didn't finish in time"));
  }

  #[test]
  fn media_input_rejections_are_recognised_by_status_and_body() {
    let not_found = HiggsfieldError::Api(HiggsfieldApiError::NotFound { raw_http_body: r#"{"detail":"Media input not found"}"#.to_string() });
    assert!(not_found.is_media_input_rejected());
    let unprocessable = HiggsfieldError::Api(HiggsfieldApiError::UnprocessableEntity { reason: "Invalid media input".to_string(), raw_http_body: String::new() });
    assert!(unprocessable.is_media_input_rejected());
    let job_missing = HiggsfieldError::Api(HiggsfieldApiError::NotFound { raw_http_body: r#"{"detail":"Job not found"}"#.to_string() });
    assert!(!job_missing.is_media_input_rejected(), "a missing job is not a media problem");
    let dead = HiggsfieldError::Api(HiggsfieldApiError::NoActiveSession { raw_http_body: String::new() });
    assert!(!dead.is_media_input_rejected());
  }

  #[test]
  fn other_errors_are_not_user_facing_rejections() {
    let dead = HiggsfieldError::Api(HiggsfieldApiError::NoActiveSession { raw_http_body: String::new() });
    assert!(dead.user_facing_rejection().is_none());
  }
}
