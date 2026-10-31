use anyhow::anyhow;
use chrono::{DateTime, Utc};
use log::warn;
use sqlx::MySqlPool;

use errors::AnyhowResult;

#[derive(Serialize)]
pub struct IpBanRecordForList {
  pub ip_address: String,
  pub maybe_target_user_token: Option<String>,
  pub maybe_target_username: Option<String>,
  pub mod_user_token: String,
  pub mod_username: String,
  pub mod_display_name: String,
  pub mod_notes: String,
  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
}

// NB: This is unconstrained because I don't anticipate a huge number of IP bans at launch
// We can reengineer with growth.
pub async fn list_ip_bans(mysql_pool: &MySqlPool) -> AnyhowResult<Vec<IpBanRecordForList>> {
  let maybe_results = sqlx::query_as!(
      IpBanRecordForList,
        r#"
SELECT
    ip_bans.ip_address,

    ip_bans.maybe_target_user_token,
    banned_users.username as maybe_target_username,

    ip_bans.mod_user_token,
    mod_users.username as mod_username,
    mod_users.display_name as mod_display_name,

    ip_bans.mod_notes,
    ip_bans.created_at,
    ip_bans.updated_at
FROM
    ip_address_bans AS ip_bans
LEFT OUTER JOIN users as banned_users
    ON ip_bans.maybe_target_user_token = banned_users.token
JOIN users as mod_users
    ON ip_bans.mod_user_token = mod_users.token
WHERE
    ip_bans.deleted_at IS NULL
        "#,
    )
      .fetch_all(mysql_pool)
      .await;

  match maybe_results {
    Err(err) => {
      match err {
        sqlx::Error::RowNotFound => {
          Ok(Vec::new())
        },
        _ => {
          warn!("list ip bans db error: {:?}", err);
          Err(anyhow!("error with query: {:?}", err))
        }
      }
    },
    Ok(results) => Ok(results),
  }
}
