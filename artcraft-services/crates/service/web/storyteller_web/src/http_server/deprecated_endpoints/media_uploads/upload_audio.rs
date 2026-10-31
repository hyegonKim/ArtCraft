use std::collections::HashSet;
use std::sync::Arc;

use actix_multipart::Multipart;
use actix_web::{web, HttpRequest, HttpResponse};
use once_cell::sync::Lazy;

use tokens::tokens::media_uploads::MediaUploadToken;

use crate::http_server::deprecated_endpoints::media_uploads::common::handle_upload::handle_upload;
use crate::http_server::deprecated_endpoints::media_uploads::common::upload_error::UploadError;
use crate::state::server_state::ServerState;

#[deprecated(note = "Use `media_files` instead of `media_uploads`.")]
#[derive(Serialize)]
pub struct UploadAudioSuccessResponse {
  pub success: bool,
  pub upload_token: MediaUploadToken,
}

#[deprecated(note = "Use `media_files` instead of `media_uploads`.")]
static ALLOWED_MIME_TYPES : Lazy<HashSet<&'static str>> = Lazy::new(|| {
  HashSet::from([
    "audio/aac",
    "audio/m4a",
    "audio/mp4", // iPhone seems to upload these
    "video/mp4",
    "audio/mpeg",
    "audio/ogg",
    "audio/opus",
    "audio/x-flac",
    "audio/x-wav",

    // NB(bt,2023-10-13): This is the only way to allow browser recording.
    // https://air.ghost.io/recording-to-an-audio-file-using-html5-and-js/
    // Chrome: "audio/webm;codecs=opus"
    // Firefox: "audio/ogg;codecs=opus"
    "audio/webm",
    "video/webm",
  ])
});

#[deprecated(note = "Use `media_files` instead of `media_uploads`.")]
pub async fn upload_audio_handler(
  http_request: HttpRequest,
  server_state: web::Data<Arc<ServerState>>,
  mut multipart_payload: Multipart,
) -> Result<HttpResponse, UploadError> {

  let response = handle_upload(
    &http_request,
    &server_state,
    multipart_payload,
    &ALLOWED_MIME_TYPES).await?;

  let media_upload_token = response.to_media_token();

  let response = UploadAudioSuccessResponse {
    success: true,
    upload_token: media_upload_token,
  };

  let body = serde_json::to_string(&response)
      .map_err(|e| UploadError::ServerError)?;

  return Ok(HttpResponse::Ok()
      .content_type("application/json")
      .body(body));
}
