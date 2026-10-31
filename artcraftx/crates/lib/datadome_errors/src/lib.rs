//! datadome_errors
//!
//! Recognize responses produced by DataDome's bot protection (used by
//! higgsfield.ai among others) rather than by the site's API, and say which
//! kind: a CAPTCHA challenge, a device-check interstitial, or a hard block.
//! Pair with `datadome_mitigation` for what to do about each.
//!
//! DataDome answers a blocked request with `403` and a small JSON body:
//!
//! ```json
//! {"url":"https://geo.captcha-delivery.com/captcha/?initialCid=...&hash=...&cid=...&t=fe&referer=...&s=...","cid":"..."}
//! ```
//!
//! The `url` path says which page it would show a browser (`/captcha/`,
//! `/interstitial/`), and `t=bv` in its query means the client is banned
//! outright rather than challenged. Responses also carry an `x-datadome`
//! header (`protected`) and, on blocks, `x-dd-b`.

// Never allow these
#![forbid(private_bounds)]
#![forbid(private_interfaces)]
#![forbid(unused_must_use)]

// Okay to toggle
#![forbid(unreachable_patterns)]
#![forbid(unused_imports)]
#![forbid(unused_mut)]
#![forbid(unused_variables)]

// Always allow
#![allow(dead_code)]

pub mod classify_datadome_response;
pub mod datadome_error;
pub mod datadome_response_signals;
pub mod filter_datadome_errors;
