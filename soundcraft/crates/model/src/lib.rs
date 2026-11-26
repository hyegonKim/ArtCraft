//! Compatibility facade for SoundCraft's shared session document crate.
//!
//! Engine and UI adapters depend on this name, while the wire schema, JSON reader/writer,
//! validation, and bundle format live in `craft-scraft-format`.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub use craft_scraft_format::*;

pub mod fcpxml;
pub mod mainstage;

pub use fcpxml::{
    AudioRoleCategory, FcpxmlAudioImportSummary, beat_map_markers_to_tempo, export_fcpxml_audio, import_fcpxml_audio, is_fcpxml_document,
    smpte_lock_clips,
};
pub use mainstage::{
    ChannelStripConfig, Concert, LivePatchSwitcher, MacroControl, MacroMapping, MacroTarget, Patch, PatchId, PatchSet, PatchSetId, RoutedMidiNote,
};
