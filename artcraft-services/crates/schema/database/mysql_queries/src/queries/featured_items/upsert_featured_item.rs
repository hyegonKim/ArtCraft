use std::marker::PhantomData;

use anyhow::anyhow;
use sqlx::{Executor, MySql};

use composite_identifiers::by_table::featured_items::featured_item_entity::FeaturedItemEntity;
use errors::AnyhowResult;

pub struct UpsertFeaturedItemArgs<'e, 'c, E>
  where E: 'e + Executor<'c, Database = MySql>
{
  pub entity: &'e FeaturedItemEntity,

  pub mysql_executor: E,

  // TODO: Not sure if this works to tell the compiler we need the lifetime annotation.
  //  See: https://doc.rust-lang.org/std/marker/struct.PhantomData.html#unused-lifetime-parameters
  pub phantom: PhantomData<&'c E>,
}

pub async fn upsert_featured_item<'e, 'c : 'e, E>(
    args: UpsertFeaturedItemArgs<'e, 'c, E>,
)
    -> AnyhowResult<()>
  where E: 'e + Executor<'c, Database = MySql>
{

  let (entity_type, entity_token) = args.entity.get_composite_keys();

  let query_result = sqlx::query!(
        r#"
INSERT INTO featured_items
SET
  entity_type = ?,
  entity_token = ?
ON DUPLICATE KEY UPDATE
  deleted_at = NULL
        "#,
      entity_type,
      entity_token,
    )
      .execute(args.mysql_executor)
      .await;

  let _record_id = match query_result {
    Ok(res) => res.last_insert_id(),
    Err(err) => return Err(anyhow!("Mysql error: {:?}", err)),
  };

  Ok(())
}
