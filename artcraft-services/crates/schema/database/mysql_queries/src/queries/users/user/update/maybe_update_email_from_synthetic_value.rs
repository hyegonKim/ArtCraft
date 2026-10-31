use crate::helpers::boolean_converters::i8_to_bool;
use crate::helpers::transform_optional_result::transform_optional_result;
use anyhow::anyhow;
use errors::AnyhowResult;
use sqlx::pool::PoolConnection;
use sqlx::{Executor, MySql};
use std::fmt::Display;
use tokens::tokens::users::UserToken;

pub struct MaybeUpdateEmailFromSyntheticValueArgs<'a> {
  pub user_token: &'a UserToken,
  pub email_address: &'a str,
  pub email_gravatar_hash: &'a str,
  pub mysql_connection: &'a mut PoolConnection<MySql>,
}
pub async fn maybe_update_email_from_synthetic_value(
  args: MaybeUpdateEmailFromSyntheticValueArgs<'_>,
)  -> AnyhowResult<()> {
  let result = sqlx::query_as!(
    UserEmailCheckRecordRaw,
        r#"
SELECT
  email_address,
  email_is_synthetic
FROM users
WHERE token = ?
LIMIT 1
        "#,
        args.user_token.as_str(),
    )
      .fetch_one(&mut **args.mysql_connection)
      .await;

  let maybe_record = transform_optional_result(result)?;

  let record = match maybe_record {
    Some(record) => record,
    None => return Err(anyhow!("User not found for token: {}", args.user_token)),
  };

  let record : UserEmailCheckRecord = record.into();

  if !record.email_is_synthetic {
    return Ok(());
  }

  let record_email = record.email_address.to_lowercase();

  // NB: So we don't get the format de-synced, let's just trust the boolean flag.
  //if !record_email.contains("synthetic") {
  //  return Ok(());
  //}

  // Normally we don't have much business logic in this crate, but let's be extra careful since we don't have a change log.
  if record_email.contains("gmail")
      || record_email.contains("aol.com")
      || record_email.contains("apple")
      || record_email.contains("att.net")
      || record_email.contains("comcast")
      || record_email.contains("earthlink")
      || record_email.contains("hotmail")
      || record_email.contains("icloud")
      || record_email.contains("live.com")
      || record_email.contains("mac.com")
      || record_email.contains("mail.ru")
      || record_email.contains("me.com")
      || record_email.contains("outlook")
      || record_email.contains("proton")
      || record_email.contains("rocketmail")
      || record_email.contains("sbcglobal")
      || record_email.contains("yahoo")
      || record_email.contains("ymail")
      || record_email.contains("zoho")
  {
    // NB: Just to super extra careful, in case we change the synthetic email format.
    // This obviously won't catch everything.
    return Ok(());
  }
  
  let update_email = args.email_address.to_lowercase();

  let query = sqlx::query!(
      r#"
UPDATE users
SET
  email_address = ?,
  email_gravatar_hash = ?,
  email_confirmed = false,
  email_confirmed_by_google = false,
  email_is_synthetic = false,
  version = version + 1
WHERE
  token = ?
LIMIT 1
        "#,
    update_email,
    args.email_gravatar_hash,
    args.user_token.as_str(),
  );

  args.mysql_connection.execute(query).await?;

  Ok(())
}

struct UserEmailCheckRecordRaw {
  email_address: String,
  email_is_synthetic: i8,
}

struct UserEmailCheckRecord {
  email_address: String,
  email_is_synthetic: bool,
}

impl From<UserEmailCheckRecordRaw> for UserEmailCheckRecord {
  fn from(raw: UserEmailCheckRecordRaw) -> Self {
    Self {
      email_address: raw.email_address,
      email_is_synthetic: i8_to_bool(raw.email_is_synthetic),
    }
  }
}
