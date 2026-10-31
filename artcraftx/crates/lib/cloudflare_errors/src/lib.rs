//! cloudflare_errors
//!
//! Recognize responses that came from Cloudflare's edge rather than the
//! origin — bot-management challenges, WAF blocks, and origin failures — so
//! every API client can react the same way. Pair with `cloudflare_mitigation`
//! for what to do about each.
//!
//! Entry points:
//! - [`classify_cloudflare_response`](classify_cloudflare_response::classify_cloudflare_response)
//!   — the full classifier: status + body + the Cloudflare response headers.
//! - [`filter_cloudflare_errors`](filter_cloudflare_errors::filter_cloudflare_errors)
//!   — the status + body shortcut most clients use.

// Never allow these
#![forbid(private_bounds)]
#![forbid(private_interfaces)]
#![forbid(unused_must_use)] // NB: It's unsafe to not close/check some things

// Okay to toggle
#![forbid(unreachable_patterns)]
#![forbid(unused_imports)]
#![forbid(unused_mut)]
#![forbid(unused_variables)]

// Always allow
#![allow(dead_code)]
#![allow(non_snake_case)]

pub mod classify_cloudflare_response;
pub mod cloudflare_error;
pub mod cloudflare_response_signals;
pub mod filter_cloudflare_errors;
