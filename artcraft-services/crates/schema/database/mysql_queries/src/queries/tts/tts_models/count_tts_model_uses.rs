//! NB: This query is potentially long-lived and not suitable for low-latency HTTP endpoints.

use log::info;
use sqlx::MySql;
use sqlx::pool::PoolConnection;

use errors::AnyhowResult;
use tokens::tokens::tts_models::TtsModelToken;

pub struct TtsModelUseCountInfo {
  pub use_count: u64,
}

pub async fn count_tts_model_uses(
  tts_model_token: &TtsModelToken,
  last_n_minutes: u64,
  mysql_connection: &mut PoolConnection<MySql>,
) -> AnyhowResult<TtsModelUseCountInfo> {

  info!("Querying for TTS model {} use count ({} minutes)", tts_model_token, last_n_minutes);

  let use_count = sqlx::query_as!(
    RawTtsModelUseCountInfo,
        r#"
    SELECT
      count(*) AS use_count
    FROM
      tts_results
    WHERE
      created_at > ( CURDATE() - INTERVAL ? MINUTE )
      AND model_token = ?
        "#,
    last_n_minutes,
    tts_model_token
  )
      .fetch_one(&mut **mysql_connection)
      .await?;

  Ok(TtsModelUseCountInfo {
    use_count: use_count.use_count as u64,
  })
}

struct RawTtsModelUseCountInfo {
  pub use_count: i64,
}
