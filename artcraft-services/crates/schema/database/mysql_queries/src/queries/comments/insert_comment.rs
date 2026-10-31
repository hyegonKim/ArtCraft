use std::marker::PhantomData;

use anyhow::anyhow;
use sqlx::{Executor, MySql};

use errors::AnyhowResult;
use tokens::tokens::comments::CommentToken;
use tokens::tokens::users::UserToken;

use crate::queries::comments::comment_entity_token::CommentEntityToken;

pub struct InsertCommentArgs<'e, 'c, E>
  where E: 'e + Executor<'c, Database = MySql>
{
  pub entity_token: &'e CommentEntityToken,

  pub uuid_idempotency_token: &'e str,

  pub user_token: &'e UserToken,

  pub comment_markdown: &'e str,
  pub comment_rendered_html: &'e str,

  pub creator_ip_address: &'e str,

  pub mysql_executor: E,

  // TODO: Not sure if this works to tell the compiler we need the lifetime annotation.
  //  See: https://doc.rust-lang.org/std/marker/struct.PhantomData.html#unused-lifetime-parameters
  pub phantom: PhantomData<&'c E>,
}

pub async fn insert_comment<'e, 'c : 'e, E>(
  args: InsertCommentArgs<'e, 'c, E>,
)
  -> AnyhowResult<CommentToken>
  where E: 'e + Executor<'c, Database = MySql>
{

  let comment_token = CommentToken::generate();
  let (entity_type, entity_token) = args.entity_token.get_composite_keys();

  let query_result = sqlx::query!(
        r#"
INSERT INTO comments
SET
  token = ?,
  uuid_idempotency_token = ?,
  user_token = ?,
  entity_type = ?,
  entity_token = ?,
  comment_markdown = ?,
  comment_rendered_html = ?,
  creator_ip_address = ?,
  editor_ip_address = ?
        "#,
      &comment_token,
      args.uuid_idempotency_token,
      args.user_token,
      entity_type,
      entity_token,
      args.comment_markdown,
      args.comment_rendered_html,
      args.creator_ip_address,
      args.creator_ip_address,
    )
      .execute(args.mysql_executor)
      .await;

  let _record_id = match query_result {
    Ok(res) => res.last_insert_id(),
    Err(err) => return Err(anyhow!("Mysql error: {:?}", err)),
  };

  Ok(comment_token)
}
