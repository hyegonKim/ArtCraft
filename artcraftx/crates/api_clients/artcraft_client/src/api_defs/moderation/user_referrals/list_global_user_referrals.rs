use chrono::{DateTime, Utc};
use serde_derive::{Deserialize, Serialize};
use crate::tokens::user_referral_codes::UserReferralCodeToken;
use crate::tokens::users::UserToken;

// --- Request ---

#[derive(Deserialize)]
pub struct ListGlobalUserReferralsQueryParams {
  pub cursor: Option<String>,
  pub limit: Option<u32>,
}

// --- Response ---

#[derive(Serialize)]
pub struct ListGlobalUserReferralsSuccessResponse {
  pub success: bool,
  pub referrals: Vec<UserReferralResponse>,
  pub maybe_cursor: Option<String>,
}

#[derive(Serialize)]
pub struct UserReferralResponse {
  pub invited_user: InvitedUserDetails,
  pub referrer_user: ReferrerUserDetails,
  pub maybe_referral_code_token: Option<UserReferralCodeToken>,
  pub maybe_referral_url: Option<String>,
  pub maybe_landing_url: Option<String>,
  pub created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct InvitedUserDetails {
  pub token: UserToken,
  pub username: String,
  pub display_name: String,
  pub email_address: String,
}

#[derive(Serialize)]
pub struct ReferrerUserDetails {
  pub token: UserToken,
  pub username: String,
  pub display_name: String,
}
