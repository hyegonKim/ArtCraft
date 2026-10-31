use anyhow::anyhow;
use chrono::{DateTime, Utc};
use log::warn;
use sqlx::MySqlPool;

use errors::AnyhowResult;
use tokens::tokens::generic_download_jobs::DownloadJobToken;

pub struct GenericDownloadJobStatus {
  pub job_token: DownloadJobToken,

  pub status: String,
  pub attempt_count: i32,
  pub maybe_downloaded_entity_type: Option<String>,
  pub maybe_downloaded_entity_token: Option<String>,

  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
}

/// Look up job status.
/// Returns Ok(None) when the record cannot be found.
pub async fn get_generic_download_job_status(job_token: &DownloadJobToken, mysql_pool: &MySqlPool)
  -> AnyhowResult<Option<GenericDownloadJobStatus>>
{
  let maybe_status = sqlx::query_as!(
      GenericDownloadJobStatus,
        r#"
SELECT
    jobs.token as `job_token: tokens::tokens::generic_download_jobs::DownloadJobToken`,

    jobs.status,
    jobs.attempt_count,
    jobs.on_success_downloaded_entity_token as maybe_downloaded_entity_token,
    jobs.on_success_downloaded_entity_type as maybe_downloaded_entity_type,

    jobs.created_at,
    jobs.updated_at

FROM generic_download_jobs as jobs

WHERE jobs.token = ?
        "#,
      job_token
    )
      .fetch_one(mysql_pool)
      .await;

  match maybe_status {
    Ok(record) => Ok(Some(record)),
    Err(err) => match err {
      sqlx::Error::RowNotFound => Ok(None),
      _ => {
        warn!("error querying job record: {:?}", err);
        Err(anyhow!("error querying job record: {:?}", err))
      }
    }
  }
}
