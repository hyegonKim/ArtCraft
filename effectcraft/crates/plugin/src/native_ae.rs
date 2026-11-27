//! Cross-platform C++ `.aex` / `.prm` / Mach-O `.plugin` binary effect adapter implementing
//! [`effectcraft_effects::plugin::EffectPlugin`].

use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

use effectcraft_effects::plugin::{EffectPlugin, PLUGIN_API_VERSION, PluginFrame, PluginHost, PluginManifest, PluginParam, PluginParamKind, PluginParams};

use crate::native_vm::{AeParamKind, AeParamSpec, AePrmHost, Limits, PluginFormat, RgbaFrame};

/// Sandboxed, zero-JIT C++ `.aex` / `.prm` / `.plugin` binary effect plug-in.
pub struct NativeAePrmPlugin {
    manifest: PluginManifest,
    source: String,
    host: Mutex<AePrmHost>,
}

impl NativeAePrmPlugin {
    /// Loads and probes a 64-bit PE32+ (`.aex`, `.prm`) or Mach-O / Universal (`.plugin`, `.bundle`)
    /// C++ binary plug-in.
    pub fn new(bytes: &[u8], file_hint: Option<&str>) -> Result<Self, String> {
        let host = AePrmHost::from_bytes(bytes.to_vec(), file_hint, Limits::default()).map_err(|e| e.to_string())?;
        let desc = host.descriptor();
        let source = file_hint.unwrap_or("native-binary").to_string();
        let fallback_stem = file_hint
            .and_then(|s| Path::new(s).file_stem())
            .map(|st| st.to_string_lossy().to_string())
            .filter(|st| !st.trim().is_empty())
            .unwrap_or_else(|| "native_plugin".to_string());

        let prefix = match desc.plugin_format {
            PluginFormat::PremierePrm => "prm",
            _ => "aex",
        };
        let raw_id = if !desc.pipl.match_name.trim().is_empty() {
            desc.pipl.match_name.trim()
        } else if !desc.pipl.name.trim().is_empty() {
            desc.pipl.name.trim()
        } else {
            fallback_stem.as_str()
        };
        let id = normalize_manifest_id(prefix, raw_id);
        let name = if !desc.pipl.name.trim().is_empty() { desc.pipl.name.trim().to_string() } else { fallback_stem };
        let category = if !desc.pipl.category.trim().is_empty() { desc.pipl.category.trim().to_string() } else { "Adobe C++ Plug-ins".to_string() };
        let version = if desc.pipl.version_major != 0 || desc.pipl.version_minor != 0 {
            format!("{}.{}", desc.pipl.version_major, desc.pipl.version_minor)
        } else {
            "1.0".to_string()
        };
        let params = build_manifest_params(host.params());

        let manifest = PluginManifest {
            api: PLUGIN_API_VERSION,
            id,
            name,
            category,
            version,
            author: String::new(),
            description: format!("{} ({:?}/{:?})", desc.entrypoint_symbol, desc.arch, desc.guest_os),
            params,
        };
        manifest.validate()?;
        Ok(Self { manifest, source, host: Mutex::new(host) })
    }

    /// Number of times `PF_Cmd_GLOBAL_SETUP` ran on the underlying VM host.
    pub fn global_setup_count(&self) -> u64 {
        let guard = self.host.lock().unwrap_or_else(|e| e.into_inner());
        guard.global_setup_count()
    }

    fn render_at(&self, frame: &mut PluginFrame, params: &PluginParams, time: f64, fps: f64) -> Result<(), String> {
        if frame.width == 0 || frame.height == 0 || frame.pixels.is_empty() {
            return Ok(());
        }
        let mut unpre_pixels = Vec::with_capacity(frame.pixels.len());
        for px in frame.pixels.iter() {
            let a = if px[3].is_finite() { px[3].clamp(0.0, 1.0) } else { 0.0 };
            if a > 1e-6 {
                unpre_pixels.push([(px[0] / a).clamp(0.0, 1.0), (px[1] / a).clamp(0.0, 1.0), (px[2] / a).clamp(0.0, 1.0), a]);
            } else {
                unpre_pixels.push([0.0, 0.0, 0.0, 0.0]);
            }
        }
        let rgba_in = RgbaFrame::new(frame.width, frame.height, unpre_pixels).map_err(|e| e.to_string())?;
        let out = {
            let mut guard = self.host.lock().unwrap_or_else(|e| e.into_inner());
            guard.render_reusing_setup(&rgba_in, params.flat, time, fps).map_err(|e| e.to_string())?
        };
        for (dst, src) in frame.pixels.iter_mut().zip(out.pixels.iter()) {
            let a = if src[3].is_finite() { src[3].clamp(0.0, 1.0) } else { 0.0 };
            let r = if src[0].is_finite() { src[0].clamp(0.0, 1.0) } else { 0.0 };
            let g = if src[1].is_finite() { src[1].clamp(0.0, 1.0) } else { 0.0 };
            let b = if src[2].is_finite() { src[2].clamp(0.0, 1.0) } else { 0.0 };
            *dst = [r * a, g * a, b * a, a];
        }
        Ok(())
    }
}

impl EffectPlugin for NativeAePrmPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn source(&self) -> String {
        self.source.clone()
    }

    fn render(&self, frame: &mut PluginFrame, params: &PluginParams, time: f64) -> Result<(), String> {
        self.render_at(frame, params, time, 30.0)
    }

    fn render_v2(&self, frame: &mut PluginFrame, params: &PluginParams, time: f64, host: &dyn PluginHost) -> Result<(), String> {
        self.render_at(frame, params, time, host.frame_rate())
    }
}

fn normalize_manifest_id(prefix: &str, raw: &str) -> String {
    let mut slug = String::with_capacity(raw.len());
    let mut prev_sep = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
            slug.push(ch);
            prev_sep = false;
        } else if !prev_sep {
            slug.push('_');
            prev_sep = true;
        }
    }
    let trimmed = slug.trim_matches(['_', '.', '-']);
    let body = if trimmed.is_empty() { "plugin" } else { trimmed };
    if body.starts_with("aex.") || body.starts_with("prm.") { body.to_string() } else { format!("{prefix}.{body}") }
}

fn sanitize_param_id(name: &str, index: usize, seen: &mut HashSet<String>) -> String {
    let mut base = String::new();
    let mut prev_under = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            base.push(ch.to_ascii_lowercase());
            prev_under = false;
        } else if !prev_under && !base.is_empty() {
            base.push('_');
            prev_under = true;
        }
    }
    let trimmed = base.trim_matches('_');
    let candidate =
        if trimmed.is_empty() || trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) { format!("param_{}", index + 1) } else { trimmed.to_string() };
    if seen.insert(candidate.clone()) {
        return candidate;
    }
    let unique = format!("{candidate}_{}", index + 1);
    seen.insert(unique.clone());
    unique
}

fn build_manifest_params(specs: &[AeParamSpec]) -> Vec<PluginParam> {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(specs.len());
    for (idx, spec) in specs.iter().enumerate() {
        let id = sanitize_param_id(&spec.name, idx, &mut seen);
        let name = if spec.name.trim().is_empty() { format!("Param {}", idx + 1) } else { spec.name.clone() };
        let kind = match &spec.kind {
            AeParamKind::Slider { default, min, max } => {
                let clean_min = if min.is_finite() { *min } else { 0.0 };
                let clean_max = if max.is_finite() && *max >= clean_min { *max } else { clean_min + 100.0 };
                let clean_def = if default.is_finite() { default.clamp(clean_min, clean_max) } else { clean_min };
                PluginParamKind::Slider {
                    default: clean_def,
                    min: clean_min,
                    max: clean_max,
                    slider_min: Some(clean_min),
                    slider_max: Some(clean_max),
                    decimals: 2,
                    integer: false,
                }
            }
            AeParamKind::Angle { default } => PluginParamKind::Angle { default: if default.is_finite() { *default } else { 0.0 } },
            AeParamKind::Checkbox { default } => PluginParamKind::Checkbox { default: *default },
            AeParamKind::Color { default } => PluginParamKind::Color { default: default.map(|v| if v.is_finite() { v.clamp(0.0, 1.0) } else { 1.0 }) },
            AeParamKind::Point { default } => {
                let norm = |v: f64| {
                    if v.is_finite() && (0.0..=1.0).contains(&v) && v != 0.0 { v } else { 0.5 }
                };
                PluginParamKind::Point { default: [norm(default[0]), norm(default[1])] }
            }
            AeParamKind::Popup { default, count } => {
                let n = (*count).clamp(1, 64);
                let options: Vec<String> = (1..=n).map(|i| format!("Option {i}")).collect();
                PluginParamKind::Popup { options, default: (*default).min(n - 1) }
            }
        };
        out.push(PluginParam { id, name, group: String::new(), kind });
    }
    out
}
