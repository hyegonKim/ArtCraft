use chrono::{Datelike, DateTime, Duration, Utc};

use tokens::tokens::users::UserToken;

pub struct PremiumUserRedisKey(pub String);

impl_string_key!(PremiumUserRedisKey);

// NB: 62 days to last over a month with enough time for debugging.
const REDIS_KEY_TTL_DURATION: Duration = Duration::milliseconds(1000 * 60 * 60 * 24 * 62);

impl PremiumUserRedisKey {
  pub fn new_for_user(user_token: &UserToken, time: DateTime<Utc>) -> Self {
    let month = time.month();
    let key = format!("premium:user:{}:{}", user_token.as_str(), month);
    Self(key)
  }

  pub fn get_redis_ttl() -> Duration {
    REDIS_KEY_TTL_DURATION
  }
}

#[cfg(test)]
mod tests {
  use chrono::TimeZone;

  use super::*;

  #[test]
  fn test_new_for_user() {
    let user_token = UserToken::new_from_str("token");
    let time = Utc.with_ymd_and_hms(2021, 1, 1, 0, 0, 0).unwrap();
    let key = PremiumUserRedisKey::new_for_user(&user_token, time);
    assert_eq!(key.as_str(), "premium:user:token:1");
  }

  #[test]
  fn test_duration() {
    assert_eq!(PremiumUserRedisKey::get_redis_ttl().num_days(), 62);
  }
}
