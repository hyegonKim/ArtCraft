#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

//! Clean-room reader and writer for Creative Cloud Libraries (`.cclibs` / `.cclib`, `LIBS v4.20.2`
//! DCX + ZIP container) and Adobe Swatch Exchange (`.ase`, `ASEF` v1.0 big-endian binary palettes).
//!
//! - [`CcElementType`] models all 19 `LIBS v4.20.2` element MIME types (`color+dcx`,
//!   `colortheme+dcx`, `gradient+dcx`, `characterstyle+dcx`, `paragraphstyle+dcx`, `brush+dcx`,
//!   `layerstyle+dcx`, `image+dcx`, `3d+dcx`, `material+dcx`, `light+dcx`, `audio+dcx`,
//!   `video+dcx`, `animation+dcx`, `template+dcx`, `component+dcx`, `text+dcx`, `agc+json`,
//!   `agc+dcx`).
//! - [`CcLibraryArchive`] reads and writes bounded `.cclibs` / `.cclib` ZIP archives with a DCX
//!   `manifest.json`, optional stored `mimetype`, element metadata, representations, and binary
//!   rendition/asset payloads.
//! - [`AsePalette`] reads and writes big-endian `ASEF` v1.0 `.ase` swatch files supporting RGB,
//!   CMYK, CIE LAB, and Grayscale color models, `Global` / `Spot` / `Process` color types, and
//!   `0xC001` / `0xC002` Color Group blocks.

mod archive;
mod ase;
mod element;

pub use archive::{
    CCLIBS_CONTAINER_MIME, CCLIBS_MANIFEST_NAME, CCLIBS_MIMETYPE_ENTRY, CcLibraryArchive, CcLibraryGroup, LIBS_SCHEMA_VERSION, is_cclibs_zip,
    valid_archive_asset_path,
};
pub use ase::{
    ASE_MAGIC, ASE_VERSION, AseColor, AseColorGroup, AseColorType, AseItem, AsePalette, AseSwatch, BLOCK_COLOR_ENTRY, BLOCK_GROUP_END,
    BLOCK_GROUP_START, is_ase_bytes,
};
pub use element::{CcColorValue, CcElement, CcElementType, CcRepresentation};

/// Resource limits enforced when decoding untrusted `.cclibs` archives and `.ase` binary files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Maximum number of entries permitted in a `.cclibs` ZIP archive.
    pub max_zip_entries: u64,
    /// Maximum total uncompressed bytes read across a `.cclibs` ZIP archive.
    pub max_total_zip_bytes: u64,
    /// Maximum byte size of `manifest.json`.
    pub max_manifest_bytes: u64,
    /// Maximum byte size of a single asset entry inside `.cclibs`.
    pub max_asset_bytes: u64,
    /// Maximum number of blocks permitted in an `.ase` binary swatch file.
    pub max_ase_blocks: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_zip_entries: 16_384,
            max_total_zip_bytes: 512 << 20,
            max_manifest_bytes: 16 << 20,
            max_asset_bytes: 128 << 20,
            max_ase_blocks: 65_536,
        }
    }
}

/// Errors returned while reading or writing `.cclibs` archives and `.ase` swatch files.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// ZIP container error when opening or writing `.cclibs`.
    #[error(transparent)]
    Zip(#[from] craft_zip::Error),
    /// Invalid or unsupported `.cclibs` archive structure or manifest.
    #[error("invalid Creative Cloud Library archive: {0}")]
    InvalidArchive(String),
    /// Invalid asset path inside a `.cclibs` archive.
    #[error("invalid .cclibs asset path: {0}")]
    InvalidAssetPath(String),
    /// Unknown or unsupported Creative Cloud Library element MIME type.
    #[error("unsupported CC Library element MIME type: {0}")]
    UnsupportedElementMime(String),
    /// Invalid or truncated `.ase` binary swatch data.
    #[error("invalid Adobe Swatch Exchange (.ase) data: {0}")]
    InvalidAse(String),
    /// Input exceeded a caller-configured resource limit.
    #[error("{what} exceeds limit of {limit}")]
    LimitExceeded {
        /// Resource description.
        what: &'static str,
        /// Configured upper bound.
        limit: u64,
    },
}

/// Crate result alias.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests;
