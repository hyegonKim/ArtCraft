use anyhow::anyhow;
use sqlx::MySqlPool;

use errors::AnyhowResult;
use tokens::tokens::twitch_event_rule::TwitchEventRuleToken;

use crate::column_types::twitch_event_category::TwitchEventCategory;

pub struct InsertTwitchEventRuleBuilder {
  pub uuid_idempotency_token: String,
  pub user_token: String,
  pub event_category: TwitchEventCategory,
  pub event_match_predicate: String,
  pub event_response: String,
  pub user_specified_rule_order: u32,
  pub rule_is_disabled: bool,
  pub ip_address_creation: String,
}

impl InsertTwitchEventRuleBuilder {

  /// Returns the newly generated token.
  pub async fn insert(&self, mysql_pool: &MySqlPool) -> AnyhowResult<String> {

    let token = TwitchEventRuleToken::generate().to_string();

    let query = sqlx::query!(
        r#"
INSERT INTO twitch_event_rules
SET
  uuid_idempotency_token = ?,
  token = ?,
  user_token = ?,
  event_category = ?,
  event_match_predicate = ?,
  event_response = ?,
  user_specified_rule_order = ?,
  rule_is_disabled = ?,
  ip_address_creation = ?,
  ip_address_last_update = ?
        "#,
      &self.uuid_idempotency_token,
      &token,
      &self.user_token,
      &self.event_category.to_str(),
      &self.event_match_predicate,
      &self.event_response,
      &self.user_specified_rule_order,
      &self.rule_is_disabled,
      &self.ip_address_creation,
      &self.ip_address_creation,
    );

    let query_result = query.execute(mysql_pool).await;

    let _record_id = match query_result {
      Ok(res) => res.last_insert_id(),
      Err(err) => return Err(anyhow!("Error creating Twitch Event Rule: {:?}", err)),
    };

    Ok(token)
  }
}
