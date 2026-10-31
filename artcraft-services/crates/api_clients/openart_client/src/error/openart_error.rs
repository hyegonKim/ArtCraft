use crate::error::api_error::ApiError;
use crate::error::client_error::ClientError;
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum OpenArtError {
  Api(ApiError),
  Client(ClientError),
}

impl Display for OpenArtError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Api(err) => write!(f, "ApiError: {}", err),
      Self::Client(err) => write!(f, "ClientError: {}", err),
    }
  }
}

impl Error for OpenArtError {}

impl From<ApiError> for OpenArtError {
  fn from(err: ApiError) -> Self {
    OpenArtError::Api(err)
  }
}

impl From<ClientError> for OpenArtError {
  fn from(err: ClientError) -> Self {
    OpenArtError::Client(err)
  }
}
