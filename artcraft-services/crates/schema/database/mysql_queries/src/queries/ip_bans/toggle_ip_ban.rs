use anyhow::anyhow;
use sqlx::MySqlPool;

use errors::AnyhowResult;

// NB: This is "toggle" instead of discrete functions for create/delete because this is
// how old code was written and I'm lazy / in a hurry.
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum IpBanToggleState {
  CreateIpBan,
  DeleteIpBan,
}

pub async fn toggle_ip_ban(
  ip_address: &str,
  mod_user_token: &str,
  ban_state: IpBanToggleState,
  mysql_pool: &MySqlPool
) -> AnyhowResult<()> {

  let query_result = match ban_state {
    IpBanToggleState::DeleteIpBan => {
      sqlx::query!(
        r#"
UPDATE
    ip_address_bans
SET
    mod_user_token = ?,
    deleted_at = CURRENT_TIMESTAMP
WHERE
    ip_address = ?
LIMIT 1
        "#,
        &mod_user_token,
        &ip_address,
      )
          .execute(mysql_pool)
          .await
    },
    IpBanToggleState::CreateIpBan => {
      sqlx::query!(
        r#"
UPDATE
    ip_address_bans
SET
    mod_user_token = ?,
    deleted_at = NULL
WHERE
    ip_address = ?
LIMIT 1
        "#,
        &mod_user_token,
        &ip_address,
      )
          .execute(mysql_pool)
          .await
    }
  };

  match query_result {
    Ok(_) => Ok(()),
    Err(err) => Err(anyhow!("Error creating or deleting ip ban: {:?}", err)),
  }
}
