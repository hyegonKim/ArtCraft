//! Object / Asset Library (`.indl` & `.dclib`): reusable page items stored with their stories,
//! styles, swatches, images, `.idms` interchange representation, object-type classification,
//! creation timestamp, keywords, and original spread/page coordinates.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{CommandSpec, always, bad, cmd, has_doc, has_selection, ok, str_param};
use crate::{EngineError, Result, Session};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LibraryItem {
    pub name: String,
    pub description: String,
    /// Native `.designcraft` snippet bytes (base64).
    pub snippet: String,
    /// Object classification (`text`, `image`, `shape`, `geometry`, `group`).
    pub object_type: String,
    /// Unix epoch seconds when added.
    pub created: i64,
    /// Searchable tags / keywords.
    pub keywords: Vec<String>,
    /// Optional `.idms` XML snippet representation (UTF-8 XML text) for InDesign interchange.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idms: Option<String>,
    pub original_x: f64,
    pub original_y: f64,
    pub original_spread: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Library {
    #[serde(skip)]
    pub path: Option<String>,
    pub items: Vec<LibraryItem>,
}

fn is_indl_path(path: &str) -> bool {
    std::path::Path::new(path).extension().and_then(std::ffi::OsStr::to_str).is_some_and(|e| e.eq_ignore_ascii_case("indl"))
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

fn build_catalog_xml(lib: &Library) -> String {
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<AssetLibrary DOMVersion=\"20.4\">\n");
    for (idx, it) in lib.items.iter().enumerate() {
        let _ = writeln!(
            s,
            "  <Asset Index=\"{idx}\" Name=\"{}\" ObjectType=\"{}\" Created=\"{}\" Keywords=\"{}\" Description=\"{}\" Src=\"items/item_{idx}.idms\" OriginalSpread=\"{}\" OriginalX=\"{:.2}\" OriginalY=\"{:.2}\"/>",
            xml_escape(&it.name),
            xml_escape(if it.object_type.is_empty() { "shape" } else { &it.object_type }),
            it.created,
            xml_escape(&it.keywords.join(", ")),
            xml_escape(&it.description),
            it.original_spread,
            it.original_x,
            it.original_y,
        );
    }
    s.push_str("</AssetLibrary>\n");
    s
}

pub(crate) fn encode_library_bytes(lib: &Library, as_indl: bool) -> Result<Vec<u8>> {
    if !as_indl {
        return serde_json::to_vec_pretty(lib).map_err(|e| EngineError::Other(e.to_string()));
    }
    let json_bytes = serde_json::to_vec_pretty(lib).map_err(|e| EngineError::Other(e.to_string()))?;
    let mut parts: Vec<(String, Vec<u8>)> =
        vec![("catalog.xml".into(), build_catalog_xml(lib).into_bytes()), ("library.json".into(), json_bytes.clone())];
    for (idx, it) in lib.items.iter().enumerate() {
        let idms_bytes = if let Some(xml) = &it.idms {
            xml.as_bytes().to_vec()
        } else if let Ok(doc) = super::file::from_bytes(&super::file::base64_decode(&it.snippet)) {
            designcraft_idml::export_snippet(&doc)
        } else {
            Vec::new()
        };
        if !idms_bytes.is_empty() {
            parts.push((format!("items/item_{idx}.idms"), idms_bytes));
        }
    }
    Ok(craft_idml::write_package(&parts).unwrap_or(json_bytes))
}

fn classify_doc(doc: &designcraft_doc::Document) -> (String, usize, f64, f64) {
    let items: Vec<&std::sync::Arc<designcraft_doc::Item>> = doc.spreads.first().map(|sp| sp.items.iter().collect()).unwrap_or_default();
    if items.is_empty() {
        return ("shape".into(), 0, 0.0, 0.0);
    }
    let bounds = items.iter().map(|it| it.bounds()).reduce(|a, b| a.union(b)).unwrap_or(designcraft_geom::Rect::ZERO);
    let obj_type = if items.len() > 1 || items[0].is_group() {
        "group"
    } else {
        match &items[0].content {
            designcraft_doc::Content::Text(_) => "text",
            designcraft_doc::Content::Graphic(_) => "image",
            _ => match items[0].shape {
                designcraft_doc::Shape::GraphicLine | designcraft_doc::Shape::Path => "geometry",
                _ => "shape",
            },
        }
    };
    (obj_type.into(), 0, bounds.x0, bounds.y0)
}

fn item_from_idms_bytes(name_hint: &str, idms_bytes: &[u8]) -> Result<LibraryItem> {
    let doc = designcraft_idml::import_snippet(idms_bytes).map_err(|e| EngineError::Other(format!("invalid .idms asset: {e}")))?;
    let snip = super::file::to_bytes(&doc)?;
    let (object_type, original_spread, original_x, original_y) = classify_doc(&doc);
    Ok(LibraryItem {
        name: name_hint.to_string(),
        description: String::new(),
        snippet: super::file::base64_encode(&snip),
        object_type,
        created: unix_now(),
        keywords: Vec::new(),
        idms: Some(String::from_utf8_lossy(idms_bytes).into_owned()),
        original_x,
        original_y,
        original_spread,
    })
}

fn decode_indl_zip(bytes: &[u8]) -> std::result::Result<Library, String> {
    use std::io::Read as _;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("invalid .indl archive: {e}"))?;
    if let Ok(mut jf) = archive.by_name("library.json") {
        let mut buf = Vec::new();
        jf.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        if let Ok(lib) = serde_json::from_slice::<Library>(&buf) {
            return Ok(lib);
        }
    }
    let mut idms_entries: Vec<(String, Vec<u8>)> = Vec::new();
    let mut catalog_xml: Option<String> = None;
    for idx in 0..archive.len() {
        let mut entry = archive.by_index(idx).map_err(|e| e.to_string())?;
        let name = entry.name().to_string();
        if name == "catalog.xml" {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
            catalog_xml = Some(String::from_utf8_lossy(&buf).into_owned());
        } else if name.ends_with(".idms") {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
            idms_entries.push((name, buf));
        }
    }
    let mut items = Vec::new();
    for (entry_name, idms_bytes) in idms_entries {
        let stem = std::path::Path::new(&entry_name).file_stem().and_then(std::ffi::OsStr::to_str).unwrap_or("Asset");
        if let Ok(it) = item_from_idms_bytes(stem, &idms_bytes) {
            items.push(it);
        }
    }
    if let Some(xml) = catalog_xml
        && let Ok(root) = craft_xml::Options::default().parse(xml.as_bytes())
    {
        for (idx, node) in root
            .children
            .iter()
            .filter_map(|n| match n {
                craft_xml::Node::El(e) if e.name == "Asset" => Some(e),
                _ => None,
            })
            .enumerate()
        {
            if let Some(it) = items.get_mut(idx) {
                for (k, v) in &node.attrs {
                    match k.as_str() {
                        "Name" if !v.is_empty() => it.name.clone_from(v),
                        "Description" => it.description.clone_from(v),
                        "ObjectType" if !v.is_empty() => it.object_type.clone_from(v),
                        "Created" => {
                            if let Ok(ts) = v.parse::<i64>() {
                                it.created = ts;
                            }
                        }
                        "Keywords" if !v.is_empty() => {
                            it.keywords = v.split(',').map(str::trim).filter(|s| !s.is_empty()).map(ToString::to_string).collect();
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    Ok(Library { path: None, items })
}

/// Scans binary Adobe InDesign `.indl` library streams for embedded `<Document ... SnippetType=` XML slices.
fn scan_binary_indl(bytes: &[u8]) -> Option<Library> {
    const INDL_MAGIC: [u8; 8] = [0x06, 0x06, 0xED, 0xF5, 0xD8, 0x1D, 0x46, 0xE5];
    let has_magic = bytes.starts_with(&INDL_MAGIC);
    let text = String::from_utf8_lossy(bytes);
    let mut items = Vec::new();
    let mut search_from = 0usize;
    while let Some(rel_start) = text[search_from..].find("<Document") {
        let start = search_from + rel_start;
        let Some(rel_end) = text[start..].find("</Document>") else { break };
        let end = start + rel_end + "</Document>".len();
        let slice = &text[start..end];
        let name = format!("Asset {}", items.len() + 1);
        if let Ok(it) = item_from_idms_bytes(&name, slice.as_bytes()) {
            items.push(it);
        }
        search_from = end;
    }
    if !items.is_empty() || has_magic { Some(Library { path: None, items }) } else { None }
}

pub(crate) fn decode_library_bytes(bytes: &[u8]) -> std::result::Result<Library, String> {
    if bytes.starts_with(b"PK\x03\x04") {
        return decode_indl_zip(bytes);
    }
    if let Ok(lib) = serde_json::from_slice::<Library>(bytes) {
        return Ok(lib);
    }
    if designcraft_idml::is_idms(bytes) {
        let it = item_from_idms_bytes("Snippet Asset", bytes).map_err(|e| e.to_string())?;
        return Ok(Library { path: None, items: vec![it] });
    }
    scan_binary_indl(bytes).ok_or_else(|| "not a valid .indl or .dclib library".into())
}

fn unix_now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

impl Library {
    fn save(&self) -> Result<()> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(p) = &self.path {
            let bytes = encode_library_bytes(self, is_indl_path(p))?;
            crate::io::write_file(p, &bytes).map_err(EngineError::Other)?;
        }
        Ok(())
    }
}

fn has_library(s: &Session) -> std::result::Result<(), String> {
    if s.library.is_some() { Ok(()) } else { Err("no library open".into()) }
}

fn no_library() -> EngineError {
    bad("library", "no library open")
}

fn parse_keywords(p: &Value) -> Option<Vec<String>> {
    match p.get("keywords") {
        Some(Value::Array(arr)) => {
            Some(arr.iter().filter_map(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(ToString::to_string).collect())
        }
        Some(Value::String(s)) => Some(s.split(',').map(str::trim).filter(|k| !k.is_empty()).map(ToString::to_string).collect()),
        _ => None,
    }
}

fn build_library_item_from_selection(s: &mut Session, p: &Value) -> Result<LibraryItem> {
    let clip = crate::cmd::edit::clip_doc(s)?;
    let snip = super::file::to_bytes(&clip)?;
    let idms_xml = String::from_utf8(designcraft_idml::export_snippet(&clip)).ok();
    let (auto_type, _, orig_x, orig_y) = classify_doc(&clip);

    let st = s.doc()?;
    let orig_spread = st
        .selection
        .items
        .first()
        .and_then(|id| st.doc.find(*id))
        .and_then(|loc| match loc.spread {
            designcraft_doc::SpreadRef::Doc(si) => Some(si),
            designcraft_doc::SpreadRef::Parent(_) => None,
        })
        .unwrap_or(0);
    let first = st.selection.items.first().and_then(|id| st.doc.item(*id));
    let kind = first
        .map(|it| if !it.label.is_empty() { it.label.clone() } else { it.default_label().trim_matches(|c| c == '<' || c == '>').to_string() })
        .unwrap_or_default();
    let n = st.selection.items.len();
    let name = str_param(p, "name").map(str::to_string).unwrap_or_else(|| if n > 1 { format!("{n} objects") } else { kind });
    let description = str_param(p, "description").unwrap_or("").to_string();
    let object_type = str_param(p, "objectType").map(str::to_string).unwrap_or(auto_type);
    let keywords = parse_keywords(p).unwrap_or_default();

    Ok(LibraryItem {
        name,
        description,
        snippet: super::file::base64_encode(&snip),
        object_type,
        created: unix_now(),
        keywords,
        idms: idms_xml,
        original_x: orig_x,
        original_y: orig_y,
        original_spread: orig_spread,
    })
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(noundo "library.new", "New Library", ["File", "New"], None, "{path?} — an empty Asset Library (.indl or .dclib, saved to `path` when given)", always, |s, p| {
            let lib = Library { path: str_param(p, "path").map(str::to_string), items: vec![] };
            lib.save()?;
            s.library = Some(lib);
            ok()
        }),
        cmd!(noundo "library.open", "Open Library", [], None, "{path | json} → {items, format}", always, |s, p| {
            let (lib, fmt) = match (str_param(p, "json"), str_param(p, "path")) {
                (Some(j), _) => {
                    let parsed: Library = serde_json::from_str(j).map_err(|e| bad("library.open", format!("not a library: {e}")))?;
                    (parsed, "json")
                }
                (None, Some(path)) => {
                    #[cfg(not(target_arch = "wasm32"))]
                    let bytes = crate::io::read_file(path).map_err(EngineError::Other)?;
                    #[cfg(target_arch = "wasm32")]
                    let bytes: Vec<u8> = Vec::new();
                    let mut parsed = decode_library_bytes(&bytes).map_err(|e| bad("library.open", format!("not a library: {e}")))?;
                    parsed.path = Some(path.to_string());
                    let fmt = if is_indl_path(path) { "indl" } else { "dclib" };
                    (parsed, fmt)
                }
                _ => return Err(bad("library.open", "`path` or `json` required")),
            };
            let n = lib.items.len();
            s.library = Some(lib);
            Ok(json!({"items": n, "format": fmt}))
        }),
        cmd!(noundo "library.saveAs", "Save Library As…", [], None, "{path}", has_library, |s, p| {
            let path = str_param(p, "path").ok_or_else(|| bad("library.saveAs", "`path` required"))?.to_string();
            let mut lib = s.library.as_ref().ok_or_else(no_library)?.clone();
            lib.path = Some(path);
            lib.save()?;
            s.library = Some(lib);
            ok()
        }),
        cmd!(query "library.list", "Library Items", [], None, "{query?, objectType?, sort?: \"name\"|\"newest\"|\"oldest\"|\"type\"} → [{index, name, description, objectType, created, keywords, originalSpread, originalX, originalY}]", has_library, |s, p| {
            let lib = s.library.as_ref().ok_or_else(no_library)?;
            let q = str_param(p, "query").unwrap_or("").trim().to_ascii_lowercase();
            let type_filter = str_param(p, "objectType").unwrap_or("").trim().to_ascii_lowercase();
            let sort = str_param(p, "sort").unwrap_or("");
            let mut entries: Vec<(usize, &LibraryItem)> = lib
                .items
                .iter()
                .enumerate()
                .filter(|(_, it)| {
                    (type_filter.is_empty() || type_filter == "all" || it.object_type.eq_ignore_ascii_case(&type_filter))
                        && (q.is_empty()
                            || it.name.to_ascii_lowercase().contains(&q)
                            || it.description.to_ascii_lowercase().contains(&q)
                            || it.keywords.iter().any(|k| k.to_ascii_lowercase().contains(&q)))
                })
                .collect();
            match sort {
                "name" => entries.sort_by(|a, b| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase())),
                "newest" => entries.sort_by(|a, b| b.1.created.cmp(&a.1.created)),
                "oldest" => entries.sort_by(|a, b| a.1.created.cmp(&b.1.created)),
                "type" => entries.sort_by(|a, b| a.1.object_type.cmp(&b.1.object_type)),
                _ => {}
            }
            Ok(Value::Array(
                entries
                    .into_iter()
                    .map(|(i, it)| {
                        json!({
                            "index": i,
                            "name": it.name,
                            "description": it.description,
                            "objectType": if it.object_type.is_empty() { "shape" } else { &it.object_type },
                            "created": it.created,
                            "keywords": it.keywords,
                            "originalSpread": it.original_spread,
                            "originalX": it.original_x,
                            "originalY": it.original_y,
                        })
                    })
                    .collect(),
            ))
        }),
        cmd!(query "library.json", "Library Contents", [], None, "{} → the library as JSON text (to save where there's no file system)", has_library, |s, _| {
            Ok(json!({"json": serde_json::to_string(s.library.as_ref().ok_or_else(no_library)?).unwrap_or_default()}))
        }),
        cmd!(noundo "library.add", "Add Item", [], None, "{name?, description?, objectType?, keywords?} — the selection → {index}", has_selection, |s, p| {
            has_library(s).map_err(|e| bad("library.add", e))?;
            let item = build_library_item_from_selection(s, p)?;
            let mut lib = s.library.as_ref().ok_or_else(no_library)?.clone();
            let index = lib.items.len();
            lib.items.push(item);
            lib.save()?;
            s.library = Some(lib);
            Ok(json!({"index": index}))
        }),
        cmd!(noundo "library.editItem", "Library Item Information…", [], None, "{index, name?, description?, objectType?, keywords?}", has_library, |s, p| {
            let mut lib = s.library.as_ref().ok_or_else(no_library)?.clone();
            let i = p.get("index").and_then(Value::as_u64).ok_or_else(|| bad("library.editItem", "`index` required"))? as usize;
            let it = lib.items.get_mut(i).ok_or_else(|| bad("library.editItem", format!("no item {i}")))?;
            if let Some(n) = str_param(p, "name") {
                it.name = n.to_string();
            }
            if let Some(d) = str_param(p, "description") {
                it.description = d.to_string();
            }
            if let Some(ot) = str_param(p, "objectType") {
                it.object_type = ot.to_string();
            }
            if let Some(kw) = parse_keywords(p) {
                it.keywords = kw;
            }
            lib.save()?;
            s.library = Some(lib);
            ok()
        }),
        cmd!(noundo "library.update", "Update Library Item", [], None, "{index} — replace item geometry/content from the current selection", has_selection, |s, p| {
            has_library(s).map_err(|e| bad("library.update", e))?;
            let i = p.get("index").and_then(Value::as_u64).ok_or_else(|| bad("library.update", "`index` required"))? as usize;
            let fresh = build_library_item_from_selection(s, p)?;
            let mut lib = s.library.as_ref().ok_or_else(no_library)?.clone();
            let slot = lib.items.get_mut(i).ok_or_else(|| bad("library.update", format!("no item {i}")))?;
            slot.snippet = fresh.snippet;
            slot.idms = fresh.idms;
            slot.object_type = fresh.object_type;
            slot.original_x = fresh.original_x;
            slot.original_y = fresh.original_y;
            slot.original_spread = fresh.original_spread;
            lib.save()?;
            s.library = Some(lib);
            ok()
        }),
        cmd!(
            "library.place",
            "Place Item",
            [],
            None,
            "{index, spread?, x?, y?} — at its original position unless x/y (top-left) are given",
            has_doc,
            |s, p| {
                let lib = s.library.as_ref().ok_or_else(|| bad("library.place", "no library open"))?;
                let i = p.get("index").and_then(Value::as_u64).ok_or_else(|| bad("library.place", "`index` required"))? as usize;
                let it = lib.items.get(i).ok_or_else(|| bad("library.place", format!("no item {i}")))?;
                let payload = if !it.snippet.is_empty() {
                    it.snippet.clone()
                } else if let Some(xml) = &it.idms {
                    super::file::base64_encode(xml.as_bytes())
                } else {
                    return Err(bad("library.place", "library item has no content"));
                };
                let q = super::with_param(p, "base64", json!(payload));
                super::file::snippet_place(s, &q)
            }
        ),
        cmd!(
            "library.placeOriginal",
            "Place Item(s) at Original Position",
            [],
            None,
            "{index} — place item onto its recorded original spread & (x, y) coordinates",
            has_doc,
            |s, p| {
                let (payload, orig_spread, orig_x, orig_y) = {
                    let lib = s.library.as_ref().ok_or_else(|| bad("library.placeOriginal", "no library open"))?;
                    let i = p.get("index").and_then(Value::as_u64).ok_or_else(|| bad("library.placeOriginal", "`index` required"))? as usize;
                    let it = lib.items.get(i).ok_or_else(|| bad("library.placeOriginal", format!("no item {i}")))?;
                    let payload = if !it.snippet.is_empty() {
                        it.snippet.clone()
                    } else if let Some(xml) = &it.idms {
                        super::file::base64_encode(xml.as_bytes())
                    } else {
                        return Err(bad("library.placeOriginal", "library item has no content"));
                    };
                    (payload, it.original_spread, it.original_x, it.original_y)
                };
                let spread_count = s.doc()?.doc.spreads.len();
                let target_spread = orig_spread.min(spread_count.saturating_sub(1));
                let q = json!({
                    "base64": payload,
                    "spread": target_spread,
                    "x": orig_x,
                    "y": orig_y,
                });
                super::file::snippet_place(s, &q)
            }
        ),
        cmd!(query "library.exportIdms", "Export Item as IDMS…", [], None, "{index, path?} → {path?, xml, bytes}", has_library, |s, p| {
            let lib = s.library.as_ref().ok_or_else(no_library)?;
            let i = p.get("index").and_then(Value::as_u64).ok_or_else(|| bad("library.exportIdms", "`index` required"))? as usize;
            let it = lib.items.get(i).ok_or_else(|| bad("library.exportIdms", format!("no item {i}")))?;
            let idms_bytes = if let Some(xml) = &it.idms {
                xml.as_bytes().to_vec()
            } else {
                let doc = super::file::from_bytes(&super::file::base64_decode(&it.snippet))?;
                designcraft_idml::export_snippet(&doc)
            };
            let xml = String::from_utf8_lossy(&idms_bytes).into_owned();
            if let Some(path) = str_param(p, "path") {
                #[cfg(not(target_arch = "wasm32"))]
                crate::io::write_file(path, &idms_bytes).map_err(EngineError::Other)?;
                Ok(json!({"path": path, "bytes": idms_bytes.len(), "xml": xml}))
            } else {
                Ok(json!({"bytes": idms_bytes.len(), "xml": xml}))
            }
        }),
        cmd!(noundo "library.importIdms", "Import IDMS into Library…", [], None, "{path?, xml?, name?} → {index}", has_library, |s, p| {
            let (bytes, default_name) = if let Some(xml) = str_param(p, "xml") {
                (xml.as_bytes().to_vec(), "Snippet".to_string())
            } else if let Some(path) = str_param(p, "path") {
                #[cfg(not(target_arch = "wasm32"))]
                let b = crate::io::read_file(path).map_err(EngineError::Other)?;
                #[cfg(target_arch = "wasm32")]
                let b: Vec<u8> = Vec::new();
                let stem = std::path::Path::new(path)
                    .file_stem()
                    .and_then(std::ffi::OsStr::to_str)
                    .unwrap_or("Snippet")
                    .to_string();
                (b, stem)
            } else {
                return Err(bad("library.importIdms", "`path` or `xml` required"));
            };
            let name = str_param(p, "name").unwrap_or(&default_name);
            let item = item_from_idms_bytes(name, &bytes)?;
            let mut lib = s.library.as_ref().ok_or_else(no_library)?.clone();
            let index = lib.items.len();
            lib.items.push(item);
            lib.save()?;
            s.library = Some(lib);
            Ok(json!({"index": index}))
        }),
        cmd!(noundo "library.remove", "Delete Item", [], None, "{index}", has_library, |s, p| {
            let mut lib = s.library.as_ref().ok_or_else(no_library)?.clone();
            let i = p.get("index").and_then(Value::as_u64).ok_or_else(|| bad("library.remove", "`index` required"))? as usize;
            if i >= lib.items.len() {
                return Err(bad("library.remove", format!("no item {i}")));
            }
            lib.items.remove(i);
            lib.save()?;
            s.library = Some(lib);
            ok()
        }),
        cmd!(noundo "conveyor.collect", "Collect", [], None, "{ids? (default: the selection)} — each object onto the Content Collector conveyor → {count}", has_selection, |s, p| {
            let ids = super::targets(s, p)?;
            let keep = s.active().map(|d| d.selection.clone());
            for id in ids {
                let name = s.doc()?.doc.item(id).map(|it| it.default_label().trim_matches(|c| c == '<' || c == '>').to_string()).unwrap_or_default();
                if let Ok(st) = s.doc_mut() {
                    st.selection = designcraft_doc::Selection::items(vec![id]);
                }
                let snip = super::file::snippet_bytes(s)?;
                s.conveyor.push((name, snip));
            }
            if let (Some(k), Ok(st)) = (keep, s.doc_mut()) {
                st.selection = k;
            }
            Ok(json!({"count": s.conveyor.len()}))
        }),
        cmd!(
            "conveyor.place",
            "Place",
            [],
            None,
            "{index? (0), spread?, x?, y?, keep?: bool (stay on the conveyor)} — the Content Placer: the collected object at x/y (top-left)",
            has_doc,
            |s, p| {
                let i = p.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                let (_, snip) = s.conveyor.get(i).cloned().ok_or_else(|| bad("conveyor.place", "the conveyor is empty"))?;
                let q = super::with_param(p, "base64", json!(super::file::base64_encode(&snip)));
                let r = super::file::snippet_place(s, &q)?;
                if !p.get("keep").and_then(Value::as_bool).unwrap_or(false) {
                    s.conveyor.remove(i);
                }
                Ok(r)
            }
        ),
        cmd!(query "conveyor.list", "Conveyor", [], None, "{} → [name]", always, |s, _| Ok(json!(s.conveyor.iter().map(|c| c.0.clone()).collect::<Vec<_>>()))),
        cmd!(noundo "conveyor.clear", "Clear Conveyor", [], None, "{}", always, |s, _| {
            s.conveyor.clear();
            ok()
        }),
        cmd!(noundo "library.close", "Close Library", [], None, "{}", has_library, |s, _| {
            s.library = None;
            ok()
        }),
    ]
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Session;

    #[test]
    fn library_round_trip() {
        let dir = std::env::temp_dir().join(format!("dc-lib-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("brand.dclib").to_string_lossy().to_string();
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        s.execute("library.new", &json!({"path": path})).unwrap();
        let r = s.execute("frame.create", &json!({"rect": [72, 72, 172, 122], "content": "text", "text": "Logo", "caret": false})).unwrap();
        s.execute("library.add", &json!({"name": "Logo block"})).unwrap();
        let mut t = Session::new();
        t.execute("file.new", &json!({})).unwrap();
        t.execute("library.open", &json!({"path": path})).unwrap();
        assert_eq!(t.execute("library.list", &json!({})).unwrap()[0]["name"], "Logo block");
        let placed = t.execute("library.place", &json!({"index": 0, "x": 300, "y": 300})).unwrap();
        let id = designcraft_doc::ItemId(placed["ids"][0].as_u64().unwrap());
        let d = &t.doc().unwrap().doc;
        let it = d.item(id).unwrap();
        assert_eq!((it.bounds().x0, it.bounds().y0), (300.0, 300.0));
        let sid = it.text_frame().unwrap().story;
        assert_eq!(d.stories[&sid].text, "Logo");
        let _ = r;
        t.execute("library.remove", &json!({"index": 0})).unwrap();
        assert!(t.execute("library.list", &json!({})).unwrap().as_array().unwrap().is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn indl_bundle_metadata_filter_idms_and_place_original() {
        let dir = std::env::temp_dir().join(format!("dc-indl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let indl_path = dir.join("system.indl").to_string_lossy().to_string();
        let idms_path = dir.join("badge.idms").to_string_lossy().to_string();

        let mut s = Session::new();
        s.execute("file.new", &json!({"pages": 2})).unwrap();
        s.execute("library.new", &json!({"path": indl_path})).unwrap();
        s.execute("frame.create", &json!({"rect": [144, 180, 300, 240], "content": "text", "text": "Pull Quote", "caret": false})).unwrap();
        s.execute("library.add", &json!({"name": "Editorial Quote", "description": "Pull quote box", "keywords": "editorial, quote"})).unwrap();

        // Export IDMS from library item and re-import it.
        let exported = s.execute("library.exportIdms", &json!({"index": 0, "path": idms_path})).unwrap();
        assert!(exported["xml"].as_str().unwrap().contains("SnippetType=\"PageItem\""));
        s.execute("library.importIdms", &json!({"path": idms_path, "name": "Imported Badge"})).unwrap();
        s.execute("library.editItem", &json!({"index": 1, "keywords": ["badge", "reimported"]})).unwrap();

        // Reopen `.indl` in a fresh session, filter by keyword, and place at original coordinates.
        let mut t = Session::new();
        t.execute("file.new", &json!({"pages": 2})).unwrap();
        let opened = t.execute("library.open", &json!({"path": indl_path})).unwrap();
        assert_eq!(opened["items"], 2);
        assert_eq!(opened["format"], "indl");

        let filtered = t.execute("library.list", &json!({"query": "editorial"})).unwrap();
        assert_eq!(filtered.as_array().unwrap().len(), 1);
        assert_eq!(filtered[0]["objectType"], "text");

        let placed = t.execute("library.placeOriginal", &json!({"index": 0})).unwrap();
        let pid = designcraft_doc::ItemId(placed["ids"][0].as_u64().unwrap());
        let pitem = t.doc().unwrap().doc.item(pid).unwrap();
        assert!((pitem.bounds().x0 - 144.0).abs() < 1e-3);
        assert!((pitem.bounds().y0 - 180.0).abs() < 1e-3);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn content_collector_and_placer() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"pages": 2})).unwrap();
        let a = s.execute("frame.create", &json!({"rect": [72, 72, 172, 122], "content": "text", "text": "Reused", "caret": false})).unwrap()["id"]
            .clone();
        let b = s.execute("frame.create", &json!({"rect": [200, 72, 260, 122]})).unwrap()["id"].clone();
        assert_eq!(s.execute("conveyor.collect", &json!({"ids": [a, b]})).unwrap()["count"], 2);
        let r = s.execute("conveyor.place", &json!({"spread": 1, "x": 300, "y": 300})).unwrap();
        let id = designcraft_doc::ItemId(r["ids"][0].as_u64().unwrap());
        let d = &s.doc().unwrap().doc;
        assert_eq!(d.item(id).unwrap().bounds().x0, 300.0);
        assert!(d.find(id).is_some_and(|l| l.spread == designcraft_doc::SpreadRef::Doc(1)));
        assert_eq!(s.execute("conveyor.list", &json!({})).unwrap().as_array().unwrap().len(), 1, "placed objects leave the conveyor");
        s.execute("conveyor.place", &json!({"keep": true})).unwrap();
        assert_eq!(s.execute("conveyor.list", &json!({})).unwrap().as_array().unwrap().len(), 1);
    }
}
