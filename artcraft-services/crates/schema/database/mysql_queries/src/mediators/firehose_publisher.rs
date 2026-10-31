use anyhow::anyhow;
use log::warn;
use sqlx::error::Error::Database;
use sqlx::mysql::MySqlQueryResult;
use sqlx::MySqlPool;

use errors::AnyhowResult;
use tokens::tokens::comments::CommentToken;
use tokens::tokens::firehose_entries::FirehoseEntryToken;
use tokens::tokens::generic_inference_jobs::InferenceJobToken;
use tokens::tokens::media_uploads::MediaUploadToken;
use tokens::tokens::users::UserToken;

// TODO(bt, 2022-12-19): Convert this to a database 'enum'. Also, create an 'enums' package similar to 'tokens'.
#[derive(Debug, Clone, Copy)]
enum FirehoseEvent {
  UserSignUp,

  // NB: We don't publish for all badges (eg. early user signup)
  UserBadgeGranted,

  TtsModelUploadStarted,
  TtsModelUploadCompleted,
  TtsInferenceStarted,
  TtsInferenceCompleted,

  W2lTemplateUploadStarted,
  W2lTemplateUploadCompleted,
  W2lInferenceStarted,
  W2lInferenceCompleted,

  VcInferenceStarted,
  VcInferenceCompleted,

  // SadTalker, not Wav2Lip
  LipsyncAnimationStarted,
  LipsyncAnimationCompleted,

  GenericDownloadStarted,
  GenericDownloadCompleted,

  MediaUploaded,
  DeviceMediaRecorded,

  CommentCreated,

  // TODO(bt, 2022-12-19): Are the following unused, merely planned (?)
  TwitterMention,
  TwitterRetweet,
  DiscordJoin,
  DiscordMessage,
  TwitchSubscribe,
  TwitchFollow,
}

impl FirehoseEvent {
  pub fn to_db_value(&self) -> &'static str {
    match self {
      FirehoseEvent::UserSignUp => "user_sign_up",
      FirehoseEvent::UserBadgeGranted=> "user_badge_granted",
      FirehoseEvent::TtsModelUploadStarted => "tts_model_upload_started",
      FirehoseEvent::TtsModelUploadCompleted => "tts_model_upload_completed",
      FirehoseEvent::TtsInferenceStarted => "tts_inference_started",
      FirehoseEvent::TtsInferenceCompleted => "tts_inference_completed",
      FirehoseEvent::W2lTemplateUploadStarted => "w2l_template_upload_started",
      FirehoseEvent::W2lTemplateUploadCompleted => "w2l_template_upload_completed",
      FirehoseEvent::W2lInferenceStarted => "w2l_inference_started",
      FirehoseEvent::W2lInferenceCompleted => "w2l_inference_completed",
      FirehoseEvent::VcInferenceStarted => "vc_inference_started",
      FirehoseEvent::VcInferenceCompleted => "vc_inference_completed",
      FirehoseEvent::LipsyncAnimationStarted => "lipsync_animation_started",
      FirehoseEvent::LipsyncAnimationCompleted => "lipsync_animation_completed",
      FirehoseEvent::GenericDownloadStarted => "generic_download_started",
      FirehoseEvent::GenericDownloadCompleted => "generic_download_completed",
      FirehoseEvent::MediaUploaded => "media_uploaded",
      FirehoseEvent::DeviceMediaRecorded => "device_media_recorded",

      FirehoseEvent::CommentCreated => "comment_created",

      // Are the following unused (?)
      FirehoseEvent::TwitterMention => "twitter_mention",
      FirehoseEvent::TwitterRetweet => "twitter_retweet",
      FirehoseEvent::DiscordJoin => "discord_join",
      FirehoseEvent::DiscordMessage => "discord_message",
      FirehoseEvent::TwitchSubscribe => "twitch_subscribe",
      FirehoseEvent::TwitchFollow => "twitch_follow",
    }
  }
}

#[derive(Clone)]
pub struct FirehosePublisher {
  pub mysql_pool: MySqlPool,
}

impl FirehosePublisher {

  pub async fn publish_user_sign_up(&self, user_token: &str) -> AnyhowResult<()> {
    let _record_id = self.insert(
    FirehoseEvent::UserSignUp,
      Some(user_token),
      Some(user_token),
    Some(user_token)
    ).await?;
    Ok(())
  }

  pub async fn publish_user_badge_granted(&self, user_token: &str, badge_slug: &str) -> AnyhowResult<()> {
    let _record_id = self.insert(
      FirehoseEvent::UserBadgeGranted,
      Some(user_token),
      Some(badge_slug),
      None,
    ).await?;
    Ok(())
  }

  pub async fn enqueue_tts_model_upload(&self, user_token: &str, job_token: &str) -> AnyhowResult<()> {
    let _record_id = self.insert(
      FirehoseEvent::TtsModelUploadStarted,
      Some(user_token),
      None,
      Some(job_token)
    ).await?;
    Ok(())
  }

  pub async fn publish_tts_model_upload_finished(&self, user_token: &UserToken, model_token: &str) -> AnyhowResult<()> {
    let _record_id = self.insert(
      FirehoseEvent::TtsModelUploadCompleted,
      Some(user_token.as_str()),
      Some(model_token),
      Some(model_token)
    ).await?;
    Ok(())
  }

  pub async fn enqueue_tts_inference(
    &self,
    maybe_user_token: Option<&str>,
    job_token: &str,
    model_token: &str
  ) -> AnyhowResult<()> {
    let _record_id = self.insert(
      FirehoseEvent::TtsInferenceStarted,
      maybe_user_token,
      Some(model_token),
      Some(job_token)
    ).await?;
    Ok(())
  }

  pub async fn tts_inference_finished(
    &self,
    maybe_user_token: Option<&str>,
    model_token: &str,
    result_token: &str
  ) -> AnyhowResult<()> {
    let _record_id = self.insert(
      FirehoseEvent::TtsInferenceCompleted,
      maybe_user_token,
      Some(model_token),
      Some(result_token)
    ).await?;
    Ok(())
  }

  pub async fn enqueue_w2l_template_upload(&self, user_token: &str, job_token: &str) -> AnyhowResult<()> {
    let _record_id = self.insert(
      FirehoseEvent::W2lTemplateUploadStarted,
      Some(user_token),
      None,
      Some(job_token)
    ).await?;
    Ok(())
  }

  pub async fn enqueue_w2l_inference(&self, maybe_user_token: Option<&str>, job_token: &str, template_token: &str) -> AnyhowResult<()> {
    let _record_id = self.insert(
      FirehoseEvent::W2lInferenceStarted,
      maybe_user_token,
      Some(template_token),
      Some(job_token)
    ).await?;
    Ok(())
  }

  pub async fn publish_w2l_template_upload_finished(&self, user_token: &str, template_token: &str) -> AnyhowResult<()> {
    let _record_id = self.insert(
    FirehoseEvent::W2lTemplateUploadCompleted,
      Some(user_token),
      Some(template_token),
    Some(template_token)
    ).await?;
    Ok(())
  }

  pub async fn w2l_inference_finished(&self, maybe_user_token: Option<&str>, job_token: &str, result_token: &str) -> AnyhowResult<()> {
    let _record_id = self.insert(
      FirehoseEvent::W2lInferenceCompleted,
      maybe_user_token,
      Some(job_token), // TODO: This could be template_token
      Some(result_token)
    ).await?;
    Ok(())
  }

  pub async fn enqueue_vc_inference(&self, maybe_user_token: Option<&UserToken>, inference_job_token: &InferenceJobToken) -> AnyhowResult<()> {
    let _record_id = self.insert(
      FirehoseEvent::VcInferenceStarted,
      maybe_user_token.map(|u| u.as_str()),
      Some(inference_job_token.as_str()),
      Some(inference_job_token.as_str()),
    ).await?;
    Ok(())
  }

  // TODO: Change result token type.
  pub async fn vc_inference_finished(&self, maybe_user_token: Option<&UserToken>, inference_job_token: &InferenceJobToken, result_token: &str) -> AnyhowResult<()> {
    let _record_id = self.insert(
      FirehoseEvent::VcInferenceCompleted,
      maybe_user_token.map(|u| u.as_str()),
      Some(inference_job_token.as_str()), // TODO: This could be vc model token
      Some(result_token)
    ).await?;
    Ok(())
  }

  pub async fn enqueue_lipsync_animation(&self, maybe_user_token: Option<&UserToken>, inference_job_token: &InferenceJobToken) -> AnyhowResult<()> {
    let _record_id = self.insert(
      FirehoseEvent::LipsyncAnimationStarted,
      maybe_user_token.map(|u| u.as_str()),
      Some(inference_job_token.as_str()),
      Some(inference_job_token.as_str()),
    ).await?;
    Ok(())
  }

  // TODO: Change result token type.
  pub async fn lipsync_animation_finished(&self, maybe_user_token: Option<&UserToken>, inference_job_token: &InferenceJobToken, result_token: &str) -> AnyhowResult<()> {
    let _record_id = self.insert(
      FirehoseEvent::LipsyncAnimationCompleted,
      maybe_user_token.map(|u| u.as_str()),
      Some(inference_job_token.as_str()), // TODO: Is this pointing to the right entity?
      Some(result_token)
    ).await?;
    Ok(())
  }

  pub async fn enqueue_generic_download(&self, user_token: &str, job_token: &str) -> AnyhowResult<()> {
    let _record_id = self.insert(
      FirehoseEvent::GenericDownloadStarted,
      Some(user_token),
      None,
      Some(job_token)
    ).await?;
    Ok(())
  }

  // NB: Entity token is optional.
  pub async fn publish_generic_download_finished(&self, user_token: &UserToken, entity_token: Option<&str>) -> AnyhowResult<()> {
    let _record_id = self.insert(
      FirehoseEvent::GenericDownloadCompleted,
      Some(user_token.as_str()),
      entity_token,
      entity_token,
    ).await?;
    Ok(())
  }

  pub async fn publish_media_uploaded(&self, maybe_user_token: Option<&UserToken>, upload_token: &MediaUploadToken) -> AnyhowResult<()> {
    let _record_id = self.insert(
      FirehoseEvent::MediaUploaded,
      maybe_user_token.map(|u| u.as_str()),
      Some(upload_token.as_str()),
      Some(upload_token.as_str()),
    ).await?;
    Ok(())
  }

  pub async fn publish_device_media_recorded(&self, maybe_user_token: Option<&UserToken>, upload_token: &MediaUploadToken) -> AnyhowResult<()> {
    let _record_id = self.insert(
      FirehoseEvent::DeviceMediaRecorded,
      maybe_user_token.map(|u| u.as_str()),
      Some(upload_token.as_str()),
      Some(upload_token.as_str()),
    ).await?;
    Ok(())
  }

  pub async fn publish_comment_created(&self, user_token: &UserToken, comment_token: &CommentToken) -> AnyhowResult<()> {
    let _record_id = self.insert(
      FirehoseEvent::CommentCreated,
      Some(user_token.as_str()),
      None, // TODO: We need a composite key of (entity_type, entity_token).
      Some(comment_token.as_str())
    ).await?;
    Ok(())
  }

  // =======================================================================

  async fn insert(
    &self,
    event_type: FirehoseEvent,
    user_token: Option<&str>,
    entity_token: Option<&str>,
    created_entity_token: Option<&str>
  ) -> AnyhowResult<u64> {
    let token = FirehoseEntryToken::generate().to_string();

    let query_result = sqlx::query!(
        r#"
INSERT INTO firehose_entries
SET
  token = ?,
  event_type = ?,
  maybe_target_user_token = ?,
  maybe_target_entity_token = ?,
  maybe_created_entity_token = ?
        "#,
      &token,
      event_type.to_db_value(),
      user_token,
      entity_token,
      created_entity_token
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
      // TODO(bt, 2022-12-20): I've never richly handled database errors/error codes.
      //  I should revisit these in the future and consider some kind of middleware.
      Err(err) => {
        warn!("Insert record DB error: {:?}", err);

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
}
