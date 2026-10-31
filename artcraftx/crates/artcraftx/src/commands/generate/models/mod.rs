//! Tauri commands that list the models the app knows about, per modality.
//!
//! All of them serve the built-in tables from the first-party `models` crate
//! verbatim; the frontend's pickers are built from these responses alone.
pub mod audio;
pub mod image;
pub mod mesh;
pub mod splat;
pub mod video;
