use reqwest::StatusCode;
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum ApiError {
  /// Session endpoint did not return a valid session id.
  /// Specifically for calls to https://openart.ai/api/auth/session
  InvalidSession,
  CouldNotParseSession { error: serde_json::Error, body: String },

  /// An error that occurred as or after the request was sent.
  ReqwestError(reqwest::Error),

  /// Some other failure response.
  UncategorizedBadResponse{
    status_code: StatusCode,
    message : String,
  }
}

impl Display for ApiError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      ApiError::InvalidSession => write!(f, "Invalid session: no session id returned from the server"),
      ApiError::CouldNotParseSession { error, body } => {
        write!(f, "Could not parse session: {}. Body: {}", error, body)
      }
      ApiError::ReqwestError(err) => write!(f, "ReqwestError: {}", err),
      ApiError::UncategorizedBadResponse { status_code, message } => {
        write!(f, "UncategorizedBadResponse: {:?} : {}", status_code, message)
      },
    }
  }
}

impl Error for ApiError {}
