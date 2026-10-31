use anyhow::anyhow;
use sqlx::MySqlPool;

use errors::AnyhowResult;
use tokens::tokens::users::UserToken;

pub async fn set_user_role(
  user_token: &UserToken,
  user_role_slug: &str,
  mysql_pool: &MySqlPool
) -> AnyhowResult<()> {

  let query_result = sqlx::query!(
        r#"
UPDATE users
SET
  user_role_slug = ?
WHERE
  token = ?
LIMIT 1
        "#,
      user_role_slug,
      user_token,
    )
      .execute(mysql_pool)
      .await;

  match query_result {
    Ok(_) => Ok(()),
    Err(err) => Err(anyhow!("couldn't update user role: {:?}", err))
  }
}
