use anyhow::anyhow;
use sqlx;
use sqlx::MySqlPool;

use errors::AnyhowResult;
use tokens::tokens::api_tokens_external::ApiTokenExternal;
use tokens::tokens::api_tokens_internal::ApiTokenInternal;

use crate::queries::api_tokens::list_available_api_tokens_for_user::list_available_api_tokens_for_user;

/// Create a new API token for the user.
/// If the user has more than five tokens, delete the least recent.
pub async fn create_api_token_for_user(
  user_token: &str,
  uuid_idempotency_token: &str,
  creator_ip_address: &str,
  mysql_pool: &MySqlPool
) -> AnyhowResult<String> {

  // TODO: Database transaction.

  let internal_token = ApiTokenInternal::generate().to_string();
  let api_token = ApiTokenExternal::generate().to_string();

  let query = sqlx::query!(
        r#"
INSERT INTO api_tokens
SET
  internal_token = ?,
  api_token = ?,
  user_token = ?,
  uuid_idempotency_token = ?,
  ip_address_creation = ?,
  ip_address_last_update = ?
        "#,
      internal_token,
      api_token,
      user_token,
      uuid_idempotency_token,
      creator_ip_address,
      creator_ip_address
    );

  let query_result = query.execute(mysql_pool).await;

  let _record_id = match query_result {
    Ok(res) => res.last_insert_id(),
    Err(err) => return Err(anyhow!("Error creating API token: {:?}", err)),
  };

  // Delete older tokens.
  let valid_api_tokens= list_available_api_tokens_for_user(user_token, mysql_pool).await?;

  let valid_api_tokens = valid_api_tokens.into_iter()
      .map(|r| r.api_token)
      .collect::<Vec<String>>();

  // TODO: This is a massive hack since SQLx doesn't support binding Vec<T>
  // This array's elements will be replaced with any valid tokens.
  let mut api_tokens = [
    api_token.as_str(),
    api_token.as_str(),
    api_token.as_str(),
    api_token.as_str(),
    api_token.as_str(),
    api_token.as_str(), // One extra copy of the latest token.
  ];

  for i in 0..5 {
    if let Some(token) = valid_api_tokens.get(i).map(|s| s.as_str()) {
      api_tokens[i] = token;
    }
  }

  let query = sqlx::query!(
        r#"
UPDATE api_tokens
SET
  deleted_at = CURRENT_TIMESTAMP,
  ip_address_last_update = ?
WHERE
  user_token = ?
AND api_token NOT IN (?, ?, ?, ?, ?)
        "#,
      creator_ip_address,
      user_token,
      &api_tokens[0],
      &api_tokens[1],
      &api_tokens[2],
      &api_tokens[3],
      &api_tokens[4],
    );

  let _query_result = query.execute(mysql_pool).await?;

  Ok(api_token)
}