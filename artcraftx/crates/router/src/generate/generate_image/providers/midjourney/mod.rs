//! First-party (cookie-session) Midjourney image generation.
//!
//! This provider drives the user's own midjourney.com session via captured
//! cookies (see `RouterMidjourneyClient`). It is distinct from:
//!   - `providers/kinovi/midjourney_*` — backend-billed via Kinovi/Seedance2Pro
//!   - `providers/artcraft/midjourney_*` — routed through the Artcraft backend
//!
//! Submission is text-to-image only for now; image references are not yet
//! supported on this path.

pub mod midjourney_8;
