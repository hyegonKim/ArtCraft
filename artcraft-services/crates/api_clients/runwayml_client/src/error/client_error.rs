use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum ClientError {
  /// No JWT bearer token in the credentials
  NoJwtBearerToken, 
  
  /// An error that occurred clientside before the request was sent.
  ReqwestError(reqwest::Error),
}

impl Display for ClientError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      ClientError::NoJwtBearerToken => write!(f, "No JWT bearer token in the credentials"),
      ClientError::ReqwestError(err) => write!(f, "Reqwest error: {}", err),
    }
  }
}

impl Error for ClientError {}
