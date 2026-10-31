use std::fmt;

/// Error converting a string into an enum variant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EnumError {
  CouldNotConvertFromString(String),
}

impl fmt::Display for EnumError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::CouldNotConvertFromString(value) => {
        write!(f, "could not convert string to enum: {value:?}")
      }
    }
  }
}

impl std::error::Error for EnumError {}
