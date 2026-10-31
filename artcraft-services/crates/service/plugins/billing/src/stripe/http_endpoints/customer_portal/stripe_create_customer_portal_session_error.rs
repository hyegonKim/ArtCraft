use actix_http::StatusCode;
use actix_web::{HttpResponse, ResponseError};

use http_server_common::response::serialize_as_json_error::serialize_as_json_error;

#[derive(Debug, Serialize, Eq, PartialEq, Copy, Clone)]
pub enum CreateCustomerPortalSessionError {
  BadRequest,
  InvalidSession,
  ServerError,
  StripeError,
}

impl ResponseError for CreateCustomerPortalSessionError {
  fn status_code(&self) -> StatusCode {
    match *self {
      CreateCustomerPortalSessionError::BadRequest => StatusCode::BAD_REQUEST,
      CreateCustomerPortalSessionError::InvalidSession => StatusCode::UNAUTHORIZED,
      CreateCustomerPortalSessionError::ServerError => StatusCode::INTERNAL_SERVER_ERROR,
      CreateCustomerPortalSessionError::StripeError => StatusCode::INTERNAL_SERVER_ERROR,
    }
  }

  fn error_response(&self) -> HttpResponse {
    serialize_as_json_error(self)
  }
}

// NB: Not using derive_more::Display since Clion doesn't understand it.
impl std::fmt::Display for CreateCustomerPortalSessionError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{:?}", self)
  }
}
