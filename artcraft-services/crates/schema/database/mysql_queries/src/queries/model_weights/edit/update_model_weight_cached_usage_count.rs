use std::marker::PhantomData;

use sqlx::{Executor, MySql};

use errors::AnyhowResult;
use tokens::tokens::model_weights::ModelWeightToken;

pub struct Args<'e, 'c, E>
where E: 'e + Executor<'c, Database = MySql>
{
  /// The model we're updating.
  pub model_weight_token: &'e ModelWeightToken,

  /// The new cached usage count.
  pub usage_count: u64,

  pub mysql_executor: E,

  // TODO: Not sure if this works to tell the compiler we need the lifetime annotation.
  //  See: https://doc.rust-lang.org/std/marker/struct.PhantomData.html#unused-lifetime-parameters
  pub phantom: PhantomData<&'c E>,
}

pub async fn update_model_weight_cached_usage_count<'e, 'c : 'e, E>(
  args: Args<'e, 'c, E>,
) -> AnyhowResult<()> where E: 'e + Executor<'c, Database = MySql> {

  // NB: Since this is an automation, don't trigger the updated_at trigger.
  // Force by writing the updated_at field with the current value.
  let _query_result = sqlx::query!(
        r#"
        UPDATE model_weights
        SET
            cached_usage_count = ?,
            updated_at = updated_at
        WHERE token = ?
        LIMIT 1
        "#,
        args.usage_count,
        args.model_weight_token,
    ).execute(args.mysql_executor).await?;

  Ok(())
}
