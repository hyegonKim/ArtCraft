use crate::statsig_request::StatsigRequest;
use serde::{Deserialize, Serialize};

/// A freshly-minted `x-statsig-id` and the metadata needed to decide when it
/// goes stale.
///
/// The signature embeds a second-precision timestamp (`number = unix - epoch`)
/// that Grok only accepts inside a freshness window, so a minted signature is
/// reusable for a short time after [`minted_at_unix`](Self::minted_at_unix) and
/// then must be re-minted. Treat [`statsig_id`](Self::statsig_id) as an opaque
/// string — this crate never re-derives or mutates it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MintedStatsig {
  /// The endpoint this signature was minted for. A signature is only valid for
  /// the `(method, path)` it was signed against.
  pub request: StatsigRequest,

  /// The opaque `x-statsig-id` header value (base64, ~94 chars).
  pub statsig_id: String,

  /// Unix seconds when the browser produced the signature.
  pub minted_at_unix: i64,

  /// How long after minting the signature is considered usable, in seconds.
  /// Grok's exact acceptance window is not published; the minter supplies a
  /// conservative default (see [`MinterConfig`](crate::MinterConfig)).
  pub ttl_secs: u64,
}

impl MintedStatsig {
  pub fn new(request: StatsigRequest, statsig_id: impl Into<String>, minted_at_unix: i64, ttl_secs: u64) -> Self {
    Self {
      request,
      statsig_id: statsig_id.into(),
      minted_at_unix,
      ttl_secs,
    }
  }

  /// Whether the signature is still within its freshness window at `now_unix`.
  /// A clock that has moved backwards (`now < minted_at`) counts as stale, so a
  /// bad clock re-mints rather than serving something Grok will reject.
  pub fn is_fresh(&self, now_unix: i64) -> bool {
    let age = now_unix.saturating_sub(self.minted_at_unix);
    age >= 0 && (age as u64) < self.ttl_secs
  }

  /// Seconds of freshness left at `now_unix` (0 once stale).
  pub fn remaining_secs(&self, now_unix: i64) -> u64 {
    let age = now_unix.saturating_sub(self.minted_at_unix);
    if age < 0 {
      return 0;
    }
    self.ttl_secs.saturating_sub(age as u64)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const TTL: u64 = 90;
  const MINTED_AT: i64 = 1_787_535_086;

  fn minted() -> MintedStatsig {
    MintedStatsig::new(StatsigRequest::new_conversation(), "sig", MINTED_AT, TTL)
  }

  #[test]
  fn fresh_within_window_stale_after() {
    let statsig = minted();
    assert!(statsig.is_fresh(MINTED_AT));
    assert!(statsig.is_fresh(MINTED_AT + (TTL as i64) - 1));
    assert!(!statsig.is_fresh(MINTED_AT + TTL as i64));
  }

  #[test]
  fn backwards_clock_is_stale() {
    assert!(!minted().is_fresh(MINTED_AT - 5));
  }

  #[test]
  fn remaining_secs_counts_down_to_zero() {
    let statsig = minted();
    assert_eq!(statsig.remaining_secs(MINTED_AT), TTL);
    assert_eq!(statsig.remaining_secs(MINTED_AT + 30), TTL - 30);
    assert_eq!(statsig.remaining_secs(MINTED_AT + TTL as i64 + 10), 0);
  }
}
