//! Grok "Imagine" prompt flags: the `--flag=value` long args Grok reads out of
//! the prompt *text* to control image/video generation (e.g. `--mode`).
//!
//! Build a final prompt with [`PromptFlags::apply_to`]; both the image websocket
//! and the video binding funnel their prompt construction through this so it is
//! defined and tested in one place.

mod generation_mode;
mod prompt_flags;

pub use generation_mode::GenerationMode;
pub use prompt_flags::PromptFlags;
