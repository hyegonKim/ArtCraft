use anyhow::anyhow;
use sqlx::MySqlPool;

use errors::AnyhowResult;

pub struct TwitchPubsubChannelPointsInsertBuilder {
  // ===== Required Fields =====

  sender_twitch_user_id: Option<String>,
  sender_twitch_username: Option<String>,
  sender_twitch_username_lowercase: Option<String>,

  destination_channel_id: Option<String>,
  destination_channel_name: Option<String>,

  // ===== Optional / Default Fields =====

  title: String,
  prompt: String,

  user_text_input: String,

  redemption_id: String,
  reward_id: String,

  reward_cost: u64,

  is_sub_only: bool,
  max_per_stream: u64,
  max_per_user_per_stream: u64,
}

impl TwitchPubsubChannelPointsInsertBuilder {
  pub fn new() -> Self {
    Self {
      sender_twitch_user_id: None,
      sender_twitch_username: None,
      sender_twitch_username_lowercase: None,
      destination_channel_id: None,
      destination_channel_name: None,
      title: "".to_string(),
      prompt: "".to_string(),
      user_text_input: "".to_string(),
      redemption_id: "".to_string(),
      reward_id: "".to_string(),
      reward_cost: 0,
      is_sub_only: false,
      max_per_stream: 0,
      max_per_user_per_stream: 0,
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

  pub fn set_title(mut self, value: &str) -> Self {
    self.title = value.to_string();
    self
  }

  pub fn set_prompt(mut self, value: &str) -> Self {
    self.prompt = value.to_string();
    self
  }

  pub fn set_user_text_input(mut self, value: Option<&str>) -> Self {
    self.user_text_input = value
        .map(|s| s.to_string())
        .unwrap_or("".to_string());
    self
  }

  pub fn set_redemption_id(mut self, value: &str) -> Self {
    self.redemption_id = value.to_string();
    self
  }

  pub fn set_reward_id(mut self, value: &str) -> Self {
    self.reward_id = value.to_string();
    self
  }

  pub fn set_is_sub_only(mut self, value: bool) -> Self {
    self.is_sub_only = value;
    self
  }

  pub fn set_reward_cost(mut self, value: u64) -> Self {
    self.reward_cost = value;
    self
  }

  pub fn set_max_per_stream(mut self, value: u64) -> Self {
    self.max_per_stream = value;
    self
  }

  pub fn set_max_per_user_per_stream(mut self, value: u64) -> Self {
    self.max_per_user_per_stream = value;
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
INSERT INTO twitch_channel_point_events
SET
    sender_twitch_user_id = ?,
    sender_twitch_username = ?,
    sender_twitch_username_lowercase = ?,
    destination_channel_id = ?,
    destination_channel_name = ?,
    title = ?,
    prompt = ?,
    user_text_input = ?,
    redemption_id = ?,
    reward_id = ?,
    reward_cost = ?,
    is_sub_only = ?,
    max_per_stream = ?,
    max_per_user_per_stream = ?
        "#,
      sender_twitch_user_id,
      sender_twitch_username,
      sender_twitch_username_lowercase,
      destination_channel_id,
      destination_channel_name,
      self.title.clone(),
      self.prompt.clone(),
      self.user_text_input.clone(),
      self.redemption_id.clone(),
      self.reward_id.clone(),
      self.reward_cost,
      self.is_sub_only,
      self.max_per_stream,
      self.max_per_user_per_stream,
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
