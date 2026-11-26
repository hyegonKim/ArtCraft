//! Import of Adobe XD (`.xd`) archives (ZIP + DCX/UCF `manifest` + AGC JSON).
//!
//! An `.xd` file is a UCF/ZIP container with `mimetype` =
//! `application/vnd.adobe.sparkler.project+dcxucf`, a root `manifest` JSON listing artboards under
//! `artwork/artboard-*/graphics/graphicContent.agc`, shared symbols/gradients/clip paths under
//! `resources/graphics/graphicContent.agc`, and embedded raster/video assets under
//! `resources/<uid>`.
//!
//! Artboards become top-level [`NodeKind::Frame`] layers positioned at their manifest
//! `uxdesign#bounds` (with vertical scroll enabled when `uxdesign#viewport.height` is shorter than
//! the artboard). Shapes (`rect`, `circle`, `ellipse`, `line`, `polygon`, `path`, `compound`),
//! styled multi-run `text`, `group`s, `clipPath` masks, `repeatGrid` containers, master symbols and
//! `syncRef` instances, image/video pattern fills, gradients, strokes, shadows/blurs, and prototype
//! `interactions` are mapped into editable FigCraft [`Document`] nodes. Unsupported Adobe XD
//! features (such as voice/audio playback actions) emit structured import warnings rather than
//! failing the import.

mod archive;
mod proto;
mod style;
mod tree;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests;

use std::io::Cursor;

use figcraft_doc::Document;
use zip::ZipArchive;

use crate::FormatError;
pub use archive::is_xd;

/// Import `bytes` when they look like an Adobe XD `.xd` archive; `None` otherwise.
pub fn try_import(bytes: &[u8], name: &str) -> Option<Result<(Document, Vec<String>), FormatError>> {
    is_xd(bytes).then(|| import(bytes, name))
}

/// Build an editable FigCraft [`Document`] and structured warning list from an Adobe XD `.xd` file.
pub fn import(bytes: &[u8], fallback_name: &str) -> Result<(Document, Vec<String>), FormatError> {
    let mut z = ZipArchive::new(Cursor::new(bytes)).map_err(|e| archive::corrupt(e.to_string()))?;
    let mut budget = archive::Budget::new();
    let mut archive_warnings = Vec::new();
    let parsed = archive::read_archive(bytes, fallback_name, &mut z, &mut budget, &mut archive_warnings)?;
    tree::build_document(parsed, z, budget, archive_warnings)
}
