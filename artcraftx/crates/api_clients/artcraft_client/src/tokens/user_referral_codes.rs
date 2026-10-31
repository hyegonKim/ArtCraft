use serde::Deserialize;
use serde::Serialize;


/// The primary key for user referral codes.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub struct UserReferralCodeToken(pub String);
impl_client_token!(UserReferralCodeToken);
