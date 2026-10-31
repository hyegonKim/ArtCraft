//! POST `/fnf/reference-media` — allocate one upload slot. This is the
//! presign the image generator's reference picker uses.

use crate::client::higgsfield_host::HiggsfieldHost;
use crate::client::send_request::{send_json_request, HttpMethod};
use crate::credentials::higgsfield_auth::HiggsfieldAuth;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::types::media_mime_type::MediaMimeType;
use crate::types::presigned_media_upload::PresignedMediaUpload;
use serde::Serialize;

const PATH: &str = "/fnf/reference-media";

pub struct CreateReferenceMediaArgs<'a> {
  pub request: CreateReferenceMediaRequest,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

#[derive(Clone, Debug, Serialize)]
pub struct CreateReferenceMediaRequest {
  /// The file's type; it becomes the slot's `content_type` and the CDN
  /// URL's extension.
  #[serde(rename = "mimetype")]
  pub mime_type: MediaMimeType,
}

impl CreateReferenceMediaRequest {
  pub fn new(mime_type: MediaMimeType) -> Self {
    Self { mime_type }
  }
}

/// Allocate a slot. Next: `PUT` the bytes to `upload_url`
/// (`upload_media_bytes`), then `confirm_media_upload`.
pub async fn create_reference_media(args: CreateReferenceMediaArgs<'_>) -> Result<PresignedMediaUpload, HiggsfieldError> {
  send_json_request(HttpMethod::Post, PATH, args.auth, args.host, Some(&args.request)).await
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::{json, Value};

  #[test]
  fn wire_body_matches_captured_request() {
    let actual: Value = serde_json::to_value(CreateReferenceMediaRequest::new(MediaMimeType::ImagePng)).unwrap();
    assert_eq!(actual, json!({"mimetype": "image/png"}));
  }

  #[test]
  fn response_parses() {
    // Captured 2026-08-31 (ids and hosts scrubbed).
    let json = r#"{"id":"00000000-0000-4000-8000-0000000000aa","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png","upload_url":"https://input-bucket.s3.amazonaws.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Signature=deadbeef","content_type":"image/png"}"#;
    let slot: PresignedMediaUpload = serde_json::from_str(json).unwrap();
    assert_eq!(slot.id.as_str(), "00000000-0000-4000-8000-0000000000aa");
    assert!(slot.url.ends_with(".png"));
    assert_eq!(slot.content_type, MediaMimeType::ImagePng);
  }

  // ── Live (ignored: needs captured cookies; free) ──

  #[tokio::test]
  #[ignore]
  async fn live_create_reference_media_slot() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_test_secrets::load_higgsfield_test_auth;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let auth = load_higgsfield_test_auth().await?;
    let slot = create_reference_media(CreateReferenceMediaArgs {
      request: CreateReferenceMediaRequest::new(MediaMimeType::ImagePng),
      auth: &auth,
      host: &HiggsfieldHost::Higgsfield,
    }).await.map_err(|err| anyhow::anyhow!("{err}"))?;

    println!("Allocated media slot {} at {} (content_type {})", slot.id, slot.url, slot.content_type);
    assert!(slot.upload_url.starts_with("https://"));
    assert_eq!(slot.content_type, MediaMimeType::ImagePng);
    Ok(())
  }
}
