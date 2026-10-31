use anyhow::anyhow;
use sqlx::MySqlPool;

use errors::AnyhowResult;

// TODO: This doesn't handle sparse updates.

pub struct UpdateTwitchEventRuleBuilder {
  pub token: String,
  pub event_match_predicate: String,
  pub event_response: String,
  pub rule_is_disabled: bool,
  pub ip_address_update: String,
}

impl UpdateTwitchEventRuleBuilder {

  pub async fn update(&self, mysql_pool: &MySqlPool) -> AnyhowResult<()> {

    let query = sqlx::query!(
        r#"
UPDATE twitch_event_rules
SET
  event_match_predicate = ?,
  event_response = ?,
  rule_is_disabled = ?,
  ip_address_last_update = ?
WHERE
  token = ?
  AND deleted_at IS NULL
LIMIT 1
        "#,
      &self.event_match_predicate,
      &self.event_response,
      &self.rule_is_disabled,
      &self.ip_address_update,
      &self.token,
    );

    let query_result = query.execute(mysql_pool).await;

    match query_result {
      Err(err) => Err(anyhow!("Error updating Twitch Event Rule: {:?}", err)),
      Ok(_r) => Ok(()),
    }
  }
}
