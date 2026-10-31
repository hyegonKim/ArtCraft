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
    "image/jpeg",
    "image/png",
    "image/webp",
  ])
});

#[deprecated(note = "Use `media_files` instead of `media_uploads`.")]
pub async fn upload_image_handler(
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
