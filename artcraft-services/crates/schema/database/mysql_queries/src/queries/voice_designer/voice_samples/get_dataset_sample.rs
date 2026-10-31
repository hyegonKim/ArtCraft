use anyhow::anyhow;
use sqlx::{MySql, MySqlPool};
use sqlx::pool::PoolConnection;

use errors::AnyhowResult;
use tokens::tokens::users::UserToken;
use tokens::tokens::zs_voice_dataset_samples::ZsVoiceDatasetSampleToken;
use tokens::tokens::zs_voice_datasets::ZsVoiceDatasetToken;

pub struct ZsDatasetSample {
  pub token: ZsVoiceDatasetSampleToken,
  pub dataset_token: ZsVoiceDatasetToken,

  pub maybe_creator_user_token: Option<UserToken>,

  // TODO: Other fields
}

pub async fn get_dataset_sample_by_token(
  dataset_sample_token: &ZsVoiceDatasetSampleToken,
  can_see_deleted: bool,
  mysql_pool: &MySqlPool,
) -> AnyhowResult<Option<ZsDatasetSample>> {
  let mut connection = mysql_pool.acquire().await?;
  get_dataset_sample_by_token_with_connection(
    dataset_sample_token,
    can_see_deleted,
    &mut connection
  ).await
}

pub async fn get_dataset_sample_by_token_with_connection(
  dataset_sample_token: &ZsVoiceDatasetSampleToken,
  can_see_deleted: bool,
  mysql_connection: &mut PoolConnection<MySql>,
) -> AnyhowResult<Option<ZsDatasetSample>> {

  let maybe_result = if can_see_deleted {
    select_include_deleted(
      dataset_sample_token,
      mysql_connection
    ).await
  } else {
    select_without_deleted(
      dataset_sample_token,
      mysql_connection
    ).await
  };

  let record = match maybe_result {
    Ok(record) => record,
    Err(sqlx::Error::RowNotFound) => {
      return Ok(None);
    },
    Err(err) => {
      return Err(anyhow!(
        "Error fetching dataset by token: {:?}",
        err
      ));
    }
  };

  Ok(Some(ZsDatasetSample {
    token: record.token,
    dataset_token: record.dataset_token,
    maybe_creator_user_token: record.maybe_creator_user_token,
  }))
}

async fn select_include_deleted(
  dataset_sample_token: &ZsVoiceDatasetSampleToken,
  mysql_connection: &mut PoolConnection<MySql>,
) -> Result<RawDataset, sqlx::Error> {
  sqlx::query_as!(
      RawDataset,
        r#"
        SELECT
        zds.token as `token: tokens::tokens::zs_voice_dataset_samples::ZsVoiceDatasetSampleToken`,
        zds.dataset_token as `dataset_token: tokens::tokens::zs_voice_datasets::ZsVoiceDatasetToken`,
        zds.maybe_creator_user_token as `maybe_creator_user_token: tokens::tokens::users::UserToken`
        FROM zs_voice_dataset_samples as zds
        WHERE
          zds.token = ?
        "#,
        dataset_sample_token.as_str()
  )
      .fetch_one(&mut **mysql_connection).await
}

async fn select_without_deleted(
  dataset_sample_token: &ZsVoiceDatasetSampleToken,
  mysql_connection: &mut PoolConnection<MySql>,
) -> Result<RawDataset, sqlx::Error> {
  sqlx::query_as!(
      RawDataset,
        r#"
        SELECT
        zds.token as `token: tokens::tokens::zs_voice_dataset_samples::ZsVoiceDatasetSampleToken`,
        zds.dataset_token as `dataset_token: tokens::tokens::zs_voice_datasets::ZsVoiceDatasetToken`,
        zds.maybe_creator_user_token as `maybe_creator_user_token: tokens::tokens::users::UserToken`
        FROM zs_voice_dataset_samples as zds
        WHERE
          zds.token = ?
          AND zds.user_deleted_at IS NULL
          AND zds.mod_deleted_at IS NULL
        "#,
        dataset_sample_token.as_str()
  )
      .fetch_one(&mut **mysql_connection).await
}
#[derive(Serialize)]
pub struct RawDataset {
  pub token: ZsVoiceDatasetSampleToken,
  pub dataset_token: ZsVoiceDatasetToken,

  pub maybe_creator_user_token: Option<UserToken>,

  // TODO: Other fields
}
