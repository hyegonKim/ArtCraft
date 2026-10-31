use sqlx::MySqlPool;

use errors::AnyhowResult;

pub async fn delete_voice_as_user(
    voice_token: &str,
    mysql_pool: &MySqlPool
) -> AnyhowResult<()> {
    let _r = sqlx::query!(
        r#"
UPDATE zs_voices
SET
  user_deleted_at = CURRENT_TIMESTAMP
WHERE
  token = ?
LIMIT 1
        "#,
      voice_token,
    )
        .execute(mysql_pool)
        .await?;
    Ok(())
}

pub async fn delete_voice_as_mod(
    voice_token: &str,
    mod_user_token: &str,
    mysql_pool: &MySqlPool
) -> AnyhowResult<()> {
    let _r = sqlx::query!(
        r#"
UPDATE zs_voices
SET
  mod_deleted_at = CURRENT_TIMESTAMP,
  maybe_mod_user_token = ?
WHERE
  token = ?
LIMIT 1
        "#,
      mod_user_token,
      voice_token,
    )
        .execute(mysql_pool)
        .await?;
    Ok(())
}

pub async fn undelete_voice_as_user(
    voice_token: &str,
    mysql_pool: &MySqlPool
) -> AnyhowResult<()> {
    let _r = sqlx::query!(
        r#"
UPDATE zs_voices
SET
  user_deleted_at = NULL
WHERE
  token = ?
LIMIT 1
        "#,
      voice_token,
    )
        .execute(mysql_pool)
        .await?;
    Ok(())
}

pub async fn undelete_voice_as_mod(
    voice_token: &str,
    mod_user_token: &str,
    mysql_pool: &MySqlPool
) -> AnyhowResult<()> {
    let _r = sqlx::query!(
        r#"
UPDATE zs_voices
SET
  mod_deleted_at = NULL,
  maybe_mod_user_token = ?
WHERE
  token = ?
LIMIT 1
        "#,
      mod_user_token,
      voice_token,
    )
        .execute(mysql_pool)
        .await?;
    Ok(())
}