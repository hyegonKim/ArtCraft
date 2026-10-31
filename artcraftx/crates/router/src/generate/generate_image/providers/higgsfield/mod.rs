//! First-party (cookie-session) Higgsfield image generation.
//!
//! Runs on the user's own higgsfield.ai account through a
//! [`HiggsfieldSession`](higgsfield_client::session::higgsfield_session::HiggsfieldSession).
//! Reference images have to live on Higgsfield first, so a request with
//! image inputs goes through the draft phase: `finalize()` downloads each
//! reference (ArtCraft CDN URL or media token) and uploads it as Higgsfield
//! reference media, then the request goes out with the resulting media ids.
//!
//! Each model has its own `build.rs` that plans the request against the
//! option menus Higgsfield actually offers — unsupported resolutions,
//! ratios and batch sizes snap to the nearest tier per the mismatch strategy.
//! The plumbing after that (draft, upload, send, cost) is shared across all
//! models via [`image_request::HiggsfieldImageRequest`].

pub mod common;
#[cfg(test)]
pub(crate) mod common_test_helpers;
pub mod cost;
pub mod draft;
pub mod gpt_image_2;
pub mod image_request;
pub mod nano_banana_2;
pub mod nano_banana_2_lite;
pub mod nano_banana_pro;
pub mod request;
pub mod seedream_4p5;
pub mod seedream_5p0_lite;
pub mod seedream_5p0_pro;
