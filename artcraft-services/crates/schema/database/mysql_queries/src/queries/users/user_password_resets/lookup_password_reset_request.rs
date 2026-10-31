use anyhow::anyhow;
use sqlx::{Error, MySql, MySqlPool, Transaction};

use errors::AnyhowResult;
use tokens::tokens::password_reset::PasswordResetToken;
use tokens::tokens::users::UserToken;

pub struct PasswordResetStateAndTransaction <'a> {
  pub transaction: Transaction<'a, MySql>,
  pub reset_state: PasswordResetLookupInfo,
}

pub struct PasswordResetLookupInfo {
  pub password_reset_token: PasswordResetToken,
  pub user_token: UserToken,
}

/// Begin a lookup transaction.
/// `select for update` ensures the record won't change, blocking race conditions.
pub async fn lookup_password_reset_request<'a>(password_reset_token: &str, pool: &'a MySqlPool) -> AnyhowResult<Option<PasswordResetStateAndTransaction<'a>>> {
  let mut transaction = pool.begin().await?;

  let result = sqlx::query_as!(
    PasswordResetLookupInfo,
        r#"
SELECT
  pw.token as `password_reset_token: tokens::tokens::password_reset::PasswordResetToken`,
  pw.user_token as `user_token: tokens::tokens::users::UserToken`
FROM user_password_resets AS pw
JOIN users AS u
  ON pw.user_token = u.token
WHERE public_reset_token = ?
  AND pw.expires_at > NOW()
  AND pw.current_password_version = u.password_version
  AND NOT pw.is_redeemed
  AND NOT u.is_banned
LIMIT 1
FOR UPDATE
        "#,
        password_reset_token,
    )
      .fetch_one(&mut *transaction)
      .await;

  match result {
    Ok(record) => Ok(Some(PasswordResetStateAndTransaction {
      transaction,
      reset_state: record,
    })),
    Err(err) => match err {
      Error::RowNotFound => Ok(None),
      _ => Err(anyhow!(err)),
    },
  }
}
