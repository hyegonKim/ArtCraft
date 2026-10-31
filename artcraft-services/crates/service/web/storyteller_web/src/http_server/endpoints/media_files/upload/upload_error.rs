use actix_web::error::ResponseError;
use actix_web::http::StatusCode;
use actix_web::HttpResponse;
use utoipa::ToSchema;

use http_server_common::response::serialize_as_json_error::serialize_as_json_error;

#[derive(Debug, Serialize, ToSchema)]
pub enum MediaFileUploadError {
  BadInput(String),
  NotAuthorized,
  NotAuthorizedVerbose(String),
  NotFoundVerbose(String),
  MustBeLoggedIn,
  ServerError,
  ServerErrorVerbose(String),
  RateLimited,
}

impl ResponseError for MediaFileUploadError {
  fn status_code(&self) -> StatusCode {
    match *self {
      Self::BadInput(_) => StatusCode::BAD_REQUEST,
      Self::NotAuthorized => StatusCode::UNAUTHORIZED,
      Self::NotAuthorizedVerbose(_) => StatusCode::UNAUTHORIZED,
      Self::NotFoundVerbose(_) => StatusCode::NOT_FOUND,
      Self::MustBeLoggedIn => StatusCode::UNAUTHORIZED,
      Self::ServerError => StatusCode::INTERNAL_SERVER_ERROR,
      Self::ServerErrorVerbose(_) => StatusCode::INTERNAL_SERVER_ERROR,
      Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
    }
  }

  fn error_response(&self) -> HttpResponse {
    serialize_as_json_error(self)
  }
}

impl std::fmt::Display for MediaFileUploadError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{:?}", self)
  }
}
