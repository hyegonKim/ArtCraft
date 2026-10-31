use crate::utils::transactor::Transactor;
use sqlx;
use sqlx::Error::Database;
use std::error::Error;
use std::fmt::{Display, Formatter};
use tokens::tokens::users::UserToken;

pub struct UpdateUsernameArgs<'e, 't> {
  pub token: &'e UserToken,

  pub username: &'e str,
  pub display_name: &'e str,

  pub username_is_not_customized: bool,
  pub ip_address: &'e str,

  pub transactor: Transactor<'e, 't>,
}

#[derive(Debug)]
pub enum UpdateUsernameError{
  UsernameIsTaken,
  DatabaseError { source: sqlx::Error },
}

impl Error for UpdateUsernameError {}

impl Display for UpdateUsernameError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      UpdateUsernameError::UsernameIsTaken => {
        write!(f, "UpdateUsernameError: username is taken")
      }
      UpdateUsernameError::DatabaseError { source } => {
        write!(f, "UpdateUsernameError: database error: {:?}", source)
      }
    }
  }
}

pub async fn update_username<'e, 't>(
  args: UpdateUsernameArgs<'e, 't>
) -> Result<(), UpdateUsernameError>
{
  let query = sqlx::query!(
      r#"
UPDATE users
SET
  username = ?,
  display_name = ?,
  username_is_not_customized = ?,
  ip_address_last_update = ?,
  version = version + 1
WHERE
  token = ?
LIMIT 1
        "#,
    args.username,
    args.display_name,
    args.username_is_not_customized,
    args.ip_address,
    args.token,
  );

  let query_result = args.transactor.execute(query).await;

  match query_result {
    Ok(_) => Ok(()),
    Err(Database(err)) => {
      let maybe_code = err.code()
          .map(|c| c.into_owned());

      if maybe_code.as_deref() == Some("23000")
          && err.message().contains("username")
      {
        return Err(UpdateUsernameError::UsernameIsTaken);
      }

      Err(UpdateUsernameError::DatabaseError { source: Database(err) })
    },
    Err(err) => {
      Err(UpdateUsernameError::DatabaseError { source: err })
    },
  }
}
