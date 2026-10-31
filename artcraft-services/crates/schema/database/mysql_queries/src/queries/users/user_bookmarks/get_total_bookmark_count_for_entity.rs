use anyhow::anyhow;
use log::warn;
use sqlx::MySql;
use sqlx::pool::PoolConnection;

use errors::AnyhowResult;

use crate::queries::users::user_bookmarks::user_bookmark_entity_token::UserBookmarkEntityToken;

pub struct BookmarkCount {
  pub total_count: usize,
}

pub async fn get_total_bookmark_count_for_entity(
  user_bookmark_entity_token: &UserBookmarkEntityToken,
  mysql_connection: &mut PoolConnection<MySql>,
) -> AnyhowResult<BookmarkCount> {

  let (entity_type, entity_token) = user_bookmark_entity_token.get_composite_keys();

  let maybe_result = sqlx::query_as!(
      RawRecord,
        r#"
SELECT
    COUNT(*) as total_count
FROM
    user_bookmarks
WHERE
    entity_type = ?
    AND entity_token = ?
    AND deleted_at IS NULL
        "#,
      entity_type,
      entity_token
    )
      .fetch_one(&mut **mysql_connection)
      .await;

  match maybe_result {
    Err(err) => match err {
      sqlx::Error::RowNotFound => Ok(BookmarkCount { total_count: 0 }),
      _ => {
        warn!("get_total_bookmark_count db error: {:?}", err);
        Err(anyhow!("error with query: {:?}", err))
      }
    },
    Ok(result) => Ok(result.into_public_type()),
  }
}

pub struct RawRecord {
  total_count: i64,
}

impl RawRecord {
  pub fn into_public_type(self) -> BookmarkCount {
    BookmarkCount {
      total_count: self.total_count as usize,
    }
  }
}
