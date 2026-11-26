//! The package manifest, `craft-extension.json`, read from an untrusted archive.
//!
//! Invariants: the size is capped before anything is parsed; unknown fields are ignored (so a newer
//! publisher's extra data never breaks an older host) but a package can never state its own trust
//! tier, because the type has no such field; permissions must be empty in version 1; every file the
//! package carries is listed with its size and SHA-256, and the list must match what the kind needs.

use crate::id::{AppId, ExtensionId, IdError};
use crate::path::{PathError, validate_all};
use crate::text::{Lines, TextError, validate_text};
use crate::version::Version;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

pub const MANIFEST_NAME: &str = "craft-extension.json";
pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;
pub const SCHEMA: u32 = 1;
pub const THEME_FILE: &str = "theme.json";
pub const PLUGIN_FILE: &str = "plugin.wasm";
pub const MAX_APPS: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Theme,
    Plugin,
    Resource,
}

/// Size limits that depend on what the package is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_files: usize,
    pub max_file: u64,
    pub max_total: u64,
}

impl Kind {
    pub fn limits(self) -> Limits {
        match self {
            Kind::Theme => Limits { max_files: 1, max_file: 64 * 1024, max_total: 64 * 1024 },
            Kind::Plugin => Limits { max_files: 1, max_file: 32 * 1024 * 1024, max_total: 32 * 1024 * 1024 },
            Kind::Resource => Limits { max_files: 512, max_file: 64 * 1024 * 1024, max_total: 256 * 1024 * 1024 },
        }
    }
}

/// Who the package says made it. A claim, never proof; trust comes from the tier the host assigns.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Publisher {
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Compat {
    /// Oldest app version the package works with.
    #[serde(default)]
    pub min_app: Option<Version>,
    /// Plug-in interface version the module was built for. Required for plug-ins, refused otherwise.
    #[serde(default)]
    pub interface: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    /// Lowercase hex SHA-256 of the file.
    pub sha256: String,
    pub size: u64,
}

/// Where the code came from: a source repository and the exact commit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub source: String,
    pub commit: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: u32,
    pub id: ExtensionId,
    pub name: String,
    pub version: Version,
    pub kind: Kind,
    pub publisher: Publisher,
    #[serde(default)]
    pub description: String,
    pub compat: BTreeMap<AppId, Compat>,
    pub files: Vec<FileEntry>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub provenance: Option<Provenance>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManifestError {
    TooLarge,
    NotReadable,
    UnsupportedSchema(u32),
    Id(IdError),
    Name(TextError),
    Description(TextError),
    Publisher(TextError),
    NoApps,
    TooManyApps,
    BadCompat,
    TooManyFiles,
    FileTooBig,
    TotalTooBig,
    BadHash,
    Files(PathError),
    MissingRequiredFile,
    PermissionsNotEmpty,
    BadProvenance,
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestError::TooLarge => f.write_str("the package description is too large"),
            ManifestError::NotReadable => f.write_str("the package description cannot be read"),
            ManifestError::UnsupportedSchema(n) => write!(f, "the package uses a format version ({n}) this app does not know"),
            ManifestError::Id(e) => write!(f, "the package id is not valid: {e}"),
            ManifestError::Name(e) => write!(f, "the package name is not valid: {e}"),
            ManifestError::Description(e) => write!(f, "the package description text is not valid: {e}"),
            ManifestError::Publisher(e) => write!(f, "the publisher name is not valid: {e}"),
            ManifestError::NoApps => f.write_str("the package does not say which apps it is for"),
            ManifestError::TooManyApps => f.write_str("the package lists too many apps"),
            ManifestError::BadCompat => f.write_str("the package's app requirements are not valid for this kind of package"),
            ManifestError::TooManyFiles => f.write_str("the package holds too many files"),
            ManifestError::FileTooBig => f.write_str("a file in the package is too big"),
            ManifestError::TotalTooBig => f.write_str("the package is too big"),
            ManifestError::BadHash => f.write_str("a file's checksum is not valid"),
            ManifestError::Files(e) => write!(f, "{e}"),
            ManifestError::MissingRequiredFile => f.write_str("the package is missing a file it needs"),
            ManifestError::PermissionsNotEmpty => f.write_str("the package asks for permissions, which are not available yet"),
            ManifestError::BadProvenance => f.write_str("the package's source details are not valid"),
        }
    }
}

impl std::error::Error for ManifestError {}

#[derive(Deserialize)]
struct Head {
    schema: u32,
}

impl Manifest {
    pub fn parse(bytes: &[u8]) -> Result<Self, ManifestError> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(ManifestError::TooLarge);
        }
        // Read the schema alone first, so a newer package is refused for being newer, not for a field
        // or a kind this host has never heard of.
        let head: Head = serde_json::from_slice(bytes).map_err(|_| ManifestError::NotReadable)?;
        if head.schema != SCHEMA {
            return Err(ManifestError::UnsupportedSchema(head.schema));
        }
        let manifest: Manifest = serde_json::from_slice(bytes).map_err(|_| ManifestError::NotReadable)?;
        manifest.validate()?;
        Ok(manifest)
    }

    fn validate(&self) -> Result<(), ManifestError> {
        validate_text(&self.name, 64, Lines::One).map_err(ManifestError::Name)?;
        if !self.description.is_empty() {
            validate_text(&self.description, 500, Lines::Many).map_err(ManifestError::Description)?;
        }
        validate_text(&self.publisher.name, 64, Lines::One).map_err(ManifestError::Publisher)?;
        if !self.permissions.is_empty() {
            return Err(ManifestError::PermissionsNotEmpty);
        }
        self.validate_compat()?;
        self.validate_files()?;
        if let Some(p) = &self.provenance {
            validate_provenance(p)?;
        }
        Ok(())
    }

    fn validate_compat(&self) -> Result<(), ManifestError> {
        if self.compat.is_empty() {
            return Err(ManifestError::NoApps);
        }
        if self.compat.len() > MAX_APPS {
            return Err(ManifestError::TooManyApps);
        }
        for compat in self.compat.values() {
            let ok = match (self.kind, compat.interface) {
                (Kind::Plugin, Some(n)) => n >= 1,
                (Kind::Plugin, None) => false,
                (_, interface) => interface.is_none(),
            };
            if !ok {
                return Err(ManifestError::BadCompat);
            }
        }
        Ok(())
    }

    fn validate_files(&self) -> Result<(), ManifestError> {
        let limits = self.kind.limits();
        if self.files.is_empty() {
            return Err(ManifestError::MissingRequiredFile);
        }
        if self.files.len() > limits.max_files {
            return Err(ManifestError::TooManyFiles);
        }
        let mut total: u64 = 0;
        for file in &self.files {
            if !is_sha256_hex(&file.sha256) {
                return Err(ManifestError::BadHash);
            }
            if file.size > limits.max_file {
                return Err(ManifestError::FileTooBig);
            }
            total = total.saturating_add(file.size);
        }
        if total > limits.max_total {
            return Err(ManifestError::TotalTooBig);
        }
        validate_all(self.files.iter().map(|f| f.path.as_str())).map_err(ManifestError::Files)?;
        let required = match self.kind {
            Kind::Theme => Some(THEME_FILE),
            Kind::Plugin => Some(PLUGIN_FILE),
            Kind::Resource => None,
        };
        if required.is_some_and(|name| self.file(name).is_none()) {
            return Err(ManifestError::MissingRequiredFile);
        }
        Ok(())
    }

    pub fn file(&self, path: &str) -> Option<&FileEntry> {
        self.files.iter().find(|f| f.path == path)
    }
}

pub(crate) fn is_sha256_hex(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn validate_provenance(p: &Provenance) -> Result<(), ManifestError> {
    let commit_ok = p.commit.len() == 40 && p.commit.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    if !valid_https_url(&p.source, 200) || !commit_ok {
        return Err(ManifestError::BadProvenance);
    }
    Ok(())
}

/// A plain `https://host/path` address: printable ASCII, no spaces, no user name or password, a host
/// present, bounded length.
pub(crate) fn valid_https_url(url: &str, max_len: usize) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split('/').next().unwrap_or("");
    url.len() <= max_len && url.bytes().all(|b| b.is_ascii_graphic()) && !authority.is_empty() && !authority.contains('@')
}

#[cfg(test)]
mod tests;
