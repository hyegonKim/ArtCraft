use core_types::enums::enum_error::EnumError as CoreEnumError;
use sqlite_identifiers::enums::enum_error::EnumError;
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum SqliteTasksError {
  SqlxError(sqlx::Error),
  EnumError(EnumError),
  //TaskNotFound,
  //TaskAlreadyExists,
  //InvalidTaskStatus,
  //InvalidTaskType,
  //InvalidGenerationProvider,
}

impl Error for SqliteTasksError {}

impl Display for SqliteTasksError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      SqliteTasksError::SqlxError(err) => write!(f, "SQLx error: {:?}", err),
      SqliteTasksError::EnumError(err) => write!(f, "Error parsing enum: {:?}", err),
    }
  }
}

impl From<sqlx::Error> for SqliteTasksError {
  fn from(err: sqlx::Error) -> Self {
    SqliteTasksError::SqlxError(err)
  }
}

impl From<EnumError> for SqliteTasksError {
  fn from(err: EnumError) -> Self {
    SqliteTasksError::EnumError(err)
  }
}

impl From<CoreEnumError> for SqliteTasksError {
  fn from(err: CoreEnumError) -> Self {
    let CoreEnumError::CouldNotConvertFromString(value) = err;
    SqliteTasksError::EnumError(EnumError::CouldNotConvertFromString(value))
  }
}
