//! GET `/fnf/media/{id}` (images) / `GET /fnf/video/{id}` (clips) — an
//! uploaded file's processing state: whether the IP (intellectual
//! property) check has finished and, for images, whether a face was
//! detected. Poll this before handing media to models that insist on the
//! IP check (Seedance 2.x). Audio has no such record (`/fnf/audio/{id}`
//! is not a status endpoint) and no IP check.

use crate::client::higgsfield_host::HiggsfieldHost;
use crate::client::send_request::{send_json_request, HttpMethod};
use crate::credentials::higgsfield_auth::HiggsfieldAuth;
use crate::endpoints::media::confirm_media_upload::MediaUploadStatus;
use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::types::ids::MediaId;
use serde::Deserialize;
use serde_json::Value;

pub struct GetMediaStatusArgs<'a> {
  pub request: GetMediaStatusRequest,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

/// Uploads are filed by family, each with its own status path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaStatusFamily {
  /// `/fnf/media/{id}`
  Image,
  /// `/fnf/video/{id}`
  Video,
}

#[derive(Clone, Debug)]
pub struct GetMediaStatusRequest {
  pub family: MediaStatusFamily,
  pub media_id: MediaId,
}

impl GetMediaStatusRequest {
  pub fn new(family: MediaStatusFamily, media_id: MediaId) -> Self {
    Self { family, media_id }
  }

  pub fn image(media_id: MediaId) -> Self {
    Self::new(MediaStatusFamily::Image, media_id)
  }

  pub fn video(media_id: MediaId) -> Self {
    Self::new(MediaStatusFamily::Video, media_id)
  }

  fn validate(&self) -> Result<(), HiggsfieldClientError> {
    if self.media_id.as_str().trim().is_empty() {
      return Err(HiggsfieldClientError::InvalidRequest("media_id is empty".to_string()));
    }
    Ok(())
  }

  fn path(&self) -> String {
    match self.family {
      MediaStatusFamily::Image => format!("/fnf/media/{}", self.media_id),
      MediaStatusFamily::Video => format!("/fnf/video/{}", self.media_id),
    }
  }
}

#[derive(Clone, Debug, Deserialize)]
pub struct GetMediaStatusResponse {
  pub id: MediaId,

  pub status: MediaUploadStatus,

  /// `true` once the IP check has run; `null`/`false` while pending (or
  /// never requested).
  #[serde(default)]
  pub ip_check_finished: Option<bool>,

  #[serde(default)]
  pub ip_detected: Option<bool>,

  #[serde(default)]
  pub is_face_detected: Option<bool>,

  #[serde(flatten)]
  pub extra: serde_json::Map<String, Value>,
}

impl GetMediaStatusResponse {
  pub fn is_ip_check_finished(&self) -> bool {
    self.ip_check_finished == Some(true)
  }

  /// Flagged as protected content (see `MediaUploadStatus::IpDetected`).
  pub fn is_ip_detected(&self) -> bool {
    self.status == MediaUploadStatus::IpDetected
  }
}

pub async fn get_media_status(args: GetMediaStatusArgs<'_>) -> Result<GetMediaStatusResponse, HiggsfieldError> {
  args.request.validate()?;
  send_json_request(HttpMethod::Get, &args.request.path(), args.auth, args.host, None::<&()>).await
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn path_and_validation() {
    let id = MediaId::new("00000000-0000-4000-8000-0000000000aa");
    assert_eq!(GetMediaStatusRequest::image(id.clone()).path(), "/fnf/media/00000000-0000-4000-8000-0000000000aa");
    assert_eq!(GetMediaStatusRequest::video(id).path(), "/fnf/video/00000000-0000-4000-8000-0000000000aa");
    assert!(matches!(GetMediaStatusRequest::image(MediaId::new("")).validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[test]
  fn video_status_parses() {
    // Live 2026-08-31 (id scrubbed): `/fnf/video/{id}` right after confirm.
    let pending: GetMediaStatusResponse = serde_json::from_str(r#"{"id":"00000000-0000-4000-8000-0000000000cc","ip_check_finished":false,"status":"uploaded"}"#).unwrap();
    assert!(!pending.is_ip_check_finished());
    assert!(pending.is_face_detected.is_none());
  }

  #[test]
  fn response_parses() {
    // Live 2026-08-31 (id scrubbed), after confirming with force_ip_check.
    let response: GetMediaStatusResponse = serde_json::from_str(r#"{"id":"00000000-0000-4000-8000-0000000000aa","ip_check_finished":true,"is_face_detected":false,"status":"uploaded"}"#).unwrap();
    assert!(response.is_ip_check_finished());
    assert_eq!(response.is_face_detected, Some(false));
    assert_eq!(response.status, MediaUploadStatus::Uploaded);

    let pending: GetMediaStatusResponse = serde_json::from_str(r#"{"id":"x","ip_check_finished":null,"status":"uploaded"}"#).unwrap();
    assert!(!pending.is_ip_check_finished());

    // Live 2026-08-31: a public figure's photo.
    let flagged: GetMediaStatusResponse = serde_json::from_str(r#"{"id":"x","ip_check_finished":true,"is_face_detected":true,"status":"ip_detected"}"#).unwrap();
    assert!(flagged.is_ip_detected());
    assert_eq!(flagged.is_face_detected, Some(true));
  }
}
