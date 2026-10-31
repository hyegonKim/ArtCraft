//! These are columns where users can control the visibility of their data.

use anyhow::anyhow;

use errors::AnyhowResult;

/// To use this in a query, the query must have type annotations.
/// See: https://www.gitmemory.com/issue/launchbadge/sqlx/1241/847154375
/// eg. twitch_event_category as `twitch_event_category: crate::column_types::twitch_event_category::TwitchEventCategory`
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
pub enum TwitchEventCategory {
  Bits,
  ChannelPoints,
  ChatCommand,
}

impl TwitchEventCategory {
  pub fn to_str(&self) -> &'static str {
    match self {
      Self::Bits => "bits",
      Self::ChannelPoints => "channel_points",
      Self::ChatCommand => "chat_command",
    }
  }

  pub fn from_str(value: &str) -> AnyhowResult<Self> {
    match value {
      "bits" => Ok(Self::Bits),
      "channel_points" => Ok(Self::ChannelPoints),
      "chat_command" => Ok(Self::ChatCommand),
      _ => Err(anyhow!("invalid value: {:?}", value)),
    }
  }
}
