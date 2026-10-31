use actix_web::error::ResponseError;
use actix_web::http::StatusCode;
use actix_web::HttpResponse;

use http_server_common::response::serialize_as_json_error::serialize_as_json_error;

#[derive(Debug, Serialize)]
pub enum UploadError {
  BadInput(String),
  NotAuthorized,
  MustBeLoggedIn,
  ServerError,
  RateLimited,
}

impl ResponseError for UploadError {
  fn status_code(&self) -> StatusCode {
    match *self {
      UploadError::BadInput(_) => StatusCode::BAD_REQUEST,
      UploadError::NotAuthorized => StatusCode::UNAUTHORIZED,
      UploadError::MustBeLoggedIn => StatusCode::UNAUTHORIZED,
      UploadError::ServerError => StatusCode::INTERNAL_SERVER_ERROR,
      UploadError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
    }
  }

  fn error_response(&self) -> HttpResponse {
    serialize_as_json_error(self)
  }
}

// NB: Not using derive_more::Display since Clion doesn't understand it.
impl std::fmt::Display for UploadError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{:?}", self)
  }
}
