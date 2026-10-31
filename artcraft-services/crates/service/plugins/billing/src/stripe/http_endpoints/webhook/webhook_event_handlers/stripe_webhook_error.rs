//! Returned by the webhook endpoint, but also dispatched event handler functions.

use std::fmt;

use actix_web::{HttpResponse, ResponseError};
use actix_web::http::StatusCode;

use http_server_common::response::serialize_as_json_error::serialize_as_json_error;

#[derive(Debug, Serialize)]
pub enum StripeWebhookError {
  BadRequest(String),
  ServerError(String),
}

impl ResponseError for StripeWebhookError {
  fn status_code(&self) -> StatusCode {
    match self {
      StripeWebhookError::BadRequest(String) => StatusCode::BAD_REQUEST,
      StripeWebhookError::ServerError(String) => StatusCode::INTERNAL_SERVER_ERROR,
    }
  }

  fn error_response(&self) -> HttpResponse {
    serialize_as_json_error(self)
  }
}

// NB: Not using derive_more::Display since Clion doesn't understand it.
impl fmt::Display for StripeWebhookError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}", self)
  }
}

