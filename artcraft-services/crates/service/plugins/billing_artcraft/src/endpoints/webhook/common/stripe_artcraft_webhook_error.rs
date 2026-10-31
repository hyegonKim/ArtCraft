//! Returned by the webhook endpoint, but also dispatched event handler functions.

use std::error::Error;
use std::fmt;

use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError};
use http_server_common::response::serialize_as_json_error::serialize_as_json_error;
use serde_derive::Serialize;

#[derive(Debug, Serialize)]
pub enum StripeArtcraftWebhookError {
  BadRequest(String),
  ServerError(String),
}

impl Error for StripeArtcraftWebhookError {}

impl ResponseError for StripeArtcraftWebhookError {
  fn status_code(&self) -> StatusCode {
    match self {
      StripeArtcraftWebhookError::BadRequest(String) => StatusCode::BAD_REQUEST,
      StripeArtcraftWebhookError::ServerError(String) => StatusCode::INTERNAL_SERVER_ERROR,
    }
  }

  fn error_response(&self) -> HttpResponse {
    serialize_as_json_error(self)
  }
}

// NB: Not using derive_more::Display since Clion doesn't understand it.
impl fmt::Display for StripeArtcraftWebhookError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}", self)
  }
}

impl From<sqlx::Error> for StripeArtcraftWebhookError {
  fn from(err: sqlx::Error) -> Self {
    StripeArtcraftWebhookError::ServerError(format!("SQLX Error: {:?}", err))
  }
}

impl From<anyhow::Error> for StripeArtcraftWebhookError {
  fn from(err: anyhow::Error) -> Self {
    StripeArtcraftWebhookError::ServerError(format!("Anyhow Error: {:?}", err))
  }
}
