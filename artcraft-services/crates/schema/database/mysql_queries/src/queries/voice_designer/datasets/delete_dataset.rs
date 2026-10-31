use sqlx::MySqlPool;

use errors::AnyhowResult;

pub async fn delete_dataset_as_user(
    dataset_token: &str,
    mysql_pool: &MySqlPool
) -> AnyhowResult<()> {
    let _r = sqlx::query!(
        r#"
UPDATE zs_voice_datasets
SET
  user_deleted_at = CURRENT_TIMESTAMP
WHERE
  token = ?
LIMIT 1
        "#,
      dataset_token,
    )
        .execute(mysql_pool)
        .await?;
    Ok(())
}

pub async fn delete_dataset_as_mod(
    dataset_token: &str,
    mod_user_token: &str,
    mysql_pool: &MySqlPool
) -> AnyhowResult<()> {
    let _r = sqlx::query!(
        r#"
UPDATE zs_voice_datasets
SET
  mod_deleted_at = CURRENT_TIMESTAMP,
  maybe_mod_user_token = ?
WHERE
  token = ?
LIMIT 1
        "#,
      mod_user_token,
      dataset_token,
    )
        .execute(mysql_pool)
        .await?;
    Ok(())
}

pub async fn undelete_dataset_as_user(
    dataset_token: &str,
    mysql_pool: &MySqlPool
) -> AnyhowResult<()> {
    let _r = sqlx::query!(
        r#"
UPDATE zs_voice_datasets
SET
  user_deleted_at = NULL
WHERE
  token = ?
LIMIT 1
        "#,
      dataset_token,
    )
        .execute(mysql_pool)
        .await?;
    Ok(())
}

pub async fn undelete_dataset_as_mod(
    dataset_token: &str,
    mod_user_token: &str,
    mysql_pool: &MySqlPool
) -> AnyhowResult<()> {
    let _r = sqlx::query!(
        r#"
UPDATE zs_voice_datasets
SET
  mod_deleted_at = NULL,
  maybe_mod_user_token = ?
WHERE
  token = ?
LIMIT 1
        "#,
      mod_user_token,
      dataset_token,
    )
        .execute(mysql_pool)
        .await?;
    Ok(())
}
