use serde_derive::{Deserialize, Serialize};

pub const CUSTOMER_PORTAL_MANAGE_PLAN_URL_PATH: &str = "/v1/stripe_artcraft/portal/manage_plan";

#[derive(Serialize, Deserialize)]
pub struct StripeArtcraftCustomerPortalManagePlanRequest {
  // TODO: Not sure if this is needed
  pub portal_config_id: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct StripeArtcraftCustomerPortalManagePlanResponse {
  pub success: bool,
  pub stripe_portal_url: String,
}

