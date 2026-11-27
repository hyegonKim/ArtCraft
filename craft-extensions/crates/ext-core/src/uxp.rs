//! Clean-room Adobe UXP (`UXPD v2.4.0`) `manifest.json` (v4 & v5) parser, validator,
//! Creative Cloud SAPCode -> Craft Suite `AppId` router, permission auditor, and conversion
//! helpers into `craft-ext-core` native extension metadata.

use crate::id::{AppId, ExtensionId, IdError};
use crate::manifest::{Compat, FileEntry, Kind, MAX_APPS, Manifest, ManifestError, Publisher};
use crate::path::{PathError, validate_path};
use crate::text::{Lines, TextError, validate_text};
use crate::version::{Version, VersionError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const UXP_MANIFEST_NAME: &str = "manifest.json";
pub const MAX_UXP_MANIFEST_BYTES: usize = 64 * 1024;
pub const MAX_UXP_HOSTS: usize = MAX_APPS;
pub const MAX_UXP_ENTRYPOINTS: usize = 64;
pub const MAX_UXP_ENTRYPOINT_ID: usize = 64;
pub const MAX_UXP_DOMAINS: usize = 64;

/// Map a Creative Cloud `host.app` SAPCode to its canonical Craft Suite `app_id` string.
pub fn map_sap_code_to_slug(sap_code: &str) -> Option<&'static str> {
    match sap_code.trim().to_ascii_uppercase().as_str() {
        "PHSP" | "PS" => Some("photocraft"),
        "ILST" | "AI" => Some("vectorcraft"),
        "IDSN" | "ID" | "AICY" => Some("designcraft"),
        "PPRO" => Some("filmcraft"),
        "AEFT" => Some("effectcraft"),
        "AUDT" => Some("soundcraft"),
        "FLPR" => Some("anicraft"),
        "XD" => Some("figcraft"),
        "LRCC" | "LTRM" | "KBRG" => Some("lightcraft"),
        _ => None,
    }
}

/// Map a Creative Cloud `host.app` SAPCode to a validated Craft Suite [`AppId`].
pub fn map_sap_code_to_app_id(sap_code: &str) -> Option<AppId> {
    let slug = map_sap_code_to_slug(sap_code)?;
    AppId::new(slug).ok()
}

/// Supported UXP manifest schema versions (`4` or `5`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum UxpManifestVersion {
    V4 = 4,
    V5 = 5,
}

impl UxpManifestVersion {
    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            4 => Some(Self::V4),
            5 => Some(Self::V5),
            _ => None,
        }
    }

    pub fn as_u32(self) -> u32 {
        self as u32
    }
}

/// Host application compatibility descriptor in a UXP manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UxpHost {
    pub app: String,
    #[serde(rename = "minVersion", deserialize_with = "deserialize_uxp_version")]
    pub min_version: Version,
    #[serde(default, rename = "maxVersion", deserialize_with = "deserialize_opt_uxp_version")]
    pub max_version: Option<Version>,
}

impl UxpHost {
    /// Resolve this host's SAPCode (`PHSP`, `ILST`, `IDSN`, etc.) to its Craft Suite [`AppId`].
    pub fn craft_app_id(&self) -> Option<AppId> {
        map_sap_code_to_app_id(&self.app)
    }
}

/// Kind of UXP entrypoint (`"panel"` or `"command"`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UxpEntrypointKind {
    Panel,
    Command,
}

/// One UI panel or menu command entrypoint declared by a UXP extension.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UxpEntrypoint {
    #[serde(rename = "type")]
    pub kind: UxpEntrypointKind,
    pub id: String,
    #[serde(deserialize_with = "deserialize_uxp_label")]
    pub label: String,
}

/// UXP `requiredPermissions.network` declaration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UxpNetworkPermission {
    All(String),
    Enabled(bool),
    Domains {
        #[serde(default)]
        domains: Vec<String>,
    },
}

/// UXP `requiredPermissions.localFileSystem` access level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UxpFileSystemAccess {
    /// Read/write inside plugin data/install sandbox only.
    #[serde(rename = "plugin")]
    Plugin,
    /// Access to temporary folder only.
    #[serde(rename = "temp")]
    Temp,
    /// User-prompted file picker tokens (`"request"`).
    #[serde(rename = "request")]
    Request,
    /// Unrestricted host filesystem access (`"fullAccess"`).
    #[serde(rename = "fullAccess")]
    FullAccess,
}

/// UXP `requiredPermissions.clipboard` access level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UxpClipboardAccess {
    #[serde(rename = "read")]
    Read,
    #[serde(rename = "readAndWrite")]
    ReadAndWrite,
}

/// UXP `requiredPermissions.webview` configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UxpWebviewPermission {
    #[serde(default, deserialize_with = "deserialize_yes_or_bool")]
    pub allow: bool,
    #[serde(default)]
    pub domains: Vec<String>,
}

/// UXP `requiredPermissions.ipc` configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UxpIpcPermission {
    #[serde(default, rename = "enablePluginCommunication")]
    pub enable_plugin_communication: bool,
}

/// UXP v4/v5 `requiredPermissions` block.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UxpPermissions {
    #[serde(default)]
    pub network: Option<UxpNetworkPermission>,
    #[serde(default)]
    pub local_file_system: Option<UxpFileSystemAccess>,
    #[serde(default)]
    pub clipboard: Option<UxpClipboardAccess>,
    #[serde(default)]
    pub webview: Option<UxpWebviewPermission>,
    #[serde(default)]
    pub ipc: Option<UxpIpcPermission>,
}

/// Security audit summary computed over a UXP manifest's `requiredPermissions`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UxpPermissionAudit {
    /// Canonical capability identifiers requested (e.g. `"network"`, `"localFileSystem:fullAccess"`).
    pub capabilities: Vec<String>,
    /// Whether unrestricted network access (`"all"` or wildcard `*` domain) is requested.
    pub unrestricted_network: bool,
    /// Whether unrestricted host filesystem access (`"fullAccess"`) is requested.
    pub full_filesystem_access: bool,
    /// Whether clipboard write capability is requested.
    pub clipboard_write: bool,
    /// Whether embedded `<webview>` execution is enabled.
    pub uses_webview: bool,
    /// Whether inter-plugin IPC messaging is enabled.
    pub uses_ipc: bool,
}

impl UxpPermissionAudit {
    /// True if no capabilities at all were requested.
    pub fn is_sandboxed_zero_permission(&self) -> bool {
        self.capabilities.is_empty()
    }

    /// True when the extension asks for elevated privileges requiring explicit user review.
    pub fn requires_elevated_review(&self) -> bool {
        self.unrestricted_network || self.full_filesystem_access || self.uses_webview || self.uses_ipc
    }
}

/// Validated Adobe UXP v4 / v5 extension manifest (`manifest.json`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct UxpManifest {
    pub manifest_version: UxpManifestVersion,
    pub id: String,
    pub name: String,
    pub version: Version,
    pub main: String,
    pub hosts: Vec<UxpHost>,
    pub entrypoints: Vec<UxpEntrypoint>,
    pub required_permissions: UxpPermissions,
}

/// Native Craft extension metadata translated from a validated [`UxpManifest`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UxpNativeExtensionMetadata {
    pub id: ExtensionId,
    pub name: String,
    pub version: Version,
    pub main: String,
    pub compat: BTreeMap<AppId, Compat>,
    pub entrypoints: Vec<UxpEntrypoint>,
    pub permission_audit: UxpPermissionAudit,
}

/// Validation or parsing failure for a UXP `manifest.json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UxpError {
    TooLarge,
    InvalidJson,
    UnsupportedManifestVersion(u32),
    InvalidId(IdError),
    InvalidName(TextError),
    InvalidVersion(VersionError),
    InvalidMainPath(PathError),
    NoHosts,
    TooManyHosts,
    UnsupportedHostApp(String),
    InvalidHostVersionRange { app: String },
    NoEntrypoints,
    TooManyEntrypoints,
    InvalidEntrypointId(String),
    DuplicateEntrypointId(String),
    InvalidEntrypointLabel(TextError),
    InvalidPermissionDomain(String),
    CraftManifest(ManifestError),
}

impl fmt::Display for UxpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => f.write_str("the UXP manifest exceeds the 64 KiB limit"),
            Self::InvalidJson => f.write_str("the UXP manifest is not valid JSON or is missing required fields"),
            Self::UnsupportedManifestVersion(v) => write!(f, "UXP manifestVersion {v} is not supported (expected 4 or 5)"),
            Self::InvalidId(e) => write!(f, "invalid UXP plugin id: {e}"),
            Self::InvalidName(e) => write!(f, "invalid UXP plugin name: {e}"),
            Self::InvalidVersion(e) => write!(f, "invalid UXP plugin version: {e}"),
            Self::InvalidMainPath(e) => write!(f, "invalid UXP main entry path: {e}"),
            Self::NoHosts => f.write_str("UXP manifest must declare at least one host application"),
            Self::TooManyHosts => f.write_str("UXP manifest declares too many host applications"),
            Self::UnsupportedHostApp(app) => write!(f, "unsupported Creative Cloud host SAPCode '{app}'"),
            Self::InvalidHostVersionRange { app } => write!(f, "host '{app}' has maxVersion lower than minVersion"),
            Self::NoEntrypoints => f.write_str("UXP manifest must declare at least one panel or command entrypoint"),
            Self::TooManyEntrypoints => f.write_str("UXP manifest declares too many entrypoints"),
            Self::InvalidEntrypointId(id) => write!(f, "invalid UXP entrypoint id '{id}'"),
            Self::DuplicateEntrypointId(id) => write!(f, "duplicate UXP entrypoint id '{id}'"),
            Self::InvalidEntrypointLabel(e) => write!(f, "invalid UXP entrypoint label: {e}"),
            Self::InvalidPermissionDomain(d) => write!(f, "invalid UXP permission domain '{d}'"),
            Self::CraftManifest(e) => write!(f, "cannot convert UXP manifest to Craft manifest: {e}"),
        }
    }
}

impl std::error::Error for UxpError {}

#[derive(Deserialize)]
struct RawHead {
    #[serde(rename = "manifestVersion")]
    manifest_version: u32,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OneOrManyHosts {
    One(UxpHost),
    Many(Vec<UxpHost>),
}

impl OneOrManyHosts {
    fn into_vec(self) -> Vec<UxpHost> {
        match self {
            Self::One(h) => vec![h],
            Self::Many(v) => v,
        }
    }
}

#[derive(Deserialize)]
struct RawUxpManifest {
    id: String,
    name: String,
    #[serde(deserialize_with = "deserialize_uxp_version")]
    version: Version,
    main: String,
    host: OneOrManyHosts,
    entrypoints: Vec<UxpEntrypoint>,
    #[serde(default, rename = "requiredPermissions")]
    required_permissions: UxpPermissions,
}

impl UxpManifest {
    /// Parse and validate a UXP `manifest.json` string (`manifestVersion` 4 or 5).
    pub fn from_json_str(json: &str) -> Result<Self, UxpError> {
        Self::from_json_bytes(json.as_bytes())
    }

    /// Parse and validate raw UTF-8 bytes of a UXP `manifest.json` with hard size bounds.
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, UxpError> {
        if bytes.len() > MAX_UXP_MANIFEST_BYTES {
            return Err(UxpError::TooLarge);
        }
        let head: RawHead = serde_json::from_slice(bytes).map_err(|_| UxpError::InvalidJson)?;
        let Some(manifest_version) = UxpManifestVersion::from_u32(head.manifest_version) else {
            return Err(UxpError::UnsupportedManifestVersion(head.manifest_version));
        };

        let raw: RawUxpManifest = serde_json::from_slice(bytes).map_err(|_| UxpError::InvalidJson)?;
        let manifest = Self {
            manifest_version,
            id: raw.id,
            name: raw.name,
            version: raw.version,
            main: raw.main,
            hosts: raw.host.into_vec(),
            entrypoints: raw.entrypoints,
            required_permissions: raw.required_permissions,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    /// Normalize the UXP `id` into a canonical Craft [`ExtensionId`].
    ///
    /// Reverse-DNS UXP IDs (e.g. `"com.example.MyPanel"`) are lowercased (`"com.example.mypanel"`).
    /// Single-token Adobe Marketplace IDs (e.g. `"a1b2c3d4"`) are namespaced under `"uxp.<id>"`.
    pub fn craft_extension_id(&self) -> Result<ExtensionId, IdError> {
        validate_raw_uxp_id(&self.id)?;
        let lowered = self.id.to_ascii_lowercase();
        if lowered.contains('.') {
            ExtensionId::new(&lowered)
        } else {
            let namespaced = format!("uxp.{lowered}");
            ExtensionId::new(&namespaced)
        }
    }

    /// Map every declared `host` entry to its target Craft Suite [`AppId`] and [`Compat`] constraint.
    ///
    /// When multiple Creative Cloud hosts map to the same Craft app (such as `LRCC` and `LTRM` both
    /// routing to `lightcraft`, or `IDSN` and `AICY` both routing to `designcraft`), the highest
    /// `minVersion` among them is retained.
    pub fn routed_compat(&self) -> Result<BTreeMap<AppId, Compat>, UxpError> {
        let mut map: BTreeMap<AppId, Compat> = BTreeMap::new();
        for host in &self.hosts {
            let app_id = host.craft_app_id().ok_or_else(|| UxpError::UnsupportedHostApp(host.app.clone()))?;
            map.entry(app_id)
                .and_modify(|existing| {
                    if existing.min_app.is_none_or(|cur| host.min_version > cur) {
                        existing.min_app = Some(host.min_version);
                    }
                })
                .or_insert(Compat { min_app: Some(host.min_version), interface: None });
        }
        Ok(map)
    }

    /// Audit the `requiredPermissions` block into canonical capability descriptors and risk flags.
    pub fn audit_permissions(&self) -> UxpPermissionAudit {
        let mut audit = UxpPermissionAudit::default();
        let perms = &self.required_permissions;

        if let Some(net) = &perms.network {
            match net {
                UxpNetworkPermission::All(s) if s.eq_ignore_ascii_case("all") => {
                    audit.unrestricted_network = true;
                    audit.capabilities.push("network:all".to_string());
                }
                UxpNetworkPermission::All(_) => {}
                UxpNetworkPermission::Enabled(true) => {
                    audit.unrestricted_network = true;
                    audit.capabilities.push("network:all".to_string());
                }
                UxpNetworkPermission::Enabled(false) => {}
                UxpNetworkPermission::Domains { domains } if !domains.is_empty() => {
                    if domains.iter().any(|d| d.contains('*')) {
                        audit.unrestricted_network = true;
                    }
                    audit.capabilities.push(format!("network:domains({})", domains.len()));
                }
                UxpNetworkPermission::Domains { .. } => {}
            }
        }

        if let Some(fs_mode) = perms.local_file_system {
            let label = match fs_mode {
                UxpFileSystemAccess::Plugin => "localFileSystem:plugin",
                UxpFileSystemAccess::Temp => "localFileSystem:temp",
                UxpFileSystemAccess::Request => "localFileSystem:request",
                UxpFileSystemAccess::FullAccess => {
                    audit.full_filesystem_access = true;
                    "localFileSystem:fullAccess"
                }
            };
            audit.capabilities.push(label.to_string());
        }

        if let Some(clip) = perms.clipboard {
            match clip {
                UxpClipboardAccess::Read => audit.capabilities.push("clipboard:read".to_string()),
                UxpClipboardAccess::ReadAndWrite => {
                    audit.clipboard_write = true;
                    audit.capabilities.push("clipboard:readAndWrite".to_string());
                }
            }
        }

        if let Some(wv) = &perms.webview
            && wv.allow
        {
            audit.uses_webview = true;
            audit.capabilities.push("webview".to_string());
        }

        if let Some(ipc) = &perms.ipc
            && ipc.enable_plugin_communication
        {
            audit.uses_ipc = true;
            audit.capabilities.push("ipc".to_string());
        }

        audit
    }

    /// Translate into [`UxpNativeExtensionMetadata`] for Craft host runtime registration.
    pub fn to_native_metadata(&self) -> Result<UxpNativeExtensionMetadata, UxpError> {
        Ok(UxpNativeExtensionMetadata {
            id: self.craft_extension_id().map_err(UxpError::InvalidId)?,
            name: self.name.clone(),
            version: self.version,
            main: self.main.clone(),
            compat: self.routed_compat()?,
            entrypoints: self.entrypoints.clone(),
            permission_audit: self.audit_permissions(),
        })
    }

    /// Convert a validated [`UxpManifest`] and hashed package file list into a native Craft [`Manifest`].
    pub fn to_craft_manifest(&self, publisher_name: &str, files: Vec<FileEntry>) -> Result<Manifest, UxpError> {
        let native = self.to_native_metadata()?;
        let manifest = Manifest {
            schema: crate::manifest::SCHEMA,
            id: native.id,
            name: native.name,
            version: native.version,
            kind: Kind::Resource,
            publisher: Publisher { name: publisher_name.to_string() },
            description: format!("UXP v{} bridge ({} entrypoint(s))", self.manifest_version.as_u32(), self.entrypoints.len()),
            compat: native.compat,
            files,
            permissions: Vec::new(),
            provenance: None,
        };
        let encoded = serde_json::to_vec(&manifest).map_err(|_| UxpError::InvalidJson)?;
        Manifest::parse(&encoded).map_err(UxpError::CraftManifest)
    }

    fn validate(&self) -> Result<(), UxpError> {
        self.craft_extension_id().map_err(UxpError::InvalidId)?;
        validate_text(&self.name, 64, Lines::One).map_err(UxpError::InvalidName)?;
        validate_path(&self.main).map_err(UxpError::InvalidMainPath)?;

        if self.hosts.is_empty() {
            return Err(UxpError::NoHosts);
        }
        if self.hosts.len() > MAX_UXP_HOSTS {
            return Err(UxpError::TooManyHosts);
        }
        for host in &self.hosts {
            if host.craft_app_id().is_none() {
                return Err(UxpError::UnsupportedHostApp(host.app.clone()));
            }
            if let Some(max_v) = host.max_version
                && max_v < host.min_version
            {
                return Err(UxpError::InvalidHostVersionRange { app: host.app.clone() });
            }
        }

        if self.entrypoints.is_empty() {
            return Err(UxpError::NoEntrypoints);
        }
        if self.entrypoints.len() > MAX_UXP_ENTRYPOINTS {
            return Err(UxpError::TooManyEntrypoints);
        }
        let mut seen_ep_ids = BTreeSet::new();
        for ep in &self.entrypoints {
            if !is_valid_entrypoint_id(&ep.id) {
                return Err(UxpError::InvalidEntrypointId(ep.id.clone()));
            }
            if !seen_ep_ids.insert(ep.id.as_str()) {
                return Err(UxpError::DuplicateEntrypointId(ep.id.clone()));
            }
            validate_text(&ep.label, 64, Lines::One).map_err(UxpError::InvalidEntrypointLabel)?;
        }

        self.validate_permissions()?;
        Ok(())
    }

    fn validate_permissions(&self) -> Result<(), UxpError> {
        if let Some(net) = &self.required_permissions.network {
            match net {
                UxpNetworkPermission::All(mode) => {
                    if !mode.eq_ignore_ascii_case("all") && !mode.eq_ignore_ascii_case("none") {
                        return Err(UxpError::InvalidPermissionDomain(mode.clone()));
                    }
                }
                UxpNetworkPermission::Enabled(_) => {}
                UxpNetworkPermission::Domains { domains } => {
                    if domains.len() > MAX_UXP_DOMAINS {
                        return Err(UxpError::InvalidPermissionDomain("too many domains".to_string()));
                    }
                    for d in domains {
                        if !is_valid_uxp_domain(d) {
                            return Err(UxpError::InvalidPermissionDomain(d.clone()));
                        }
                    }
                }
            }
        }

        if let Some(wv) = &self.required_permissions.webview {
            if wv.domains.len() > MAX_UXP_DOMAINS {
                return Err(UxpError::InvalidPermissionDomain("too many webview domains".to_string()));
            }
            for d in &wv.domains {
                if !is_valid_uxp_domain(d) {
                    return Err(UxpError::InvalidPermissionDomain(d.clone()));
                }
            }
        }

        Ok(())
    }
}

fn validate_raw_uxp_id(raw: &str) -> Result<(), IdError> {
    if raw.is_empty() {
        return Err(IdError::Empty);
    }
    if raw.len() > 60 {
        return Err(IdError::TooLong);
    }
    if !raw.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')) {
        return Err(IdError::BadCharacter);
    }
    for segment in raw.split('.') {
        let edge_ok = |c: Option<char>| matches!(c, Some(ch) if ch.is_ascii_alphanumeric());
        if !edge_ok(segment.chars().next()) || !edge_ok(segment.chars().next_back()) {
            return Err(IdError::BadShape);
        }
    }
    Ok(())
}

fn is_valid_entrypoint_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_UXP_ENTRYPOINT_ID
        && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        && id.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
}

fn is_valid_uxp_domain(domain: &str) -> bool {
    if domain.is_empty() || domain.len() > 200 || !domain.bytes().all(|b| b.is_ascii_graphic()) {
        return false;
    }
    let rest = domain.strip_prefix("https://").or_else(|| domain.strip_prefix("wss://")).unwrap_or(domain);
    let host = rest.split('/').next().unwrap_or("");
    !host.is_empty() && !host.contains('@')
}

/// Parse UXP versions (`"24.2.0"` or 2-segment host versions `"24.0"` -> `24.0.0`).
fn parse_uxp_version_str(text: &str) -> Result<Version, VersionError> {
    if let Ok(v) = Version::parse(text) {
        return Ok(v);
    }
    // Adobe UXP host blocks frequently specify 2-segment versions such as `"24.0"`.
    if text.split('.').count() == 2 {
        return Version::parse(&format!("{text}.0"));
    }
    Err(VersionError)
}

fn deserialize_uxp_version<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Version, D::Error> {
    let s = String::deserialize(d)?;
    parse_uxp_version_str(&s).map_err(serde::de::Error::custom)
}

fn deserialize_opt_uxp_version<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Version>, D::Error> {
    let opt = Option::<String>::deserialize(d)?;
    opt.map(|s| parse_uxp_version_str(&s).map_err(serde::de::Error::custom)).transpose()
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawLabel {
    Plain(String),
    Localized(BTreeMap<String, String>),
}

fn deserialize_uxp_label<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    match RawLabel::deserialize(d)? {
        RawLabel::Plain(s) => Ok(s),
        RawLabel::Localized(map) => map
            .get("default")
            .or_else(|| map.get("en"))
            .or_else(|| map.values().next())
            .cloned()
            .ok_or_else(|| serde::de::Error::custom("localized label object is empty")),
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum YesOrBool {
    Flag(bool),
    Word(String),
}

fn deserialize_yes_or_bool<'de, D: serde::Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    match YesOrBool::deserialize(d)? {
        YesOrBool::Flag(b) => Ok(b),
        YesOrBool::Word(s) => match s.to_ascii_lowercase().as_str() {
            "yes" | "true" => Ok(true),
            "no" | "false" => Ok(false),
            _ => Err(serde::de::Error::custom("expected 'yes', 'no', or boolean")),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_every_creative_cloud_sap_code_to_exact_craft_app_id() {
        let expected = [
            ("PHSP", "photocraft"),
            ("PS", "photocraft"),
            ("ILST", "vectorcraft"),
            ("AI", "vectorcraft"),
            ("IDSN", "designcraft"),
            ("ID", "designcraft"),
            ("AICY", "designcraft"),
            ("PPRO", "filmcraft"),
            ("AEFT", "effectcraft"),
            ("AUDT", "soundcraft"),
            ("FLPR", "anicraft"),
            ("XD", "figcraft"),
            ("LRCC", "lightcraft"),
            ("LTRM", "lightcraft"),
            ("KBRG", "lightcraft"),
        ];
        for (sap, app_slug) in expected {
            assert_eq!(map_sap_code_to_slug(sap), Some(app_slug), "{sap}");
            assert_eq!(map_sap_code_to_app_id(sap).unwrap().as_str(), app_slug, "{sap}");
        }
        assert!(map_sap_code_to_app_id("UNKNOWN").is_none());
    }

    #[test]
    fn parses_uxp_v5_manifest_single_host_permissions_and_converts_to_craft_manifest() {
        let raw = json!({
            "manifestVersion": 5,
            "id": "com.example.RetouchPro",
            "name": "Retouch Pro Panel",
            "version": "2.4.0",
            "main": "dist/index.html",
            "host": {
                "app": "PHSP",
                "minVersion": "24.2",
                "maxVersion": "26.9.0"
            },
            "entrypoints": [
                {
                    "type": "panel",
                    "id": "retouch.mainPanel",
                    "label": { "default": "Retouch Pro", "ja": "リタッチプロ" }
                },
                {
                    "type": "command",
                    "id": "retouch.quickHeal",
                    "label": "Quick Frequency Separation"
                }
            ],
            "requiredPermissions": {
                "network": { "domains": ["https://api.example.com"] },
                "localFileSystem": "request",
                "clipboard": "readAndWrite",
                "webview": { "allow": "yes", "domains": ["https://ui.example.com"] },
                "ipc": { "enablePluginCommunication": true }
            }
        })
        .to_string();

        let uxp = UxpManifest::from_json_str(&raw).unwrap();
        assert_eq!(uxp.manifest_version, UxpManifestVersion::V5);
        assert_eq!(uxp.craft_extension_id().unwrap().as_str(), "com.example.retouchpro");
        assert_eq!(uxp.hosts.len(), 1);
        assert_eq!(uxp.hosts[0].min_version, Version::new(24, 2, 0));
        assert_eq!(uxp.entrypoints.len(), 2);
        assert_eq!(uxp.entrypoints[0].label, "Retouch Pro");

        let audit = uxp.audit_permissions();
        assert!(!audit.unrestricted_network);
        assert!(!audit.full_filesystem_access);
        assert!(audit.clipboard_write);
        assert!(audit.uses_webview);
        assert!(audit.uses_ipc);
        assert!(audit.requires_elevated_review());

        let files = vec![FileEntry { path: "dist/index.html".to_string(), sha256: "a".repeat(64), size: 128 }];
        let craft = uxp.to_craft_manifest("Example Studio", files).unwrap();
        assert_eq!(craft.id.as_str(), "com.example.retouchpro");
        assert_eq!(craft.compat.get(&AppId::new("photocraft").unwrap()).unwrap().min_app, Some(Version::new(24, 2, 0)));
    }

    #[test]
    fn parses_uxp_v4_multi_host_array_and_merges_duplicate_target_apps() {
        let raw = json!({
            "manifestVersion": 4,
            "id": "b7e81234",
            "name": "Suite Asset Sync",
            "version": "1.0.3",
            "main": "main.js",
            "host": [
                { "app": "IDSN", "minVersion": "18.0.0" },
                { "app": "AICY", "minVersion": "18.5.0" },
                { "app": "ILST", "minVersion": "27.1.0" },
                { "app": "LRCC", "minVersion": "6.0" },
                { "app": "LTRM", "minVersion": "12.2" }
            ],
            "entrypoints": [
                { "type": "command", "id": "syncNow", "label": "Sync Assets" }
            ]
        })
        .to_string();

        let uxp = UxpManifest::from_json_str(&raw).unwrap();
        assert_eq!(uxp.manifest_version, UxpManifestVersion::V4);
        assert_eq!(uxp.craft_extension_id().unwrap().as_str(), "uxp.b7e81234");
        assert!(uxp.audit_permissions().is_sandboxed_zero_permission());

        let compat = uxp.routed_compat().unwrap();
        assert_eq!(compat.len(), 3);
        assert_eq!(compat[&AppId::new("designcraft").unwrap()].min_app, Some(Version::new(18, 5, 0)));
        assert_eq!(compat[&AppId::new("vectorcraft").unwrap()].min_app, Some(Version::new(27, 1, 0)));
        assert_eq!(compat[&AppId::new("lightcraft").unwrap()].min_app, Some(Version::new(12, 2, 0)));
    }

    #[test]
    fn rejects_unsupported_versions_hostile_paths_unknown_hosts_and_bad_permissions() {
        // Unsupported manifestVersion 3 and 6
        let v3 = json!({
            "manifestVersion": 3, "id": "com.a.b", "name": "X", "version": "1.0.0",
            "main": "index.js", "host": {"app": "PS", "minVersion": "24.0"},
            "entrypoints": [{"type": "command", "id": "c", "label": "C"}]
        })
        .to_string();
        assert_eq!(UxpManifest::from_json_str(&v3), Err(UxpError::UnsupportedManifestVersion(3)));

        // Path traversal in `main`
        let bad_main = json!({
            "manifestVersion": 5, "id": "com.a.b", "name": "X", "version": "1.0.0",
            "main": "../escape.js", "host": {"app": "PS", "minVersion": "24.0"},
            "entrypoints": [{"type": "command", "id": "c", "label": "C"}]
        })
        .to_string();
        assert!(matches!(UxpManifest::from_json_str(&bad_main), Err(UxpError::InvalidMainPath(_))));

        // Unknown host SAPCode
        let bad_host = json!({
            "manifestVersion": 5, "id": "com.a.b", "name": "X", "version": "1.0.0",
            "main": "index.js", "host": {"app": "FAKEAPP", "minVersion": "1.0"},
            "entrypoints": [{"type": "command", "id": "c", "label": "C"}]
        })
        .to_string();
        assert_eq!(UxpManifest::from_json_str(&bad_host), Err(UxpError::UnsupportedHostApp("FAKEAPP".to_string())));

        // Inverted host minVersion > maxVersion
        let inverted = json!({
            "manifestVersion": 5, "id": "com.a.b", "name": "X", "version": "1.0.0",
            "main": "index.js", "host": {"app": "PPRO", "minVersion": "25.0", "maxVersion": "24.0"},
            "entrypoints": [{"type": "command", "id": "c", "label": "C"}]
        })
        .to_string();
        assert_eq!(UxpManifest::from_json_str(&inverted), Err(UxpError::InvalidHostVersionRange { app: "PPRO".to_string() }));

        // Duplicate entrypoint IDs
        let dup_ep = json!({
            "manifestVersion": 5, "id": "com.a.b", "name": "X", "version": "1.0.0",
            "main": "index.js", "host": {"app": "AEFT", "minVersion": "24.0"},
            "entrypoints": [
                {"type": "panel", "id": "same", "label": "One"},
                {"type": "command", "id": "same", "label": "Two"}
            ]
        })
        .to_string();
        assert_eq!(UxpManifest::from_json_str(&dup_ep), Err(UxpError::DuplicateEntrypointId("same".to_string())));

        // Oversized input
        let huge = " ".repeat(MAX_UXP_MANIFEST_BYTES + 1);
        assert_eq!(UxpManifest::from_json_str(&huge), Err(UxpError::TooLarge));
    }
}
