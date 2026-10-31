use chrono::Utc;
use sqlx::MySqlPool;

use errors::AnyhowResult;

/// table: api_tokens
#[derive(Debug)]
pub struct AvailableApiToken {
  pub internal_token: String,
  pub api_token: String,
  pub maybe_short_description: Option<String>,
  pub created_at: chrono::DateTime<Utc>,
  pub updated_at: chrono::DateTime<Utc>,
}

/// Query available (non-deleted) API tokens for a user.
/// A user can only have five active tokens at a time, so we only return the five most recent.
pub async fn list_available_api_tokens_for_user(
  user_token: &str,
  pool: &MySqlPool,
) -> AnyhowResult<Vec<AvailableApiToken>> {

  let records : Vec<AvailableApiTokenInternal> = sqlx::query_as!(
      AvailableApiTokenInternal,
        r#"
SELECT
  internal_token,
  api_token,
  maybe_short_description,
  created_at,
  updated_at
FROM api_tokens
WHERE
  user_token = ?
  AND deleted_at IS NULL
ORDER BY created_at DESC
LIMIT 5
        "#,
      user_token,
    )
      .fetch_all(pool)
      .await?;

  let mut records = records.into_iter()
      .map(|record : AvailableApiTokenInternal | {
        AvailableApiToken {
          internal_token: record.internal_token,
          api_token: record.api_token,
          maybe_short_description: record.maybe_short_description,
          created_at: record.created_at,
          updated_at: record.updated_at,
        }
      })
      .collect::<Vec<AvailableApiToken>>();

  // Queried in DESC order, but sort returned results ordered ASC.
  records.sort_by_key(|r| r.created_at);

  Ok(records)
}

#[derive(Debug)]
struct AvailableApiTokenInternal {
  pub internal_token: String,
  pub api_token: String,
  pub maybe_short_description: Option<String>,
  pub created_at: chrono::DateTime<Utc>,
  pub updated_at: chrono::DateTime<Utc>,
}
