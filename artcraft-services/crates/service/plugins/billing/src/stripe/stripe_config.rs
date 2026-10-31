
/// Configuration for Stripe, including secrets.
/// Inject these into Actix http handlers, etc.
/// Do not log!
#[derive(Clone)]
pub struct StripeConfig {
  pub checkout: StripeCheckoutConfigs,
  pub portal: StripeCustomerPortalConfigs,
  pub secrets: StripeSecrets,
}

/// Allow a URL config option to specify either a full URL or a partial path
/// (that will be paired with other configs).
#[derive(Clone)]
pub enum FullUrlOrPath {
  FullUrl(String),
  Path(String),
}

#[derive(Clone)]
pub struct StripeCheckoutConfigs {
  pub success_url: FullUrlOrPath,
  pub cancel_url: FullUrlOrPath,
}

#[derive(Clone)]
pub struct StripeCustomerPortalConfigs {
  pub return_url: FullUrlOrPath,

  /// The portal config id to use as a fallback.
  pub default_portal_config_id: String,
}

#[derive(Clone)]
pub struct StripeSecrets {
  /// The Stripe API key to use for the frontend or mobile apps.
  /// This is allowed in client code.
  pub publishable_key: Option<String>,

  /// The Stripe API key to use serverside. This is a secret value.
  pub secret_key: String,

  /// The Stripe secret used to validate inbound webhook payloads.
  pub secret_webhook_signing_key: String,
}