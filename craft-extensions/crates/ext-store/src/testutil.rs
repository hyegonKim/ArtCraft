//! Test helpers: build packages in memory.

use craft_ext_core::manifest::MANIFEST_NAME;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

pub fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

pub fn dark_theme() -> Vec<u8> {
    json!({
        "schema": 1,
        "palette": {
            "dark": true, "background": "#0f1720", "surface": "#18222e", "text": "#e8eef4", "muted_text": "#a9b7c6",
            "line": "#2b3a4a", "accent": "#4da3ff", "good": "#4cc38a", "warn": "#e0b04a", "bad": "#ff6b6b"
        },
        "apps": {"photocraft": {"chrome": "#111111"}}
    })
    .to_string()
    .into_bytes()
}

pub fn low_contrast_theme() -> Vec<u8> {
    let mut v: Value = serde_json::from_slice(&dark_theme()).unwrap_or(Value::Null);
    v["palette"]["text"] = json!("#101820");
    v.to_string().into_bytes()
}

pub fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, content) in entries {
        w.start_file(*name, opts).unwrap();
        w.write_all(content).unwrap();
    }
    w.finish().unwrap().into_inner()
}

/// A theme package for the given id and version, working in the given apps.
pub fn theme_package(id: &str, version: &str, apps: &[&str], theme: &[u8]) -> Vec<u8> {
    let compat: serde_json::Map<String, Value> = apps.iter().map(|a| (a.to_string(), json!({}))).collect();
    let manifest = json!({
        "schema": 1, "id": id, "name": "Sample theme", "version": version, "kind": "theme",
        "publisher": {"name": "Example"}, "compat": compat,
        "files": [{"path": "theme.json", "sha256": hex(theme), "size": theme.len()}]
    })
    .to_string()
    .into_bytes();
    zip_of(&[(MANIFEST_NAME, &manifest), ("theme.json", theme)])
}

pub fn plugin_package(id: &str, version: &str) -> Vec<u8> {
    let wasm: &[u8] = b"\0asm\x01\0\0\0";
    let manifest = json!({
        "schema": 1, "id": id, "name": "Sample plug-in", "version": version, "kind": "plugin",
        "publisher": {"name": "Example"}, "compat": {"photocraft": {"interface": 1}},
        "files": [{"path": "plugin.wasm", "sha256": hex(wasm), "size": wasm.len()}]
    })
    .to_string()
    .into_bytes();
    zip_of(&[(MANIFEST_NAME, &manifest), ("plugin.wasm", wasm)])
}
