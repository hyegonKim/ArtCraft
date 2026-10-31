use sqlx::{Executor, MySql};

use errors::AnyhowResult;
use tokens::tokens::comments::CommentToken;

#[derive(Copy, Clone)]
pub enum DeleteCommentAs {
  Author,
  Moderator,
  ObjectOwner,
}

pub async fn delete_comment<'e, 'c, E>(
  comment_token: &'e CommentToken,
  delete_as: DeleteCommentAs,
  mysql_executor: E
)
  -> AnyhowResult<()>
  where E: 'e + Executor<'c, Database = MySql>
{

  match delete_as {
    DeleteCommentAs::Author => {
      sqlx::query!(
        r#"
UPDATE comments
SET
  user_deleted_at = CURRENT_TIMESTAMP,
  version = version + 1
WHERE
  token = ?
LIMIT 1
        "#,
        comment_token
      )
    }
    DeleteCommentAs::Moderator => {
      sqlx::query!(
        r#"
UPDATE comments
SET
  mod_deleted_at = CURRENT_TIMESTAMP,
  version = version + 1
WHERE
  token = ?
LIMIT 1
        "#,
        comment_token
      )
    }
    DeleteCommentAs::ObjectOwner => {

      sqlx::query!(
        r#"
UPDATE comments
SET
  object_owner_deleted_at = CURRENT_TIMESTAMP,
  version = version + 1
WHERE
  token = ?
LIMIT 1
        "#,
        comment_token
      )
    }
  }
      .execute(mysql_executor)
      .await?;

  Ok(())
}
