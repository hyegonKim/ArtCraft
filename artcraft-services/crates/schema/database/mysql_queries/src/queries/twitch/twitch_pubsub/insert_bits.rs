use anyhow::anyhow;
use sqlx::MySqlPool;

use errors::AnyhowResult;

pub struct TwitchPubsubBitsInsertBuilder {
  // ===== Required Fields =====

  sender_twitch_user_id: Option<String>,
  sender_twitch_username: Option<String>,
  sender_twitch_username_lowercase: Option<String>,

  destination_channel_id: Option<String>,
  destination_channel_name: Option<String>,

  // ===== Optional / Default Fields =====

  is_anonymous: bool,

  bits_used: u64,
  total_bits_used: u64,

  chat_message: String,
}

impl TwitchPubsubBitsInsertBuilder {
  pub fn new() -> Self {
    Self {
      sender_twitch_user_id: None,
      sender_twitch_username: None,
      sender_twitch_username_lowercase: None,
      destination_channel_id: None,
      destination_channel_name: None,
      is_anonymous: false,
      bits_used: 0,
      total_bits_used: 0,
      chat_message: "".to_string(),
    }
  }

  pub fn set_sender_twitch_user_id(mut self, value: &str) -> Self {
    self.sender_twitch_user_id = Some(value.to_string());
    self
  }

  pub fn set_sender_twitch_username(mut self, value: &str) -> Self {
    self.sender_twitch_username = Some(value.to_string());
    self.sender_twitch_username_lowercase = Some(value.to_lowercase());
    self
  }

  pub fn set_destination_channel_id(mut self, value: &str) -> Self {
    self.destination_channel_id = Some(value.to_string());
    self
  }

  pub fn set_destination_channel_name(mut self, value: &str) -> Self {
    self.destination_channel_name = Some(value.to_string());
    self
  }

  pub fn set_is_anonymous(mut self, value: bool) -> Self {
    self.is_anonymous = value;
    self
  }

  pub fn set_bits_used(mut self, value: u64) -> Self {
    self.bits_used = value;
    self
  }

  pub fn set_total_bits_used(mut self, value: u64) -> Self {
    self.total_bits_used = value;
    self
  }

  pub fn set_chat_message(mut self, value: &str) -> Self {
    self.chat_message = value.to_string();
    self
  }

  pub async fn insert(&mut self, mysql_pool: &MySqlPool) -> AnyhowResult<()> {
    let sender_twitch_user_id = self.sender_twitch_user_id
        .clone()
        .ok_or(anyhow!("no sender_twitch_user_id"))?;

    let sender_twitch_username = self.sender_twitch_username
        .clone()
        .ok_or(anyhow!("no sender_twitch_username"))?;

    let sender_twitch_username_lowercase = self.sender_twitch_username_lowercase
        .clone()
        .ok_or(anyhow!("no sender_twitch_username_lowercase"))?;

    let destination_channel_id = self.destination_channel_id
        .clone()
        .ok_or(anyhow!("no destination_channel_id"))?;

    let destination_channel_name = self.destination_channel_name
        .clone()
        .ok_or(anyhow!("no destination_channel_name"))?;

    let query = sqlx::query!(
        r#"
INSERT INTO twitch_bits_events
SET
    sender_twitch_user_id = ?,
    sender_twitch_username = ?,
    sender_twitch_username_lowercase = ?,
    destination_channel_id = ?,
    destination_channel_name = ?,
    is_anonymous= ?,
    bits_used = ?,
    total_bits_used = ?,
    chat_message = ?
        "#,
      sender_twitch_user_id,
      sender_twitch_username,
      sender_twitch_username_lowercase,
      destination_channel_id,
      destination_channel_name,
      self.is_anonymous,
      self.bits_used,
      self.total_bits_used,
      self.chat_message.clone(),
      );

    let query_result = query.execute(mysql_pool)
        .await;

    let _record_id = match query_result {
      Ok(res) => {
        res.last_insert_id()
      },
      Err(err) => {
        return Err(anyhow!("Twitch pubsub bits insert DB error: {:?}", err));
      }
    };

    Ok(())
  }
}
