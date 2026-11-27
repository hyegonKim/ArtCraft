//! ZIP + DCX/UCF manifest reader for Adobe XD (`.xd`) archives.

use std::collections::BTreeMap;
use std::io::{self, Cursor, Read};

use serde_json::Value;
use zip::ZipArchive;

use crate::FormatError;

pub(super) const XD_MIME: &str = "application/vnd.adobe.sparkler.project+dcxucf";
pub(super) const MAX_ARCHIVE_BYTES: usize = 512 * 1024 * 1024;
pub(super) const MAX_DECOMPRESSED: u64 = 1024 * 1024 * 1024;
pub(super) const MAX_JSON_BYTES: u64 = 32 * 1024 * 1024;
pub(super) const MAX_JSON_VALUES: usize = 4_000_000;
pub(super) const MAX_ASSET_BYTES: u64 = 256 * 1024 * 1024;
pub(super) const MAX_ENTRIES: usize = 50_000;
pub(super) const MAX_ARTBOARDS: usize = 2_000;

pub(super) fn corrupt(msg: impl Into<String>) -> FormatError {
    FormatError::Corrupt(format!("Adobe XD: {}", msg.into()))
}

/// Fast check whether `bytes` is an Adobe XD `.xd` archive.
pub fn is_xd(bytes: &[u8]) -> bool {
    if bytes.len() < 4 || &bytes[..2] != b"PK" {
        return false;
    }
    let Ok(mut z) = ZipArchive::new(Cursor::new(bytes)) else { return false };
    if z.len() > MAX_ENTRIES {
        return false;
    }
    if let Ok(mut f) = z.by_name("mimetype") {
        let mut buf = [0_u8; 96];
        if let Ok(n) = f.read(&mut buf)
            && let Ok(s) = std::str::from_utf8(&buf[..n])
            && s.trim().starts_with(XD_MIME)
        {
            return true;
        }
    }
    if let Ok(f) = z.by_name("manifest")
        && f.size() <= MAX_JSON_BYTES
    {
        let mut head = Vec::with_capacity(4096);
        if f.take(4096).read_to_end(&mut head).is_ok()
            && let Ok(s) = std::str::from_utf8(&head)
            && (s.contains(XD_MIME) || s.contains("graphicContent.agc"))
        {
            return true;
        }
    }
    false
}

/// Decompression and JSON structure budget across the whole `.xd` archive.
pub(super) struct Budget {
    remaining: u64,
    exceeded: bool,
    json_values: usize,
    json_exceeded: bool,
}

impl Budget {
    pub(super) fn new() -> Self {
        Self { remaining: MAX_DECOMPRESSED, exceeded: false, json_values: MAX_JSON_VALUES, json_exceeded: false }
    }

    fn check(&self) -> Result<(), FormatError> {
        if self.exceeded {
            Err(corrupt("decompressed archive contents exceed 1 GiB"))
        } else if self.json_exceeded {
            Err(corrupt("JSON structure exceeds import value budget"))
        } else {
            Ok(())
        }
    }

    pub(super) fn read_bytes(&mut self, z: &mut ZipArchive<Cursor<&[u8]>>, path: &str, limit: u64) -> Result<Vec<u8>, FormatError> {
        self.check()?;
        if !is_safe_archive_path(path) {
            return Err(corrupt(format!("unsafe archive member path `{path}`")));
        }
        let entry = z.by_name(path).map_err(|e| corrupt(format!("{path}: {e}")))?;
        if entry.size() > limit {
            return Err(corrupt(format!("{path} exceeds member size limit")));
        }
        let mut out = Vec::new();
        let mut reader = BudgetReader { inner: entry, budget: self };
        reader.by_ref().take(limit.saturating_add(1)).read_to_end(&mut out).map_err(|e| corrupt(format!("{path}: {e}")))?;
        if out.len() as u64 > limit {
            return Err(corrupt(format!("{path} exceeds member size limit")));
        }
        self.check()?;
        Ok(out)
    }

    pub(super) fn read_json(&mut self, z: &mut ZipArchive<Cursor<&[u8]>>, path: &str) -> Result<Value, FormatError> {
        let data = self.read_bytes(z, path, MAX_JSON_BYTES)?;
        let mut count = 1usize;
        let (mut quoted, mut escaped) = (false, false);
        for &b in &data {
            if quoted {
                if escaped {
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == b'"' {
                    quoted = false;
                }
            } else if b == b'"' {
                quoted = true;
            } else if matches!(b, b'[' | b'{' | b',' | b':') {
                count += 1;
            }
        }
        if count > self.json_values {
            self.json_exceeded = true;
            return Err(corrupt("JSON structure exceeds import value budget"));
        }
        self.json_values -= count;
        serde_json::from_slice(&data).map_err(|e| corrupt(format!("{path}: {e}")))
    }
}

struct BudgetReader<'a, R> {
    inner: R,
    budget: &'a mut Budget,
}

impl<R: Read> Read for BudgetReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.budget.remaining == 0 {
            let mut probe = [0_u8; 1];
            return match self.inner.read(&mut probe)? {
                0 => Ok(0),
                _ => {
                    self.budget.exceeded = true;
                    Err(io::Error::other("decompression budget exhausted"))
                }
            };
        }
        let max = buf.len().min(usize::try_from(self.budget.remaining).unwrap_or(usize::MAX));
        let n = self.inner.read(&mut buf[..max])?;
        self.budget.remaining -= n as u64;
        Ok(n)
    }
}

pub(super) fn is_safe_archive_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 512
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains('\0')
        && path.split('/').all(|seg| !seg.is_empty() && seg != "." && seg != "..")
}

#[derive(Clone, Debug)]
pub(super) struct ArtboardEntry {
    pub id: String,
    pub name: String,
    pub bounds: Option<[f64; 4]>,
    pub viewport_height: Option<f64>,
    pub is_home: bool,
    pub agc: Value,
}

#[derive(Clone, Debug)]
pub(super) struct AssetEntry {
    pub path: String,
    pub mime: String,
}

pub(super) struct ParsedArchive {
    pub title: String,
    pub home_artboard: Option<String>,
    pub artboards: Vec<ArtboardEntry>,
    pub pasteboard_agc: Option<Value>,
    pub resources_agc: Option<Value>,
    pub assets: BTreeMap<String, AssetEntry>,
}

fn num_field(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64).filter(|n| n.is_finite())
}

fn parse_bounds(v: &Value) -> Option<[f64; 4]> {
    let b = v.get("uxdesign#bounds")?;
    let x = num_field(b, "x").unwrap_or(0.0).clamp(-1_000_000.0, 1_000_000.0);
    let y = num_field(b, "y").unwrap_or(0.0).clamp(-1_000_000.0, 1_000_000.0);
    let w = num_field(b, "width")?.clamp(1.0, 1_000_000.0);
    let h = num_field(b, "height")?.clamp(1.0, 1_000_000.0);
    Some([x, y, w, h])
}

fn parse_viewport_height(v: &Value) -> Option<f64> {
    let vp = v.get("uxdesign#viewport")?;
    let h = num_field(vp, "height")?;
    (h > 0.0).then(|| h.clamp(1.0, 1_000_000.0))
}

fn join_path(prefix: &str, seg: &str) -> String {
    let seg = seg.trim_matches('/');
    if seg.is_empty() {
        prefix.trim_matches('/').to_string()
    } else if prefix.is_empty() {
        seg.to_string()
    } else {
        format!("{}/{seg}", prefix.trim_matches('/'))
    }
}

/// Locate the `.agc` path inside an artboard/pasteboard/resources manifest node.
fn find_agc_path(node: &Value, node_dir: &str, z: &mut ZipArchive<Cursor<&[u8]>>) -> Option<String> {
    if let Some(comps) = node.get("components").and_then(Value::as_array) {
        for c in comps {
            if let Some(p) = c.get("path").and_then(Value::as_str) {
                let full = join_path(node_dir, p);
                if p.ends_with(".agc") && z.by_name(&full).is_ok() {
                    return Some(full);
                }
                if p.ends_with(".agc") && z.by_name(p).is_ok() {
                    return Some(p.to_string());
                }
            }
        }
    }
    if let Some(children) = node.get("children").and_then(Value::as_array) {
        for ch in children {
            let sub = ch.get("path").and_then(Value::as_str).map_or_else(|| node_dir.to_string(), |p| join_path(node_dir, p));
            if let Some(found) = find_agc_path(ch, &sub, z) {
                return Some(found);
            }
        }
    }
    let default_agc = join_path(node_dir, "graphics/graphicContent.agc");
    if z.by_name(&default_agc).is_ok() {
        return Some(default_agc);
    }
    let direct_agc = join_path(node_dir, "graphicContent.agc");
    if z.by_name(&direct_agc).is_ok() {
        return Some(direct_agc);
    }
    None
}

fn collect_resource_assets(node: &Value, prefix: &str, out: &mut BTreeMap<String, AssetEntry>) {
    let dir = node
        .get("path")
        .and_then(Value::as_str)
        .map_or_else(|| prefix.to_string(), |p| if p == "." { prefix.to_string() } else { join_path(prefix, p) });
    if let Some(comps) = node.get("components").and_then(Value::as_array) {
        for c in comps.iter().take(MAX_ENTRIES) {
            let Some(rel_path) = c.get("path").and_then(Value::as_str) else { continue };
            if rel_path.ends_with(".agc") {
                continue;
            }
            let full = join_path(&dir, rel_path);
            if !is_safe_archive_path(&full) {
                continue;
            }
            let mime = c.get("type").and_then(Value::as_str).unwrap_or("").to_string();
            let uid = rel_path.rsplit('/').next().unwrap_or(rel_path).to_string();
            if !uid.is_empty() {
                out.insert(uid.clone(), AssetEntry { path: full.clone(), mime: mime.clone() });
            }
            if let Some(id) = c.get("id").and_then(Value::as_str).filter(|s| !s.is_empty()) {
                out.entry(id.to_string()).or_insert_with(|| AssetEntry { path: full.clone(), mime: mime.clone() });
            }
        }
    }
    if let Some(children) = node.get("children").and_then(Value::as_array) {
        for ch in children.iter().take(256) {
            collect_resource_assets(ch, &dir, out);
        }
    }
}

pub(super) fn read_archive(
    bytes: &[u8],
    fallback_name: &str,
    z: &mut ZipArchive<Cursor<&[u8]>>,
    budget: &mut Budget,
    warnings: &mut Vec<String>,
) -> Result<ParsedArchive, FormatError> {
    if bytes.len() > MAX_ARCHIVE_BYTES {
        return Err(corrupt("archive exceeds 512 MiB size limit"));
    }
    if z.len() > MAX_ENTRIES {
        return Err(corrupt("archive has too many entries"));
    }
    if z.by_name("mimetype").is_ok() {
        let mime_bytes = budget.read_bytes(z, "mimetype", 1024)?;
        let mime = std::str::from_utf8(&mime_bytes).unwrap_or("").trim();
        if !mime.is_empty() && !mime.starts_with(XD_MIME) {
            return Err(corrupt(format!("unexpected mimetype `{mime}`")));
        }
    }

    let manifest = budget.read_json(z, "manifest")?;
    if let Some(ty) = manifest.get("type").and_then(Value::as_str)
        && ty != XD_MIME
    {
        return Err(corrupt(format!("unexpected manifest type `{ty}`")));
    }

    let title = manifest
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            let trimmed = fallback_name.trim_end_matches(".xd").trim_end_matches(".XD").trim();
            if trimmed.is_empty() { "Untitled" } else { trimmed }
        })
        .to_string();

    let home_artboard =
        manifest.get("uxdesign#homeArtboard").and_then(Value::as_str).map(str::to_string).or_else(|| {
            manifest.get("meta").and_then(|m| m.get("ux")).and_then(|ux| ux.get("homeArtboard")).and_then(Value::as_str).map(str::to_string)
        });

    let mut artboards = Vec::new();
    let mut pasteboard_agc = None;
    let mut resources_agc = None;
    let mut assets = BTreeMap::new();

    if let Some(children) = manifest.get("children").and_then(Value::as_array) {
        for section in children {
            let sec_name = section.get("name").and_then(Value::as_str).unwrap_or("");
            let sec_path = section.get("path").and_then(Value::as_str).unwrap_or(sec_name);
            if sec_name == "artwork" || sec_path == "artwork" {
                if let Some(items) = section.get("children").and_then(Value::as_array) {
                    for item in items {
                        let item_name = item.get("name").and_then(Value::as_str).unwrap_or("Artboard");
                        let item_path = item.get("path").and_then(Value::as_str).unwrap_or("");
                        let item_dir = if item_path.starts_with("artwork/") { item_path.to_string() } else { join_path(sec_path, item_path) };
                        if item_name == "pasteboard" || item_path == "pasteboard" {
                            if let Some(agc_path) = find_agc_path(item, &item_dir, z) {
                                match budget.read_json(z, &agc_path) {
                                    Ok(v) => pasteboard_agc = Some(v),
                                    Err(e) => warnings.push(format!("Pasteboard AGC skipped ({e})")),
                                }
                            }
                            continue;
                        }
                        if artboards.len() >= MAX_ARTBOARDS {
                            warnings.push(format!("Artboards past {MAX_ARTBOARDS} were skipped"));
                            break;
                        }
                        let Some(agc_path) = find_agc_path(item, &item_dir, z) else {
                            warnings.push(format!("Artboard `{item_name}` has no graphicContent.agc and was skipped"));
                            continue;
                        };
                        let agc = budget.read_json(z, &agc_path)?;
                        let id = item.get("id").and_then(Value::as_str).unwrap_or(item_path).to_string();
                        let is_home = item.get("uxdesign#isHomeArtboard").and_then(Value::as_bool).unwrap_or(false)
                            || home_artboard.as_deref() == Some(id.as_str());
                        artboards.push(ArtboardEntry {
                            id,
                            name: item_name.to_string(),
                            bounds: parse_bounds(item),
                            viewport_height: parse_viewport_height(item),
                            is_home,
                            agc,
                        });
                    }
                }
            } else if sec_name == "resources" || sec_path == "resources" {
                collect_resource_assets(section, "", &mut assets);
                if let Some(agc_path) = find_agc_path(section, sec_path, z) {
                    match budget.read_json(z, &agc_path) {
                        Ok(v) => resources_agc = Some(v),
                        Err(e) => warnings.push(format!("Resources AGC skipped ({e})")),
                    }
                }
            } else {
                collect_resource_assets(section, "", &mut assets);
            }
        }
    }

    // Fallback scan for `resources/<uid>` entries not listed in manifest components.
    let names: Vec<String> = (0..z.len()).filter_map(|i| z.by_index(i).ok().map(|f| f.name().to_string())).collect();
    for name in names {
        if let Some(uid) = name.strip_prefix("resources/")
            && !uid.is_empty()
            && !uid.contains('/')
            && !uid.ends_with(".agc")
            && is_safe_archive_path(&name)
        {
            assets.entry(uid.to_string()).or_insert_with(|| AssetEntry { path: name, mime: String::new() });
        }
    }

    if resources_agc.is_none() && z.by_name("resources/graphics/graphicContent.agc").is_ok() {
        if let Ok(v) = budget.read_json(z, "resources/graphics/graphicContent.agc") {
            resources_agc = Some(v);
        }
    }

    Ok(ParsedArchive { title, home_artboard, artboards, pasteboard_agc, resources_agc, assets })
}
