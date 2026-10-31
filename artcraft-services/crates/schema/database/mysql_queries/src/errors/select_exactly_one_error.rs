use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum SelectExactlyOneError {
  NotFound,
  DatabaseError(sqlx::Error),
}

impl Error for SelectExactlyOneError {}

impl Display for SelectExactlyOneError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      SelectExactlyOneError::NotFound => write!(f, "Record not found"),
      SelectExactlyOneError::DatabaseError(e) => write!(f, "Database error: {}", e),
    }
  }
}

impl From<sqlx::Error> for SelectExactlyOneError {
  fn from(e: sqlx::Error) -> Self {
    match e {
      sqlx::Error::RowNotFound => return SelectExactlyOneError::NotFound,
      _ => {} // Fall-through
    }
    SelectExactlyOneError::DatabaseError(e)
  }
}
