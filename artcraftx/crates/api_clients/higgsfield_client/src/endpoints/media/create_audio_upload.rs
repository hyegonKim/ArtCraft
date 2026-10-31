//! POST `/fnf/audio` — allocate an upload slot for an audio track. Keyed by
//! file extension rather than MIME type; the web app only presigns `wav`,
//! `mp3` and `webm`.

use crate::client::higgsfield_host::HiggsfieldHost;
use crate::client::send_request::{send_json_request, HttpMethod};
use crate::credentials::higgsfield_auth::HiggsfieldAuth;
use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::types::media_mime_type::MediaMimeType;
use crate::types::presigned_media_upload::PresignedMediaUpload;
use crate::types::string_enum::string_enum;
use serde::Serialize;

const PATH: &str = "/fnf/audio";

string_enum! {
  /// The audio formats the web app will presign ("Audio uploads support
  /// only wav, mp3 or webm").
  AudioUploadExtension {
    Wav => "wav",
    Mp3 => "mp3",
    Webm => "webm",
  }
}

impl AudioUploadExtension {
  /// The extension for an audio MIME type, if it's one the web app takes.
  pub fn for_mime_type(mime_type: &MediaMimeType) -> Option<Self> {
    match mime_type {
      MediaMimeType::AudioWav | MediaMimeType::AudioXWav => Some(Self::Wav),
      MediaMimeType::AudioMpeg => Some(Self::Mp3),
      MediaMimeType::AudioWebm => Some(Self::Webm),
      _ => None,
    }
  }
}

pub struct CreateAudioUploadArgs<'a> {
  pub request: CreateAudioUploadRequest,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

#[derive(Clone, Debug, Serialize)]
pub struct CreateAudioUploadRequest {
  pub extension: AudioUploadExtension,

  /// The file name (the web app falls back to `upload.<extension>`).
  pub name: String,
}

impl CreateAudioUploadRequest {
  pub fn new(extension: AudioUploadExtension, name: impl Into<String>) -> Self {
    Self { extension, name: name.into() }
  }

  /// With the web app's fallback name.
  pub fn unnamed(extension: AudioUploadExtension) -> Self {
    let name = format!("upload.{extension}");
    Self { extension, name }
  }

  fn validate(&self) -> Result<(), HiggsfieldClientError> {
    if self.name.trim().is_empty() {
      return Err(HiggsfieldClientError::InvalidRequest("name is empty".to_string()));
    }
    Ok(())
  }
}

/// Allocate the slot. NB: the slot's `content_type` is what the server
/// chose (`audio/x-wav` for a `wav`); `PUT` with that. Next:
/// `upload_media_bytes`, then `confirm_audio_upload`.
pub async fn create_audio_upload(args: CreateAudioUploadArgs<'_>) -> Result<PresignedMediaUpload, HiggsfieldError> {
  args.request.validate()?;
  send_json_request(HttpMethod::Post, PATH, args.auth, args.host, Some(&args.request)).await
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::{json, Value};

  #[test]
  fn wire_body_matches_the_web_apps_adapter() {
    let actual: Value = serde_json::to_value(CreateAudioUploadRequest::new(AudioUploadExtension::Wav, "tone_1s.wav")).unwrap();
    assert_eq!(actual, json!({"extension": "wav", "name": "tone_1s.wav"}));
    let unnamed: Value = serde_json::to_value(CreateAudioUploadRequest::unnamed(AudioUploadExtension::Mp3)).unwrap();
    assert_eq!(unnamed, json!({"extension": "mp3", "name": "upload.mp3"}));
  }

  #[test]
  fn extension_from_mime_type() {
    assert_eq!(AudioUploadExtension::for_mime_type(&MediaMimeType::AudioWav), Some(AudioUploadExtension::Wav));
    assert_eq!(AudioUploadExtension::for_mime_type(&MediaMimeType::AudioMpeg), Some(AudioUploadExtension::Mp3));
    assert_eq!(AudioUploadExtension::for_mime_type(&MediaMimeType::AudioAac), None);
    assert_eq!(AudioUploadExtension::for_mime_type(&MediaMimeType::VideoMp4), None);
  }

  #[test]
  fn validation() {
    assert!(matches!(CreateAudioUploadRequest::new(AudioUploadExtension::Wav, " ").validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[test]
  fn response_parses() {
    // Live 2026-08-31 (ids / signatures scrubbed): note `audio/x-wav`.
    let json = r#"{"content_type":"audio/x-wav","id":"00000000-0000-4000-8000-0000000000dd","upload_url":"https://input-bucket.s3.amazonaws.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000dd.wav?X-Amz-Signature=deadbeef","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000dd.wav"}"#;
    let slot: PresignedMediaUpload = serde_json::from_str(json).unwrap();
    assert_eq!(slot.content_type, MediaMimeType::AudioXWav);
    assert!(slot.url.ends_with(".wav"));
  }

  // ── Live (ignored: needs captured cookies; free) ──

  #[tokio::test]
  #[ignore]
  async fn live_create_audio_upload_slot() -> anyhow::Result<()> {
    use crate::test_utils::higgsfield_test_secrets::load_higgsfield_test_auth;
    use crate::test_utils::setup_test_logging::setup_test_logging;
    setup_test_logging();

    let auth = load_higgsfield_test_auth().await?;
    let slot = create_audio_upload(CreateAudioUploadArgs {
      request: CreateAudioUploadRequest::new(AudioUploadExtension::Wav, "tone_1s.wav"),
      auth: &auth,
      host: &HiggsfieldHost::Higgsfield,
    }).await.map_err(|err| anyhow::anyhow!("{err}"))?;
    println!("Allocated audio slot {} at {} ({})", slot.id, slot.url, slot.content_type);
    assert!(slot.content_type.is_audio());
    Ok(())
  }
}
