use anyhow::anyhow;
use sqlx::{Executor, MySql};

use composite_identifiers::by_table::featured_items::featured_item_entity::FeaturedItemEntity;
use errors::AnyhowResult;

use crate::helpers::boolean_converters::i64_to_bool;

pub struct IsFeaturedItem {
  pub is_featured: bool,
}

pub async fn get_is_featured_by_token<'e, 'c, E>(
    entity: &'e FeaturedItemEntity,
    mysql_executor: E
)
    -> AnyhowResult<IsFeaturedItem>
  where E: 'e + Executor<'c, Database = MySql>
{
    let (entity_type, entity_token) = entity.get_composite_keys();

    let maybe_result = sqlx::query_as!(
      RawIsFeaturedItem,
      r#"
SELECT deleted_at IS NULL AS is_featured
FROM featured_items
WHERE entity_type = ?
AND entity_token = ?
LIMIT 1
      "#,
      entity_type,
      entity_token
    )
    .fetch_one(mysql_executor)
    .await;

  match maybe_result {
    Ok(RawIsFeaturedItem { is_featured }) => {
      Ok(IsFeaturedItem {
        is_featured: i64_to_bool(is_featured),
      })
    }
    Err(err) => match err {
      sqlx::Error::RowNotFound => Ok(IsFeaturedItem {
        is_featured: false,
      }),
      _ => Err(anyhow!("Error querying for IP ban: {:?}", err)),
    }
  }
}

struct RawIsFeaturedItem {
    is_featured: i64,
}
