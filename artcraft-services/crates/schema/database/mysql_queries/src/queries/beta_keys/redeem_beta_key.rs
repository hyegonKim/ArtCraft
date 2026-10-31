use sqlx::{MySql, Transaction};

use errors::AnyhowResult;
use tokens::tokens::users::UserToken;

pub async fn redeem_beta_key<'a, 'b>(
  key_value: &'a str,
  redeemer_user_token: &'a UserToken,
  //mysql_executor: E
  transaction: &'a mut Transaction<'b, MySql>,
)
  -> AnyhowResult<()>
  //where E: 'e + Executor<'c, Database = MySql>
{
      sqlx::query!(
        r#"
UPDATE beta_keys
SET
  maybe_redeemer_user_token = ?,
  maybe_redeemed_at = CURRENT_TIMESTAMP
WHERE
  key_value = ?
LIMIT 1
        "#,
       redeemer_user_token.as_str(),
       key_value
      )
      .execute(&mut **transaction)
      .await?;

  Ok(())
}
