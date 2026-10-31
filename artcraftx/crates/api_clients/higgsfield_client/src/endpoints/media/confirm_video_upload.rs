//! POST `/fnf/video/{id}/upload` — tell the gateway a video slot has its
//! bytes. Optionally trims the clip (`?start_seconds=&end_seconds=`).

use crate::client::higgsfield_host::HiggsfieldHost;
use crate::client::send_request::{send_json_request, HttpMethod};
use crate::credentials::higgsfield_auth::HiggsfieldAuth;
use crate::endpoints::media::confirm_media_upload::ConfirmMediaUploadResponse;
use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::types::ids::MediaId;
use serde::Serialize;

pub struct ConfirmVideoUploadArgs<'a> {
  pub request: ConfirmVideoUploadRequest,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

#[derive(Clone, Debug)]
pub struct ConfirmVideoUploadRequest {
  pub media_id: MediaId,

  /// The web app sends `true`.
  pub force_nsfw_check: bool,

  /// Omitted by the web app unless set.
  pub maybe_force_ip_check: Option<bool>,

  /// Trim: keep from this offset (seconds). Sent as a query parameter.
  pub maybe_start_seconds: Option<f64>,

  /// Trim: keep up to this offset (seconds). Sent as a query parameter.
  pub maybe_end_seconds: Option<f64>,
}

impl ConfirmVideoUploadRequest {
  /// The web app's defaults (NSFW check on, no trim).
  pub fn new(media_id: MediaId) -> Self {
    Self { media_id, force_nsfw_check: true, maybe_force_ip_check: None, maybe_start_seconds: None, maybe_end_seconds: None }
  }

  /// Keep only `[start, end]` seconds of the clip.
  pub fn trimmed(mut self, start_seconds: f64, end_seconds: f64) -> Self {
    self.maybe_start_seconds = Some(start_seconds);
    self.maybe_end_seconds = Some(end_seconds);
    self
  }

  fn validate(&self) -> Result<(), HiggsfieldClientError> {
    if self.media_id.as_str().trim().is_empty() {
      return Err(HiggsfieldClientError::InvalidRequest("media_id is empty".to_string()));
    }
    if let (Some(start), Some(end)) = (self.maybe_start_seconds, self.maybe_end_seconds) {
      if !(start >= 0.0 && end > start) {
        return Err(HiggsfieldClientError::InvalidRequest(format!("trim range {start}..{end} is not 0 <= start < end")));
      }
    }
    Ok(())
  }

  fn path(&self) -> String {
    let mut path = format!("/fnf/video/{}/upload", self.media_id);
    let mut query = Vec::new();
    if let Some(start) = self.maybe_start_seconds {
      query.push(format!("start_seconds={start}"));
    }
    if let Some(end) = self.maybe_end_seconds {
      query.push(format!("end_seconds={end}"));
    }
    if !query.is_empty() {
      path.push('?');
      path.push_str(&query.join("&"));
    }
    path
  }

  fn to_body(&self) -> ConfirmVideoUploadBody {
    ConfirmVideoUploadBody { force_nsfw_check: self.force_nsfw_check, maybe_force_ip_check: self.maybe_force_ip_check }
  }
}

/// Confirm the upload. The response is the image confirm's shape (`{id,
/// status, ip_check_finished}`) plus the clip's probed metadata (`duration`,
/// `frame_rate`, `frames_count`, `width`, `height`, `size_bytes`,
/// `thumbnail_url`, `url`) in `extra`.
pub async fn confirm_video_upload(args: ConfirmVideoUploadArgs<'_>) -> Result<ConfirmMediaUploadResponse, HiggsfieldError> {
  args.request.validate()?;
  let body = args.request.to_body();
  send_json_request(HttpMethod::Post, &args.request.path(), args.auth, args.host, Some(&body)).await
}

// ── Wire format ──

#[derive(Serialize)]
struct ConfirmVideoUploadBody {
  force_nsfw_check: bool,
  #[serde(rename = "force_ip_check", skip_serializing_if = "Option::is_none")]
  maybe_force_ip_check: Option<bool>,
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::{json, Value};

  #[test]
  fn path_and_body_match_the_web_apps_adapter() {
    let request = ConfirmVideoUploadRequest::new(MediaId::new("00000000-0000-4000-8000-0000000000cc"));
    assert_eq!(request.path(), "/fnf/video/00000000-0000-4000-8000-0000000000cc/upload");
    let body: Value = serde_json::to_value(request.to_body()).unwrap();
    assert_eq!(body, json!({"force_nsfw_check": true}));

    let mut trimmed = request.clone().trimmed(1.5, 4.0);
    trimmed.maybe_force_ip_check = Some(false);
    assert_eq!(trimmed.path(), "/fnf/video/00000000-0000-4000-8000-0000000000cc/upload?start_seconds=1.5&end_seconds=4");
    let body: Value = serde_json::to_value(trimmed.to_body()).unwrap();
    assert_eq!(body, json!({"force_nsfw_check": true, "force_ip_check": false}));
  }

  #[test]
  fn response_parses() {
    // Live 2026-08-31 (ids / hosts scrubbed), confirmed with force_ip_check.
    let response: ConfirmMediaUploadResponse = serde_json::from_str(r#"{"duration":1.0,"frame_rate":25.0,"frames_count":25,"height":64,"id":"00000000-0000-4000-8000-0000000000cc","ip_check_finished":false,"size":2246,"size_bytes":2246,"status":"uploaded","thumbnail_url":"https://cdn.example.com/00000000-0000-4000-8000-0000000000cc_thumb.webp","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000cc.mp4","width":64}"#).unwrap();
    assert!(response.is_uploaded());
    assert_eq!(response.ip_check_finished, Some(false));
    assert_eq!(response.extra.get("frames_count"), Some(&Value::from(25)));
  }

  #[test]
  fn validation() {
    assert!(matches!(ConfirmVideoUploadRequest::new(MediaId::new("")).validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
    let backwards = ConfirmVideoUploadRequest::new(MediaId::new("m")).trimmed(4.0, 1.0);
    assert!(matches!(backwards.validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
  }
}
