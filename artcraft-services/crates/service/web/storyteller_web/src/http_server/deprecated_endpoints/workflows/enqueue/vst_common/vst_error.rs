use actix_http::StatusCode;
use actix_web::{HttpResponse, ResponseError};
use utoipa::ToSchema;

use crate::http_server::web_utils::response_error_helpers::to_simple_json_error;

#[derive(Debug, ToSchema)]
pub enum VstError {
  BadInput(String),
  NotAuthorized,
  ServerError,
  RateLimited,
}

impl ResponseError for VstError {
  fn status_code(&self) -> StatusCode {
    match *self {
      VstError::BadInput(_) => StatusCode::BAD_REQUEST,
      VstError::NotAuthorized => StatusCode::UNAUTHORIZED,
      VstError::ServerError => StatusCode::INTERNAL_SERVER_ERROR,
      VstError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
    }
  }

  fn error_response(&self) -> HttpResponse {
    let error_reason = match self {
      VstError::BadInput(reason) => reason.to_string(),
      VstError::NotAuthorized => "unauthorized".to_string(),
      VstError::ServerError => "server error".to_string(),
      VstError::RateLimited => "rate limited".to_string(),
    };

    to_simple_json_error(&error_reason, self.status_code())
  }
}

// NB: Not using derive_more::Display since Clion doesn't understand it.
impl std::fmt::Display for VstError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{:?}", self)
  }
}
