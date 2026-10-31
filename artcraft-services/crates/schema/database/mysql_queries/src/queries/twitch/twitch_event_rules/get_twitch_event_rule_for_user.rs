use anyhow::anyhow;
use chrono::Utc;
use sqlx::MySqlPool;

use errors::AnyhowResult;

use crate::column_types::twitch_event_category::TwitchEventCategory;
use crate::helpers::boolean_converters::i8_to_bool;

#[derive(Debug)]
pub struct TwitchEventRule {
  pub token: String,
  pub event_category: TwitchEventCategory,
  pub event_match_predicate: String,
  pub event_response: String,
  pub user_specified_rule_order: u32,
  pub rule_is_disabled: bool,
  pub created_at: chrono::DateTime<Utc>,
  pub updated_at: chrono::DateTime<Utc>,
  pub deleted_at: Option<chrono::DateTime<Utc>>,
}

/// Query record by token and user token
pub async fn get_twitch_event_rule_for_user(
  twitch_event_rule_token: &str,
  user_token: &str,
  pool: &MySqlPool,
) -> AnyhowResult<Option<TwitchEventRule>> {

  let maybe_record = sqlx::query_as!(
      TwitchEventRuleInternal,
        r#"
SELECT
  token,
  event_category as `event_category: crate::column_types::twitch_event_category::TwitchEventCategory`,
  event_match_predicate,
  event_response,
  user_specified_rule_order,
  rule_is_disabled,
  created_at,
  updated_at,
  deleted_at
FROM twitch_event_rules
WHERE
  token = ?
  AND user_token = ?
        "#,
      twitch_event_rule_token,
      user_token,
    )
      .fetch_one(pool)
      .await;

  let record : TwitchEventRuleInternal = match maybe_record {
    Ok(record) => record,
    Err(sqlx::Error::RowNotFound) => return Ok(None),
    Err(ref err) => return Err(anyhow!("database query error: {:?}", err)),
  };

  Ok(Some(TwitchEventRule {
    token: record.token,
    event_category: record.event_category,
    event_match_predicate: record.event_match_predicate,
    event_response: record.event_response,
    user_specified_rule_order: record.user_specified_rule_order,
    rule_is_disabled: i8_to_bool(record.rule_is_disabled),
    created_at: record.created_at,
    updated_at: record.updated_at,
    deleted_at: record.deleted_at,
  }))
}

#[derive(Debug)]
struct TwitchEventRuleInternal {
  pub token: String,
  pub event_category: TwitchEventCategory,
  pub event_match_predicate: String,
  pub event_response: String,
  pub user_specified_rule_order: u32,
  pub rule_is_disabled: i8,
  pub created_at: chrono::DateTime<Utc>,
  pub updated_at: chrono::DateTime<Utc>,
  pub deleted_at: Option<chrono::DateTime<Utc>>,
}
