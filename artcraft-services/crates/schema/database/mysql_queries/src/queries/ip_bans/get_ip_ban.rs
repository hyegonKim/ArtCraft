use anyhow::anyhow;
use chrono::{DateTime, Utc};
use sqlx::MySqlPool;

use errors::AnyhowResult;

#[derive(Serialize)]
pub struct IpBanRecord {
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

pub async fn get_ip_ban(ip_address: &str, mysql_pool: &MySqlPool) -> AnyhowResult<Option<IpBanRecord>> {
  // NB: Lookup failure is Err(RowNotFound).
  let maybe_result = sqlx::query_as!(
      IpBanRecord,
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
    ip_bans.ip_address = ?
LIMIT 1
        "#,
      ip_address
    )
      .fetch_one(mysql_pool)
      .await;

  match maybe_result {
    Ok(result) => Ok(Some(result)),
    Err(err) => match err {
      sqlx::Error::RowNotFound => Ok(None),
      _ => Err(anyhow!("Error querying for IP ban: {:?}", err)),
    }
  }
}
