use crate::minted_statsig::MintedStatsig;
use crate::statsig_oracle::{StatsigError, StatsigOracle};
use crate::statsig_request::StatsigRequest;
use log::{debug, info};
use std::collections::HashMap;
use tokio::sync::Mutex;

/// Default freshness window applied to a minted signature. Conservative: Grok's
/// real acceptance window is not published, so we re-mint well before a
/// second-precision timestamp could plausibly age out.
const DEFAULT_TTL_SECS: u64 = 90;

/// Default slack: serve a cached signature only while it has more than this many
/// seconds of freshness left, so a request never goes out on a signature about
/// to expire mid-flight.
const DEFAULT_REFRESH_MARGIN_SECS: u64 = 20;

/// Caches one [`MintedStatsig`] per endpoint and re-mints through a
/// [`StatsigOracle`] when the cached one is missing or about to expire.
///
/// Cheap, hot path: a fresh cache hit is a map lookup under a short-held lock.
/// The slow path (driving the browser) runs *without* the lock held, so a mint
/// for one endpoint never blocks a cache read for another; the trade-off is
/// that a burst of misses for the *same* endpoint may mint more than once,
/// which is harmless.
pub struct StatsigMinter<O: StatsigOracle> {
  oracle: O,
  config: MinterConfig,
  cache: Mutex<HashMap<StatsigRequest, MintedStatsig>>,
  clock: Box<dyn Fn() -> i64 + Send + Sync>,
}

/// Freshness policy for a [`StatsigMinter`]. This crate owns the TTL: whatever
/// the oracle returns, the minter stamps [`ttl_secs`](Self::ttl_secs) onto it
/// so the policy lives in one place.
#[derive(Clone, Copy, Debug)]
pub struct MinterConfig {
  /// Seconds a freshly-minted signature is treated as usable.
  pub ttl_secs: u64,

  /// Re-mint once a cached signature has fewer than this many seconds left.
  pub refresh_margin_secs: u64,
}

impl Default for MinterConfig {
  fn default() -> Self {
    Self {
      ttl_secs: DEFAULT_TTL_SECS,
      refresh_margin_secs: DEFAULT_REFRESH_MARGIN_SECS,
    }
  }
}

impl<O: StatsigOracle> StatsigMinter<O> {
  /// Build a minter over `oracle` with the default freshness policy.
  pub fn new(oracle: O) -> Self {
    Self::with_config(oracle, MinterConfig::default())
  }

  pub fn with_config(oracle: O, config: MinterConfig) -> Self {
    Self {
      oracle,
      config,
      cache: Mutex::new(HashMap::new()),
      clock: Box::new(default_now_unix),
    }
  }

  /// Return a usable `x-statsig-id` string for `request`, serving a fresh
  /// cached signature when possible and minting a new one otherwise.
  pub async fn statsig_for(&self, request: &StatsigRequest) -> Result<String, StatsigError> {
    let now = (self.clock)();

    if let Some(fresh) = self.cached_fresh(request, now).await {
      debug!("statsig cache hit for {} {}", request.method, request.path);
      return Ok(fresh);
    }

    info!("minting statsig for {} {}", request.method, request.path);
    let mut minted = self.oracle.mint(request).await?;
    // The minter owns TTL policy; normalize whatever the oracle reported.
    minted.ttl_secs = self.config.ttl_secs;

    let statsig_id = minted.statsig_id.clone();
    self.cache.lock().await.insert(request.clone(), minted);
    Ok(statsig_id)
  }

  /// Drop any cached signature for `request` (e.g. after the server rejects it
  /// with a `StatsigSignatureRejected`, so the next call re-mints).
  pub async fn invalidate(&self, request: &StatsigRequest) {
    self.cache.lock().await.remove(request);
  }

  /// A cached signature for `request` if it still has more than the refresh
  /// margin of freshness left.
  async fn cached_fresh(&self, request: &StatsigRequest, now: i64) -> Option<String> {
    let cache = self.cache.lock().await;
    let minted = cache.get(request)?;
    if minted.remaining_secs(now) > self.config.refresh_margin_secs {
      Some(minted.statsig_id.clone())
    } else {
      None
    }
  }
}

fn default_now_unix() -> i64 {
  chrono::Utc::now().timestamp()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::minted_statsig::MintedStatsig;
  use std::sync::Arc;
  use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};

  const BASE: i64 = 1_787_535_086;

  #[tokio::test]
  async fn mints_once_then_serves_from_cache() {
    let oracle = CountingOracle::new();
    let minter = minter_at(oracle.clone(), BASE);

    let request = StatsigRequest::new_conversation();
    let first = minter.statsig_for(&request).await.unwrap();
    let second = minter.statsig_for(&request).await.unwrap();

    assert_eq!(first, second);
    assert_eq!(oracle.calls(), 1, "second call should hit the cache");
  }

  #[tokio::test]
  async fn re_mints_once_the_margin_is_crossed() {
    let oracle = CountingOracle::new();
    let clock = Arc::new(AtomicI64::new(BASE));
    let minter = minter_with_clock(oracle.clone(), clock.clone());

    let request = StatsigRequest::new_conversation();
    minter.statsig_for(&request).await.unwrap();

    // Still fresh (well inside ttl - margin): cached.
    clock.store(BASE + 30, Ordering::SeqCst);
    minter.statsig_for(&request).await.unwrap();
    assert_eq!(oracle.calls(), 1);

    // Past ttl - margin (90 - 20 = 70): re-mint.
    clock.store(BASE + 75, Ordering::SeqCst);
    minter.statsig_for(&request).await.unwrap();
    assert_eq!(oracle.calls(), 2);
  }

  #[tokio::test]
  async fn distinct_endpoints_are_cached_separately() {
    let oracle = CountingOracle::new();
    let minter = minter_at(oracle.clone(), BASE);

    minter.statsig_for(&StatsigRequest::new("POST", "/a")).await.unwrap();
    minter.statsig_for(&StatsigRequest::new("POST", "/b")).await.unwrap();
    assert_eq!(oracle.calls(), 2);
  }

  #[tokio::test]
  async fn invalidate_forces_a_re_mint() {
    let oracle = CountingOracle::new();
    let minter = minter_at(oracle.clone(), BASE);

    let request = StatsigRequest::new_conversation();
    minter.statsig_for(&request).await.unwrap();
    minter.invalidate(&request).await;
    minter.statsig_for(&request).await.unwrap();
    assert_eq!(oracle.calls(), 2);
  }

  #[tokio::test]
  async fn propagates_oracle_errors() {
    let minter = minter_at(FailingOracle, BASE);
    let error = minter.statsig_for(&StatsigRequest::new_conversation()).await.unwrap_err();
    assert!(matches!(error, StatsigError::WebviewUnavailable));
  }

  fn minter_at<O: StatsigOracle>(oracle: O, now: i64) -> StatsigMinter<O> {
    let mut minter = StatsigMinter::new(oracle);
    minter.clock = Box::new(move || now);
    minter
  }

  fn minter_with_clock<O: StatsigOracle>(oracle: O, clock: Arc<AtomicI64>) -> StatsigMinter<O> {
    let mut minter = StatsigMinter::new(oracle);
    minter.clock = Box::new(move || clock.load(Ordering::SeqCst));
    minter
  }

  #[derive(Clone)]
  struct CountingOracle {
    calls: Arc<AtomicUsize>,
  }

  impl CountingOracle {
    fn new() -> Self {
      Self { calls: Arc::new(AtomicUsize::new(0)) }
    }

    fn calls(&self) -> usize {
      self.calls.load(Ordering::SeqCst)
    }
  }

  impl StatsigOracle for CountingOracle {
    async fn mint(&self, request: &StatsigRequest) -> Result<MintedStatsig, StatsigError> {
      let n = self.calls.fetch_add(1, Ordering::SeqCst);
      // A distinct value per mint so cache hits vs re-mints are observable.
      Ok(MintedStatsig::new(request.clone(), format!("sig-{n}"), BASE, 1))
    }
  }

  struct FailingOracle;

  impl StatsigOracle for FailingOracle {
    async fn mint(&self, _request: &StatsigRequest) -> Result<MintedStatsig, StatsigError> {
      Err(StatsigError::WebviewUnavailable)
    }
  }
}
