use sqlx::MySql;
use sqlx::pool::PoolConnection;

use errors::AnyhowResult;
use tokens::tokens::tts_models::TtsModelToken;

pub async fn update_tts_model_ratings(
  tts_model_token: &TtsModelToken,
  mysql_connection: &mut PoolConnection<MySql>
) -> AnyhowResult<()> {
  let token = tts_model_token.as_str();
  let query = sqlx::query!(
        r#"
UPDATE tts_models
SET
  user_ratings_total_count = (
    SELECT COUNT(*)
    FROM user_ratings
    WHERE entity_type = "tts_model"
    AND entity_token = ?
    AND rating_value IN ("positive", "negative")
  ),
  user_ratings_positive_count = (
    SELECT COUNT(*)
    FROM user_ratings
    WHERE entity_type = "tts_model"
    AND entity_token = ?
    AND rating_value = "positive"
  ),
  user_ratings_negative_count = (
    SELECT COUNT(*)
    FROM user_ratings
    WHERE entity_type = "tts_model"
    AND entity_token = ?
    AND rating_value = "negative"
  ),
  version = version + 1

WHERE
  token = ?
LIMIT 1
        "#,
      // Args
      token,
      token,
      token,
      token
    );

  let _r = query.execute(&mut** mysql_connection).await?;
  Ok(())
}
