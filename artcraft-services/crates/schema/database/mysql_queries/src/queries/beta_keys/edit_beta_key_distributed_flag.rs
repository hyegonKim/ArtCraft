use sqlx::{Executor, MySql};

use errors::AnyhowResult;
use tokens::tokens::beta_keys::BetaKeyToken;

pub async fn edit_beta_key_distributed_flag<'a, 'e, E>(
  token: &'a BetaKeyToken,
  is_distributed: bool,
  mysql_executor: E
)
  -> AnyhowResult<()>
  where E: 'a + Executor<'e, Database = MySql>
{
      sqlx::query!(
        r#"
UPDATE beta_keys
SET
  is_distributed = ?
WHERE
  token = ?
LIMIT 1
        "#,
       is_distributed,
       token.as_str(),
      )
      .execute(mysql_executor)
      .await?;

  Ok(())
}
