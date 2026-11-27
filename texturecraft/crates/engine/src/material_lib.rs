//! User smart materials: a layer (or folder) saved with its pixels and images as a `.tcmat` file
//! (the `.tcraft` container holding a one-texture-set project), and added back above the
//! selection in any project.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{Value, json};
use texturecraft_compose::PixelStore;
use texturecraft_core::{EffectKind, LayerId, LayerKind, Project, Source, TextureSet};

use crate::commands::{CommandSpec, always, bad, has_layer, layer_param, strp};
use crate::materials::Place;
use crate::session::{Document, StackTarget};
use crate::{EngineError, Result, Session, cmd};

/// Smart material file extension.
pub const SMART_MATERIAL_EXT: &str = "tcmat";
const SMART_MATERIAL_JSON_FORMAT: &str = "texturecraft.smartMaterial.v1";
const MAX_HEX_LEN: usize = 64 * 1024 * 1024;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("material.save", "Create smart material", ["Layers"], None, "{layer?, path: .tcmat file, name?}", has_layer, save),
        cmd!(
            "material.load",
            "Add smart material from file",
            [],
            None,
            "{path: .tcmat smart material, .mcmat MaterialCraft material or .mcraft package, above?: layer id, inside?: bool, index?: n} — above the selection by default, or placed like material.apply",
            always,
            load
        ),
        cmd!("material.exportJson", "Export smart material JSON", [], None, "{layer?, name?} -> portable JSON smart material envelope", has_layer, export_json),
        cmd!(
            "material.importJson",
            "Import smart material JSON",
            [],
            None,
            "{data?: envelope object | hex: encoded .tcmat/.mcmat/.mcraft bytes, above?: layer id, inside?: bool, index?: n}",
            always,
            import_json
        ),
    ]
}

fn fe(e: texturecraft_formats::FormatError) -> EngineError {
    EngineError::Other(e.to_string())
}

/// Image resources a layer subtree uses (fill sources and fill effects).
pub(crate) fn images_used(p: &Project, id: LayerId, out: &mut Vec<String>, depth: usize) {
    if depth > texturecraft_core::project::MAX_DEPTH {
        return;
    }
    let Some(l) = p.layers.get(&id) else { return };
    if let LayerKind::Fill(f) = &l.kind {
        for s in f.sources.values() {
            if let Source::Image { name, .. } = s {
                out.push(name.clone());
            }
        }
        out.extend(f.link.as_ref().map(|link| link.package.clone()));
    }
    for e in l.mask.iter().flat_map(|m| m.effects.iter()) {
        if let EffectKind::Fill { source: Source::Image { name, .. }, .. } = &e.kind {
            out.push(name.clone());
        }
    }
    for c in l.children() {
        images_used(p, *c, out, depth + 1);
    }
}

impl Session {
    /// Serialize layer `id` (with children, pixels and images) as a smart material file.
    pub fn smart_material_bytes(&self, id: LayerId, name: &str) -> Result<Vec<u8>> {
        let (ts, _, _) = self.doc.project.locate(id).ok_or_else(|| EngineError::Other("layer not in a stack".into()))?;
        let src_set = self.doc.project.texture_set(ts)?;
        let mut project = Project::new(name, "builtin:none");
        let mut set = TextureSet::new(name, 0, src_set.resolution);
        set.uv_tiles = src_set.uv_tiles.clone();
        set.tile_resolutions = src_set.tile_resolutions.clone();
        set.channels = src_set.channels.clone();
        project.texture_sets.push(set);
        let mut mini = Document { bake_paint: BTreeMap::new(), project, pixels: PixelStore::default(), baked: BTreeMap::new(), resources: BTreeMap::new() };
        let new = crate::layer_ops::copy_tree(&self.doc, id, &mut mini, 0, None, false, 0)?;
        if let Ok(l) = mini.project.layer_mut(new) {
            l.name = name.to_string();
        }
        let mut used = Vec::new();
        images_used(&mini.project, new, &mut used, 0);
        let resources = used.iter().filter_map(|n| self.doc.resources.get(n).map(|b| (n.clone(), b.as_ref().clone()))).collect();
        let pf = texturecraft_formats::ProjectFile {
            project: mini.project,
            pixels: mini.pixels,
            baked: BTreeMap::new(),
            mesh: None,
            high_poly: Vec::new(),
            cage: None,
            bake_paint: BTreeMap::new(),
            thumbnail: None,
            resources,
            warnings: Vec::new(),
        };
        texturecraft_formats::save_project_bytes(&pf).map_err(fe)
    }

    /// Add a smart material file above the selection (one undo step). Returns the new layer.
    pub fn add_smart_material_bytes(&mut self, bytes: &[u8]) -> Result<LayerId> {
        self.add_smart_material_bytes_at(bytes, Place::Selection)
    }

    /// [`Session::add_smart_material_bytes`] at `place` (shelf drag and drop). MaterialCraft
    /// portable materials and packages become linked fill layers.
    pub fn add_smart_material_bytes_at(&mut self, bytes: &[u8], place: Place) -> Result<LayerId> {
        if texturecraft_formats::mcmat::is_mcmat(bytes) || texturecraft_formats::mcmat::is_mcraft(bytes) {
            return self.add_linked_material_bytes_at(bytes, place);
        }
        let pf = texturecraft_formats::load_project_bytes(bytes).map_err(fe)?;
        let root = pf
            .project
            .texture_sets
            .first()
            .and_then(|t| t.stack.last().copied())
            .ok_or_else(|| EngineError::Other("the smart material file has no layer".into()))?;
        let resources: BTreeMap<String, Arc<Vec<u8>>> = pf.resources.into_iter().map(|(k, v)| (k, Arc::new(v))).collect();
        let src = Document { bake_paint: BTreeMap::new(), project: pf.project, pixels: pf.pixels, baked: BTreeMap::new(), resources: resources.clone() };
        let id = self.edit("Add smart material", |s| {
            for (k, v) in &resources {
                s.doc.resources.entry(k.clone()).or_insert_with(|| v.clone());
            }
            let ts = s.ts();
            let sel = s.selected_layer();
            let inside = sel.and_then(|l| s.doc.project.layer(l).ok()).is_some_and(|l| l.is_group());
            let id = crate::layer_ops::copy_tree(&src, root, &mut s.doc, ts, sel, inside, 0)?;
            place.apply(s, id)?;
            s.selected.insert(ts, id);
            s.target = StackTarget::Content;
            Ok(id)
        })?;
        self.sync_images();
        Ok(id)
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len().saturating_mul(2));
    for &b in bytes {
        out.push(HEX[usize::from(b >> 4)] as char);
        out.push(HEX[usize::from(b & 0x0f)] as char);
    }
    out
}

fn decode_hex(command: &str, hex: &str) -> Result<Vec<u8>> {
    let trimmed = hex.trim();
    if trimmed.is_empty() {
        return Err(bad(command, "`hex` must not be empty"));
    }
    if trimmed.len() > MAX_HEX_LEN {
        return Err(bad(command, "smart material payload exceeds maximum allowed size"));
    }
    if !trimmed.len().is_multiple_of(2) {
        return Err(bad(command, "`hex` length must be even"));
    }
    let nibble = |b: u8| -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    };
    let mut out = Vec::with_capacity(trimmed.len() / 2);
    for pair in trimmed.as_bytes().as_chunks::<2>().0 {
        let hi = nibble(pair[0]).ok_or_else(|| bad(command, "invalid hex character in smart material payload"))?;
        let lo = nibble(pair[1]).ok_or_else(|| bad(command, "invalid hex character in smart material payload"))?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn export_json(s: &mut Session, p: &Value) -> Result<Value> {
    let id = layer_param(s, "material.exportJson", p)?;
    let name = strp(p, "name").map(str::to_string).unwrap_or_else(|| s.doc.project.layer(id).map(|l| l.name.clone()).unwrap_or_default());
    let bytes = s.smart_material_bytes(id, &name)?;
    Ok(json!({
        "format": SMART_MATERIAL_JSON_FORMAT,
        "name": name,
        "byteLength": bytes.len(),
        "hex": encode_hex(&bytes),
    }))
}

fn import_json(s: &mut Session, p: &Value) -> Result<Value> {
    let envelope = p.get("data").unwrap_or(p);
    if let Some(fmt) = strp(envelope, "format")
        && fmt != SMART_MATERIAL_JSON_FORMAT
    {
        return Err(bad("material.importJson", format!("unsupported smart material format `{fmt}`")));
    }
    let hex = strp(envelope, "hex").ok_or_else(|| bad("material.importJson", "`hex` (or `data.hex`) is required"))?;
    let bytes = decode_hex("material.importJson", hex)?;
    let id = s.add_smart_material_bytes_at(&bytes, Place::from_params(p))?;
    Ok(json!({ "layer": id.0 }))
}

#[cfg(not(target_arch = "wasm32"))]
fn save(s: &mut Session, p: &Value) -> Result<Value> {
    let id = layer_param(s, "material.save", p)?;
    let path = strp(p, "path").ok_or_else(|| bad("material.save", "`path` is required"))?;
    let name = strp(p, "name").map(str::to_string).unwrap_or_else(|| s.doc.project.layer(id).map(|l| l.name.clone()).unwrap_or_default());
    let bytes = s.smart_material_bytes(id, &name)?;
    let path = if path.ends_with(&format!(".{SMART_MATERIAL_EXT}")) { path.to_string() } else { format!("{path}.{SMART_MATERIAL_EXT}") };
    if let Some(dir) = std::path::Path::new(&path).parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| bad("material.save", format!("cannot create {}: {e}", dir.display())))?;
    }
    let tmp = format!("{path}.tmp");
    std::fs::write(&tmp, bytes).and_then(|_| std::fs::rename(&tmp, &path)).map_err(|e| bad("material.save", format!("cannot write {path}: {e}")))?;
    Ok(json!({ "path": path }))
}

#[cfg(not(target_arch = "wasm32"))]
fn load(s: &mut Session, p: &Value) -> Result<Value> {
    let path = strp(p, "path").ok_or_else(|| bad("material.load", "`path` is required"))?;
    let bytes = std::fs::read(path).map_err(|e| bad("material.load", format!("cannot read {path}: {e}")))?;
    let id = s.add_smart_material_bytes_at(&bytes, Place::from_params(p))?;
    Ok(json!({ "layer": id.0 }))
}

#[cfg(target_arch = "wasm32")]
fn save(_: &mut Session, _: &Value) -> Result<Value> {
    Err(EngineError::Other("file paths are not available in the browser".into()))
}

#[cfg(target_arch = "wasm32")]
fn load(_: &mut Session, _: &Value) -> Result<Value> {
    Err(EngineError::Other("file paths are not available in the browser".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smart_material_json_exports_imports_and_undoes() {
        let mut src = Session::new_project("src", texturecraft_geom::primitives::sphere(1.0, 12, 8), "builtin:sphere", None, 128).unwrap();
        let added = crate::materials::apply(&mut src, "steel_frost_coated").unwrap();
        let exported = crate::execute(&mut src, "material.exportJson", &json!({"layer": added.0, "name": "Portable Frost"})).unwrap();
        assert_eq!(exported["format"], SMART_MATERIAL_JSON_FORMAT);
        assert_eq!(exported["name"], "Portable Frost");

        let mut dst = Session::new_project("dst", texturecraft_geom::primitives::sphere(1.0, 12, 8), "builtin:sphere", None, 128).unwrap();
        let stack_before = dst.doc.project.texture_sets[0].stack.clone();
        let imported = crate::execute(&mut dst, "material.importJson", &json!({"data": exported})).unwrap();
        let new_id = LayerId(imported["layer"].as_u64().unwrap());
        assert_eq!(dst.doc.project.layer(new_id).unwrap().name, "Portable Frost");
        assert!(dst.doc.project.layer(new_id).unwrap().is_group());
        dst.undo().unwrap();
        assert_eq!(dst.doc.project.texture_sets[0].stack, stack_before);

        assert!(crate::execute(&mut dst, "material.importJson", &json!({"hex": "zz"})).is_err());
        assert!(crate::execute(&mut dst, "material.importJson", &json!({"format": "other.v1", "hex": "00"})).is_err());
    }
}
