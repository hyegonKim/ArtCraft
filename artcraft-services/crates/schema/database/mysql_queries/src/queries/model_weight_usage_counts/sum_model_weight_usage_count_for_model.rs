use anyhow::anyhow;
use num_traits::cast::ToPrimitive;
use sqlx::{Executor, MySql};
use sqlx::types::BigDecimal;

use errors::AnyhowResult;
use tokens::tokens::model_weights::ModelWeightToken;

pub struct UsageCount {
  pub total_usage_count: u64,
}

pub async fn sum_model_weight_usage_count_for_models<'e, 'c, E>(
  model_token: &'e ModelWeightToken,
  mysql_executor: E
)
  -> AnyhowResult<Option<UsageCount>>
where E: 'e + Executor<'c, Database = MySql>
{

  let result = sqlx::query_as!(
      RawUsageCount,
        r#"
SELECT
    sum(usage_count) as total_usage_count
FROM
    model_weight_usage_counts
WHERE
    token = ?
GROUP BY token
LIMIT 1
        "#,
      model_token.as_str()
    )
      .fetch_one(mysql_executor)
      .await;

  let result = match result {
    Ok(result) => result,
    Err(err) => return match err {
      sqlx::Error::RowNotFound => Ok(None),
      _ => Err(anyhow!("Error querying for IP ban: {:?}", err)),
    }
  };

  // TODO(bt,2024-09-07): It sucks that we have to use BigDecimal ...
  let count = result.total_usage_count
      .map(|count| count.to_u64())
      .flatten()
      .map(|count| UsageCount {
        total_usage_count: count,
      });

  Ok(count)
}

pub struct RawUsageCount {
  total_usage_count: Option<BigDecimal>,
}
