use serde_derive::{Deserialize, Serialize};

pub const MODERATION_SEND_ALERT_PATH: &str = "/v1/moderation/alerts/send";

#[derive(Deserialize)]
pub struct ModerationSendAlertRequest {
  /// Optional title for the alert. Defaults to "Test Moderation Alert".
  pub title: Option<String>,

  /// Optional description for the alert. Defaults to "This is a test moderation alert."
  pub description: Option<String>,

  /// Optional urgency of the page.
  pub urgency: Option<ModerationSendAlertUrgency>,
}

#[derive(Deserialize, Copy, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ModerationSendAlertUrgency {
  High,
  Medium,
  Low,
}

#[derive(Serialize)]
pub struct ModerationSendAlertResponse {
  pub success: bool,
}
