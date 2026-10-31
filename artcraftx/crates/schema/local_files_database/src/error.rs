use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum LocalFilesDbError {
  SqlxError(sqlx::Error),
}

impl Error for LocalFilesDbError {}

impl Display for LocalFilesDbError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      LocalFilesDbError::SqlxError(err) => write!(f, "SQLx error: {:?}", err),
    }
  }
}

impl From<sqlx::Error> for LocalFilesDbError {
  fn from(err: sqlx::Error) -> Self {
    LocalFilesDbError::SqlxError(err)
  }
}
