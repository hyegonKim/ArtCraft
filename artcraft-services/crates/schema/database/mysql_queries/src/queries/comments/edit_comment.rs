use std::marker::PhantomData;

use anyhow::anyhow;
use sqlx::{Executor, MySql};

use errors::AnyhowResult;
use tokens::tokens::comments::CommentToken;

pub struct Args<'e, 'c, E>
  where E: 'e + Executor<'c, Database = MySql>
{
  pub comment_token: &'e CommentToken,

  pub comment_markdown: &'e str,
  pub comment_rendered_html: &'e str,

  pub editor_ip_address: &'e str,

  pub mysql_executor: E,

  // NB: phantom can be passed as Default::default()
  // TODO: Not sure if this works to tell the compiler we need the lifetime annotation.
  //  See: https://doc.rust-lang.org/std/marker/struct.PhantomData.html#unused-lifetime-parameters
  pub phantom: PhantomData<&'c E>,
}

pub async fn edit_comment<'e, 'c : 'e, E>(
  args: Args<'e, 'c, E>,
) -> AnyhowResult<()>
  where E: 'e + Executor<'c, Database = MySql>
{
  let query_result = sqlx::query!(
        r#"
UPDATE comments
SET
  comment_markdown = ?,
  comment_rendered_html = ?,
  editor_ip_address = ?,
  edited_at = NOW(),
  version = version + 1
WHERE
  token = ?
        "#,
      args.comment_markdown,
      args.comment_rendered_html,
      args.editor_ip_address,
      args.comment_token,
    )
      .execute(args.mysql_executor)
      .await;

  let _record_id = match query_result {
    Ok(res) => res.last_insert_id(),
    Err(err) => return Err(anyhow!("Mysql error: {:?}", err)),
  };

  Ok(())
}
