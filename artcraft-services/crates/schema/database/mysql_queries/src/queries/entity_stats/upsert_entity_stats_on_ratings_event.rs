use std::marker::PhantomData;

use anyhow::anyhow;
use sqlx::{Executor, MySql};

use errors::AnyhowResult;

use crate::queries::entity_stats::stats_entity_token::StatsEntityToken;

/// NB: "Neutral" means one of two things here:
/// 1. The user has not voted on the entity, ie. no previous vote exists (null).
/// 2. The user has voted on the entity, but the vote is "neutral".
#[derive(Copy, Clone)]
pub enum RatingsAction {
  /// Add one to positive
  NeutralToPositive,

  /// Add one to negative
  NeutralToNegative,

  /// Remove one from positive
  PositiveToNeutral,

  /// Remove one from negative
  NegativeToNeutral,

  /// Remove one from negative, add one to positive
  NegativeToPositive,

  /// Remove one from positive, add one to negative
  PositiveToNegative,
}

pub struct UpsertEntityStatsArgs<'e, 'c, E>
  where E: 'e + Executor<'c, Database = MySql>
{
  pub stats_entity_token: &'e StatsEntityToken,
  pub action: RatingsAction,
  pub mysql_executor: E,

  // TODO: Not sure if this works to tell the compiler we need the lifetime annotation.
  //  See: https://doc.rust-lang.org/std/marker/struct.PhantomData.html#unused-lifetime-parameters
  pub phantom: PhantomData<&'c E>,
}

pub async fn upsert_entity_stats_on_ratings_event<'e, 'c : 'e, E>(
  args: UpsertEntityStatsArgs<'e, 'c, E>,
) -> AnyhowResult<()>
  where E: 'e + Executor<'c, Database = MySql>
{
  let (entity_type, entity_token) = args.stats_entity_token.get_composite_keys();

  let query = match args.action {
    RatingsAction::NeutralToPositive => {
      sqlx::query!(
        r#"
        INSERT INTO entity_stats SET
          entity_type = ?,
          entity_token = ?,
          ratings_positive_count = 1
        ON DUPLICATE KEY UPDATE
          ratings_positive_count = ratings_positive_count + 1
        "#,
        entity_type,
        entity_token,
      )
    }
    RatingsAction::NeutralToNegative => {
      sqlx::query!(
        r#"
        INSERT INTO entity_stats SET
          entity_type = ?,
          entity_token = ?,
          ratings_negative_count = 1
        ON DUPLICATE KEY UPDATE
          ratings_negative_count = ratings_negative_count + 1
        "#,
        entity_type,
        entity_token,
      )
    }
    RatingsAction::PositiveToNeutral => {
      sqlx::query!(
        r#"
        INSERT INTO entity_stats SET
          entity_type = ?,
          entity_token = ?,
          ratings_positive_count = 0
        ON DUPLICATE KEY UPDATE
          ratings_positive_count = ABS(CAST(ratings_positive_count AS SIGNED) - 1)
        "#,
        entity_type,
        entity_token,
      )
    }
    RatingsAction::NegativeToNeutral => {
      sqlx::query!(
        r#"
        INSERT INTO entity_stats SET
          entity_type = ?,
          entity_token = ?,
          ratings_negative_count = 0
        ON DUPLICATE KEY UPDATE
          ratings_negative_count = ABS(CAST(ratings_negative_count AS SIGNED) - 1)
        "#,
        entity_type,
        entity_token,
      )
    }
    RatingsAction::PositiveToNegative => {
      sqlx::query!(
        r#"
        INSERT INTO entity_stats SET
          entity_type = ?,
          entity_token = ?,
          ratings_positive_count = 0,
          ratings_negative_count = 1
        ON DUPLICATE KEY UPDATE
          ratings_positive_count = ABS(CAST(ratings_positive_count AS SIGNED) - 1),
          ratings_negative_count = ratings_negative_count + 1
        "#,
        entity_type,
        entity_token,
      )
    }
    RatingsAction::NegativeToPositive => {
      sqlx::query!(
        r#"
        INSERT INTO entity_stats SET
          entity_type = ?,
          entity_token = ?,
          ratings_positive_count = 1,
          ratings_negative_count = 0
        ON DUPLICATE KEY UPDATE
          ratings_positive_count = ratings_positive_count + 1,
          ratings_negative_count = ABS(CAST(ratings_negative_count AS SIGNED) - 1)
        "#,
        entity_type,
        entity_token,
      )
    }
  };

  let query_result = query.execute(args.mysql_executor).await;

  let _record_id = match query_result {
    Ok(res) => res.last_insert_id(),
    Err(err) => return Err(anyhow!("Mysql error: {:?}", err)),
  };

  Ok(())
}
