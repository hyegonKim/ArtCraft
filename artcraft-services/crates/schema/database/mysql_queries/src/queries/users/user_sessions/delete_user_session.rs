use sqlx::MySqlPool;

use errors::AnyhowResult;

pub async fn delete_user_session(session_token: &str, mysql_pool: &MySqlPool) -> AnyhowResult<()> {
  let _query_result = sqlx::query!(
        r#"
UPDATE user_sessions
SET deleted_at = CURRENT_TIMESTAMP()
WHERE
    token = ?
    AND deleted_at IS NULL
        "#,
        session_token.to_string(),
    )
      .execute(mysql_pool)
      .await;

  Ok(())
}
