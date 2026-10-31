use crate::enums::common::payments_namespace::PaymentsNamespace;
use serde_derive::{Deserialize, Serialize};
use crate::tokens::users::UserToken;

pub const MODERATOR_USER_LOOKUP_BY_STRIPE_CUSTOMER_ID_PATH: &str = "/v1/moderation/users/lookup_by_stripe_customer_id";

#[derive(Deserialize)]
pub struct ModeratorUserLookupByStripeCustomerIdRequest {
  pub stripe_customer_id: String,
}

#[derive(Serialize)]
pub struct ModeratorUserLookupByStripeCustomerIdResponse {
  pub success: bool,
  pub users: Vec<ModeratorUserLookupByStripeCustomerIdEntry>,
}

#[derive(Serialize)]
pub struct ModeratorUserLookupByStripeCustomerIdEntry {
  pub subscription_namespace: PaymentsNamespace,
  pub maybe_stripe_subscription_id: Option<String>,
  pub token: UserToken,
  pub email_address: String,
  pub username: String,
  pub display_name: String,
}
