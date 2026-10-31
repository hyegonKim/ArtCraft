use std::error::Error;
use std::fmt::{Debug, Display, Formatter};

/// Implement this marker trait for error subtypes.
/// It'll then be useful to implement two From<T> for each subtype:
///  1.  From<sqlx::Error> for MysqlCrateErrorSubtype
///  2.  From<sqlx::Error> for MysqlError<MysqlCrateErrorSubtype>
pub trait MysqlCrateErrorSubtype: Error + Debug + Display {}

/// All crate errors can be wrapped in this type.
/// Query-specific errors can be handled by the subtype.
/// Consumers can match on the wrapper type (eg. for error type 
/// conversion) or subtype (for handling specific error cases).
#[derive(Debug)]
pub struct MysqlError<E: MysqlCrateErrorSubtype> {
  /// Inner error type
  pub inner: E
}

impl <E> Error for MysqlError <E> where E: MysqlCrateErrorSubtype {}

impl <E> Display for MysqlError<E> where E: MysqlCrateErrorSubtype
{
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "MysqlError: {}", self.inner)
  }
}
