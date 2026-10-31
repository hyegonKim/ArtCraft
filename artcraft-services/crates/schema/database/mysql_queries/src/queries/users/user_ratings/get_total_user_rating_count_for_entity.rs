use anyhow::anyhow;
use log::warn;
use sqlx::MySql;
use sqlx::pool::PoolConnection;

use errors::AnyhowResult;

use crate::composite_keys::by_table::user_ratings::user_rating_entity::UserRatingEntity;

pub struct PositiveRatingCount {
  pub positive_count : usize,
}

pub async fn get_total_user_rating_count_for_entity(
  user_rating_entity: &UserRatingEntity,
  mysql_connection: &mut PoolConnection<MySql>,
) -> AnyhowResult<PositiveRatingCount> {

  let (entity_type, entity_token) = user_rating_entity.get_composite_keys();

  let maybe_result = sqlx::query_as!(
      RawRecord,
        r#"
SELECT
    COUNT(*) as positive_count
FROM
    user_ratings
WHERE
    entity_type = ?
    AND entity_token = ?
    AND rating_value = 'positive'
        "#,
      entity_type,
      entity_token
    )
      .fetch_one(&mut **mysql_connection)
      .await;

  match maybe_result {
    Err(err) => match err {
      sqlx::Error::RowNotFound => Ok(PositiveRatingCount { positive_count: 0 }),
      _ => {
        warn!("get_total_user_rating_count db error: {:?}", err);
        Err(anyhow!("error with query: {:?}", err))
      }
    },
    Ok(result) => Ok(result.into_public_type()),
  }
}

pub struct RawRecord {
  positive_count: i64,
}

impl RawRecord {
  pub fn into_public_type(self) -> PositiveRatingCount {
    PositiveRatingCount {
      positive_count: self.positive_count as usize,
    }
  }
}
