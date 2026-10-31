use crate::helpers::transform_optional_result::transform_optional_result;
use crate::queries::users::user::get::lookup_user_for_login_result::{UserRecordForLogin, UserRecordForLoginRaw};
use crate::utils::transactor::Transactor;
use errors::AnyhowResult;

pub async fn lookup_user_for_login_by_email_with_transactor(
  email: &str,
  mut transactor: Transactor<'_, '_>,
) -> AnyhowResult<Option<UserRecordForLogin>> {

  let query = sqlx::query_as!(
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
    );

  let result = match transactor {
    Transactor::Pool { pool } => {
      query.fetch_one(pool).await
    },
    Transactor::Connection { connection } => {
      query.fetch_one(connection).await
    },
    Transactor::Transaction { transaction } => {
      query.fetch_one(&mut **transaction).await
    },
  };

  let maybe_record = transform_optional_result(result)?;

  Ok(maybe_record.map(|record|record.into()))
}
