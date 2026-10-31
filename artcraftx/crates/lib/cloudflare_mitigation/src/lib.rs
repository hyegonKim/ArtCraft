//! cloudflare_mitigation
//!
//! What to do when Cloudflare's edge answers instead of the origin. The
//! errors themselves live in `cloudflare_errors`; this crate holds the
//! counter-measures every API client and the desktop app share:
//!
//! - [`strategy`] — map a [`CloudflareError`](cloudflare_errors::cloudflare_error::CloudflareError)
//!   to a [`CloudflareMitigation`](strategy::cloudflare_mitigation::CloudflareMitigation):
//!   retry with backoff, or send the user back to a browser.
//! - [`clearance`] — keep replayed `cf_clearance` cookies valid by presenting
//!   the same User-Agent (on the same browser family's fingerprint) as the
//!   browser that earned them.
//! - [`headers`] — exact header casing/order for protocol paths Cloudflare
//!   fingerprints (websocket upgrades).

pub mod clearance;
pub mod headers;
pub mod strategy;
