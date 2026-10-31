use anyhow::anyhow;
use log::warn;
use sqlx::MySqlPool;

use errors::AnyhowResult;

#[derive(Serialize, Clone)]
pub struct VoiceStats {
  pub public_count: i64,
  pub all_count: i64,
}

/// Return this when there are no results.
const NO_RESULTS_SENTINEL : VoiceStats = VoiceStats {
  public_count: -1,
  all_count: -1,
};

pub async fn get_voice_count_stats(
  mysql_pool: &MySqlPool
) -> AnyhowResult<VoiceStats> {

  // NB: Lookup failure is Err(RowNotFound).
  let maybe_result = sqlx::query_as!(
      VoiceStats,
        r#"
SELECT public_count, all_count FROM (
  (SELECT count(*) as public_count
   FROM tts_models
   WHERE user_deleted_at IS NULL
   AND mod_deleted_at IS NULL) as pc,
  (SELECT count(*) as all_count
   FROM tts_models) as ac
)
      "#,
    )
      .fetch_one(mysql_pool)
      .await;

  let result : VoiceStats = match maybe_result {
    Ok(result) => result,
    Err(sqlx::Error::RowNotFound) => NO_RESULTS_SENTINEL.clone(),
    Err(e) => {
      warn!("get voice count stats error: {:?}", e);
      return Err(anyhow!("couldn't fetch voice count stats"));
    }
  };

  Ok(result)
}
