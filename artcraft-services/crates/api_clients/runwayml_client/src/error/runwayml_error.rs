use crate::error::api_error::ApiError;
use crate::error::client_error::ClientError;
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum RunwayMlError {
  Api(ApiError),
  Client(ClientError),
}

impl Display for RunwayMlError{
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Api(err) => write!(f, "ApiError: {}", err),
      Self::Client(err) => write!(f, "ClientError: {}", err),
    }
  }
}

impl Error for RunwayMlError {}

impl From<ApiError> for RunwayMlError {
  fn from(err: ApiError) -> Self {
    RunwayMlError::Api(err)
  }
}

impl From<ClientError> for RunwayMlError {
  fn from(err: ClientError) -> Self {
    RunwayMlError::Client(err)
  }
}
