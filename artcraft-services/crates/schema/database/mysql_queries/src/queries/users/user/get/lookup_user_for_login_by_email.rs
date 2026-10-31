use sqlx::MySqlPool;

use crate::helpers::transform_optional_result::transform_optional_result;
use crate::queries::users::user::get::lookup_user_for_login_result::{UserRecordForLogin, UserRecordForLoginRaw};
use errors::AnyhowResult;

pub async fn lookup_user_for_login_by_email(email: &str, pool: &MySqlPool) -> AnyhowResult<Option<UserRecordForLogin>> {
  // NB: Lookup failure is Err(RowNotFound).
  let result = sqlx::query_as!(
    UserRecordForLoginRaw,
        r#"
SELECT
  token as `token: tokens::tokens::users::UserToken`,
  username,
  display_name,
  username_is_not_customized,
  email_address,
  password_hash as `password_hash: crate::queries::users::user::get::lookup_user_for_login_result::VecBytes`,
  password_version,
  is_banned,
  maybe_feature_flags
FROM users
WHERE email_address = ?
LIMIT 1
        "#,
        email.to_string(),
    )
      .fetch_one(pool)
      .await;

  let maybe_record = transform_optional_result(result)?;

  Ok(maybe_record.map(|record|record.into()))
}
