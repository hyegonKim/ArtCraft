//! POST `/fnf/media/{id}/upload` — tell the gateway a presigned slot has its
//! bytes. Until this is called the media doesn't exist as far as generation
//! requests are concerned.

use crate::client::higgsfield_host::HiggsfieldHost;
use crate::client::send_request::{send_json_request, HttpMethod};
use crate::credentials::higgsfield_auth::HiggsfieldAuth;
use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::types::ids::{JobId, MediaId};
use crate::types::string_enum::string_enum;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub struct ConfirmMediaUploadArgs<'a> {
  pub request: ConfirmMediaUploadRequest,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

#[derive(Clone, Debug)]
pub struct ConfirmMediaUploadRequest {
  pub media_id: MediaId,

  /// The original file name, shown in the web app's media library.
  pub filename: String,

  /// The web app always sends `true`.
  pub force_nsfw_check: bool,

  /// The web app sends `false`.
  pub force_ip_check: bool,

  /// The web app passes a job id when the "upload" is really a previous
  /// generation being registered as reference media. Omitted otherwise.
  pub maybe_job_id: Option<JobId>,
}

impl ConfirmMediaUploadRequest {
  /// The web app's defaults (NSFW check on, IP check off).
  pub fn new(media_id: MediaId, filename: impl Into<String>) -> Self {
    Self { media_id, filename: filename.into(), force_nsfw_check: true, force_ip_check: false, maybe_job_id: None }
  }

  fn validate(&self) -> Result<(), HiggsfieldClientError> {
    if self.media_id.as_str().trim().is_empty() {
      return Err(HiggsfieldClientError::InvalidRequest("media_id is empty".to_string()));
    }
    if self.filename.trim().is_empty() {
      return Err(HiggsfieldClientError::InvalidRequest("filename is empty".to_string()));
    }
    Ok(())
  }

  fn path(&self) -> String {
    format!("/fnf/media/{}/upload", self.media_id)
  }

  fn to_body(&self) -> ConfirmMediaUploadBody<'_> {
    ConfirmMediaUploadBody {
      maybe_job_id: self.maybe_job_id.as_ref(),
      filename: &self.filename,
      force_nsfw_check: self.force_nsfw_check,
      force_ip_check: self.force_ip_check,
    }
  }
}

string_enum! {
  /// Where the media is in its lifecycle.
  MediaUploadStatus {
    /// Usable as a reference.
    Uploaded => "uploaded",

    /// The IP / likeness check flagged the file as protected content (a
    /// recognised public figure or copyrighted still — observed live on
    /// photos of Jim Varney as Ernest). The media stays listed but every
    /// generation that references it answers `404 "Media input not
    /// found"`; the web app shows "Protected content is not allowed".
    IpDetected => "ip_detected",

    Pending => "pending",
    Failed => "failed",
  }
}

impl MediaUploadStatus {
  /// The server will refuse this media in generation requests.
  pub fn is_blocked(&self) -> bool {
    matches!(self, Self::IpDetected | Self::Failed)
  }
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConfirmMediaUploadResponse {
  pub id: MediaId,

  pub status: MediaUploadStatus,

  /// `null` until the IP check has run (it runs asynchronously, and only
  /// when requested).
  #[serde(default)]
  pub ip_check_finished: Option<bool>,

  #[serde(flatten)]
  pub extra: serde_json::Map<String, Value>,
}

impl ConfirmMediaUploadResponse {
  pub fn is_uploaded(&self) -> bool {
    self.status == MediaUploadStatus::Uploaded
  }

  /// Flagged as protected content; see [`MediaUploadStatus::IpDetected`].
  pub fn is_ip_detected(&self) -> bool {
    self.status == MediaUploadStatus::IpDetected
  }
}

/// Confirm the upload. On success the file can be referenced from
/// generation requests immediately.
pub async fn confirm_media_upload(args: ConfirmMediaUploadArgs<'_>) -> Result<ConfirmMediaUploadResponse, HiggsfieldError> {
  args.request.validate()?;
  let body = args.request.to_body();
  send_json_request(HttpMethod::Post, &args.request.path(), args.auth, args.host, Some(&body)).await
}

// ── Wire format ──

#[derive(Serialize)]
struct ConfirmMediaUploadBody<'a> {
  #[serde(rename = "job_id", skip_serializing_if = "Option::is_none")]
  maybe_job_id: Option<&'a JobId>,
  filename: &'a str,
  force_nsfw_check: bool,
  force_ip_check: bool,
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  #[test]
  fn wire_body_and_path_match_captured_request() {
    let request = ConfirmMediaUploadRequest::new(MediaId::new("00000000-0000-4000-8000-0000000000aa"), "shiba_ref.png");
    assert_eq!(request.path(), "/fnf/media/00000000-0000-4000-8000-0000000000aa/upload");
    let actual: Value = serde_json::to_value(request.to_body()).unwrap();
    assert_eq!(actual, json!({"filename": "shiba_ref.png", "force_nsfw_check": true, "force_ip_check": false}));
  }

  #[test]
  fn validation() {
    assert!(matches!(ConfirmMediaUploadRequest::new(MediaId::new(""), "a.png").validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
    assert!(matches!(ConfirmMediaUploadRequest::new(MediaId::new("m"), " ").validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[test]
  fn response_parses() {
    let response: ConfirmMediaUploadResponse = serde_json::from_str(r#"{"id":"00000000-0000-4000-8000-0000000000aa","status":"uploaded","ip_check_finished":null}"#).unwrap();
    assert!(response.is_uploaded());
    assert_eq!(response.ip_check_finished, None);

    // Live 2026-08-31: a photo of a public figure, confirmed with force_ip_check.
    let flagged: ConfirmMediaUploadResponse = serde_json::from_str(r#"{"id":"00000000-0000-4000-8000-0000000000ab","ip_check_finished":true,"status":"ip_detected"}"#).unwrap();
    assert!(flagged.is_ip_detected());
    assert!(flagged.status.is_blocked());
    assert!(!flagged.is_uploaded());

    let unknown: ConfirmMediaUploadResponse = serde_json::from_str(r#"{"id":"x","status":"quarantined","ip_check_finished":true,"note":"n"}"#).unwrap();
    assert_eq!(unknown.status, MediaUploadStatus::Other("quarantined".to_string()));
    assert!(unknown.extra.contains_key("note"));
  }
}
