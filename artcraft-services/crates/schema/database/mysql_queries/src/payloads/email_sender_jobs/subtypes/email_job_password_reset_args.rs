
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmailJobPasswordResetArgs {
  /// REQUIRED.
  /// The password reset code
  #[serde(rename = "k")] // NB: DO NOT CHANGE. It could break live jobs. Renamed to consume fewer bytes
  #[serde(skip_serializing_if = "Option::is_none")]
  pub password_reset_secret_key: Option<String>,
}

#[cfg(test)]
mod tests {
  use crate::payloads::email_sender_jobs::subtypes::email_job_password_reset_args::EmailJobPasswordResetArgs;

  fn assert_json_deserializes_to_match(json: &str, original: &EmailJobPasswordResetArgs) {
    let duplicate: EmailJobPasswordResetArgs = serde_json::de::from_str(json).unwrap();
    assert_eq!(&duplicate, original);
  }

  #[test]
  fn test_secret_key() {
    let args = EmailJobPasswordResetArgs {
      password_reset_secret_key: Some("code".to_string()),
    };
    let json = serde_json::ser::to_string(&args).unwrap();
    assert_eq!(json, r#"{"k":"code"}"#.to_string());
    assert_json_deserializes_to_match(&json, &args);
  }
}
