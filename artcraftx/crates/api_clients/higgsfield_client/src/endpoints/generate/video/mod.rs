//! Video generation bindings. Every model here is text-to-video (plus
//! optional reference media the web app calls `medias`); each exposes
//! duration as a one-second slider with its own range, and the shared
//! [`VideoDurationRange`](crate::types::video_duration::VideoDurationRange)
//! constant on each request type says which.
//!
//! Completed video jobs come back through the same status endpoints as
//! images; `results.raw` / `results.min` are `type: "video"` `.mp4` URLs
//! with a `thumbnail_url` poster frame.

pub mod grok_imagine_1p5;
pub mod kling_3p0;
pub mod minimax_h3;
pub mod seedance_2p0;
pub mod seedance_2p0_mini;
pub mod seedance_2p5;
pub mod seedance_2p5_edit;

#[cfg(test)]
pub(crate) mod test_fixtures;
