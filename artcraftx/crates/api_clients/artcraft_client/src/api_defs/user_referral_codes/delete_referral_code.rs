use serde_derive::{Deserialize, Serialize};
use crate::tokens::user_referral_codes::UserReferralCodeToken;

#[derive(Deserialize)]
pub struct DeleteReferralCodePathInfo {
  pub token: UserReferralCodeToken,
}

#[derive(Serialize)]
pub struct DeleteReferralCodeResponse {
  pub success: bool,
}
