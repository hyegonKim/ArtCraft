use std::marker::PhantomData;

use chrono::NaiveDate;
use log::info;
use sqlx::{Executor, MySql};

use errors::AnyhowResult;
use tokens::tokens::model_weights::ModelWeightToken;

pub struct Args<'e, 'c, E>
where E: 'e + Executor<'c, Database = MySql>
{
  pub model_token: &'e ModelWeightToken,
  pub date: NaiveDate,
  pub usage_count: u64,

  /// We should only need a sparse matrix for our histogram.
  /// There will be a lot of "0" usage count "holes", and we should skip creating
  /// records for them. This is just an extra explicit check to make sure we don't
  /// insert unless we really want to do so.
  pub insert_on_zero: bool,

  pub mysql_executor: E,

  // TODO: Not sure if this works to tell the compiler we need the lifetime annotation.
  //  See: https://doc.rust-lang.org/std/marker/struct.PhantomData.html#unused-lifetime-parameters
  pub phantom: PhantomData<&'c E>,
}

pub async fn upsert_model_weight_usage_count_for_date<'e, 'c : 'e, E>(
  args: Args<'e, 'c, E>,
) -> AnyhowResult<()>
where E: 'e + Executor<'c, Database = MySql>
{
  if !args.insert_on_zero && args.usage_count == 0 {
    info!("Skipping insert for zero usage count");
    return Ok(());
  }

  let query = sqlx::query!(
        r#"
INSERT INTO model_weight_usage_counts
SET
  token = ?,
  on_date = ?,
  usage_count = ?


ON DUPLICATE KEY UPDATE
  usage_count = ?
        "#,
      // Insert
      args.model_token.as_str(),
      args.date,
      args.usage_count,
      args.usage_count,
    );

  let _r = query.execute(args.mysql_executor).await?;

  Ok(())
}
