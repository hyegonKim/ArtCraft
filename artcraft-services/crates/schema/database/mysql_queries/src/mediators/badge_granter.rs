use anyhow::anyhow;
use log::warn;
use sqlx::error::Error::Database;
use sqlx::mysql::MySqlQueryResult;
use sqlx::MySqlPool;

use errors::AnyhowResult;
use reusable_types::achievements::user_badge::UserBadge;
use tokens::tokens::users::UserToken;

use crate::mediators::firehose_publisher::FirehosePublisher;

#[derive(Clone)]
pub struct BadgeGranter {
  pub mysql_pool: MySqlPool,
  pub firehose_publisher: FirehosePublisher, // NB: Type is Copy/Clone safe due to internal Arc.
}

struct ExistenceRecord {
  pub does_exist: i64,
}

struct CountRecord {
  pub count: i64,
}

impl BadgeGranter {

  pub async fn grant_early_user_badge(&self, user_token: &str) -> AnyhowResult<()> {
    let _record_id = self.insert(
      UserBadge::EarlyUser,
      user_token,
    ).await?;
    Ok(())
  }

  /// This needs to be called *after* successful upload.
  pub async fn maybe_grant_tts_model_uploads_badge(&self, user_token: &UserToken) -> AnyhowResult<()> {
    let count = self.count_tts_models_uploaded(user_token.as_str()).await?;

    let mut maybe_badge = None;

    if count >= 1000 {
      maybe_badge = Some(UserBadge::TtsModelUploader1000);
    } else if count >= 500 {
      maybe_badge = Some(UserBadge::TtsModelUploader500);
    } else if count >= 250 {
      maybe_badge = Some(UserBadge::TtsModelUploader250);
    } else if count >= 200 {
      maybe_badge = Some(UserBadge::TtsModelUploader200);
    } else if count >= 150 {
      maybe_badge = Some(UserBadge::TtsModelUploader150);
    } else if count >= 100 {
      maybe_badge = Some(UserBadge::TtsModelUploader100);
    } else if count >= 50 {
      maybe_badge = Some(UserBadge::TtsModelUploader50);
    } else if count >= 20 {
      maybe_badge = Some(UserBadge::TtsModelUploader20);
    } else if count >= 10 {
      maybe_badge = Some(UserBadge::TtsModelUploader10);
    } else if count >= 5 {
      maybe_badge = Some(UserBadge::TtsModelUploader5);
    } else if count >= 1 {
      maybe_badge = Some(UserBadge::TtsModelUploader1);
    }

    let badge = match maybe_badge {
      Some(badge) => badge,
      None => return Ok(()),
    };

    if self.has_badge(user_token.as_str(), badge).await? {
      return Ok(())
    }

    let _record_id = self.insert(
      badge,
      user_token.as_str(),
    ).await?;

    self.firehose_publisher.publish_user_badge_granted(user_token.as_str(), badge.to_db_value())
        .await?;

    Ok(())
  }

  /// This needs to be called *after* successful upload.
  pub async fn maybe_grant_voice_conversion_model_uploads_badge(&self, user_token: &UserToken) -> AnyhowResult<()> {
    let count = self.count_voice_conversion_models_uploaded(user_token).await?;

    let mut maybe_badge = None;

    if count >= 1000 {
      maybe_badge = Some(UserBadge::VoiceConversionModelUploader1000);
    } else if count >= 500 {
      maybe_badge = Some(UserBadge::VoiceConversionModelUploader500);
    } else if count >= 250 {
      maybe_badge = Some(UserBadge::VoiceConversionModelUploader250);
    } else if count >= 200 {
      maybe_badge = Some(UserBadge::VoiceConversionModelUploader200);
    } else if count >= 150 {
      maybe_badge = Some(UserBadge::VoiceConversionModelUploader150);
    } else if count >= 100 {
      maybe_badge = Some(UserBadge::VoiceConversionModelUploader100);
    } else if count >= 50 {
      maybe_badge = Some(UserBadge::VoiceConversionModelUploader50);
    } else if count >= 20 {
      maybe_badge = Some(UserBadge::VoiceConversionModelUploader20);
    } else if count >= 10 {
      maybe_badge = Some(UserBadge::VoiceConversionModelUploader10);
    } else if count >= 5 {
      maybe_badge = Some(UserBadge::VoiceConversionModelUploader5);
    } else if count >= 1 {
      maybe_badge = Some(UserBadge::VoiceConversionModelUploader1);
    }

    let badge = match maybe_badge {
      Some(badge) => badge,
      None => return Ok(()),
    };

    if self.has_badge(user_token.as_str(), badge).await? {
      return Ok(())
    }

    let _record_id = self.insert(
      badge,
      user_token.as_str(),
    ).await?;

    self.firehose_publisher.publish_user_badge_granted(user_token.as_str(), badge.to_db_value())
        .await?;

    Ok(())
  }


  /// This needs to be called *after* successful upload.
  pub async fn maybe_grant_w2l_template_uploads_badge(&self, user_token: &str) -> AnyhowResult<()> {
    let count = self.count_w2l_templates_uploaded(user_token).await?;

    let mut maybe_badge = None;

    if count >= 10000 {
      maybe_badge = Some(UserBadge::W2lTemplateUploader10000);
    } else if count >= 5000 {
      maybe_badge = Some(UserBadge::W2lTemplateUploader5000);
    } else if count >= 2000 {
      maybe_badge = Some(UserBadge::W2lTemplateUploader2000);
    } else if count >= 1000 {
      maybe_badge = Some(UserBadge::W2lTemplateUploader1000);
    } else if count >= 500 {
      maybe_badge = Some(UserBadge::W2lTemplateUploader500);
    } else if count >= 200 {
      maybe_badge = Some(UserBadge::W2lTemplateUploader200);
    } else if count >= 100 {
      maybe_badge = Some(UserBadge::W2lTemplateUploader100);
    } else if count >= 50 {
      maybe_badge = Some(UserBadge::W2lTemplateUploader50);
    } else if count >= 10 {
      maybe_badge = Some(UserBadge::W2lTemplateUploader10);
    } else if count >= 1 {
      maybe_badge = Some(UserBadge::W2lTemplateUploader1);
    }

    let badge = match maybe_badge {
      Some(badge) => badge,
      None => return Ok(()),
    };

    if self.has_badge(user_token, badge).await? {
      return Ok(())
    }

    let _record_id = self.insert(
      badge,
      user_token,
    ).await?;

    self.firehose_publisher.publish_user_badge_granted(user_token, badge.to_db_value())
        .await?;

    Ok(())
  }

  /// This needs to be called *after* successful upload.
  pub async fn maybe_grant_vocoder_model_uploads_badge(&self, user_token: &UserToken) -> AnyhowResult<()> {
    let count = self.count_vocoder_models_uploaded(user_token.as_str()).await?;

    let mut maybe_badge = None;

    if count >= 1000 {
      maybe_badge = Some(UserBadge::VocoderModelUploader1000);
    } else if count >= 500 {
      maybe_badge = Some(UserBadge::VocoderModelUploader500);
    } else if count >= 250 {
      maybe_badge = Some(UserBadge::VocoderModelUploader250);
    } else if count >= 200 {
      maybe_badge = Some(UserBadge::VocoderModelUploader200);
    } else if count >= 150 {
      maybe_badge = Some(UserBadge::VocoderModelUploader150);
    } else if count >= 100 {
      maybe_badge = Some(UserBadge::VocoderModelUploader100);
    } else if count >= 50 {
      maybe_badge = Some(UserBadge::VocoderModelUploader50);
    } else if count >= 20 {
      maybe_badge = Some(UserBadge::VocoderModelUploader20);
    } else if count >= 10 {
      maybe_badge = Some(UserBadge::VocoderModelUploader10);
    } else if count >= 5 {
      maybe_badge = Some(UserBadge::VocoderModelUploader5);
    } else if count >= 1 {
      maybe_badge = Some(UserBadge::VocoderModelUploader1);
    }

    let badge = match maybe_badge {
      Some(badge) => badge,
      None => return Ok(()),
    };

    if self.has_badge(user_token.as_str(), badge).await? {
      return Ok(())
    }

    let _record_id = self.insert(
      badge,
      user_token.as_str(),
    ).await?;

    self.firehose_publisher.publish_user_badge_granted(user_token.as_str(), badge.to_db_value())
        .await?;

    Ok(())
  }

  /// This needs to be called *after* successful upload.
  pub async fn maybe_grant_softvc_vocoder_model_uploads_badge(&self, user_token: &UserToken) -> AnyhowResult<()> {
    let count = self.count_softvc_vocoder_models_uploaded(user_token.as_str()).await?;

    let mut maybe_badge = None;

    if count >= 1000 {
      maybe_badge = Some(UserBadge::VocoderRocketVcModelUploader1000);
    } else if count >= 500 {
      maybe_badge = Some(UserBadge::VocoderRocketVcModelUploader500);
    } else if count >= 250 {
      maybe_badge = Some(UserBadge::VocoderRocketVcModelUploader250);
    } else if count >= 200 {
      maybe_badge = Some(UserBadge::VocoderRocketVcModelUploader200);
    } else if count >= 150 {
      maybe_badge = Some(UserBadge::VocoderRocketVcModelUploader150);
    } else if count >= 100 {
      maybe_badge = Some(UserBadge::VocoderRocketVcModelUploader100);
    } else if count >= 50 {
      maybe_badge = Some(UserBadge::VocoderRocketVcModelUploader50);
    } else if count >= 20 {
      maybe_badge = Some(UserBadge::VocoderRocketVcModelUploader20);
    } else if count >= 10 {
      maybe_badge = Some(UserBadge::VocoderRocketVcModelUploader10);
    } else if count >= 5 {
      maybe_badge = Some(UserBadge::VocoderRocketVcModelUploader5);
    } else if count >= 1 {
      maybe_badge = Some(UserBadge::VocoderRocketVcModelUploader1);
    }

    let badge = match maybe_badge {
      Some(badge) => badge,
      None => return Ok(()),
    };

    if self.has_badge(user_token.as_str(), badge).await? {
      return Ok(())
    }

    let _record_id = self.insert(
      badge,
      user_token.as_str(),
    ).await?;

    self.firehose_publisher.publish_user_badge_granted(user_token.as_str(), badge.to_db_value())
        .await?;

    Ok(())
  }

  // =======================================================================

  pub async fn has_badge(&self, user_token: &str, user_badge: UserBadge) -> AnyhowResult<bool> {
    let maybe_result = sqlx::query_as!(
      ExistenceRecord,
        r#"
SELECT 1 as does_exist
FROM user_badges
WHERE
  user_token = ?
AND
  badge_slug = ?
LIMIT 1
        "#,
      user_token,
      user_badge.to_db_value()
    )
        .fetch_one(&self.mysql_pool)
        .await;

    let exists = match maybe_result {
      Ok(_record) => true,
      Err(err) => {
        match err {
          sqlx::Error::RowNotFound => false,
          _ => {
            warn!("query error: {:?}", err);
            return Err(anyhow!("error querying: {:?}", err));
          }
        }
      }
    };

    Ok(exists)
  }

  // =======================================================================

  async fn count_tts_models_uploaded(
    &self,
    user_token: &str,
  ) -> AnyhowResult<u64> {
    // NB: This could get expensive!
    let maybe_result = sqlx::query_as!(
      CountRecord,
        r#"
SELECT count(*) as count
FROM tts_models
WHERE
  creator_user_token = ?
LIMIT 1
        "#,
      user_token
    )
        .fetch_one(&self.mysql_pool)
        .await;

    self.handle_count_query(maybe_result)
  }

  async fn count_voice_conversion_models_uploaded(
    &self,
    user_token: &UserToken,
  ) -> AnyhowResult<u64> {
    // NB: This could get expensive!
    let maybe_result = sqlx::query_as!(
      CountRecord,
        r#"
SELECT count(*) as count
FROM voice_conversion_models
WHERE
  creator_user_token = ?
LIMIT 1
        "#,
      user_token
    )
        .fetch_one(&self.mysql_pool)
        .await;

    self.handle_count_query(maybe_result)
  }

  async fn count_vocoder_models_uploaded(
    &self,
    user_token: &str,
  ) -> AnyhowResult<u64> {
    // NB: This could get expensive!
    let maybe_result = sqlx::query_as!(
      CountRecord,
        r#"
SELECT count(*) as count
FROM vocoder_models
WHERE
  creator_user_token = ?
  AND vocoder_type = 'hifigan'
LIMIT 1
        "#,
      user_token
    )
        .fetch_one(&self.mysql_pool)
        .await;

    self.handle_count_query(maybe_result)
  }

  async fn count_softvc_vocoder_models_uploaded(
    &self,
    user_token: &str,
  ) -> AnyhowResult<u64> {
    // NB: This could get expensive!
    let maybe_result = sqlx::query_as!(
      CountRecord,
        r#"
SELECT count(*) as count
FROM vocoder_models
WHERE
  creator_user_token = ?
  AND vocoder_type = 'hifigan_rocket_vc'
LIMIT 1
        "#,
      user_token
    )
        .fetch_one(&self.mysql_pool)
        .await;

    self.handle_count_query(maybe_result)
  }

  async fn count_w2l_templates_uploaded(
    &self,
    user_token: &str,
  ) -> AnyhowResult<u64> {
    // NB: This could get expensive!
    // Especially at the scale we'll likely have W2L templates.
    let maybe_result = sqlx::query_as!(
      CountRecord,
        r#"
SELECT count(*) as count
FROM w2l_templates
WHERE
  creator_user_token = ?
LIMIT 1
        "#,
      user_token
    )
        .fetch_one(&self.mysql_pool)
        .await;

    self.handle_count_query(maybe_result)
  }

  async fn insert(
    &self,
    user_badge: UserBadge,
    user_token: &str,
  ) -> AnyhowResult<u64> {
    let query_result = sqlx::query!(
        r#"
INSERT INTO user_badges
SET
  user_token = ?,
  badge_slug = ?
        "#,
      user_token,
      user_badge.to_db_value(),
    )
        .execute(&self.mysql_pool)
        .await;

    let record_id = Self::handle_results(query_result)?;
    Ok(record_id)
  }

  fn handle_results(query_result: Result<MySqlQueryResult, sqlx::Error>) -> AnyhowResult<u64> {
    let record_id = match query_result {
      Ok(res) => {
        res.last_insert_id()
      },
      Err(err) => {
        warn!("Insert badge record DB error: {:?}", err);

        // NB: SQLSTATE[23000]: Integrity constraint violation
        // NB: MySQL Error Code 1062: Duplicate key insertion (this is harder to access)
        match err {
          Database(err) => {
            let _maybe_code = err.code().map(|c| c.into_owned());
            /*match maybe_code.as_deref() {
              Some("23000") => {
                if err.message().contains("username") {
                  return Err(UsernameTaken);
                } else if err.message().contains("email_address") {
                  return Err(EmailTaken);
                }
              }
              _ => {},
            }*/
          },
          _ => {},
        }
        return Err(anyhow!("Error inserting record"));
      }
    };

    Ok(record_id)
  }

  fn handle_count_query(&self, query_result: Result<CountRecord, sqlx::Error>) -> AnyhowResult<u64> {
    let count = match query_result {
      Ok(record) => record.count as u64,
      Err(err) => {
        match err {
          sqlx::Error::RowNotFound => 0,
          _ => {
            warn!("query error: {:?}", err);
            return Err(anyhow!("error querying: {:?}", err));
          }
        }
      }
    };

    Ok(count)
  }
}
