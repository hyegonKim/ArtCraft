use std::marker::PhantomData;

use anyhow::anyhow;
use sqlx::{Executor, MySql};

use errors::AnyhowResult;
use tokens::tokens::user_bookmarks::UserBookmarkToken;
use tokens::tokens::users::UserToken;

use crate::queries::users::user_bookmarks::user_bookmark_entity_token::UserBookmarkEntityToken;

pub struct CreateUserBookmarkArgs<'e, 'c, E>
  where E: 'e + Executor<'c, Database = MySql>
{
  pub entity_token: &'e UserBookmarkEntityToken,

  pub user_token: &'e UserToken,

  pub mysql_executor: E,

  // TODO: Not sure if this works to tell the compiler we need the lifetime annotation.
  //  See: https://doc.rust-lang.org/std/marker/struct.PhantomData.html#unused-lifetime-parameters
  pub phantom: PhantomData<&'c E>,
}

pub async fn upsert_user_bookmark<'e, 'c : 'e, E>(
    args: CreateUserBookmarkArgs<'e, 'c, E>,
)
    -> AnyhowResult<UserBookmarkToken>
  where E: 'e + Executor<'c, Database = MySql>
{

  let user_bookmark_token = UserBookmarkToken::generate();
  let (entity_type, entity_token) = args.entity_token.get_composite_keys();

  let query_result = sqlx::query!(
        r#"
INSERT INTO user_bookmarks
SET
  token = ?,
  user_token = ?,
  entity_type = ?,
  entity_token = ?

ON DUPLICATE KEY UPDATE
  deleted_at = NULL,
  version = version + 1
        "#,
      &user_bookmark_token,
      args.user_token,
      entity_type,
      entity_token,
    )
      .execute(args.mysql_executor)
      .await;

  let _record_id = match query_result {
    Ok(res) => res.last_insert_id(),
    Err(err) => return Err(anyhow!("Mysql error: {:?}", err)),
  };

  Ok(user_bookmark_token)
}
