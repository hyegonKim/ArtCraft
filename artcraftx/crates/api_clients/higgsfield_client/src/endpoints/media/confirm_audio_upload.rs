//! POST `/fnf/audio/{id}/upload` — tell the gateway an audio slot has its
//! bytes. The web app sends no body fields of its own.

use crate::client::higgsfield_host::HiggsfieldHost;
use crate::client::send_request::{send_json_request, HttpMethod};
use crate::credentials::higgsfield_auth::HiggsfieldAuth;
use crate::endpoints::media::confirm_media_upload::ConfirmMediaUploadResponse;
use crate::error::higgsfield_client_error::HiggsfieldClientError;
use crate::error::higgsfield_error::HiggsfieldError;
use crate::types::ids::MediaId;
use serde::Serialize;

pub struct ConfirmAudioUploadArgs<'a> {
  pub request: ConfirmAudioUploadRequest,
  pub auth: &'a HiggsfieldAuth,
  pub host: &'a HiggsfieldHost,
}

#[derive(Clone, Debug)]
pub struct ConfirmAudioUploadRequest {
  pub media_id: MediaId,
}

impl ConfirmAudioUploadRequest {
  pub fn new(media_id: MediaId) -> Self {
    Self { media_id }
  }

  fn validate(&self) -> Result<(), HiggsfieldClientError> {
    if self.media_id.as_str().trim().is_empty() {
      return Err(HiggsfieldClientError::InvalidRequest("media_id is empty".to_string()));
    }
    Ok(())
  }

  fn path(&self) -> String {
    format!("/fnf/audio/{}/upload", self.media_id)
  }
}

/// Confirm the upload. The response has the same shape as the image
/// confirm (`{id, status, ip_check_finished}`).
pub async fn confirm_audio_upload(args: ConfirmAudioUploadArgs<'_>) -> Result<ConfirmMediaUploadResponse, HiggsfieldError> {
  args.request.validate()?;
  send_json_request(HttpMethod::Post, &args.request.path(), args.auth, args.host, Some(&EmptyBody {})).await
}

// ── Wire format ──

/// `{}` — the web app spreads its (empty) `extra` here.
#[derive(Serialize)]
struct EmptyBody {}

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::{json, Value};

  #[test]
  fn path_and_body_match_the_web_apps_adapter() {
    let request = ConfirmAudioUploadRequest::new(MediaId::new("00000000-0000-4000-8000-0000000000dd"));
    assert_eq!(request.path(), "/fnf/audio/00000000-0000-4000-8000-0000000000dd/upload");
    let body: Value = serde_json::to_value(EmptyBody {}).unwrap();
    assert_eq!(body, json!({}));
  }

  #[test]
  fn validation() {
    assert!(matches!(ConfirmAudioUploadRequest::new(MediaId::new(" ")).validate(), Err(HiggsfieldClientError::InvalidRequest(_))));
  }
}
