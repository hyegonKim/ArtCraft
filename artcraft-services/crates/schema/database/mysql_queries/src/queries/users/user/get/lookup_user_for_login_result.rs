use crate::helpers::boolean_converters::i8_to_bool;
use tokens::tokens::users::UserToken;

// NB: Upgrade to sqlx 0.7 broke deserialization of Vec<u8>:
// https://github.com/launchbadge/sqlx/issues/2875
pub type VecBytes = Vec<u8>;

/// Shared result type for user lookups for login (via email or username)
#[derive(Debug)]
pub struct UserRecordForLogin {
  pub token: UserToken,

  /// Username is lowercase only
  pub username: String,

  /// Display name is the username in the user's original choice of case
  pub display_name: String,

  /// If the user created their account with SSO and received a randomized username,
  /// this reports back with whether the user has customized it.
  pub username_is_not_customized: bool,

  pub email_address: String,

  /// Password hash for password-based login
  /// SSO-only user accounts do not have a password hash (it is set to "*").
  pub password_hash: VecBytes,

  /// Vector clock, incremented on password changes.
  pub password_version: u32,

  /// Whether the user is banned.
  pub is_banned: bool,

  /// Optional comma-separated list of parseable `UserFeatureFlag` enum features
  pub maybe_feature_flags: Option<String>,
}

#[derive(Debug)]
pub (crate) struct UserRecordForLoginRaw {
  pub token: UserToken,
  pub username: String,
  pub display_name: String,
  pub username_is_not_customized: i8,
  pub email_address: String,
  pub password_hash: VecBytes,
  pub password_version: u32,
  pub is_banned: i8,
  pub maybe_feature_flags: Option<String>,
}

impl From<UserRecordForLoginRaw> for UserRecordForLogin {
  fn from(raw: UserRecordForLoginRaw) -> Self {
    Self {
      token: raw.token,
      username: raw.username,
      display_name: raw.display_name,
      username_is_not_customized: i8_to_bool(raw.username_is_not_customized),
      email_address: raw.email_address,
      password_hash: raw.password_hash,
      password_version: raw.password_version,
      is_banned: i8_to_bool(raw.is_banned),
      maybe_feature_flags: raw.maybe_feature_flags,
    }
  }
}
