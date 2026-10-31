use chrono::NaiveDate;
use sqlx::{Executor, MySql, MySqlPool};

use errors::AnyhowResult;
use tokens::tokens::model_weights::ModelWeightToken;

use crate::helpers::numeric_converters::try_i64_to_u64_or_min;

#[derive(Clone)]
pub struct ModelUseCount {
  pub record_count: u64,
}

struct RawResult {
  record_count: i64,
}

pub async fn count_model_use_using_media_files_on_date<'e, 'c, E>(
  mysql_executor: E,
  token: &ModelWeightToken,
  date: NaiveDate,
) -> AnyhowResult<ModelUseCount> where E: 'e + Executor<'c, Database=MySql> {
  // NB(1): This query supports **MODEL WEIGHT TOKENS** only as input parameters,
  //       but it will still include the count of the jobs that are being run by
  //       robots that are still using the old-format "TM:" prefixed tts_models
  //       tokens
  //
  // NB(2): This avoids a full table scan on a date-to-timestamp conversion.
  //        See https://stackoverflow.com/a/14769096
  //        Basically it requires an interval of timestamps:
  //        (f.created_at >= CURDATE() AND f.created_at < CURDATE() + INTERVAL 1 DAY)
  let query = sqlx::query_as!(
      RawResult,
        r#"
SELECT
  COUNT(*) as record_count
  FROM media_files as f
  left outer join model_weights as w
     on f.maybe_origin_model_token = w.token
  left outer join model_weights as w_old
     on f.maybe_origin_model_token = w_old.maybe_migration_old_model_token
  WHERE (w.token = ? OR w_old.token = ?)
  AND f.created_at >= ?
  AND f.created_at < ? + INTERVAL 1 DAY
        "#,
      token.as_str(),
      token.as_str(),
      date,
      date,
    );

  let result = query.fetch_one(mysql_executor).await?;

  Ok(ModelUseCount {
    record_count: try_i64_to_u64_or_min(result.record_count),
  })
}
