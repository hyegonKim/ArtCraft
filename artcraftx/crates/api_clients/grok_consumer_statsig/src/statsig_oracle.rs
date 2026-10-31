use crate::minted_statsig::MintedStatsig;
use crate::statsig_request::StatsigRequest;

/// Drives a real browser to produce one `x-statsig-id` for a given endpoint.
///
/// This is the seam between this (platform-agnostic) crate and the app.
/// `artcraftx` implements it against a hidden Tauri WebView: navigate to
/// grok.com, inject [`MINT_HARNESS_SCRIPT`](crate::MINT_HARNESS_SCRIPT), and
/// read the value back over the WebView's IPC channel. Because macOS requires
/// WebView work on the main run loop, the implementation — not this crate —
/// owns the event loop; the minter only calls [`mint`](Self::mint).
///
/// Implementations should be cheap to call repeatedly (reuse one hidden WebView
/// rather than spawning one per call) and must be cancellation-safe: a dropped
/// `mint` future should abandon the pending signature, not wedge the WebView.
pub trait StatsigOracle {
  /// Produce a fresh signature for `request`, or explain why it could not.
  fn mint(
    &self,
    request: &StatsigRequest,
  ) -> impl std::future::Future<Output = Result<MintedStatsig, StatsigError>> + Send;
}

/// Why minting a signature failed.
#[derive(Debug, thiserror::Error)]
pub enum StatsigError {
  /// No WebView was available to run the signer (e.g. the app is headless or
  /// the hidden window was torn down).
  #[error("no webview available to mint a statsig")]
  WebviewUnavailable,

  /// The grok.com page never reached a state where the signer could run —
  /// typically a Cloudflare challenge or a failed/blocked navigation.
  #[error("grok.com did not become ready to sign (page load / cloudflare): {0}")]
  PageNotReady(String),

  /// The harness ran but did not return a usable signature before the deadline.
  #[error("timed out waiting for the browser to return a statsig")]
  Timeout,

  /// The harness returned, but the value was empty or malformed.
  #[error("browser returned an invalid statsig: {0}")]
  InvalidResult(String),

  /// Any other implementation-specific failure (IPC error, eval error, …).
  #[error("statsig oracle failed: {0}")]
  Other(String),
}
