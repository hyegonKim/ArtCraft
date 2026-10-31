use chrono::Utc;
use sqlx::MySqlPool;

use errors::AnyhowResult;

use crate::column_types::twitch_event_category::TwitchEventCategory;
use crate::helpers::boolean_converters::i8_to_bool;

#[derive(Debug, Serialize)]
pub struct TwitchEventRule {
  pub token: String,
  pub event_category: TwitchEventCategory,
  pub event_match_predicate: String,
  pub event_response: String,
  pub user_specified_rule_order: u32,
  pub rule_is_disabled: bool,
  pub created_at: chrono::DateTime<Utc>,
  pub updated_at: chrono::DateTime<Utc>,
}

/// Query non-deleted Twitch event rules for a user
pub async fn list_twitch_event_rules_for_user(
  user_token: &str,
  pool: &MySqlPool,
) -> AnyhowResult<Vec<TwitchEventRule>> {

  let records : Vec<TwitchEventRuleInternal> = sqlx::query_as!(
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
  updated_at
FROM twitch_event_rules
WHERE
  user_token = ?
  AND deleted_at IS NULL
        "#,
      user_token,
    )
      .fetch_all(pool)
      .await?;

  let mut records = records.into_iter()
      .map(|record : TwitchEventRuleInternal | {
        TwitchEventRule {
          token: record.token,
          event_category: record.event_category,
          event_match_predicate: record.event_match_predicate,
          event_response: record.event_response,
          user_specified_rule_order: record.user_specified_rule_order,
          rule_is_disabled: i8_to_bool(record.rule_is_disabled),
          created_at: record.created_at,
          updated_at: record.updated_at,
        }
      })
      .collect::<Vec<TwitchEventRule>>();

  // Queried in DESC order, but sort returned results ordered ASC.
  records.sort_by_key(|r| r.user_specified_rule_order);

  Ok(records)
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
}
