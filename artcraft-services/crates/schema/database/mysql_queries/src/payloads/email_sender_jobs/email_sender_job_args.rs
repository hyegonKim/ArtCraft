
// TODO: Switch to protobuf or nom to better support forward-compatible changes

use errors::AnyhowResult;

use crate::payloads::email_sender_jobs::subtypes::email_job_password_reset_args::EmailJobPasswordResetArgs;

/// Used to encode extra state for the `email_sender_jobs` table.
/// This should act somewhat like a serialized protobuf stored inside a record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmailSenderJobArgs {
  /// REQUIRED.
  /// Actual type-specific arguments.
  #[serde(rename = "a")] // NB: DO NOT CHANGE. It could break live jobs. Renamed to consume fewer bytes
  #[serde(skip_serializing_if = "Option::is_none")]
  pub args: Option<PolymorphicEmailSenderJobArgs>,
}

// NB: Keep the enum variant names short as these get serialized to json in the db
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PolymorphicEmailSenderJobArgs {
  // TODO: Args?
  /// Welcome email
  W,

  /// Forgot password / password reset args
  Pr(EmailJobPasswordResetArgs),
}

impl EmailSenderJobArgs {

  pub fn from_json(json: &str) -> AnyhowResult<Self> {
    Ok(serde_json::from_str(json)?)
  }

  pub fn to_json(&self) -> AnyhowResult<String> {
    Ok(serde_json::to_string(self)?)
  }
}
