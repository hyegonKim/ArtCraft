use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum SelectOptionalRecordError {
  DatabaseError(sqlx::Error),
  
  /// Manually constructed error from mapping raw results into a final struct
  /// Specifically, this results from manually mapping an Option<T> to T, where T should have been non-nullable, 
  /// but sqlx couldn't enforce that at compile time.
  RequiredFieldWasNull(&'static str),
}

impl Error for SelectOptionalRecordError {}

impl Display for SelectOptionalRecordError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      SelectOptionalRecordError::DatabaseError(e) => write!(f, "Database error: {}", e),
      SelectOptionalRecordError::RequiredFieldWasNull(field_name) => write!(f, "Required field was null: {}", field_name),
    }
  }
}

impl From<sqlx::Error> for SelectOptionalRecordError {
  fn from(e: sqlx::Error) -> Self {
    SelectOptionalRecordError::DatabaseError(e)
  }
}
