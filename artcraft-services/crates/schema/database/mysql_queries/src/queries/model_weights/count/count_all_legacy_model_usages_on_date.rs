use chrono::NaiveDate;
use sqlx::{Executor, MySql};

use errors::AnyhowResult;
use tokens::tokens::model_weights::ModelWeightToken;

use crate::helpers::numeric_converters::try_i64_to_u64_or_min;

#[derive(Clone)]
pub struct LegacyModelUseCounts {
  pub counts: Vec<LegacyModelUseCount>,
}

#[derive(Clone)]
pub struct LegacyModelUseCount {
  pub token: ModelWeightToken,
  pub record_count: u64,
}

struct RawResult {
  token: Option<String>,
  usage_count: i64,
}

pub async fn count_all_legacy_model_usages_on_date<'e, 'c, E>(
  mysql_executor: E,
  date: NaiveDate,
) -> AnyhowResult<LegacyModelUseCounts> where E: 'e + Executor<'c, Database=MySql> {
  // NB(1): This queries legacy tts_results records and transforms those reported
  //        model usages into modern model_weights tokens.
  //
  // NB(2): This avoids a full table scan on a date-to-timestamp conversion.
  //        See https://stackoverflow.com/a/14769096
  //        Basically it requires an interval of timestamps:
  //        (f.created_at >= CURDATE() AND f.created_at < CURDATE() + INTERVAL 1 DAY)
  let query = sqlx::query_as!(
      RawResult,
        r#"
SELECT
  distinct coalesce(w.token, w_new.token) as token,
  COUNT(*) as usage_count
FROM
    tts_results as r
LEFT OUTER JOIN model_weights as w
   ON r.model_token = w.maybe_migration_old_model_token
LEFT OUTER JOIN model_weights as w_new
   ON r.model_token = w_new.token
WHERE r.created_at >= ?
  AND r.created_at < ? + INTERVAL 1 DAY
GROUP BY coalesce(w.token, w_new.token)
ORDER BY usage_count DESC
        "#,
      date,
      date,
    );

  let results = query.fetch_all(mysql_executor).await?;

  let results = results.into_iter()
      .filter_map(|record| match record.token {
        None => None, // NB: This shouldn't happen, but SQLx is strict!
        Some(token) => Some(LegacyModelUseCount {
          token: ModelWeightToken::new_from_str(&token),
          record_count: try_i64_to_u64_or_min(record.usage_count),
        })
      })
      .collect();

  Ok(LegacyModelUseCounts {
    counts: results,
  })
}
