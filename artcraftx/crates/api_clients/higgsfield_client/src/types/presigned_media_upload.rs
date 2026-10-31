//! What the presign endpoints hand back: where to PUT the bytes and where
//! they'll be served from afterwards.

use crate::types::ids::MediaId;
use crate::types::media_input::MediaInput;
use crate::types::media_mime_type::MediaMimeType;
use serde::Deserialize;

/// One presigned upload slot. Returned singly by `/fnf/reference-media`
/// and as a list by `/fnf/media/batch`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct PresignedMediaUpload {
  pub id: MediaId,

  /// The public CDN URL the file will have once uploaded. This is what
  /// generation requests reference.
  pub url: String,

  /// A presigned storage URL (S3); `PUT` the raw bytes here with
  /// `Content-Type: <content_type>`. Short-lived and secret-bearing — don't
  /// log it.
  pub upload_url: String,

  /// As the server will store it; can differ from what was asked for (a
  /// `wav` audio presign comes back as `audio/x-wav`). Use this as the
  /// `Content-Type` of the `PUT`.
  pub content_type: MediaMimeType,

  /// Video presigns come with the poster frame the server will extract.
  #[serde(default)]
  pub thumbnail_url: Option<String>,
}

impl PresignedMediaUpload {
  /// The descriptor to put in a generation request once the bytes are
  /// uploaded and confirmed. The descriptor's `type` follows the file
  /// family (`media_input` / `video_input` / `audio_input`).
  pub fn to_media_input(&self) -> MediaInput {
    if self.content_type.is_video() {
      MediaInput::uploaded_video(self.id.clone(), self.url.clone())
    } else if self.content_type.is_audio() {
      MediaInput::uploaded_audio(self.id.clone(), self.url.clone())
    } else {
      MediaInput::uploaded(self.id.clone(), self.url.clone())
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::types::media_input::MediaInputKind;

  #[test]
  fn parses_and_converts() {
    let json = r#"{"id":"00000000-0000-4000-8000-000000000001","url":"https://cdn.example.com/user_x/00000000-0000-4000-8000-000000000001.png","upload_url":"https://bucket.s3.amazonaws.com/user_x/00000000-0000-4000-8000-000000000001.png?X-Amz-Signature=abc","content_type":"image/png"}"#;
    let slot: PresignedMediaUpload = serde_json::from_str(json).unwrap();
    assert_eq!(slot.content_type, MediaMimeType::ImagePng);
    assert!(slot.upload_url.contains("X-Amz-Signature"));
    let input = slot.to_media_input();
    assert_eq!(input.id.as_str(), "00000000-0000-4000-8000-000000000001");
    assert_eq!(input.url, slot.url);
    assert_eq!(input.kind, MediaInputKind::MediaInput);
    assert!(slot.thumbnail_url.is_none());
  }

  #[test]
  fn video_and_audio_slots_parse_and_convert() {
    // Live 2026-08-31 (ids / signatures scrubbed).
    let video: PresignedMediaUpload = serde_json::from_str(r#"{"content_type":"video/mp4","id":"00000000-0000-4000-8000-000000000002","thumbnail_url":"https://cdn.example.com/00000000-0000-4000-8000-000000000002_thumb.webp","upload_url":"https://bucket.s3.amazonaws.com/user_x/00000000-0000-4000-8000-000000000002.mp4?X-Amz-Signature=abc","url":"https://cdn.example.com/user_x/00000000-0000-4000-8000-000000000002.mp4"}"#).unwrap();
    assert_eq!(video.to_media_input().kind, MediaInputKind::VideoInput);
    assert!(video.thumbnail_url.as_deref().unwrap().ends_with("_thumb.webp"));

    let audio: PresignedMediaUpload = serde_json::from_str(r#"{"content_type":"audio/x-wav","id":"00000000-0000-4000-8000-000000000003","upload_url":"https://bucket.s3.amazonaws.com/user_x/00000000-0000-4000-8000-000000000003.wav?X-Amz-Signature=abc","url":"https://cdn.example.com/user_x/00000000-0000-4000-8000-000000000003.wav"}"#).unwrap();
    assert_eq!(audio.content_type, MediaMimeType::AudioXWav);
    assert_eq!(audio.to_media_input().kind, MediaInputKind::AudioInput);
  }
}
