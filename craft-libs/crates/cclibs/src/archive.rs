use std::collections::BTreeMap;

use craft_zip::{Archive, Compression, Writer};
use serde::{Deserialize, Serialize};

use crate::{AseColorGroup, AseItem, AsePalette, AseSwatch, CcElement, CcElementType, Error, Limits, Result};

/// Canonical DCX container MIME type written to the leading `mimetype` entry and `manifest.json`.
pub const CCLIBS_CONTAINER_MIME: &str = "application/vnd.adobe.library+dcx";
/// Leading stored entry name inside a `.cclibs` / `.cclib` DCX ZIP package.
pub const CCLIBS_MIMETYPE_ENTRY: &str = "mimetype";
/// Root DCX manifest entry name inside a `.cclibs` / `.cclib` ZIP package.
pub const CCLIBS_MANIFEST_NAME: &str = "manifest.json";
/// Creative Cloud Libraries (`LIBS`) schema baseline supported by this crate.
pub const LIBS_SCHEMA_VERSION: &str = "4.20.2";

/// Optional organizational group/folder inside a Creative Cloud Library.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CcLibraryGroup {
    /// Group identifier referenced by [`CcElement::group_id`].
    pub id: String,
    /// Display name of the group.
    pub name: String,
}

#[derive(Serialize, Deserialize)]
struct WireManifest {
    id: String,
    name: String,
    #[serde(rename = "type", default = "default_container_type")]
    container_type: String,
    #[serde(default = "default_libs_version")]
    libs_version: String,
    #[serde(default = "default_manifest_format_version")]
    manifest_format_version: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    groups: Vec<CcLibraryGroup>,
    #[serde(rename = "children", alias = "elements", default)]
    elements: Vec<CcElement>,
}

fn default_container_type() -> String {
    CCLIBS_CONTAINER_MIME.to_string()
}

fn default_libs_version() -> String {
    LIBS_SCHEMA_VERSION.to_string()
}

const fn default_manifest_format_version() -> u32 {
    5
}

/// Whether `path` is a safe relative asset path inside a `.cclibs` archive (no root slash,
/// backslashes, parent traversal `..`, control bytes, or reserved `manifest.json` / `mimetype` names).
pub fn valid_archive_asset_path(path: &str) -> bool {
    if path.is_empty()
        || path == CCLIBS_MANIFEST_NAME
        || path == CCLIBS_MIMETYPE_ENTRY
        || path.contains('\\')
        || path.starts_with('/')
        || path.ends_with('/')
        || path.bytes().any(|b| b < 0x20)
    {
        return false;
    }
    path.split('/').all(|seg| !seg.is_empty() && seg != "." && seg != "..")
}

/// Bounded signature probe checking whether `bytes` is a `.cclibs` / `.cclib` DCX ZIP archive.
pub fn is_cclibs_zip(bytes: &[u8]) -> bool {
    if !bytes.starts_with(b"PK\x03\x04") {
        return false;
    }
    if craft_zip::stored_mimetype(bytes) == Some(CCLIBS_CONTAINER_MIME.as_bytes()) {
        return true;
    }
    let limits = Limits::default();
    let Ok(mut archive) = Archive::open(bytes, craft_zip::Limits { max_entries: limits.max_zip_entries, max_total_bytes: limits.max_manifest_bytes })
    else {
        return false;
    };
    if archive.has_duplicate_names() || !archive.contains(CCLIBS_MANIFEST_NAME) {
        return false;
    }
    let Ok(Some(manifest_bytes)) = archive.read(CCLIBS_MANIFEST_NAME, limits.max_manifest_bytes) else {
        return false;
    };
    serde_json::from_slice::<WireManifest>(&manifest_bytes).is_ok_and(|m| m.container_type == CCLIBS_CONTAINER_MIME || !m.elements.is_empty())
}

/// In-memory representation of a `.cclibs` / `.cclib` Creative Cloud Library archive (`LIBS v4.20.2`).
#[derive(Clone, Debug, PartialEq)]
pub struct CcLibraryArchive {
    /// Library UUID or identifier.
    pub id: String,
    /// Display name of the library.
    pub name: String,
    /// DCX container MIME type (`"application/vnd.adobe.library+dcx"`).
    pub container_type: String,
    /// `LIBS` schema version (`"4.20.2"`).
    pub libs_version: String,
    /// DCX manifest format version (`5`).
    pub manifest_format_version: u32,
    /// Optional library groups/folders.
    pub groups: Vec<CcLibraryGroup>,
    /// Elements stored in this library across any of the 19 [`CcElementType`] variants.
    pub elements: Vec<CcElement>,
    /// Binary asset payloads keyed by their canonical relative ZIP path.
    pub assets: BTreeMap<String, Vec<u8>>,
}

impl CcLibraryArchive {
    /// Create a new empty Creative Cloud Library archive with `LIBS v4.20.2` defaults.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            container_type: default_container_type(),
            libs_version: default_libs_version(),
            manifest_format_version: default_manifest_format_version(),
            groups: Vec::new(),
            elements: Vec::new(),
            assets: BTreeMap::new(),
        }
    }

    /// Append a [`CcElement`] to the library.
    pub fn add_element(&mut self, element: CcElement) {
        self.elements.push(element);
    }

    /// Insert or replace a binary asset payload after validating its relative archive path.
    pub fn set_asset(&mut self, path: impl Into<String>, bytes: Vec<u8>) -> Result<()> {
        let path = path.into();
        if !valid_archive_asset_path(&path) {
            return Err(Error::InvalidAssetPath(path));
        }
        self.assets.insert(path, bytes);
        Ok(())
    }

    /// Look up a binary asset payload by its relative archive path.
    pub fn asset(&self, path: &str) -> Option<&[u8]> {
        self.assets.get(path).map(Vec::as_slice)
    }

    /// Iterate over all elements matching `element_type`.
    pub fn elements_by_type(&self, element_type: CcElementType) -> impl Iterator<Item = &CcElement> {
        self.elements.iter().filter(move |e| e.element_type == element_type)
    }

    /// Read a `.cclibs` / `.cclib` ZIP archive using [`Limits::default`].
    pub fn from_zip_bytes(bytes: &[u8]) -> Result<Self> {
        Self::from_zip_bytes_with_limits(bytes, Limits::default())
    }

    /// Read a `.cclibs` / `.cclib` ZIP archive enforcing explicit [`Limits`].
    pub fn from_zip_bytes_with_limits(bytes: &[u8], limits: Limits) -> Result<Self> {
        let mut archive =
            Archive::open(bytes, craft_zip::Limits { max_entries: limits.max_zip_entries, max_total_bytes: limits.max_total_zip_bytes })?;
        if archive.has_duplicate_names() {
            return Err(Error::InvalidArchive("duplicate entry names in ZIP archive".into()));
        }
        let manifest_bytes =
            archive.read(CCLIBS_MANIFEST_NAME, limits.max_manifest_bytes)?.ok_or_else(|| Error::InvalidArchive("missing manifest.json".into()))?;
        let wire: WireManifest = serde_json::from_slice(&manifest_bytes).map_err(|e| Error::InvalidArchive(format!("manifest.json: {e}")))?;
        if let Some(mime_bytes) = archive.read(CCLIBS_MIMETYPE_ENTRY, 256)? {
            let mime_str = std::str::from_utf8(&mime_bytes).map_err(|_| Error::InvalidArchive("mimetype entry is not valid UTF-8".into()))?;
            if mime_str.trim() != CCLIBS_CONTAINER_MIME {
                return Err(Error::InvalidArchive(format!("unexpected mimetype: {}", mime_str.trim())));
            }
        }
        let assets = read_archive_assets(&mut archive, limits.max_asset_bytes)?;
        validate_referenced_assets(&wire.elements, &assets)?;
        Ok(Self {
            id: wire.id,
            name: wire.name,
            container_type: wire.container_type,
            libs_version: wire.libs_version,
            manifest_format_version: wire.manifest_format_version,
            groups: wire.groups,
            elements: wire.elements,
            assets,
        })
    }

    /// Serialize this library into deterministic `.cclibs` DCX + ZIP bytes (`mimetype` stored first,
    /// `manifest.json` deflated, followed by canonical sorted asset entries).
    pub fn to_zip_bytes(&self) -> Result<Vec<u8>> {
        validate_referenced_assets(&self.elements, &self.assets)?;
        for path in self.assets.keys() {
            if !valid_archive_asset_path(path) {
                return Err(Error::InvalidAssetPath(path.clone()));
            }
        }
        let wire = WireManifest {
            id: self.id.clone(),
            name: self.name.clone(),
            container_type: self.container_type.clone(),
            libs_version: self.libs_version.clone(),
            manifest_format_version: self.manifest_format_version,
            groups: self.groups.clone(),
            elements: self.elements.clone(),
        };
        let manifest_json = serde_json::to_vec_pretty(&wire).map_err(|e| Error::InvalidArchive(format!("serialize manifest: {e}")))?;
        let mut writer = Writer::new();
        writer.add(CCLIBS_MIMETYPE_ENTRY, CCLIBS_CONTAINER_MIME.as_bytes(), Compression::Stored)?;
        writer.add(CCLIBS_MANIFEST_NAME, &manifest_json, Compression::Deflated)?;
        for (path, payload) in &self.assets {
            writer.add(path, payload, Compression::Deflated)?;
        }
        writer.finish().map_err(Into::into)
    }

    /// Export all `color+dcx` and `colortheme+dcx` elements in this library into an [`AsePalette`].
    pub fn to_ase_palette(&self) -> AsePalette {
        let mut palette = AsePalette::new();
        for elem in &self.elements {
            if let Some(cv) = elem.extract_color()
                && let Some((color, color_type)) = cv.to_ase()
            {
                palette.push_swatch(AseSwatch { name: elem.name.clone(), color, color_type });
            } else if let Some(theme) = elem.extract_color_theme() {
                let swatches: Vec<AseSwatch> = theme
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, cv)| {
                        cv.to_ase().map(|(color, color_type)| AseSwatch { name: format!("{} {}", elem.name, idx + 1), color, color_type })
                    })
                    .collect();
                if !swatches.is_empty() {
                    palette.push_group(AseColorGroup { name: elem.name.clone(), swatches });
                }
            }
        }
        palette
    }

    /// Construct a [`CcLibraryArchive`] from an [`AsePalette`], mapping standalone swatches to
    /// `color+dcx` elements and color groups to `colortheme+dcx` elements.
    pub fn from_ase_palette(id: impl Into<String>, name: impl Into<String>, palette: &AsePalette) -> Self {
        let mut lib = Self::new(id, name);
        for (idx, item) in palette.items.iter().enumerate() {
            let elem_id = format!("swatch-{}", idx + 1);
            match item {
                AseItem::Swatch(s) => {
                    lib.add_element(CcElement::from_color(elem_id, s.name.clone(), s.color, s.color_type));
                }
                AseItem::Group(g) => {
                    let pairs: Vec<_> = g.swatches.iter().map(|s| (s.color, s.color_type)).collect();
                    lib.add_element(CcElement::from_color_theme(elem_id, g.name.clone(), &pairs));
                }
            }
        }
        lib
    }
}

fn read_archive_assets(archive: &mut Archive<'_>, max_asset_bytes: u64) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut names = Vec::new();
    let mut index = 0usize;
    while let Some(name) = archive.entry_name(index)? {
        let is_dir = archive.entry_is_dir(index)?.unwrap_or(false);
        if !is_dir && name != CCLIBS_MANIFEST_NAME && name != CCLIBS_MIMETYPE_ENTRY {
            if !valid_archive_asset_path(&name) {
                return Err(Error::InvalidAssetPath(name));
            }
            names.push(name);
        }
        index = index.saturating_add(1);
    }
    let mut assets = BTreeMap::new();
    for name in names {
        if let Some(data) = archive.read(&name, max_asset_bytes)? {
            assets.insert(name, data);
        }
    }
    Ok(assets)
}

fn validate_referenced_assets(elements: &[CcElement], assets: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    for elem in elements {
        for rep in &elem.representations {
            if let Some(path) = &rep.path {
                if !valid_archive_asset_path(path) {
                    return Err(Error::InvalidAssetPath(path.clone()));
                }
                if !assets.contains_key(path) {
                    return Err(Error::InvalidArchive(format!(
                        "element {:?} representation {:?} references missing asset {:?}",
                        elem.id, rep.id, path
                    )));
                }
            }
        }
    }
    Ok(())
}
