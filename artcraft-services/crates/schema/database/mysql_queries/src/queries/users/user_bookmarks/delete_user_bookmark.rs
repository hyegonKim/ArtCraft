use sqlx::{Executor, MySql};

use errors::AnyhowResult;
use tokens::tokens::user_bookmarks::UserBookmarkToken;
use tokens::tokens::users::UserToken;

// NB: UserToken is only supplied as an optimistic form of validation
pub async fn delete_user_bookmark<'e, 'c, E>(
    user_bookmark_token: &'e UserBookmarkToken,
    user_token: &'e UserToken,
    mysql_executor: E
)
    -> AnyhowResult<()>
  where E: 'e + Executor<'c, Database = MySql>
{

    sqlx::query!(
      r#"
UPDATE user_bookmarks
SET
deleted_at = CURRENT_TIMESTAMP,
version = version + 1
WHERE
token = ?
AND user_token = ?
LIMIT 1
      "#,
      user_bookmark_token,
      user_token
    )
    .execute(mysql_executor)
    .await?;

  Ok(())
}
