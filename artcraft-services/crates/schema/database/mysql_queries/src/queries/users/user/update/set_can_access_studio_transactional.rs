use anyhow::anyhow;
use sqlx::{MySql, Transaction};

use errors::AnyhowResult;
use tokens::tokens::users::UserToken;

pub struct SetCanAccessStudioArgs<'a, 'b> {
  // The action's target user token.
  pub subject_user_token: &'a UserToken,

  pub can_access_studio: bool,

  pub transaction: &'a mut Transaction<'b, MySql>,
}

pub async fn set_can_access_studio_transactional(args: SetCanAccessStudioArgs<'_, '_>) -> AnyhowResult<()> {
  let query_result = sqlx::query!(
        r#"
UPDATE users
SET
    can_access_studio = ?,
    version = version + 1

WHERE users.token = ?
LIMIT 1
        "#,
      args.can_access_studio,
      args.subject_user_token,
    )
      .execute(&mut **args.transaction)
      .await;

  if let Err(err) = query_result {
    return Err(anyhow!("error with query: {:?}", err));
  }

  Ok(())
}
