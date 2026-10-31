use std::fmt;

use actix_web::error::ResponseError;
use actix_web::http::StatusCode;
use actix_web::HttpResponse;

use crate::error::simple_json_error_response::simple_json_error_response;

/// Common server errors that should handle 90% of cases.
#[derive(Debug, Serialize)]
pub enum CommonServerError {
  BadInput,
  BadInputWithReason(String),
  NotAuthorized,
  NotFound,
  ServerError,
  ServerErrorWithReason(String),
}

impl ResponseError for CommonServerError {
  fn status_code(&self) -> StatusCode {
    match *self {
      Self::BadInput => StatusCode::BAD_REQUEST,
      Self::BadInputWithReason(_) => StatusCode::BAD_REQUEST,
      Self::NotAuthorized => StatusCode::UNAUTHORIZED,
      Self::NotFound => StatusCode::NOT_FOUND,
      Self::ServerError => StatusCode::INTERNAL_SERVER_ERROR,
      Self::ServerErrorWithReason(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
  }

  fn error_response(&self) -> HttpResponse {
    let error_reason = match self {
      Self::BadInput => "bad input",
      Self::BadInputWithReason(_) => "bad input",
      Self::NotAuthorized => "not authorized",
      Self::NotFound => "not found",
      Self::ServerError => "server error",
      Self::ServerErrorWithReason(_) => "server error",
    };

    simple_json_error_response(error_reason, self.status_code())
  }
}

// NB: Not using derive_more::Display since Clion doesn't understand it.
impl fmt::Display for CommonServerError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}", self)
  }
}
