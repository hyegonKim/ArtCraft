//! Adobe `.ccx` (Creative Cloud Extension) ZIP archive reader, traversal-safe validator,
//! unpacker, and bridge into native [`craft_ext_core::manifest::Manifest`] packages.

use crate::unpack::{Caps, MAX_ENTRIES};
use craft_ext_core::manifest::{FileEntry, Kind, Manifest};
use craft_ext_core::path::{PathError, validate_all, validate_path};
use craft_ext_core::uxp::{MAX_UXP_MANIFEST_BYTES, UXP_MANIFEST_NAME, UxpError, UxpManifest, UxpNativeExtensionMetadata};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::Path;
use zip::ZipArchive;

const RATIO_FLOOR: u64 = 1024 * 1024;

/// Validated `.ccx` package metadata and bridged Craft extension descriptors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CcxPackage {
    /// Parsed UXP v4/v5 manifest from `manifest.json`.
    pub uxp_manifest: UxpManifest,
    /// Native Craft host extension metadata (routed `AppId`s, entrypoints, permission audit).
    pub native_metadata: UxpNativeExtensionMetadata,
    /// Converted native Craft [`Manifest`] with SHA-256 checksums for every payload file.
    pub craft_manifest: Manifest,
    /// Hashed payload entries inside the `.ccx` archive (excluding `manifest.json`).
    pub files: Vec<FileEntry>,
}

/// Why a `.ccx` package was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CcxError {
    TooBig,
    NotAnArchive,
    TooManyEntries,
    BadPath(PathError),
    Encrypted,
    Link,
    MissingManifest,
    Manifest(UxpError),
    MissingMainFile(String),
    FileTooBig,
    TotalTooBig,
    SizeMismatch,
    Bomb,
    Unsupported,
    Write,
}

impl fmt::Display for CcxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooBig => f.write_str("the .ccx package exceeds the archive size limit"),
            Self::NotAnArchive => f.write_str("the .ccx file is not a valid ZIP archive"),
            Self::TooManyEntries => f.write_str("the .ccx archive contains too many entries"),
            Self::BadPath(e) => write!(f, "unsafe path in .ccx archive: {e}"),
            Self::Encrypted => f.write_str("encrypted .ccx entries are not allowed"),
            Self::Link => f.write_str("symbolic links in .ccx archives are not allowed"),
            Self::MissingManifest => f.write_str("the .ccx archive is missing manifest.json"),
            Self::Manifest(e) => write!(f, "{e}"),
            Self::MissingMainFile(main) => write!(f, "the .ccx archive is missing its declared main file '{main}'"),
            Self::FileTooBig => f.write_str("a file in the .ccx archive exceeds the per-file size limit"),
            Self::TotalTooBig => f.write_str("the .ccx archive unpacked payload exceeds the total size limit"),
            Self::SizeMismatch => f.write_str("a .ccx entry's unpacked length disagreed with its ZIP header"),
            Self::Bomb => f.write_str("a .ccx entry exceeded the maximum decompression ratio"),
            Self::Unsupported => f.write_str("the .ccx archive uses an unsupported compression stream"),
            Self::Write => f.write_str("failed to write unpacked .ccx files to disk"),
        }
    }
}

impl std::error::Error for CcxError {}

/// Inspect and validate an in-memory `.ccx` ZIP archive without touching the filesystem.
pub fn inspect_ccx_archive(bytes: &[u8], publisher_name: &str, caps: Caps) -> Result<CcxPackage, CcxError> {
    process_ccx_archive(bytes, publisher_name, None, caps)
}

/// Validate and safely unpack a `.ccx` ZIP archive into `dest`, returning its bridged [`CcxPackage`].
pub fn unpack_ccx_into(bytes: &[u8], dest: &Path, publisher_name: &str, caps: Caps) -> Result<CcxPackage, CcxError> {
    process_ccx_archive(bytes, publisher_name, Some(dest), caps)
}

fn process_ccx_archive(bytes: &[u8], publisher_name: &str, dest: Option<&Path>, caps: Caps) -> Result<CcxPackage, CcxError> {
    if bytes.len() as u64 > caps.max_archive {
        return Err(CcxError::TooBig);
    }
    let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(|_| CcxError::NotAnArchive)?;
    if zip.len() > MAX_ENTRIES {
        return Err(CcxError::TooManyEntries);
    }

    // Pass 1: inspect all raw entry headers before decompressing any bytes.
    let mut file_entries: Vec<(usize, String)> = Vec::new();
    for index in 0..zip.len() {
        let entry = zip.by_index_raw(index).map_err(|_| CcxError::NotAnArchive)?;
        let raw_name = entry.name().map_err(|_| CcxError::NotAnArchive)?.into_owned();
        let trimmed = raw_name.strip_suffix('/').unwrap_or(&raw_name);
        validate_path(trimmed).map_err(CcxError::BadPath)?;
        if entry.encrypted() {
            return Err(CcxError::Encrypted);
        }
        if entry.is_symlink() {
            return Err(CcxError::Link);
        }
        if !entry.is_dir() {
            file_entries.push((index, trimmed.to_string()));
        }
    }

    // Verify no duplicate paths or case collisions across all file paths in the archive.
    validate_all(file_entries.iter().map(|(_, p)| p.as_str())).map_err(CcxError::BadPath)?;

    // Pass 2: bounded read & validation of `manifest.json`.
    let manifest_idx = file_entries.iter().find(|(_, name)| name == UXP_MANIFEST_NAME).map(|(idx, _)| *idx).ok_or(CcxError::MissingManifest)?;

    let mut manifest_bytes = Vec::new();
    {
        let entry = zip.by_index(manifest_idx).map_err(|_| CcxError::Unsupported)?;
        entry.take(MAX_UXP_MANIFEST_BYTES as u64 + 1).read_to_end(&mut manifest_bytes).map_err(|_| CcxError::Unsupported)?;
    }
    let uxp_manifest = UxpManifest::from_json_bytes(&manifest_bytes).map_err(CcxError::Manifest)?;

    // Pass 3: verify declared `main` entrypoint exists in the archive.
    if !file_entries.iter().any(|(_, name)| name == &uxp_manifest.main) {
        return Err(CcxError::MissingMainFile(uxp_manifest.main.clone()));
    }

    if let Some(root) = dest {
        fs::create_dir_all(root).map_err(|_| CcxError::Write)?;
    }

    // Pass 4: stream, verify bounds/ratio, hash, and optionally write every payload file.
    let limits = Kind::Resource.limits();
    let mut total_unpacked: u64 = 0;
    let mut hashed_files: Vec<FileEntry> = Vec::new();
    let mut buf = vec![0u8; 64 * 1024];

    for (index, rel_path) in &file_entries {
        if rel_path == UXP_MANIFEST_NAME {
            if let Some(root) = dest {
                let out_path = root.join(UXP_MANIFEST_NAME);
                let mut f = fs::OpenOptions::new().write(true).create_new(true).open(out_path).map_err(|_| CcxError::Write)?;
                f.write_all(&manifest_bytes).map_err(|_| CcxError::Write)?;
            }
            continue;
        }

        let entry = zip.by_index(*index).map_err(|_| CcxError::Unsupported)?;
        let declared_size = entry.size();
        if declared_size > limits.max_file {
            return Err(CcxError::FileTooBig);
        }
        if declared_size > RATIO_FLOOR && declared_size / entry.compressed_size().max(1) > caps.max_ratio {
            return Err(CcxError::Bomb);
        }

        let mut out_file = match dest {
            Some(root) => {
                let target = root.join(rel_path);
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(|_| CcxError::Write)?;
                }
                Some(fs::OpenOptions::new().write(true).create_new(true).open(target).map_err(|_| CcxError::Write)?)
            }
            None => None,
        };

        let mut reader = entry.take(declared_size.saturating_add(1));
        let mut hasher = Sha256::new();
        let mut file_bytes: u64 = 0;

        loop {
            let n = reader.read(&mut buf).map_err(|_| CcxError::Unsupported)?;
            if n == 0 {
                break;
            }
            file_bytes = file_bytes.saturating_add(n as u64);
            if file_bytes > declared_size {
                return Err(CcxError::SizeMismatch);
            }
            total_unpacked = total_unpacked.saturating_add(n as u64);
            if total_unpacked > limits.max_total {
                return Err(CcxError::TotalTooBig);
            }
            let chunk = buf.get(..n).ok_or(CcxError::Unsupported)?;
            hasher.update(chunk);
            if let Some(out) = out_file.as_mut() {
                out.write_all(chunk).map_err(|_| CcxError::Write)?;
            }
        }

        if file_bytes != declared_size {
            return Err(CcxError::SizeMismatch);
        }

        let sha256: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
        hashed_files.push(FileEntry { path: rel_path.clone(), sha256, size: file_bytes });
    }

    let native_metadata = uxp_manifest.to_native_metadata().map_err(CcxError::Manifest)?;
    let craft_manifest = uxp_manifest.to_craft_manifest(publisher_name, hashed_files.clone()).map_err(CcxError::Manifest)?;

    Ok(CcxPackage { uxp_manifest, native_metadata, craft_manifest, files: hashed_files })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::zip_of;
    use craft_ext_core::id::AppId;
    use serde_json::json;

    fn sample_uxp_manifest_bytes() -> Vec<u8> {
        json!({
            "manifestVersion": 5,
            "id": "com.acme.VectorTools",
            "name": "Acme Vector Tools",
            "version": "1.2.0",
            "main": "ui/panel.html",
            "host": [
                { "app": "ILST", "minVersion": "27.0" },
                { "app": "XD", "minVersion": "55.0.0" }
            ],
            "entrypoints": [
                { "type": "panel", "id": "vector.panel", "label": "Vector Tools" },
                { "type": "command", "id": "vector.align", "label": "Smart Align" }
            ],
            "requiredPermissions": {
                "localFileSystem": "plugin",
                "clipboard": "read"
            }
        })
        .to_string()
        .into_bytes()
    }

    #[test]
    fn inspects_and_unpacks_valid_ccx_archive_and_bridges_to_craft_manifest() {
        let manifest_bytes = sample_uxp_manifest_bytes();
        let html = b"<!doctype html><html><body>Vector Tools</body></html>";
        let script = b"console.log('ready');";
        let ccx = zip_of(&[("manifest.json", &manifest_bytes), ("ui/panel.html", html), ("ui/main.js", script)]);

        let inspected = inspect_ccx_archive(&ccx, "Acme Corp", Caps::default()).unwrap();
        assert_eq!(inspected.native_metadata.id.as_str(), "com.acme.vectortools");
        assert!(inspected.native_metadata.compat.contains_key(&AppId::new("vectorcraft").unwrap()));
        assert!(inspected.native_metadata.compat.contains_key(&AppId::new("figcraft").unwrap()));
        assert_eq!(inspected.files.len(), 2);
        assert_eq!(inspected.craft_manifest.files.len(), 2);

        let temp = tempfile::tempdir().unwrap();
        let dest = temp.path().join("unpacked_ccx");
        let unpacked = unpack_ccx_into(&ccx, &dest, "Acme Corp", Caps::default()).unwrap();
        assert_eq!(unpacked, inspected);
        assert_eq!(fs::read(dest.join("ui/panel.html")).unwrap(), html);
        assert_eq!(fs::read(dest.join("ui/main.js")).unwrap(), script);
        assert!(dest.join("manifest.json").is_file());
    }

    #[test]
    fn refuses_traversal_paths_missing_main_and_non_zip_ccx_inputs() {
        let manifest_bytes = sample_uxp_manifest_bytes();

        // Traversal entry in .ccx ZIP
        let traversal = zip_of(&[("manifest.json", &manifest_bytes), ("ui/panel.html", b"ok"), ("../outside.js", b"evil")]);
        assert_eq!(inspect_ccx_archive(&traversal, "Acme", Caps::default()), Err(CcxError::BadPath(PathError::DotSegment)));

        // Windows reserved device name in .ccx ZIP
        let reserved = zip_of(&[("manifest.json", &manifest_bytes), ("ui/panel.html", b"ok"), ("ui/nul.js", b"evil")]);
        assert_eq!(inspect_ccx_archive(&reserved, "Acme", Caps::default()), Err(CcxError::BadPath(PathError::ReservedName)));

        // Missing declared `main` ("ui/panel.html")
        let missing_main = zip_of(&[("manifest.json", &manifest_bytes), ("ui/other.html", b"wrong")]);
        assert_eq!(inspect_ccx_archive(&missing_main, "Acme", Caps::default()), Err(CcxError::MissingMainFile("ui/panel.html".to_string())));

        // Missing manifest.json
        let no_manifest = zip_of(&[("ui/panel.html", b"ok")]);
        assert_eq!(inspect_ccx_archive(&no_manifest, "Acme", Caps::default()), Err(CcxError::MissingManifest));

        // Garbage bytes
        assert_eq!(inspect_ccx_archive(b"not a zip archive", "Acme", Caps::default()), Err(CcxError::NotAnArchive));
    }
}
