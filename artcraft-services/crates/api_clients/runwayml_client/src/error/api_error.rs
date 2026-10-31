use jwt_light::error::JwtError;
use reqwest::StatusCode;
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum ApiError {
  /// Inability to parse JWT.
  JwtError(JwtError),

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
      ApiError::JwtError(err) => write!(f, "JwtError: {}", err),
      ApiError::ReqwestError(err) => write!(f, "ReqwestError: {}", err),
      ApiError::UncategorizedBadResponse { status_code, message } => {
        write!(f, "UncategorizedBadResponse: {:?} : {}", status_code, message)
      }
    }
  }
}

impl Error for ApiError {}

impl From<JwtError> for ApiError {
  fn from(err: JwtError) -> Self {
    ApiError::JwtError(err)
  }
}
