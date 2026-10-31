use std::marker::PhantomData;

use anyhow::anyhow;
use sqlx::{Executor, MySql};

use errors::AnyhowResult;

use crate::queries::entity_stats::stats_entity_token::StatsEntityToken;

#[derive(Copy, Clone)]
pub enum BookmarkAction {
  Add,
  Delete,
}

pub struct UpsertEntityStatsArgs<'e, 'c, E>
  where E: 'e + Executor<'c, Database = MySql>
{
  pub stats_entity_token: &'e StatsEntityToken,
  pub action: BookmarkAction,
  pub mysql_executor: E,

  // NB: phantom can be passed as Default::default()
  // TODO: Not sure if this works to tell the compiler we need the lifetime annotation.
  //  See: https://doc.rust-lang.org/std/marker/struct.PhantomData.html#unused-lifetime-parameters
  pub phantom: PhantomData<&'c E>,
}

pub async fn upsert_entity_stats_on_bookmark_event<'e, 'c : 'e, E>(
  args: UpsertEntityStatsArgs<'e, 'c, E>,
) -> AnyhowResult<()>
  where E: 'e + Executor<'c, Database = MySql>
{
  let (entity_type, entity_token) = args.stats_entity_token.get_composite_keys();

  let query = match args.action {
    BookmarkAction::Add => {
      sqlx::query!(
        r#"
        INSERT INTO entity_stats SET
          entity_type = ?,
          entity_token = ?,
          bookmark_count = 1
        ON DUPLICATE KEY UPDATE
          bookmark_count = bookmark_count + 1
        "#,
        entity_type,
        entity_token,
      )
    }
    BookmarkAction::Delete => {
      sqlx::query!(
        r#"
        INSERT INTO entity_stats SET
          entity_type = ?,
          entity_token = ?,
          bookmark_count = 0
        ON DUPLICATE KEY UPDATE
          bookmark_count = ABS(CAST(bookmark_count AS SIGNED) - 1)
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
