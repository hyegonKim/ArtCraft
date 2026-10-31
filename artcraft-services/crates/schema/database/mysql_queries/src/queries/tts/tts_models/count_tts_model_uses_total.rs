//! NB: This query is potentially long-lived and not suitable for low-latency HTTP endpoints.

use log::info;
use sqlx::MySql;
use sqlx::pool::PoolConnection;

use errors::AnyhowResult;
use tokens::tokens::tts_models::TtsModelToken;

pub struct TtsModelTotalUseCountInfo {
  pub total_use_count: u64,
}

pub async fn count_tts_model_uses_total(
  tts_model_token: &TtsModelToken,
  mysql_connection: &mut PoolConnection<MySql>,
) -> AnyhowResult<TtsModelTotalUseCountInfo> {

  info!("Querying for TTS model {} total use count", tts_model_token);

  let total_use_count = sqlx::query_as!(
    RawTtsModelTotalUseCountInfo,
        r#"
    SELECT
      count(*) AS total_use_count
    FROM
      tts_results
    WHERE
      model_token = ?
        "#,
    tts_model_token
  )
      .fetch_one(&mut **mysql_connection)
      .await?;

  Ok(TtsModelTotalUseCountInfo {
    total_use_count: total_use_count.total_use_count as u64,
  })
}

struct RawTtsModelTotalUseCountInfo {
  pub total_use_count: i64,
}
