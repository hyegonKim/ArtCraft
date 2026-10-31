// NB: Incrementally getting rid of build warnings...
#![forbid(unused_imports)]
#![forbid(unused_mut)]
#![forbid(unused_variables)]

use sqlx;
use sqlx::mysql::MySqlQueryResult;
use sqlx::MySqlPool;

pub async fn edit_tts_model_moderator_details(
  mysql_pool: &MySqlPool,
  tts_model_token: &str,
  is_public_listing_approved: bool,
  is_locked_from_user_modification: bool,
  is_locked_from_use: bool,
  maybe_suggested_unique_bot_command: Option<&str>,
  is_front_page_featured: bool,
  is_twitch_featured: bool,
  moderator_user_token: &str,
  maybe_mod_comments: Option<&str>,
  use_default_mel_multiply_factor: bool,
  maybe_custom_mel_multiply_factor: Option<f64>,
) -> Result<MySqlQueryResult, sqlx::Error> {
  sqlx::query!(
        r#"
UPDATE tts_models
SET
    is_public_listing_approved = ?,
    is_locked_from_user_modification = ?,
    is_locked_from_use = ?,
    maybe_mod_comments = ?,
    maybe_mod_user_token = ?,
    is_front_page_featured = ?,
    is_twitch_featured = ?,
    maybe_suggested_unique_bot_command = ?,
    use_default_mel_multiply_factor = ?,
    maybe_custom_mel_multiply_factor = ?,
    version = version + 1
WHERE token = ?
LIMIT 1
        "#,
      is_public_listing_approved,
      is_locked_from_user_modification,
      is_locked_from_use,
      maybe_mod_comments,
      moderator_user_token,
      is_front_page_featured,
      is_twitch_featured,
      maybe_suggested_unique_bot_command,
      use_default_mel_multiply_factor,
      maybe_custom_mel_multiply_factor,
      tts_model_token
    )
      .execute(mysql_pool)
      .await
}
