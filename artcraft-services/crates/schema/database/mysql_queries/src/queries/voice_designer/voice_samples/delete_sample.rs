use sqlx::MySqlPool;

use errors::AnyhowResult;
use tokens::tokens::users::UserToken;
use tokens::tokens::zs_voice_dataset_samples::ZsVoiceDatasetSampleToken;

pub async fn delete_sample_as_user(
  voice_dataset_sample_token: &ZsVoiceDatasetSampleToken,
  mysql_pool: &MySqlPool
) -> AnyhowResult<()> {
  let _r = sqlx::query!(
        r#"
UPDATE zs_voice_dataset_samples
SET
  user_deleted_at = CURRENT_TIMESTAMP
WHERE
  token = ?
LIMIT 1
        "#,
      voice_dataset_sample_token,
    )
      .execute(mysql_pool)
      .await?;
  Ok(())
}

pub async fn delete_sample_as_mod(
  voice_dataset_sample_token: &ZsVoiceDatasetSampleToken,
  mod_user_token: &UserToken,
  mysql_pool: &MySqlPool
) -> AnyhowResult<()> {
  let _r = sqlx::query!(
        r#"
UPDATE zs_voice_dataset_samples
SET
  mod_deleted_at = CURRENT_TIMESTAMP,
  maybe_mod_user_token = ?
WHERE
  token = ?
LIMIT 1
        "#,
      mod_user_token,
      voice_dataset_sample_token,
    )
      .execute(mysql_pool)
      .await?;
  Ok(())
}

pub async fn undelete_sample_as_user(
  voice_dataset_sample_token: &ZsVoiceDatasetSampleToken,
  mysql_pool: &MySqlPool
) -> AnyhowResult<()> {
  let _r = sqlx::query!(
        r#"
UPDATE zs_voice_dataset_samples
SET
  user_deleted_at = NULL
WHERE
  token = ?
LIMIT 1
        "#,
      voice_dataset_sample_token,
    )
      .execute(mysql_pool)
      .await?;
  Ok(())
}

pub async fn undelete_sample_as_mod(
  voice_dataset_sample_token: &ZsVoiceDatasetSampleToken,
  mod_user_token: &UserToken,
  mysql_pool: &MySqlPool
) -> AnyhowResult<()> {
  let _r = sqlx::query!(
        r#"
UPDATE zs_voice_dataset_samples
SET
  mod_deleted_at = NULL,
  maybe_mod_user_token = ?
WHERE
  token = ?
LIMIT 1
        "#,
      mod_user_token,
      voice_dataset_sample_token,
    )
      .execute(mysql_pool)
      .await?;
  Ok(())
}
