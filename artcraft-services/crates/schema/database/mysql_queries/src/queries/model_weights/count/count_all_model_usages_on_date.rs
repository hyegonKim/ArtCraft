use chrono::NaiveDate;
use sqlx::{Executor, MySql};

use errors::AnyhowResult;
use tokens::tokens::model_weights::ModelWeightToken;

use crate::helpers::numeric_converters::try_i64_to_u64_or_min;

#[derive(Clone)]
pub struct ModelUseCounts {
  pub counts: Vec<ModelUseCount>,
}

#[derive(Clone)]
pub struct ModelUseCount {
  pub token: ModelWeightToken,

  /// This is the actual calculated usage count.
  /// This is the source of truth and should be written over any inaccurate data.
  pub latest_usage_count: u64,

  /// This is a join against any potentially previously stored calculation.
  /// Previous calculations may have happened before end of day and may not be
  /// correct for the full day totals. If this cached count matches the actual
  /// count, however, then we can skip rewriting the record.
  pub maybe_previously_cached_model_use_count: Option<u64>,
}

struct RawResult {
  token: Option<String>,
  usage_count: i64,
  maybe_previously_cached_model_use_count: Option<u32>,
}

pub async fn count_all_model_usages_on_date<'e, 'c, E>(
  mysql_executor: E,
  date: NaiveDate,
) -> AnyhowResult<ModelUseCounts> where E: 'e + Executor<'c, Database=MySql> {
  // NB(1): This query groups model_weights tokens as well as the old
  //       format "TM:" prefixed tts_models tokens that some streamers are using.
  //
  // NB(2): This avoids a full table scan on a date-to-timestamp conversion.
  //        See https://stackoverflow.com/a/14769096
  //        Basically it requires an interval of timestamps:
  //        (f.created_at >= CURDATE() AND f.created_at < CURDATE() + INTERVAL 1 DAY)
  //
  // NB(3): We also join against model_weight_usage_counts to get the previously
  //        cached count. We should rewrite that value if there's a mismatch.
  let query = sqlx::query_as!(
      RawResult,
        r#"
SELECT
  distinct coalesce(w.token, w_old.token) as token,
  mwuc.usage_count as maybe_previously_cached_model_use_count,
  COUNT(*) as usage_count
FROM
    media_files as f
LEFT OUTER JOIN model_weights as w
   ON f.maybe_origin_model_token = w.token
LEFT OUTER JOIN model_weights as w_old
   ON f.maybe_origin_model_token = w_old.maybe_migration_old_model_token
LEFT OUTER JOIN model_weight_usage_counts as mwuc
   ON coalesce(w.token, w_old.token) = mwuc.token
   AND mwuc.on_date = ?
WHERE f.maybe_origin_model_token IS NOT NULL
  AND f.created_at >= ?
  AND f.created_at < ? + INTERVAL 1 DAY
GROUP BY coalesce(w.token, w_old.token), mwuc.usage_count
ORDER BY usage_count DESC
        "#,
      date,
      date,
      date,
    );

  let results = query.fetch_all(mysql_executor).await?;

  let results = results.into_iter()
      .filter_map(|record| match record.token {
        None => None, // NB: This shouldn't happen, but SQLx is strict!
        Some(token) => Some(ModelUseCount {
          token: ModelWeightToken::new_from_str(&token),
          latest_usage_count: try_i64_to_u64_or_min(record.usage_count),
          maybe_previously_cached_model_use_count: record.maybe_previously_cached_model_use_count
              .map(|n| n as u64),
        })
      })
      .collect();

  Ok(ModelUseCounts {
    counts: results,
  })
}
