use crate::utils::transactor::Transactor;
use errors::AnyhowResult;
use sqlx;
use std::fmt::Display;
use tokens::tokens::users::UserToken;

pub struct UpdatePasswordArgs<'e, 't> {
  pub user_token: &'e UserToken,
  pub password_hash: &'e str,
  pub ip_address: &'e str,
  pub transactor: Transactor<'e, 't>,
}

pub async fn update_password<'e, 't>(
  args: UpdatePasswordArgs<'e, 't>
) -> AnyhowResult<()>
{
  let query = sqlx::query!(
      r#"
UPDATE users
SET
  password_hash = ?,
  ip_address_last_update = ?,
  is_without_password = false,
  password_version = password_version + 1,
  version = version + 1
WHERE
  token = ?
LIMIT 1
        "#,
    args.password_hash,
    args.ip_address,
    args.user_token.as_str(),
  );

  args.transactor.execute(query).await?;

  Ok(())
}
