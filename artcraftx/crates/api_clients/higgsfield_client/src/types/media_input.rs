//! The descriptor generation requests use to point at a reference file.

use crate::types::ids::MediaId;
use crate::types::string_enum::string_enum;
use serde::{Deserialize, Serialize};

string_enum! {
  /// Where a reference comes from. The web app builds `media_input` for
  /// images it uploaded, `video_input` / `audio_input` for uploaded clips
  /// and tracks, and `image_job` / `video_job` when a previous generation
  /// is reused as a reference (then `id` is the job id).
  MediaInputKind {
    MediaInput => "media_input",
    VideoInput => "video_input",
    AudioInput => "audio_input",
    ImageJob => "image_job",
    VideoJob => "video_job",
  }
}

/// A reference file as the API knows it: the id the upload endpoints
/// handed out and the CDN URL they published it at. Bytes are never sent
/// with a generation request — only this descriptor.
///
/// On the wire: `{"id": "<uuid>", "type": "media_input", "url": "https://…"}`.
/// (The web app also tacks on `ipCheckFinished` / `ipStatus`; the server
/// drops them, so this client doesn't send them.)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaInput {
  pub id: MediaId,

  #[serde(rename = "type")]
  pub kind: MediaInputKind,

  pub url: String,
}

impl MediaInput {
  /// An image uploaded through the media endpoints (see
  /// `endpoints::media` / `HiggsfieldSession::upload_reference_media`).
  pub fn uploaded(id: impl Into<MediaId>, url: impl Into<String>) -> Self {
    Self { id: id.into(), kind: MediaInputKind::MediaInput, url: url.into() }
  }

  /// An uploaded video clip.
  pub fn uploaded_video(id: impl Into<MediaId>, url: impl Into<String>) -> Self {
    Self { id: id.into(), kind: MediaInputKind::VideoInput, url: url.into() }
  }

  /// An uploaded audio track.
  pub fn uploaded_audio(id: impl Into<MediaId>, url: impl Into<String>) -> Self {
    Self { id: id.into(), kind: MediaInputKind::AudioInput, url: url.into() }
  }

  /// A previous image generation, by job id and result URL.
  pub fn from_image_job(job_id: impl Into<MediaId>, url: impl Into<String>) -> Self {
    Self { id: job_id.into(), kind: MediaInputKind::ImageJob, url: url.into() }
  }

  /// A previous video generation, by job id and result URL.
  pub fn from_video_job(job_id: impl Into<MediaId>, url: impl Into<String>) -> Self {
    Self { id: job_id.into(), kind: MediaInputKind::VideoJob, url: url.into() }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::{json, Value};

  #[test]
  fn serializes_like_the_web_app() {
    let input = MediaInput::uploaded("00000000-0000-4000-8000-000000000001", "https://cdn.example.com/user_x/00000000-0000-4000-8000-000000000001.png");
    let actual: Value = serde_json::to_value(&input).unwrap();
    assert_eq!(actual, json!({
      "id": "00000000-0000-4000-8000-000000000001",
      "type": "media_input",
      "url": "https://cdn.example.com/user_x/00000000-0000-4000-8000-000000000001.png",
    }));
  }

  #[test]
  fn parses_the_server_echo_and_the_web_apps_extra_fields() {
    // As echoed in job set / job status params.
    let echoed: MediaInput = serde_json::from_str(r#"{"id":"abc","type":"media_input","url":"https://cdn.example.com/x.png"}"#).unwrap();
    assert_eq!(echoed, MediaInput::uploaded("abc", "https://cdn.example.com/x.png"));

    // As the web app sends it (extra fields ignored).
    let sent: MediaInput = serde_json::from_str(r#"{"id":"abc","url":"https://cdn.example.com/x.png","type":"media_input","ipCheckFinished":null,"ipStatus":"uploaded"}"#).unwrap();
    assert_eq!(sent, echoed);

    let job: MediaInput = serde_json::from_str(r#"{"id":"job-1","type":"video_job","url":"https://cdn.example.com/x.mp4"}"#).unwrap();
    assert_eq!(job.kind, MediaInputKind::VideoJob);

    let unknown: MediaInput = serde_json::from_str(r#"{"id":"x","type":"element","url":"u"}"#).unwrap();
    assert_eq!(unknown.kind, MediaInputKind::Other("element".to_string()));
  }
}
