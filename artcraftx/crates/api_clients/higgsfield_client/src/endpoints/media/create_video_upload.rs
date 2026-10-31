//! POST `/fnf/video` — allocate an upload slot for a video clip. (The
//! image presigns reject video types; this is the web app's video path.)

use crate::client::higgsfield_host::HiggsfieldHost;
use crate::client::send_request::{send_json_request, HttpMethod};
use crate::credentials::higgsfield_auth::HiggsfieldAuth;
use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::types::media_mime_type::MediaMimeType;
use crate::types::presigned_media_upload::PresignedMediaUpload;
use serde::Serialize;

const PATH: &str = "/fnf/video";

pub struct CreateVideoUploadArgs<'a> {
  pub request: CreateVideoUploadRequest,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

#[derive(Clone, Debug, Serialize)]
pub struct CreateVideoUploadRequest {
  /// The clip's type; the web app defaults to `video/mp4`.
  #[serde(rename = "mimetype")]
  pub mime_type: MediaMimeType,
}

impl CreateVideoUploadRequest {
  pub fn new(mime_type: MediaMimeType) -> Self {
    Self { mime_type }
  }

  fn validate(&self) -> Result<(), HiggsfieldClientError> {
    if !self.mime_type.is_video() {
      return Err(HiggsfieldClientError::InvalidRequest(format!("/fnf/video presigns video clips, not {}", self.mime_type)));
    }
    Ok(())
  }
}

impl Default for CreateVideoUploadRequest {
  fn default() -> Self {
    Self::new(MediaMimeType::VideoMp4)
  }
}

/// Allocate the slot (its `thumbnail_url` is where the server will put the
/// poster frame). Next: `PUT` the bytes to `upload_url`
/// (`upload_media_bytes`), then `confirm_video_upload`.
pub async fn create_video_upload(args: CreateVideoUploadArgs<'_>) -> Result<PresignedMediaUpload, HiggsfieldError> {
  args.request.validate()?;
  send_json_request(HttpMethod::Post, PATH, args.auth, args.host, Some(&args.request)).await
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::{json, Value};

  #[test]
  fn wire_body_matches_the_web_apps_adapter() {
    let actual: Value = serde_json::to_value(CreateVideoUploadRequest::default()).unwrap();
    assert_eq!(actual, json!({"mimetype": "video/mp4"}));
  }

  #[test]
  fn validation() {
    assert!(matches!(CreateVideoUploadRequest::new(MediaMimeType::ImagePng).validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
    assert!(CreateVideoUploadRequest::new(MediaMimeType::VideoQuicktime).validate().is_ok());
  }

  #[test]
  fn response_parses() {
    // Live 2026-08-31 (ids / signatures scrubbed).
    let json = r#"{"content_type":"video/mp4","id":"00000000-0000-4000-8000-0000000000cc","thumbnail_url":"https://cdn.example.com/00000000-0000-4000-8000-0000000000cc_thumb.webp","upload_url":"https://input-bucket.s3.amazonaws.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000cc.mp4?X-Amz-Signature=deadbeef","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000cc.mp4"}"#;
    let slot: PresignedMediaUpload = serde_json::from_str(json).unwrap();
    assert_eq!(slot.content_type, MediaMimeType::VideoMp4);
    assert!(slot.url.ends_with(".mp4"));
    assert!(slot.thumbnail_url.is_some());
  }

  // ── Live (ignored: needs captured cookies; free) ──

  #[tokio::test]
  #[ignore]
  async fn live_create_video_upload_slot() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_test_secrets::load_higgsfield_test_auth;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let auth = load_higgsfield_test_auth().await?;
    let slot = create_video_upload(CreateVideoUploadArgs {
      request: CreateVideoUploadRequest::default(),
      auth: &auth,
      host: &HiggsfieldHost::Higgsfield,
    }).await.map_err(|err| anyhow::anyhow!("{err}"))?;
    println!("Allocated video slot {} at {} (thumbnail {:?})", slot.id, slot.url, slot.thumbnail_url);
    assert_eq!(slot.content_type, MediaMimeType::VideoMp4);
    Ok(())
  }
}
