use anyhow::anyhow;
use sqlx::MySqlPool;

use errors::AnyhowResult;

pub async fn edit_api_token(
  user_token: &str,
  api_token: &str,
  maybe_short_description: Option<&str>,
  ip_address_update: &str,
  mysql_pool: &MySqlPool
) -> AnyhowResult<bool> {

  let query = sqlx::query!(
        r#"
UPDATE api_tokens
SET
    maybe_short_description = ?,
    ip_address_last_update = ?
WHERE
    user_token = ?
    AND api_token = ?
LIMIT 1
        "#,
      maybe_short_description,
      ip_address_update,
      user_token,
      api_token,
    );

  let result = query.execute(mysql_pool).await;

  match result {
    Err(err) => Err(anyhow!("error with query: {:?}", err)),
    Ok(_r) => Ok(true),
  }
}
