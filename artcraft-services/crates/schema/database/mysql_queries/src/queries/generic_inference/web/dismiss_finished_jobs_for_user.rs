use anyhow::anyhow;
use sqlx;
use sqlx::MySql;
use sqlx::pool::PoolConnection;

use errors::AnyhowResult;
use tokens::tokens::users::UserToken;

/// Mark finished jobs as dismissed by the user.
pub async fn dismiss_finished_jobs_for_user(
  mysql_connection: &mut PoolConnection<MySql>,
  user_token: &UserToken
) -> AnyhowResult<()> {
  let query_result = sqlx::query!(
        r#"
UPDATE generic_inference_jobs
SET
  is_dismissed_by_user = TRUE
WHERE maybe_creator_user_token = ?
AND status IN (
  'dead',
  'cancelled_by_system',
  'cancelled_by_user',
  'complete_failure',
  'complete_success'
 )
AND created_at > DATE_SUB(NOW(), INTERVAL 36 HOUR)
LIMIT 1000
        "#,
        user_token,
    )
      .execute(&mut **mysql_connection)
      .await;

  if let Err(err) = query_result {
    return Err(anyhow!("error with dismiss jobs query: {:?}", err));
  }

  Ok(())
}
