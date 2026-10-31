use serde_derive::{Deserialize, Serialize};

pub const LOG_WEB_REFERRAL_URL_PATH: &str = "/v1/web_referrals/record";

#[derive(Deserialize)]
pub struct LogWebReferralRequest {
  /// Optional: The referral URL the user arrived from when signing up.
  /// The browser can send `document.referrer` to the backend so we know how people are finding us.
  /// If the browser doesn't send this parameter, we'll try the `referer` header.
  pub maybe_referral_url: Option<String>,
}

#[derive(Serialize)]
pub struct LogWebReferralResponse {
  pub success: bool,
}
