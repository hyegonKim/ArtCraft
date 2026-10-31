//! First-party (cookie-session) Higgsfield video generation.
//!
//! Runs on the user's own higgsfield.ai account through a
//! [`HiggsfieldSession`](higgsfield_client::session::higgsfield_session::HiggsfieldSession).
//! Keyframes and image / video / audio references have to live on
//! Higgsfield first, so any request with media goes through the draft
//! phase: `finalize()` downloads each reference (ArtCraft CDN URL or media
//! token), uploads it as Higgsfield reference media (with the IP check the
//! Seedance models insist on), and the request goes out with the resulting
//! media ids tagged by role (start / end frame, image, video, audio).
//!
//! Each model has its own `build.rs` that plans the request against the
//! option menus Higgsfield actually offers — unsupported resolutions,
//! durations, ratios and batch sizes snap to the nearest tier per the
//! mismatch strategy, and reference kinds a model can't take are dropped
//! with a warning. The plumbing after that (draft, upload, send, cost) is
//! shared across all models via [`video_request::HiggsfieldVideoRequest`].

pub mod common;
#[cfg(test)]
pub(crate) mod common_test_helpers;
pub mod cost;
pub mod draft;
pub mod grok_imagine_1p5;
pub mod kling_3p0_pro;
pub mod kling_3p0_standard;
pub mod minimax_h3;
pub mod request;
pub mod seedance_2p0;
pub mod seedance_2p0_mini;
pub mod seedance_2p5;
pub mod seedance_2p5_edit;
pub mod video_request;
