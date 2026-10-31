use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use errors::AnyhowError;

#[derive(Debug)]
pub enum MediaFileInsertBuilderError {
  /// A user-provided field is missing (400 error).
  MissingRequiredField(String),

  // TODO: We need to refactor to ensure these are actually DB errors (500s).
  ProbablyDatabaseError(AnyhowError),
}

impl Error for MediaFileInsertBuilderError {}

impl Display for MediaFileInsertBuilderError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      MediaFileInsertBuilderError::MissingRequiredField(reason) => {
        write!(f, "Missing required field: {}", reason)
      }
      MediaFileInsertBuilderError::ProbablyDatabaseError(reason) => {
        write!(f, "Probably database error: {:?}", reason)
      }
    }
  }
}
