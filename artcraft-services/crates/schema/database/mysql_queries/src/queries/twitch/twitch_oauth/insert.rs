use anyhow::anyhow;
use sqlx::MySqlPool;

use errors::AnyhowResult;
use tokens::tokens::twitch_oauth_tokens_internal::TwitchOauthInternalToken;

pub struct TwitchOauthTokenInsertBuilder {
  // ===== Required Fields =====

  /// Old APIs return a u32, but this should be a string.
  /// Twitch is migrating IDs to strings.
  twitch_user_id: String,

  /// Twitch username / channel name.
  twitch_username: String,

  /// The secret we use
  access_token: String,

  /// Internal mechanism we use to group tokens across refresh events
  oauth_grouping_token: String,

  // ===== Optional / Default Fields =====

  /// Other half of the access token, for renewal
  maybe_refresh_token: Option<String>,

  /// Storyteller/FakeYou username.
  maybe_user_token: Option<String>,

  /// Abuse tracking.
  maybe_ip_address_creation: Option<String>,

  /// Probably 'bearer'
  token_type: Option<String>,

  /// TTL seconds from issue
  expires_in_seconds: Option<u32>,

  /// Number of times we've refreshed the token
  refresh_count: u32,

  // ===== OAuth Scopes =====

  has_bits_read: bool,
  has_channel_read_redemptions: bool,
  has_channel_read_subscriptions: bool,
  has_chat_edit: bool,
  has_chat_read: bool,
}

impl TwitchOauthTokenInsertBuilder {
  pub fn new(
    twitch_user_id: &str,
    twitch_username: &str,
    access_token: &str,
    oauth_grouping_token: &str
  ) -> Self {
    Self {
      twitch_user_id: twitch_user_id.to_string(),
      twitch_username: twitch_username.to_string(),
      access_token: access_token.to_string(),
      oauth_grouping_token: oauth_grouping_token.to_string(),
      maybe_refresh_token: None,
      maybe_user_token: None,
      maybe_ip_address_creation: None,
      token_type: None,
      expires_in_seconds: None,
      has_bits_read: false,
      has_channel_read_subscriptions: false,
      has_channel_read_redemptions: false,
      has_chat_edit: false,
      has_chat_read: false,
      refresh_count: 0,
    }
  }

  pub fn set_refresh_token(mut self, refresh_token: Option<&str>) -> Self {
    self.maybe_refresh_token = refresh_token.map(|t| t.to_string());
    self
  }

  /// NB: This is the FakeYou/Storyteller user account, not Twitch!
  pub fn set_user_token(mut self, maybe_user_token: Option<&str>) -> Self {
    self.maybe_user_token = maybe_user_token.map(|t| t.to_string());
    self
  }

  pub fn set_ip_address_creation(mut self, ip_address_creation: Option<&str>) -> Self {
    self.maybe_ip_address_creation = ip_address_creation.map(|t| t.to_string());
    self
  }

  pub fn set_token_type(mut self, token_type: Option<&str>) -> Self {
    self.token_type = token_type.map(|t| t.to_string());
    self
  }

  pub fn set_expires_in_seconds(mut self, expires_in_seconds: Option<u32>) -> Self {
    self.expires_in_seconds = expires_in_seconds;
    self
  }

  pub fn set_refresh_count(mut self, refresh_count: u32) -> Self {
    self.refresh_count = refresh_count;
    self
  }

  pub fn has_bits_read(mut self, value: bool) -> Self {
    self.has_bits_read = value;
    self
  }

  pub fn has_channel_read_redemptions(mut self, value: bool) -> Self {
    self.has_channel_read_redemptions = value;
    self
  }

  pub fn has_channel_read_subscriptions(mut self, value: bool) -> Self {
    self.has_channel_read_subscriptions = value;
    self
  }

  pub fn has_chat_edit(mut self, value: bool) -> Self {
    self.has_chat_edit = value;
    self
  }

  pub fn has_chat_read(mut self, value: bool) -> Self {
    self.has_chat_read = value;
    self
  }

  pub async fn insert(&mut self, mysql_pool: &MySqlPool) -> AnyhowResult<()> {
    // An internal token that we may use in the future.
    let internal_token = TwitchOauthInternalToken::generate();

    // NB: We have to duplicate these since the string literals must not
    // include concatenation. Boo.
    let query = if let Some(expires_in_seconds) = self.expires_in_seconds {
      sqlx::query!(
        r#"
INSERT INTO twitch_oauth_tokens
SET
  internal_token = ?,
  oauth_refresh_grouping_token = ?,
  maybe_user_token = ?,
  twitch_user_id = ?,
  twitch_username = ?,
  twitch_username_lowercase = ?,
  access_token = ?,
  maybe_refresh_token = ?,
  refresh_count = ?,
  token_type = ?,
  expires_in_seconds = ?,
  has_bits_read = ?,
  has_channel_read_redemptions= ?,
  has_channel_read_subscriptions = ?,
  has_chat_edit = ?,
  has_chat_read = ?,
  ip_address_creation = ?,
  expires_at = DATE_ADD(NOW(), INTERVAL ? SECOND)
        "#,
        internal_token.to_string(),
        self.oauth_grouping_token.clone(),
        self.maybe_user_token.clone(),
        self.twitch_user_id.clone(),
        self.twitch_username.clone(),
        self.twitch_username.clone().to_lowercase(),
        self.access_token.clone(),
        self.maybe_refresh_token.clone(),
        self.refresh_count,
        self.token_type.clone(),
        expires_in_seconds,
        self.has_bits_read.clone(),
        self.has_channel_read_redemptions.clone(),
        self.has_channel_read_subscriptions.clone(),
        self.has_chat_edit.clone(),
        self.has_chat_read.clone(),
        self.maybe_ip_address_creation.clone(),
        expires_in_seconds,
      )
    } else {
      sqlx::query!(
        r#"
INSERT INTO twitch_oauth_tokens
SET
  internal_token = ?,
  oauth_refresh_grouping_token = ?,
  maybe_user_token = ?,
  twitch_user_id = ?,
  twitch_username = ?,
  twitch_username_lowercase = ?,
  access_token = ?,
  maybe_refresh_token = ?,
  refresh_count = ?,
  token_type = ?,
  expires_in_seconds = NULL,
  has_bits_read = ?,
  has_channel_read_redemptions= ?,
  has_channel_read_subscriptions = ?,
  has_chat_edit = ?,
  has_chat_read = ?,
  ip_address_creation = ?,
  expires_at = NULL
        "#,
        internal_token.clone(),
        self.oauth_grouping_token.clone(),
        self.maybe_user_token.clone(),
        self.twitch_user_id.clone(),
        self.twitch_username.clone(),
        self.twitch_username.clone().to_lowercase(),
        self.access_token.clone(),
        self.maybe_refresh_token.clone(),
        self.refresh_count,
        self.token_type.clone(),
        self.has_bits_read.clone(),
        self.has_channel_read_redemptions.clone(),
        self.has_channel_read_subscriptions.clone(),
        self.has_chat_edit.clone(),
        self.has_chat_read.clone(),
        self.maybe_ip_address_creation.clone(),
      )
    };

    let query_result = query.execute(mysql_pool)
        .await;

    let _record_id = match query_result {
      Ok(res) => {
        res.last_insert_id()
      },
      Err(err) => {
        return Err(anyhow!("Twitch token insert DB error: {:?}", err));
      }
    };

    Ok(())
  }
}
