use cloudflare_errors::cloudflare_error::CloudflareError;
use std::time::Duration;

/// Base delay for origin-failure retries; doubles per attempt.
const ORIGIN_FAILURE_BASE_DELAY: Duration = Duration::from_secs(5);
const ORIGIN_FAILURE_MAX_DELAY: Duration = Duration::from_secs(120);

/// Rate limits back off harder: Cloudflare's window is typically a minute.
const RATE_LIMIT_BASE_DELAY: Duration = Duration::from_secs(30);
const RATE_LIMIT_MAX_DELAY: Duration = Duration::from_secs(300);

/// The counter-measure for a Cloudflare edge response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudflareMitigation {
  /// The origin is failing or we're rate limited: wait this long, then retry
  /// the same request unchanged.
  RetryAfter(Duration),

  /// A challenge only a browser can pass. Send the user through the site's
  /// login window again to earn a fresh `cf_clearance` — and make sure the
  /// replaying client presents the same User-Agent that window used
  /// (see `clearance::profile_for_captured_user_agent`).
  ReauthenticateInBrowser,

  /// The firewall rejected this client outright. Retrying or re-logging in
  /// won't help; surface it and stop.
  GiveUp,
}

impl CloudflareMitigation {
  /// Whether the caller should try the request again (after
  /// [`Self::retry_delay`]).
  pub fn should_retry(&self) -> bool {
    matches!(self, Self::RetryAfter(_))
  }

  pub fn retry_delay(&self) -> Option<Duration> {
    match self {
      Self::RetryAfter(delay) => Some(*delay),
      _ => None,
    }
  }

  /// Whether the session needs a human in a browser before continuing.
  pub fn needs_browser(&self) -> bool {
    matches!(self, Self::ReauthenticateInBrowser)
  }
}

/// The mitigation for an error on the given retry `attempt` (0 = first
/// retry). Delays grow exponentially and cap.
pub fn mitigation_for(error: &CloudflareError, attempt: u32) -> CloudflareMitigation {
  match error {
    CloudflareError::ChallengeInterstitial403 => CloudflareMitigation::ReauthenticateInBrowser,
    CloudflareError::AccessDenied1020 => CloudflareMitigation::GiveUp,
    // An unexpected redirect is a client bug (wrong host/scheme), not
    // something waiting fixes.
    CloudflareError::MovedPermanently301 => CloudflareMitigation::GiveUp,
    CloudflareError::RateLimited429 => {
      CloudflareMitigation::RetryAfter(backoff(RATE_LIMIT_BASE_DELAY, RATE_LIMIT_MAX_DELAY, attempt))
    }
    CloudflareError::BadGateway502
      | CloudflareError::ServiceUnavailable503
      | CloudflareError::GatewayTimeout504
      | CloudflareError::OriginError5xx(_)
      | CloudflareError::TimeoutOccurred524 => {
      CloudflareMitigation::RetryAfter(backoff(ORIGIN_FAILURE_BASE_DELAY, ORIGIN_FAILURE_MAX_DELAY, attempt))
    }
  }
}

fn backoff(base: Duration, max: Duration, attempt: u32) -> Duration {
  let multiplier = 2u32.saturating_pow(attempt.min(16));
  base.saturating_mul(multiplier).min(max)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn challenge_means_browser() {
    let mitigation = mitigation_for(&CloudflareError::ChallengeInterstitial403, 0);
    assert_eq!(mitigation, CloudflareMitigation::ReauthenticateInBrowser);
    assert!(mitigation.needs_browser());
    assert!(!mitigation.should_retry());
  }

  #[test]
  fn hard_block_gives_up() {
    assert_eq!(mitigation_for(&CloudflareError::AccessDenied1020, 0), CloudflareMitigation::GiveUp);
    assert_eq!(mitigation_for(&CloudflareError::MovedPermanently301, 3), CloudflareMitigation::GiveUp);
  }

  #[test]
  fn origin_failures_back_off_and_cap() {
    assert_eq!(mitigation_for(&CloudflareError::BadGateway502, 0), CloudflareMitigation::RetryAfter(Duration::from_secs(5)));
    assert_eq!(mitigation_for(&CloudflareError::GatewayTimeout504, 1), CloudflareMitigation::RetryAfter(Duration::from_secs(10)));
    assert_eq!(mitigation_for(&CloudflareError::TimeoutOccurred524, 2), CloudflareMitigation::RetryAfter(Duration::from_secs(20)));
    assert_eq!(mitigation_for(&CloudflareError::OriginError5xx(522), 10), CloudflareMitigation::RetryAfter(Duration::from_secs(120)));
    assert_eq!(mitigation_for(&CloudflareError::OriginError5xx(522), 40).retry_delay(), Some(Duration::from_secs(120)));
  }

  #[test]
  fn rate_limits_back_off_harder() {
    assert_eq!(mitigation_for(&CloudflareError::RateLimited429, 0), CloudflareMitigation::RetryAfter(Duration::from_secs(30)));
    assert_eq!(mitigation_for(&CloudflareError::RateLimited429, 5), CloudflareMitigation::RetryAfter(Duration::from_secs(300)));
  }
}
