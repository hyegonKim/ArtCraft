use chrono::{Datelike, DateTime, Duration, Utc};
use tokens::tokens::anonymous_visitor_tracking::AnonymousVisitorTrackingToken;

pub struct PremiumAvtRedisKey(pub String);

impl_string_key!(PremiumAvtRedisKey);

// NB: 62 days to last over a month with enough time for debugging.
const REDIS_KEY_TTL_DURATION: Duration = Duration::milliseconds(1000 * 60 * 60 * 24 * 62);

impl PremiumAvtRedisKey {
  pub fn new_for_user(avt: &AnonymousVisitorTrackingToken, time: DateTime<Utc>) -> Self {
    let month = time.month();
    let key = format!("premium:avt:{}:{}", avt.as_str(), month);
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
    let avt = AnonymousVisitorTrackingToken::new_from_str("tracking");
    let time = Utc.with_ymd_and_hms(2021, 7, 4, 0, 0, 0).unwrap();
    let key = PremiumAvtRedisKey::new_for_user(&avt, time);
    assert_eq!(key.as_str(), "premium:avt:tracking:7");
  }

  #[test]
  fn test_duration() {
    assert_eq!(PremiumAvtRedisKey::get_redis_ttl().num_days(), 62);
  }
}
