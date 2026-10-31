use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum ClientError {
  /// No cookies in credentials
  NoCookiesInCredentials,
  
  /// No session in the credentials
  NoSessionInfoInCredentials,
  
  /// An error that occurred clientside before the request was sent.
  ReqwestError(reqwest::Error),
  
  /// An I/O error, e.g. reading from a file or network stream.
  IoError(std::io::Error),
  
  /// Another error that does not fit the above categories.
  Other(String),
}

impl Display for ClientError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::NoCookiesInCredentials => write!(f, "No cookies are present in the credentials"),
      Self::NoSessionInfoInCredentials => write!(f, "No session in the credentials"),
      Self::ReqwestError(err) => write!(f, "Reqwest error: {}", err),
      Self::IoError(err) => write!(f, "I/O error: {}", err),
      Self::Other(message) => write!(f, "Other client error: {}", message),
    }
  }
}

impl Error for ClientError {}
