use anyhow::anyhow;
use sqlx::MySqlPool;

use errors::AnyhowResult;

pub async fn delete_twitch_event_rule(
  event_rule_token: &str,
  ip_address_delete: &str,
  mysql_pool: &MySqlPool
) -> AnyhowResult<bool> {

  let query = sqlx::query!(
        r#"
UPDATE twitch_event_rules
SET
    deleted_at = CURRENT_TIMESTAMP,
    ip_address_last_update = ?
WHERE
    token = ?
LIMIT 1
        "#,
      ip_address_delete,
      event_rule_token,
    );

  let result = query.execute(mysql_pool).await;

  match result {
    Err(err) => Err(anyhow!("error with query: {:?}", err)),
    Ok(_r) => Ok(true),
  }
}
