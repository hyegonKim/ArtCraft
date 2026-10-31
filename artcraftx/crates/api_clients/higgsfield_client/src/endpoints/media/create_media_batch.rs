//! POST `/fnf/media/batch` — allocate upload slots for several image files
//! at once. This is the presign the video generator's frame and reference
//! pickers use (also for a single file). Images only: the server's 422
//! lists jpeg / jpg / png / webp / gif / heic / heif / avif.

use crate::client::higgsfield_host::HiggsfieldHost;
use crate::client::send_request::{send_json_request, HttpMethod};
use crate::credentials::higgsfield_auth::HiggsfieldAuth;
use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::types::media_mime_type::MediaMimeType;
use crate::types::presigned_media_upload::PresignedMediaUpload;
use crate::types::string_enum::string_enum;
use serde::Serialize;

const PATH: &str = "/fnf/media/batch";

string_enum! {
  /// Where an upload comes from, as the server files it. The generators'
  /// pickers send `user_upload`; the rest are the server's 422 list
  /// (other surfaces of the web app).
  MediaUploadSource {
    UserUpload => "user_upload",
    Grid => "grid",
    Inpaint => "inpaint",
    ElementUpload => "elem_upload",
    Ignore => "ignore",
    ColorGrade => "color_grade",
    Chat => "chat",
    Community => "community",
    MarketingStudioProduct => "ms_product",
    MarketingStudioAvatar => "ms_avatar",
    MarketingStudioV2Logo => "ms_v2_logo",
    Agent => "agent",
    StoryboardPanel => "sb_panel",
    SoulUpload => "soul_upload",
    ViralHubRender => "viral_hub_render",
    VideoFrame => "video_frame",
  }
}

impl Default for MediaUploadSource {
  fn default() -> Self {
    Self::UserUpload
  }
}

pub struct CreateMediaBatchArgs<'a> {
  pub request: CreateMediaBatchRequest,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

#[derive(Clone, Debug, Serialize)]
pub struct CreateMediaBatchRequest {
  /// One entry per file, in order; the response has one slot per entry in
  /// the same order. Image types only.
  #[serde(rename = "mimetypes")]
  pub mime_types: Vec<MediaMimeType>,

  /// How the server files the upload; the generators send `user_upload`.
  pub source: MediaUploadSource,

  /// Ask for an intellectual-property check up front. The web app sends
  /// `false` here and lets the confirm step decide.
  pub force_ip_check: bool,
}

impl CreateMediaBatchRequest {
  /// The web app's defaults (`source: user_upload`, `force_ip_check:
  /// false`).
  pub fn new(mime_types: Vec<MediaMimeType>) -> Self {
    Self { mime_types, source: MediaUploadSource::default(), force_ip_check: false }
  }

  fn validate(&self) -> Result<(), HiggsfieldClientError> {
    if self.mime_types.is_empty() {
      return Err(HiggsfieldClientError::InvalidRequest("mime_types is empty".to_string()));
    }
    if let Some(non_image) = self.mime_types.iter().find(|mime| !mime.is_image()) {
      return Err(HiggsfieldClientError::InvalidRequest(format!(
        "/fnf/media/batch only presigns images; {non_image} must go through /fnf/reference-media",
      )));
    }
    Ok(())
  }
}

/// Allocate the slots. Next, per slot: `PUT` the bytes to `upload_url`
/// (`upload_media_bytes`), then `confirm_media_upload`.
pub async fn create_media_batch(args: CreateMediaBatchArgs<'_>) -> Result<Vec<PresignedMediaUpload>, HiggsfieldError> {
  args.request.validate()?;
  send_json_request(HttpMethod::Post, PATH, args.auth, args.host, Some(&args.request)).await
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::{json, Value};

  #[test]
  fn wire_body_matches_captured_request() {
    let request = CreateMediaBatchRequest::new(vec![MediaMimeType::ImagePng]);
    let actual: Value = serde_json::to_value(request).unwrap();
    assert_eq!(actual, json!({"mimetypes": ["image/png"], "source": "user_upload", "force_ip_check": false}));
  }

  #[test]
  fn validation() {
    assert!(matches!(CreateMediaBatchRequest::new(vec![]).validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
    // Live 2026-08-31: video/audio types are a 422 on this endpoint.
    let err = CreateMediaBatchRequest::new(vec![MediaMimeType::ImagePng, MediaMimeType::VideoMp4]).validate().unwrap_err();
    assert!(err.to_string().contains("reference-media"), "{err}");
    assert!(CreateMediaBatchRequest::new(vec![MediaMimeType::ImageGif, MediaMimeType::ImageAvif]).validate().is_ok());
  }

  #[test]
  fn response_parses() {
    // Captured 2026-08-31 (ids and hosts scrubbed).
    let json = r#"[{"id":"00000000-0000-4000-8000-0000000000bb","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000bb.png","upload_url":"https://input-bucket.s3.amazonaws.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000bb.png?X-Amz-Signature=deadbeef","content_type":"image/png"}]"#;
    let slots: Vec<PresignedMediaUpload> = serde_json::from_str(json).unwrap();
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0].to_media_input().id.as_str(), "00000000-0000-4000-8000-0000000000bb");
  }

  // ── Live (ignored: needs captured cookies; free) ──

  #[tokio::test]
  #[ignore]
  async fn live_create_media_batch_slots() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_test_secrets::load_higgsfield_test_session;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let session = load_higgsfield_test_session()?;
    let auth = session.auth().await.map_err(|err| anyhow::anyhow!("{err}"))?;

    let slots = create_media_batch(CreateMediaBatchArgs {
      request: CreateMediaBatchRequest::new(vec![MediaMimeType::ImagePng, MediaMimeType::ImageJpeg]),
      auth: &auth,
      host: &HiggsfieldHost::Higgsfield,
    }).await.map_err(|err| anyhow::anyhow!("{err}"))?;

    for slot in &slots {
      println!("Allocated media slot {} at {} (content_type {})", slot.id, slot.url, slot.content_type);
    }
    assert_eq!(slots.len(), 2);
    assert_eq!(slots[0].content_type, MediaMimeType::ImagePng);
    assert_eq!(slots[1].content_type, MediaMimeType::ImageJpeg);
    Ok(())
  }
}
