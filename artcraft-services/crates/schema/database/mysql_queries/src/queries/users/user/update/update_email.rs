use crate::utils::transactor::Transactor;
use sqlx;
use sqlx::Error::Database;
use std::error::Error;
use std::fmt::{Display, Formatter};
use tokens::tokens::users::UserToken;

pub struct UpdateEmailArgs<'e, 't> {
  pub token: &'e UserToken,
  pub email_address: &'e str,
  pub email_gravatar_hash: &'e str,
  pub ip_address: &'e str,
  pub transactor: Transactor<'e, 't>,
}

#[derive(Debug)]
pub enum UpdateEmailError {
  EmailIsTaken,
  DatabaseError { source: sqlx::Error },
}

impl Error for UpdateEmailError {}

impl Display for UpdateEmailError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      UpdateEmailError::EmailIsTaken => {
        write!(f, "UpdateEmailError: email is taken")
      }
      UpdateEmailError::DatabaseError { source } => {
        write!(f, "UpdateEmailError: database error: {:?}", source)
      }
    }
  }
}

pub async fn update_email<'e, 't>(
  args: UpdateEmailArgs<'e, 't>
) -> Result<(), UpdateEmailError>
{
  let query = sqlx::query!(
      r#"
UPDATE users
SET
  email_address = ?,
  email_confirmed = false,
  email_confirmed_by_google = false,
  email_is_synthetic = false,
  email_gravatar_hash = ?,
  ip_address_last_update = ?,
  version = version + 1
WHERE
  token = ?
LIMIT 1
        "#,
    args.email_address,
    args.email_gravatar_hash,
    args.ip_address,
    args.token,
  );

  let query_result = args.transactor.execute(query).await;

  match query_result {
    Ok(_) => Ok(()),
    Err(Database(err)) => {
      let maybe_code = err.code()
          .map(|c| c.into_owned());

      // MySQL error code 23000 is a duplicate key constraint violation
      if maybe_code.as_deref() == Some("23000")
          && err.message().contains("email_address")
      {
        return Err(UpdateEmailError::EmailIsTaken);
      }

      Err(UpdateEmailError::DatabaseError { source: Database(err) })
    },
    Err(err) => {
      Err(UpdateEmailError::DatabaseError { source: err })
    },
  }
}
