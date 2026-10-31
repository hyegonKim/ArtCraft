use log::info;
use sqlx::MySql;
use sqlx::pool::PoolConnection;

use errors::AnyhowResult;
use tokens::tokens::tts_models::TtsModelToken;

/// List of all TTS model tokens.
/// This is typically used to run jobs that calculate analytics on models.
/// Currently does not need to be batched, but future scale may dictate batching.
#[derive(Clone)]
pub struct TtsModelTokens {
  pub tokens: Vec<TtsModelTokenInfo>,
}

#[derive(Clone)]
pub struct TtsModelTokenInfo {
  pub token: TtsModelToken,
  //pub ietf_language_tag: String,
  //pub ietf_primary_language_subtag: String,
  //pub creator_set_visibility: Visibility,
  //pub maybe_user_deleted_at: Option<DateTime<Utc>>,
  //pub maybe_mod_deleted_at: Option<DateTime<Utc>>,
}

pub async fn list_all_tts_model_tokens(mysql_connection: &mut PoolConnection<MySql>) -> AnyhowResult<TtsModelTokens> {
  info!("Querying for all TTS model tokens");

  let tokens = sqlx::query_as!(
    RawTtsModelTokenInfo,
        r#"
SELECT
    token as `token: tokens::tokens::tts_models::TtsModelToken`
FROM tts_models
        "#)
        .fetch_all(&mut **mysql_connection)
        .await?;

  let tokens = tokens.into_iter()
      .map(|token| TtsModelTokenInfo {
        token: token.token,
      })
      .collect::<Vec<TtsModelTokenInfo>>();

  Ok(TtsModelTokens {
    tokens,
  })
}

struct RawTtsModelTokenInfo {
  pub token: TtsModelToken,
}
