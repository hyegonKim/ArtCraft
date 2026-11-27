//! # effectcraft-plugin (L3)
//!
//! Loads **WebAssembly effect plug-ins** (WebAssembly ABI v1 and v2, see `docs/plugins.md`) AND
//! **cross-platform C++ Adobe After Effects `.aex` / Premiere Pro `.prm` / Mach-O `.plugin` binary
//! plug-ins** (`x86_64` PE32+ and `x86_64`/`AArch64` Mach-O/Universal, zero-JIT) into the effect
//! registry ([`effectcraft_effects::plugin`]).
//!
//! A WebAssembly plug-in is a `.wasm` (or `.wat`) module with no imports that exports:
//!
//! | export | signature | |
//! |---|---|---|
//! | `memory` | memory | linear memory shared with the host |
//! | `ec_api_version` | `() -> i32` | 1 for ABI 1, 2 for ABI 2 (neighbouring frames) |
//! | `ec_manifest_ptr`, `ec_manifest_len` | `() -> i32` | a UTF-8 JSON [`PluginManifest`] in memory |
//! | `ec_alloc` | `(bytes: i32) -> i32` | a buffer of at least `bytes` bytes, 8-byte aligned; it may reuse the previous one |
//! | `ec_render` (ABI 1) | `(pixels: i32, width: i32, height: i32, params: i32, nparams: i32, time: f64, scale: f64) -> i32` | process the pixels in place; 0 = ok |
//! | `ec_render2` (ABI 2) | `ec_render`'s arguments, then `prev: i32, prev_time: f64, next: i32, next_time: f64, flags: i32` | the same, with the neighbouring frames; 0 = ok |
//!
//! Native C++ `.aex` / `.prm` / `.plugin` binaries execute inside [`native_vm`], a pure-Rust,
//! memory-safe, zero-JIT (`W^X` compliant) `x86_64` + `AArch64` user-mode VM compiled
//! unconditionally on every target including `wasm32-unknown-unknown`.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::sync::Arc;

use effectcraft_effects::EffectSpec;
pub use effectcraft_effects::plugin::{
    EffectPlugin, MIN_WASM_API_VERSION, PLUGIN_API_VERSION, PluginFrame, PluginImage, PluginManifest, PluginParamKind, PluginParams, WASM_API_VERSION,
    register_plugin,
};

pub mod native_ae;
pub mod native_vm;
#[cfg(feature = "wasm")]
mod wasm;

pub use native_ae::NativeAePrmPlugin;

/// Error message returned when loading or validating an effect plug-in fails.
pub type PluginLoadError = String;

/// Whether this build can load WebAssembly plug-ins.
pub fn wasm_available() -> bool {
    cfg!(feature = "wasm")
}

/// Whether `bytes` (or `file_hint`) identifies a compiled PE32+ or Mach-O / Universal binary plug-in.
pub fn is_native_plugin_binary(bytes: &[u8], file_hint: Option<&str>) -> bool {
    if bytes.starts_with(b"MZ") {
        return true;
    }
    if bytes.len() >= 4 {
        let le = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let be = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if matches!(le, 0xFEED_FACF | 0xFEED_FACE) || matches!(be, 0xCAFE_BABE | 0xCAFE_BABF) {
            return true;
        }
    }
    if let Some(hint) = file_hint {
        let lower = hint.trim().to_ascii_lowercase();
        if !bytes.starts_with(b"\0asm")
            && !bytes.starts_with(b"(module")
            && (lower.ends_with(".aex") || lower.ends_with(".prm") || lower.ends_with(".plugin") || lower.ends_with(".bundle"))
        {
            return true;
        }
    }
    false
}

/// Load and register a compiled 64-bit PE32+ (`.aex`, `.prm`) or Mach-O / Universal (`.plugin`,
/// `.bundle`) C++ binary plug-in into the effect registry. Works on all targets including `wasm32`.
pub fn load_native_binary(bytes: &[u8], file_hint: Option<&str>) -> Result<&'static EffectSpec, PluginLoadError> {
    let plugin = NativeAePrmPlugin::new(bytes, file_hint)?;
    register_plugin(Arc::new(plugin))
}

/// Load and register a WebAssembly plug-in (`bytes` = a `.wasm` binary or `.wat` text;
/// `source` = where it came from, for listings). Returns the registered effect.
pub fn load_wasm(bytes: &[u8], source: &str) -> Result<&'static EffectSpec, PluginLoadError> {
    #[cfg(feature = "wasm")]
    {
        let p = wasm::WasmPlugin::new(bytes, source)?;
        register_plugin(Arc::new(p))
    }
    #[cfg(not(feature = "wasm"))]
    {
        let _ = (bytes, source);
        Err("WebAssembly plug-ins are not available in this build".into())
    }
}

/// Auto-detects binary magic (`\0asm` / `.wat` -> [`load_wasm`], PE32+ `MZ` or Mach-O/Fat
/// `0xFEEDFACF`/`0xCAFEBABE` -> [`load_native_binary`]) and registers the effect plug-in.
pub fn load_plugin(bytes: &[u8], source: &str) -> Result<&'static EffectSpec, PluginLoadError> {
    if is_native_plugin_binary(bytes, Some(source)) { load_native_binary(bytes, Some(source)) } else { load_wasm(bytes, source) }
}

/// [`load_plugin`] for the engine's plug-in loader hook: `{id, name, category, version, api, params}`.
pub fn loader(bytes: &[u8], source: &str) -> Result<serde_json::Value, String> {
    let spec = load_plugin(bytes, source)?;
    let m = effectcraft_effects::plugin::plugin(spec.id).map(|p| p.manifest().clone());
    Ok(serde_json::json!({
        "id": spec.id,
        "name": spec.name,
        "category": spec.category,
        "version": m.as_ref().map(|m| m.version.clone()).unwrap_or_default(),
        "api": m.as_ref().map(|m| m.api).unwrap_or(PLUGIN_API_VERSION),
        "params": spec.params.len(),
    }))
}

/// Returns the plug-in loader function pointer (available on all targets, including `wasm32` and
/// `cfg(not(feature = "wasm"))`).
pub fn plugin_loader() -> fn(&[u8], &str) -> Result<serde_json::Value, String> {
    loader
}

fn preview_effect_plugin(
    p: &dyn EffectPlugin,
    width: u32,
    height: u32,
    pixels: &[[f32; 4]],
    times: &[f64],
) -> Result<(serde_json::Value, Vec<Vec<[f32; 4]>>), String> {
    let m = p.manifest();
    let mut flat = Vec::with_capacity(m.flat_len());
    for param in &m.params {
        match &param.kind {
            PluginParamKind::Slider { default, .. } | PluginParamKind::Angle { default } => flat.push(*default),
            PluginParamKind::Checkbox { default } => flat.push(if *default { 1.0 } else { 0.0 }),
            PluginParamKind::Popup { default, .. } => flat.push(*default as f64),
            // Points default to fractions of the layer: the frame is the layer.
            PluginParamKind::Point { default } => flat.extend([default[0] * f64::from(width), default[1] * f64::from(height)]),
            PluginParamKind::Color { default } => flat.extend(default.iter().copied()),
            PluginParamKind::Point3 { default } => flat.extend(default.iter().copied()),
            PluginParamKind::Layer => flat.push(-1.0),
            PluginParamKind::Text { .. } | PluginParamKind::Hidden { .. } => {}
        }
    }
    let params = PluginParams { manifest: m, flat: &flat, texts: &[] };
    let mut frames = Vec::with_capacity(times.len());
    for &t in times {
        let mut px = pixels.to_vec();
        let mut frame = PluginFrame { width, height, pixels: &mut px, scale: 1.0, origin: [0.0; 2], layer_size: [f64::from(width), f64::from(height)] };
        p.render(&mut frame, &params, t)?;
        frames.push(px);
    }
    let manifest = serde_json::to_value(m).map_err(|e| e.to_string())?;
    Ok((manifest, frames))
}

/// Render a module's effect without registering it (the Plug-in Library's preview): default
/// parameters, `pixels` (premultiplied RGBA, `width × height`) at each of `times` (seconds).
/// Returns the manifest as JSON and one frame per time.
pub fn preview(bytes: &[u8], width: u32, height: u32, pixels: &[[f32; 4]], times: &[f64]) -> Result<(serde_json::Value, Vec<Vec<[f32; 4]>>), String> {
    if is_native_plugin_binary(bytes, None) {
        let p = NativeAePrmPlugin::new(bytes, Some("preview.aex"))?;
        return preview_effect_plugin(&p, width, height, pixels, times);
    }
    #[cfg(feature = "wasm")]
    {
        let p = wasm::WasmPlugin::new(bytes, "preview")?;
        preview_effect_plugin(&p, width, height, pixels, times)
    }
    #[cfg(not(feature = "wasm"))]
    {
        let _ = (width, height, pixels, times);
        Err("WebAssembly plug-ins are not available in this build".into())
    }
}

/// Returns the plug-in previewer function pointer (available on all targets).
pub fn plugin_previewer() -> fn(&[u8], u32, u32, &[[f32; 4]], &[f64]) -> Result<(serde_json::Value, Vec<Vec<[f32; 4]>>), String> {
    preview
}

#[cfg(test)]
mod native_tests;
// Two attributes, so clippy sees `cfg(test)` and treats the module as test code.
#[cfg(test)]
#[cfg(feature = "wasm")]
mod tests;
