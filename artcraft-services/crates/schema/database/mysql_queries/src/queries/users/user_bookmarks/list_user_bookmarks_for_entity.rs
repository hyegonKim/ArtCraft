use anyhow::anyhow;
use chrono::{DateTime, Utc};
use log::warn;
use sqlx::MySqlPool;

use errors::AnyhowResult;
use tokens::tokens::user_bookmarks::UserBookmarkToken;
use tokens::tokens::users::UserToken;

use crate::queries::users::user_bookmarks::user_bookmark_entity_token::UserBookmarkEntityToken;

pub struct UserBookmark {
  pub token: UserBookmarkToken,

  pub user_token: UserToken,
  pub username: String,
  pub user_display_name: String,
  pub user_gravatar_hash: String,

  pub created_at: DateTime<Utc>,
  pub updated_at: DateTime<Utc>,
  pub maybe_deleted_at: Option<DateTime<Utc>>,
}

pub async fn list_user_bookmarks_for_entity(
  user_bookmark_entity_token: UserBookmarkEntityToken,
  mysql_pool: &MySqlPool
) -> AnyhowResult<Vec<UserBookmark>> {

  let (entity_type, entity_token) = user_bookmark_entity_token.get_composite_keys();

  let maybe_results= sqlx::query_as!(
      RawUserBookmarkRecord,
        r#"
SELECT
    f.token as `token: tokens::tokens::user_bookmarks::UserBookmarkToken`,
    f.user_token as `user_token: tokens::tokens::users::UserToken`,
    u.username,
    u.display_name as user_display_name,
    u.email_gravatar_hash as user_gravatar_hash,

    f.created_at,
    f.updated_at,
    f.deleted_at

FROM
    user_bookmarks AS f
JOIN users AS u
    ON f.user_token = u.token
WHERE
    f.entity_type = ?
    AND f.entity_token = ?
    AND f.deleted_at IS NULL
ORDER BY f.id DESC
LIMIT 50
        "#,
      entity_type,
      entity_token
    )
      .fetch_all(mysql_pool)
      .await;

  match maybe_results {
    Err(err) => match err {
      sqlx::Error::RowNotFound => Ok(Vec::new()),
      _ => {
        warn!("list user_bookmarks db error: {:?}", err);
        Err(anyhow!("error with query: {:?}", err))
      }
    },
    Ok(results) => Ok(results.into_iter()
        .map(|user_bookmark| user_bookmark.into_public_type())
        .collect()),
  }
}

pub struct RawUserBookmarkRecord {
  token: UserBookmarkToken,

  user_token: UserToken,
  username: String,
  user_display_name: String,
  user_gravatar_hash: String,

  created_at: DateTime<Utc>,
  updated_at: DateTime<Utc>,
  deleted_at: Option<DateTime<Utc>>,
}

impl RawUserBookmarkRecord {
  pub fn into_public_type(self) -> UserBookmark {
    UserBookmark {
      token: self.token,
      user_token: self.user_token,
      username: self.username,
      user_display_name: self.user_display_name,
      user_gravatar_hash: self.user_gravatar_hash,
      created_at: self.created_at,
      updated_at: self.updated_at,
      maybe_deleted_at: self.deleted_at,
    }
  }
}
