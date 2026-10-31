use crate::errors::mysql_error::{MysqlCrateErrorSubtype, MysqlError};
use log::error;
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum UpsertError {
  /// A duplicate key error occurred.
  DuplicateKeyError(String),

  /// An uncategorized error occurred.
  SqlxError(sqlx::Error),
}

impl Error for UpsertError {}

impl MysqlCrateErrorSubtype for UpsertError {}

impl Display for UpsertError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      UpsertError::DuplicateKeyError(reason) => write!(f, "DuplicateKeyError: {}", reason),
      UpsertError::SqlxError(e) => write!(f, "SQLx error: {:?}", e),
    }
  }
}

impl From<sqlx::Error> for UpsertError {
  fn from(err: sqlx::Error) -> Self {
    if let Some(db_err) = err.as_database_error() {
      // NB: SQLSTATE[23000]: Integrity constraint violation
      // NB: MySQL Error Code 1062: Duplicate key insertion (this is harder to access)
      let is_integrity_violation = db_err.code().as_deref() == Some("23000");
      let is_duplicate_key = db_err.message().contains("Duplicate entry");

      if is_integrity_violation && is_duplicate_key {
        error!("Detected duplicate key error: {:?}", db_err);
        return Self::DuplicateKeyError(db_err.message().to_string());
      }
    }

    Self::SqlxError(err)
  }
}

impl From<sqlx::Error> for MysqlError<UpsertError> {
  fn from(err: sqlx::Error) -> Self {
    let inner = UpsertError::from(err);
    MysqlError { inner }
  }
}
