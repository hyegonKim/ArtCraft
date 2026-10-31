use anyhow::anyhow;
use chrono::{DateTime, Utc};
use sqlx::MySqlPool;

use errors::AnyhowResult;

#[derive(Serialize)]
pub struct EventRecord {
  pub event_token: String,
  pub event_type: String,
  pub maybe_target_user_token: Option<String>,
  pub maybe_target_username: Option<String>,
  pub maybe_target_display_name: Option<String>,
  pub maybe_target_user_gravatar_hash: Option<String>,
  pub maybe_target_entity_token: Option<String>,
  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
}

pub async fn list_public_event_feed_items(mysql_pool: &MySqlPool) -> AnyhowResult<Vec<EventRecord>> {
  // NB: Lookup failure is Err(RowNotFound).
  // NB: Since this is publicly exposed, we don't query sensitive data.
  let maybe_events = sqlx::query_as!(
      EventRecord,
        r#"
SELECT
    events.token as event_token,
    events.event_type,
    events.maybe_target_user_token,
    users.username as maybe_target_username,
    users.display_name as maybe_target_display_name,
    users.email_gravatar_hash as maybe_target_user_gravatar_hash,
    events.maybe_target_entity_token,
    events.created_at,
    events.updated_at
FROM firehose_entries as events
LEFT OUTER JOIN users
ON events.maybe_target_user_token = users.token
ORDER BY events.id DESC
LIMIT 25
        "#,
    )
      .fetch_all(mysql_pool)
      .await;

  match maybe_events {
    Ok(events) => Ok(events),
    Err(err) => match err {
      sqlx::Error::RowNotFound => Ok(Vec::new()),
      _ => {
        Err(anyhow!("error querying for events: {:?}", err))
      }
    }
  }
}
