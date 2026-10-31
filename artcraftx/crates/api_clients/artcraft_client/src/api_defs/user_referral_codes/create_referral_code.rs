use serde_derive::{Deserialize, Serialize};
use crate::tokens::user_referral_codes::UserReferralCodeToken;

#[derive(Deserialize)]
pub struct CreateReferralCodeRequest {
  /// The referral code string. Alphanumeric plus underscore, period, dash.
  pub code: String,
}

#[derive(Serialize)]
pub struct CreateReferralCodeResponse {
  pub success: bool,
  pub token: UserReferralCodeToken,
  pub code: String,
  pub code_lowercase: String,
}
