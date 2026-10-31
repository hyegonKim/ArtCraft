use crate::errors::select_exactly_one_error::SelectExactlyOneError;
use std::error::Error;
use std::fmt::{Display, Formatter};
use crate::errors::select_optional_record_error::SelectOptionalRecordError;

#[derive(Debug)]
pub enum WalletSpendError {
  /// Requested an invalid amount to spend.
  InvalidAmountToSpend,
  
  /// Not enough funds to cover the spend
  InsufficientBalance {
    requested_to_spend_amount: u64,
    available_amount: u64,
  },
  
  /// Error selecting the wallet
  SelectError(SelectExactlyOneError),

  /// Error selecting the wallet
  SelectOptionalError(SelectOptionalRecordError),
  
  /// Error updating the wallet
  SqlxError(sqlx::Error),
}

impl Error for WalletSpendError {}


impl Display for WalletSpendError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      WalletSpendError::InvalidAmountToSpend => write!(f, "Invalid amount to spend"),
      WalletSpendError::InsufficientBalance { requested_to_spend_amount, available_amount } => {
        write!(f, "Insufficient balance: requested to spend {}, but only {} available",
          requested_to_spend_amount, available_amount)
      },
      WalletSpendError::SelectError(err) => write!(f, "Error selecting wallet: {}", err),
      WalletSpendError::SelectOptionalError(err) => write!(f, "Error selecting wallet: {}", err),
      WalletSpendError::SqlxError(err) => write!(f, "Database error: {}", err),
    }
  }
}

impl From<SelectExactlyOneError> for WalletSpendError {
  fn from(err: SelectExactlyOneError) -> Self {
    WalletSpendError::SelectError(err)
  }
}

impl From<SelectOptionalRecordError> for WalletSpendError {
  fn from(err: SelectOptionalRecordError) -> Self {
    WalletSpendError::SelectOptionalError(err)
  }
}

impl From<sqlx::Error> for WalletSpendError {
  fn from(err: sqlx::Error) -> Self {
    WalletSpendError::SqlxError(err)
  }
}

