use sqlx::MySqlPool;

use errors::AnyhowResult;

pub async fn delete_tts_model_as_user(
  tts_model_token: &str,
  creator_ip_address: &str,
  mysql_pool: &MySqlPool,
) -> AnyhowResult<()> {
  let _r = sqlx::query!(
        r#"
UPDATE tts_models
SET
  creator_ip_address_last_update = ?,
  user_deleted_at = CURRENT_TIMESTAMP
WHERE
  token = ?
LIMIT 1
        "#,
      creator_ip_address,
      tts_model_token,
    )
      .execute(mysql_pool)
      .await?;
  Ok(())
}

pub async fn delete_tts_model_as_mod(
  tts_model_token: &str,
  mod_user_token: &str,
  mysql_pool: &MySqlPool,
) -> AnyhowResult<()> {
  let _r = sqlx::query!(
        r#"
UPDATE tts_models
SET
  maybe_mod_user_token = ?,
  mod_deleted_at = CURRENT_TIMESTAMP
WHERE
  token = ?
LIMIT 1
        "#,
      mod_user_token,
      tts_model_token,
    )
      .execute(mysql_pool)
      .await?;
  Ok(())
}

pub async fn undelete_tts_model_as_user(
  tts_model_token: &str,
  creator_ip_address: &str,
  mysql_pool: &MySqlPool,
) -> AnyhowResult<()> {
  let _r = sqlx::query!(
        r#"
UPDATE tts_models
SET
  creator_ip_address_last_update = ?,
  user_deleted_at = NULL
WHERE
  token = ?
LIMIT 1
        "#,
      creator_ip_address,
      tts_model_token,
    )
      .execute(mysql_pool)
      .await?;
  Ok(())
}

pub async fn undelete_tts_model_as_mod(
  tts_model_token: &str,
  mod_user_token: &str,
  mysql_pool: &MySqlPool,
) -> AnyhowResult<()> {
  let _r = sqlx::query!(
        r#"
UPDATE tts_models
SET
  maybe_mod_user_token = ?,
  mod_deleted_at = NULL
WHERE
  token = ?
LIMIT 1
        "#,
      mod_user_token,
      tts_model_token,
    )
      .execute(mysql_pool)
      .await?;
  Ok(())
}
