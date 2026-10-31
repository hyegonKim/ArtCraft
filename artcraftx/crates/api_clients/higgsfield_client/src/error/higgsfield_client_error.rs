use crate::types::ids::{JobId, MediaId};
use crate::types::job_status::JobStatus;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::time::Duration;

/// Errors raised entirely on the client side — before or independent of the
/// request reaching Higgsfield.
#[derive(Debug)]
pub enum HiggsfieldClientError {
  /// The auth has an empty bearer token.
  MissingBearerToken,

  /// The bearer token contains characters that can't go in a header.
  InvalidBearerToken,

  /// A JWT didn't parse or lacks the claims we need (`exp`, `sid`, `sub`).
  InvalidSessionToken(String),

  /// The cookies are empty.
  MissingCookies,

  /// The cookies contain characters that can't go in a header.
  InvalidCookies,

  /// The cookies lack Clerk's `__client` cookie, so no session token can be
  /// minted. Re-capture from a logged-in browser.
  MissingClerkClientCookie,

  /// The request failed our own validation before being sent — e.g. an empty
  /// prompt or a batch size the endpoint doesn't accept.
  InvalidRequest(String),

  /// Building the HTTP client failed.
  WreqClientBuild(wreq::Error),

  /// Building the HTTP request failed (bad URL, bad header value, ...).
  WreqRequestBuild(wreq::Error),

  /// The request body could not be serialized to JSON.
  RequestSerialization(serde_json::Error),

  /// Waiting for a job: the server doesn't know the id.
  JobNotFound(JobId),

  /// Waiting for a job: it ended in a non-success state.
  JobFailed {
    job_id: JobId,
    status: JobStatus,
    /// The server's `meta.fail_reason`, when it gave one.
    maybe_reason: Option<String>,
  },

  /// Waiting for a job: it didn't finish within the timeout.
  JobTimedOut {
    job_id: JobId,
    last_status: JobStatus,
    waited: Duration,
  },

  /// Waiting for an upload's IP check: it didn't finish within the timeout
  /// (or was never requested — confirm with `force_ip_check`).
  MediaIpCheckTimedOut {
    media_id: MediaId,
    waited: Duration,
  },

  /// The upload was flagged as protected content (a recognised public
  /// figure or copyrighted image); the server will refuse it in every
  /// generation request (`404 "Media input not found"`). Nothing to retry
  /// — pick different media.
  MediaProtectedContent {
    media_id: MediaId,
  },

  /// The upload's source URL is on a first-party domain (ArtCraft /
  /// FakeYou); our own CDN assets must not be re-uploaded to Higgsfield.
  /// See `session::upload_source_guard`.
  UploadSourceDomainBlocked {
    source_url: String,
    domain: &'static str,
  },
}

impl Error for HiggsfieldClientError {}

impl Display for HiggsfieldClientError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::MissingBearerToken => write!(f, "No bearer token present."),
      Self::InvalidBearerToken => write!(f, "The bearer token contains invalid characters."),
      Self::InvalidSessionToken(msg) => write!(f, "The session token is not a usable Clerk JWT: {}", msg),
      Self::MissingCookies => write!(f, "No cookies present."),
      Self::InvalidCookies => write!(f, "The cookies contain invalid characters."),
      Self::MissingClerkClientCookie => write!(f, "The cookies lack Clerk's `__client` cookie; log in again to capture it."),
      Self::InvalidRequest(msg) => write!(f, "Invalid request (rejected client-side before sending): {}", msg),
      Self::WreqClientBuild(err) => write!(f, "Failed to build the HTTP client: {}", err),
      Self::WreqRequestBuild(err) => write!(f, "Failed to build the HTTP request: {}", err),
      Self::RequestSerialization(err) => write!(f, "Failed to serialize the request body to JSON: {}", err),
      Self::JobNotFound(job_id) => write!(f, "Job {} is unknown to the server.", job_id),
      Self::JobFailed { job_id, status, maybe_reason } => match maybe_reason {
        Some(reason) => write!(f, "Job {} ended in state {}: {}", job_id, status, reason),
        None => write!(f, "Job {} ended in state {}.", job_id, status),
      },
      Self::JobTimedOut { job_id, last_status, waited } =>
        write!(f, "Job {} did not finish within {}s (last status: {}).", job_id, waited.as_secs(), last_status),
      Self::MediaIpCheckTimedOut { media_id, waited } =>
        write!(f, "Media {} IP check did not finish within {}s (was it requested with force_ip_check?).", media_id, waited.as_secs()),
      Self::MediaProtectedContent { media_id } =>
        write!(f, "Media {} was flagged as protected content (recognised likeness or copyrighted image); Higgsfield will not use it as a reference.", media_id),
      Self::UploadSourceDomainBlocked { source_url, domain } =>
        write!(f, "Refusing to upload media sourced from {} — {} is a first-party domain whose assets must not be sent to Higgsfield.", source_url, domain),
    }
  }
}

impl From<serde_json::Error> for HiggsfieldClientError {
  fn from(error: serde_json::Error) -> Self {
    Self::RequestSerialization(error)
  }
}
