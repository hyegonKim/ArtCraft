//! # grok_consumer_statsig
//!
//! Mints valid `x-statsig-id` request signatures for grok.com by running Grok's
//! own client-side signer inside a real browser (the app's Tauri WebView), then
//! caches the result so it can be reused across HTTP requests made by
//! [`grok_consumer_client`].
//!
//! ## Why a browser is required
//!
//! grok.com gates its chat / media-generation POST endpoints (e.g.
//! `POST /rest/app-chat/conversations/new`) behind an `x-statsig-id` anti-bot
//! header. The header folds in a "genuine HEX" fingerprint that Grok's current
//! signer derives by sampling an actually-rendered, CSS-animated SVG via
//! `getComputedStyle` + `requestAnimationFrame`. That is a real-DOM
//! measurement: a headless re-implementation only works against a frozen
//! snapshot of Grok's (rotating) path constants and drifts out of date. That
//! dated headless port was removed from `grok_consumer_client`; running Grok's
//! real JS in a WebView sidesteps the whole rotation problem.
//!
//! ## What the signature needs (and does not)
//!
//! - **Login: not required by the algorithm.** The payload encodes only
//!   `(seed, timestamp, sha256(method!path!…), mark)` — no user id, cookie, or
//!   token. Grok attaches `x-statsig-id` to pre-login requests too. A logged-in
//!   session is only useful so the WebView loads grok.com's bundles *past
//!   Cloudflare* smoothly; the app's existing xAI login already persists that
//!   session in WebKit, so the statsig WebView inherits it.
//! - **Browser fingerprint / UA: not part of the signature.** The genuine-hex
//!   is spec-defined CSS interpolation, consistent across engines, so a
//!   WKWebView is fine. The one constraint is that the WebView's User-Agent
//!   must match the one that obtained `cf_clearance` (Cloudflare binds
//!   `cf_clearance` to the UA), which is the same rule the login flow follows.
//!
//! ## Architecture
//!
//! This crate is a leaf: it owns the Rust-side pieces (request identity, TTL /
//! freshness, the per-endpoint cache) and defines a single [`StatsigOracle`]
//! seam. On macOS a WebView must be driven from the app's main run loop, so the
//! crate never spins up its own event loop — `artcraftx` implements
//! [`StatsigOracle`] against a hidden Tauri WebView and hands it to
//! [`StatsigMinter`].
//!
//! ```text
//!   grok_consumer_client  ──needs a statsig──▶  StatsigMinter (this crate)
//!                                                    │  cache hit? return it
//!                                                    │  else ▼
//!                                                StatsigOracle  (impl in artcraftx)
//!                                                    │  drive hidden grok.com WebView,
//!                                                    │  run MINT_HARNESS_SCRIPT, read back
//!                                                    ▼
//!                                                MintedStatsig { statsig_id, minted_at, ttl }
//! ```
//!
//! [`grok_consumer_client`]: https://docs.rs/grok_consumer_client

mod browser_context;
mod mint_harness;
mod minted_statsig;
mod statsig;
mod statsig_material;
mod statsig_minter;
mod statsig_oracle;
mod statsig_request;

// Harvesting fresh signatures from a real browser and caching them.
pub mod statsig_cache_file;
#[cfg(feature = "webview-harvest")]
pub mod harvest_via_webview;

pub use browser_context::{BrowserContext, DEFAULT_USER_AGENT};
pub use mint_harness::MINT_HARNESS_SCRIPT;
pub use minted_statsig::MintedStatsig;
pub use statsig::Statsig;
pub use statsig_material::StatsigMaterial;
pub use statsig_minter::{MinterConfig, StatsigMinter};
pub use statsig_oracle::{StatsigError, StatsigOracle};
pub use statsig_request::StatsigRequest;
