use anyhow::anyhow;
use log::error;
use sqlx::{MySql, MySqlPool};
use sqlx::pool::PoolConnection;

use errors::AnyhowResult;
use tokens::tokens::media_files::MediaFileToken;
use tokens::tokens::zs_voice_dataset_samples::ZsVoiceDatasetSampleToken;

pub struct DatasetSampleRecord {
  pub token: ZsVoiceDatasetSampleToken,
  pub media_file_token: MediaFileToken,
}

/// Query for a media upload to see if we already uploaded it.
pub async fn get_dataset_sample_by_uuid(
  uuid_idempotency_token: &str,
  mysql_pool: &MySqlPool,
) -> AnyhowResult<Option<DatasetSampleRecord>> {
  let mut connection = mysql_pool.acquire().await?;
  get_dataset_sample_by_uuid_with_connection(
    uuid_idempotency_token,
    &mut connection
  ).await
}

/// Query for a media upload to see if we already uploaded it.
pub async fn get_dataset_sample_by_uuid_with_connection(
  uuid_idempotency_token: &str,
  mysql_connection: &mut PoolConnection<MySql>,
) -> AnyhowResult<Option<DatasetSampleRecord>> {
  let maybe_result = sqlx::query_as!(
      RawDatasetSampleRecord,
        r#"
SELECT
    token as `token: tokens::tokens::zs_voice_dataset_samples::ZsVoiceDatasetSampleToken`,
    media_file_token as `media_file_token: tokens::tokens::media_files::MediaFileToken`
FROM zs_voice_dataset_samples
WHERE
    uuid_idempotency_token = ?
    AND user_deleted_at IS NULL
    AND mod_deleted_at IS NULL
        "#,
    uuid_idempotency_token,
  )
      .fetch_one(&mut **mysql_connection)
      .await;

  match maybe_result {
    Err(err) => match err {
      sqlx::Error::RowNotFound => Ok(None),
      _ => {
        error!("list media uploads db error: {:?}", err);
        Err(anyhow!("error with query: {:?}", err))
      }
    },
    Ok(upload) => Ok(Some(DatasetSampleRecord {
      token: upload.token,
      media_file_token: upload.media_file_token,
    }))
  }
}

struct RawDatasetSampleRecord {
  pub token: ZsVoiceDatasetSampleToken,
  pub media_file_token: MediaFileToken,
}
