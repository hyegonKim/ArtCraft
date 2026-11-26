//! Unpacking an extension archive that nobody has vouched for.
//!
//! Invariants: the archive must be exactly what its manifest says. Every entry is named in the
//! manifest (so nothing is smuggled in), every name passes the shared path rules (so nothing escapes
//! the folder), links and encrypted entries are refused, each file is streamed with a hard byte limit
//! (so a lying size header or a compression bomb cannot fill the disk or memory), and its SHA-256 and
//! size must match before the call returns. Existing files are never overwritten.

use craft_ext_core::manifest::{MANIFEST_NAME, MAX_MANIFEST_BYTES, Manifest, ManifestError};
use craft_ext_core::path::{PathError, validate_path};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::Path;
use zip::ZipArchive;

/// Limits applied before and while reading. The defaults suit the largest kind (a resource pack).
#[derive(Clone, Copy, Debug)]
pub struct Caps {
    /// Largest archive accepted, in bytes.
    pub max_archive: u64,
    /// Largest allowed ratio of unpacked to packed size for one file.
    pub max_ratio: u64,
}

impl Default for Caps {
    fn default() -> Self {
        Self { max_archive: 300 * 1024 * 1024, max_ratio: 200 }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnpackError {
    TooBig,
    NotAnArchive,
    TooManyEntries,
    NoManifest,
    Manifest(ManifestError),
    BadName(PathError),
    Encrypted,
    Link,
    Unlisted,
    Missing,
    SizeMismatch,
    HashMismatch,
    Bomb,
    Unsupported,
    Write,
}

impl fmt::Display for UnpackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnpackError::TooBig => f.write_str("the package file is too big"),
            UnpackError::NotAnArchive => f.write_str("this is not a package file"),
            UnpackError::TooManyEntries => f.write_str("the package holds too many files"),
            UnpackError::NoManifest => f.write_str("the package has no description of itself"),
            UnpackError::Manifest(e) => write!(f, "{e}"),
            UnpackError::BadName(e) => write!(f, "{e}"),
            UnpackError::Encrypted => f.write_str("the package is locked with a password"),
            UnpackError::Link => f.write_str("the package contains a shortcut, which is not allowed"),
            UnpackError::Unlisted => f.write_str("the package holds a file its description does not mention"),
            UnpackError::Missing => f.write_str("the package is missing a file its description promises"),
            UnpackError::SizeMismatch => f.write_str("a file in the package is not the size it claims to be"),
            UnpackError::HashMismatch => f.write_str("a file in the package has been changed or damaged"),
            UnpackError::Bomb => f.write_str("a file in the package unpacks to far more than it should"),
            UnpackError::Unsupported => f.write_str("the package is packed in a way this app does not support"),
            UnpackError::Write => f.write_str("the package's files could not be saved"),
        }
    }
}

impl std::error::Error for UnpackError {}

/// Most entries (files and folders) any archive may hold, checked before anything is read.
pub const MAX_ENTRIES: usize = 600;
/// Ratio checks apply only above this size; tiny files may compress very well and cost nothing.
const RATIO_FLOOR: u64 = 1024 * 1024;

fn hex_digest(hasher: Sha256) -> String {
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Checks and unpacks `bytes` into `dest`, which must be an empty or new folder. On error `dest` may
/// hold partial files; the caller removes it.
pub fn unpack_into(bytes: &[u8], dest: &Path, caps: Caps) -> Result<Manifest, UnpackError> {
    if bytes.len() as u64 > caps.max_archive {
        return Err(UnpackError::TooBig);
    }
    let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(|_| UnpackError::NotAnArchive)?;
    if zip.len() > MAX_ENTRIES {
        return Err(UnpackError::TooManyEntries);
    }

    // Pass 1: names and kinds only, nothing decompressed.
    let mut files: Vec<(usize, String)> = Vec::new();
    for index in 0..zip.len() {
        let entry = zip.by_index_raw(index).map_err(|_| UnpackError::NotAnArchive)?;
        let raw = entry.name().map_err(|_| UnpackError::NotAnArchive)?.into_owned();
        let name = raw.strip_suffix('/').unwrap_or(&raw);
        validate_path(name).map_err(UnpackError::BadName)?;
        if entry.encrypted() {
            return Err(UnpackError::Encrypted);
        }
        if entry.is_symlink() {
            return Err(UnpackError::Link);
        }
        if !entry.is_dir() {
            files.push((index, name.to_string()));
        }
    }

    // Pass 2: the manifest, read with a hard limit.
    let manifest_index = files.iter().find(|(_, n)| n == MANIFEST_NAME).map(|(i, _)| *i).ok_or(UnpackError::NoManifest)?;
    let mut text = Vec::new();
    {
        let entry = zip.by_index(manifest_index).map_err(|_| UnpackError::Unsupported)?;
        entry.take(MAX_MANIFEST_BYTES as u64 + 1).read_to_end(&mut text).map_err(|_| UnpackError::Unsupported)?;
    }
    let manifest = Manifest::parse(&text).map_err(UnpackError::Manifest)?;

    // Pass 3: the archive and the manifest must agree exactly.
    let listed: BTreeSet<&str> = manifest.files.iter().map(|f| f.path.as_str()).collect();
    if files.iter().any(|(_, n)| n != MANIFEST_NAME && !listed.contains(n.as_str())) {
        return Err(UnpackError::Unlisted);
    }
    if manifest.files.iter().any(|f| !files.iter().any(|(_, n)| *n == f.path)) {
        return Err(UnpackError::Missing);
    }

    // Pass 4: stream each listed file out, counting bytes and hashing as it goes.
    fs::create_dir_all(dest).map_err(|_| UnpackError::Write)?;
    for want in &manifest.files {
        let index = files.iter().find(|(_, n)| *n == want.path).map(|(i, _)| *i).ok_or(UnpackError::Missing)?;
        let entry = zip.by_index(index).map_err(|_| UnpackError::Unsupported)?;
        if entry.size() != want.size {
            return Err(UnpackError::SizeMismatch);
        }
        if entry.size() > RATIO_FLOOR && entry.size() / entry.compressed_size().max(1) > caps.max_ratio {
            return Err(UnpackError::Bomb);
        }
        let target = dest.join(&want.path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|_| UnpackError::Write)?;
        }
        let mut out = fs::OpenOptions::new().write(true).create_new(true).open(&target).map_err(|_| UnpackError::Write)?;
        let mut reader = entry.take(want.size + 1);
        let mut hasher = Sha256::new();
        let mut total: u64 = 0;
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = reader.read(&mut buf).map_err(|_| UnpackError::Unsupported)?;
            if n == 0 {
                break;
            }
            total += n as u64;
            if total > want.size {
                return Err(UnpackError::SizeMismatch);
            }
            let chunk = buf.get(..n).ok_or(UnpackError::Unsupported)?;
            hasher.update(chunk);
            out.write_all(chunk).map_err(|_| UnpackError::Write)?;
        }
        if total != want.size {
            return Err(UnpackError::SizeMismatch);
        }
        if hex_digest(hasher) != want.sha256 {
            return Err(UnpackError::HashMismatch);
        }
    }
    Ok(manifest)
}

#[cfg(test)]
mod tests;
