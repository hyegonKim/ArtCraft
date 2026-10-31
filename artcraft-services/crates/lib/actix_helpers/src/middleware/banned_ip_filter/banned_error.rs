use actix_http::body::BoxBody;
use actix_http::header::CONTENT_TYPE;
use actix_web::{HttpResponse, HttpResponseBuilder, ResponseError};
use actix_web::http::header::ContentType;
use actix_web::http::StatusCode;

#[derive(Debug)]
pub struct BannedError;

impl std::fmt::Display for BannedError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "BannedError")
  }
}

impl std::error::Error for BannedError {}

impl ResponseError for BannedError {
  fn status_code(&self) -> StatusCode {
    StatusCode::UNAUTHORIZED
  }

  fn error_response(&self) -> HttpResponse<BoxBody> {
    // NB: I'm setting a string error because I mistakenly got caught by this in local dev
    // and couldn't figure out the issue for a bit. At least I can grep for this string.
    // However, I need to balance this requirement with not cluing in those that are banned.
    HttpResponseBuilder::new(self.status_code())
        .append_header((CONTENT_TYPE, ContentType::json()))
        .body(r#"{"success": false, "error_message": "ERR64: database error"}"#)
  }
}
