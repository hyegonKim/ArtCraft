use anyhow::anyhow;
use sqlx::MySqlPool;

use errors::AnyhowResult;

pub struct UpsertIpBanArgs<'a> {
  pub ip_address: &'a str,
  pub maybe_target_user_token: Option<&'a str>,
  pub mod_user_token: &'a str,
  pub mod_notes: &'a str,
  pub mysql_pool: &'a MySqlPool,
}

pub async fn upsert_ip_ban(args: UpsertIpBanArgs<'_>) -> AnyhowResult<()> {
  let query_result = sqlx::query!(
        r#"
INSERT INTO
    ip_address_bans
SET
    ip_address = ?,
    maybe_target_user_token = ?,
    mod_user_token = ?,
    mod_notes = ?
ON DUPLICATE KEY UPDATE
    expires_at = NULL,
    deleted_at = NULL,
    ip_address = ?,
    maybe_target_user_token = ?,
    mod_user_token = ?,
    mod_notes = ?
        "#,
      // Insert
      args.ip_address,
      args.maybe_target_user_token,
      args.mod_user_token,
      args.mod_notes,
      // Update
      args.ip_address,
      args.maybe_target_user_token,
      args.mod_user_token,
      args.mod_notes,
    )
      .execute(args.mysql_pool)
      .await;

  match query_result {
    Ok(_) => Ok(()),
    Err(err) => {
      Err(anyhow!("error with query: {:?}", err))
    }
  }
}
