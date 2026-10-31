use sqlx::{Executor, MySql};

use composite_identifiers::by_table::featured_items::featured_item_entity::FeaturedItemEntity;
use errors::AnyhowResult;

pub async fn delete_featured_item<'e, 'c, E>(
    entity: &'e FeaturedItemEntity,
    mysql_executor: E
)
    -> AnyhowResult<()>
  where E: 'e + Executor<'c, Database = MySql>
{
    let (entity_type, entity_token) = entity.get_composite_keys();

    sqlx::query!(
      r#"
UPDATE featured_items
SET
  deleted_at = CURRENT_TIMESTAMP
WHERE
  entity_type = ?
  AND entity_token = ?
LIMIT 1
      "#,
      entity_type,
      entity_token
    )
    .execute(mysql_executor)
    .await?;

  Ok(())
}
