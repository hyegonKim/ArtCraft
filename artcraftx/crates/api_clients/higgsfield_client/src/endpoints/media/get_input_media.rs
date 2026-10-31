//! GET `/fnf/input-images/{id}`, `/fnf/input-videos/{id}`,
//! `/fnf/input-audios/{id}` — look up an uploaded file's descriptor by id
//! (what the web app calls `getMedia`). Handy for re-using an upload from
//! an earlier session without keeping its URL around.

use crate::client::higgsfield_host::HiggsfieldHost;
use crate::client::send_request::{send_json_request, HttpMethod};
use crate::credentials::higgsfield_auth::HiggsfieldAuth;
use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::types::ids::MediaId;
use crate::types::media_input::{MediaInput, MediaInputKind};
use serde::Deserialize;
use serde_json::Value;

/// Which collection the id lives in (uploads are filed by family).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputMediaFamily {
  Image,
  Video,
  Audio,
}

impl InputMediaFamily {
  fn path_segment(self) -> &'static str {
    match self {
      Self::Image => "input-images",
      Self::Video => "input-videos",
      Self::Audio => "input-audios",
    }
  }
}

pub struct GetInputMediaArgs<'a> {
  pub request: GetInputMediaRequest,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

#[derive(Clone, Debug)]
pub struct GetInputMediaRequest {
  pub family: InputMediaFamily,
  pub media_id: MediaId,
}

impl GetInputMediaRequest {
  pub fn new(family: InputMediaFamily, media_id: MediaId) -> Self {
    Self { family, media_id }
  }

  fn validate(&self) -> Result<(), HiggsfieldClientError> {
    if self.media_id.as_str().trim().is_empty() {
      return Err(HiggsfieldClientError::InvalidRequest("media_id is empty".to_string()));
    }
    Ok(())
  }

  fn path(&self) -> String {
    format!("/fnf/{}/{}", self.family.path_segment(), self.media_id)
  }
}

/// The descriptor plus whatever else the family reports (videos: `hls_url`,
/// `storyboard`, `width`/`height`; audio: `duration`, `waveform_url`, ...).
#[derive(Clone, Debug, Deserialize)]
pub struct GetInputMediaResponse {
  pub id: MediaId,

  #[serde(rename = "type")]
  pub kind: MediaInputKind,

  pub url: String,

  #[serde(flatten)]
  pub extra: serde_json::Map<String, Value>,
}

impl GetInputMediaResponse {
  pub fn to_media_input(&self) -> MediaInput {
    MediaInput { id: self.id.clone(), kind: self.kind.clone(), url: self.url.clone() }
  }
}

pub async fn get_input_media(args: GetInputMediaArgs<'_>) -> Result<GetInputMediaResponse, HiggsfieldError> {
  args.request.validate()?;
  send_json_request(HttpMethod::Get, &args.request.path(), args.auth, args.host, None::<&()>).await
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn paths() {
    let id = MediaId::new("00000000-0000-4000-8000-0000000000aa");
    assert_eq!(GetInputMediaRequest::new(InputMediaFamily::Image, id.clone()).path(), "/fnf/input-images/00000000-0000-4000-8000-0000000000aa");
    assert_eq!(GetInputMediaRequest::new(InputMediaFamily::Video, id.clone()).path(), "/fnf/input-videos/00000000-0000-4000-8000-0000000000aa");
    assert_eq!(GetInputMediaRequest::new(InputMediaFamily::Audio, id).path(), "/fnf/input-audios/00000000-0000-4000-8000-0000000000aa");
    assert!(matches!(GetInputMediaRequest::new(InputMediaFamily::Image, MediaId::new(" ")).validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
  }

  #[test]
  fn responses_parse() {
    // Live 2026-08-31 (ids / hosts scrubbed).
    let image: GetInputMediaResponse = serde_json::from_str(r#"{"id":"00000000-0000-4000-8000-0000000000aa","type":"media_input","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png"}"#).unwrap();
    assert_eq!(image.to_media_input(), MediaInput::uploaded("00000000-0000-4000-8000-0000000000aa", "https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000aa.png"));

    // The audio confirm echoes the same record shape.
    let audio: GetInputMediaResponse = serde_json::from_str(r#"{"id":"00000000-0000-4000-8000-0000000000dd","type":"audio_input","name":"tone_1s.wav","filename":"tone_1s.wav","url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000dd.wav","download_url":"https://cdn.example.com/user_TESTUSER0000000000000000000/00000000-0000-4000-8000-0000000000dd.wav?download=tone_1s.wav","user_id":"user_TESTUSER0000000000000000000","waveform_url":"https://waveforms.example.com/waveform/00000000-0000-4000-8000-0000000000ee.json","duration":1.0,"extension":"wav","size_bytes":16044,"prompt":null,"status":"uploaded","created_at":1788159896.552181,"uploaded_at":1788159896.552181,"last_used_at":null}"#).unwrap();
    assert_eq!(audio.kind, MediaInputKind::AudioInput);
    assert_eq!(audio.extra.get("duration"), Some(&Value::from(1.0)));
  }
}
