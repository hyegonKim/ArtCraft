use serde_derive::{Deserialize, Serialize};

use crate::api_defs::moderation::user_referrals::list_global_user_referrals::UserReferralResponse;

// --- Request ---

#[derive(Deserialize)]
pub struct ListUserReferralsForUserQueryParams {
  pub cursor: Option<String>,
  pub limit: Option<u32>,
}

#[derive(Deserialize)]
pub struct ListUserReferralsForUserPathInfo {
  pub username: String,
}

// --- Response ---

#[derive(Serialize)]
pub struct ListUserReferralsForUserSuccessResponse {
  pub success: bool,
  pub referrals: Vec<UserReferralResponse>,
  pub maybe_cursor: Option<String>,
}
