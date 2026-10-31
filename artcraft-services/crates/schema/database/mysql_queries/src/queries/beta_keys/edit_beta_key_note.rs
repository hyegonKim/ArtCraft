use sqlx::{Executor, MySql};

use errors::AnyhowResult;
use tokens::tokens::beta_keys::BetaKeyToken;

pub async fn edit_beta_key_note<'a, 'e, E>(
  token: &'a BetaKeyToken,
  maybe_note: Option<&'a str>,
  mysql_executor: E
)
  -> AnyhowResult<()>
  where E: 'a + Executor<'e, Database = MySql>
{
      sqlx::query!(
        r#"
UPDATE beta_keys
SET
  maybe_notes = ?
WHERE
  token = ?
LIMIT 1
        "#,
       maybe_note,
       token.as_str(),
      )
      .execute(mysql_executor)
      .await?;

  Ok(())
}
