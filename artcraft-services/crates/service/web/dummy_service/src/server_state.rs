use crate::env_args::EnvArgs;

#[derive(Clone)]
pub struct ServerState {
  pub hostname: String,
  pub flags: StaticFeatureFlags,
}

#[derive(Clone)]
pub struct StaticFeatureFlags {
  /// If we're suffering an outage, set the alert category to display a predefined message to users.
  pub maybe_status_alert_category: Option<String>,

  /// If we're suffering an outage, set custom text for the alert message.
  pub maybe_status_alert_custom_message: Option<String>,
}

impl ServerState {
  pub fn build(args: &EnvArgs) -> Self {
    let server_hostname = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or("hostname-unknown".to_string());

    Self {
      hostname: server_hostname,
      flags: StaticFeatureFlags {
        maybe_status_alert_category: args.maybe_status_alert_category.clone(),
        maybe_status_alert_custom_message: args.maybe_status_alert_custom_message.clone(),
      },
    }
  }
}
