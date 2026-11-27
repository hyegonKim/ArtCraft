//! Pure Creative Cloud Desktop Workspace Hub data and binary format inspectors:
//!
//! - [`HubTab`]: the 4 workspace hub tabs (`Files`, `Fonts`, `Libraries`, `Plugins`).
//! - [`FontCategoryFilter`] and [`BundledFontSpec`]: font specimen metadata and filtering.
//! - `.ase` (Adobe Swatch Exchange), `.aco` (Adobe Color Swatch v1/v2), and `.cclibs` (Creative
//!   Cloud Library JSON & ZIP container) header/swatch parsers plus starter handoff builders.
//! - [`SuitePluginSpec`]: Craft App Suite extension formats (`.8bf`, `.aip`, `.prm`, `.aex`,
//!   `.drfx`/`.ograf`, `.atn`, WASM, CLAP, UXP/MCP) and Zero-JIT C++ Binary VM metadata.
//!
//! Pure data and byte parsing only (no filesystem or process I/O), so both desktop and
//! `wasm32-unknown-unknown` targets share it unchanged.

use serde::{Deserialize, Serialize};

/// Maximum library file payload inspected in memory (16 MiB).
pub const MAX_LIBRARY_BYTES: usize = 16 * 1024 * 1024;
/// Maximum swatches or archive entries decoded from a single library file.
pub const MAX_SWATCHES: usize = 2048;

/// Active rail tab in the Creative Workspace Hub dialog (`Files · Fonts · Libraries · Plugins`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HubTab {
    #[default]
    Files,
    Fonts,
    Libraries,
    Plugins,
}

impl HubTab {
    /// All four workspace hub tabs in Creative Cloud Desktop rail order.
    pub const ALL: [Self; 4] = [Self::Files, Self::Fonts, Self::Libraries, Self::Plugins];

    /// Canonical English UI label for this tab.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Files => "Files",
            Self::Fonts => "Fonts",
            Self::Libraries => "Libraries",
            Self::Plugins => "Plugins",
        }
    }
}

/// Filter chip in the Fonts tab (`All · Variable · CJK · Mono`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FontCategoryFilter {
    #[default]
    All,
    Variable,
    Cjk,
    Monospace,
}

impl FontCategoryFilter {
    pub const ALL: [Self; 4] = [Self::All, Self::Variable, Self::Cjk, Self::Monospace];

    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "All Fonts",
            Self::Variable => "Variable",
            Self::Cjk => "CJK & World",
            Self::Monospace => "Monospace",
        }
    }

    pub const fn matches(self, is_variable: bool, is_cjk: bool, is_monospace: bool) -> bool {
        match self {
            Self::All => true,
            Self::Variable => is_variable,
            Self::Cjk => is_cjk,
            Self::Monospace => is_monospace,
        }
    }
}

/// Which bundled UI family renders a font entry in the specimen preview.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecimenStyle {
    Proportional,
    Display,
    Monospace,
}

/// Metadata for a font family bundled with ArtCraft Launcher.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BundledFontSpec {
    pub family: &'static str,
    pub role: &'static str,
    pub axes: &'static str,
    pub license: &'static str,
    pub is_variable: bool,
    pub is_cjk: bool,
    pub is_monospace: bool,
    pub style: SpecimenStyle,
}

/// The three OFL-1.1 variable font families bundled with ArtCraft Launcher.
pub const BUNDLED_FONTS: [BundledFontSpec; 3] = [
    BundledFontSpec {
        family: "Inter Variable",
        role: "Interface body & controls",
        axes: "wght 100–900, opsz 14–32",
        license: "OFL-1.1",
        is_variable: true,
        is_cjk: false,
        is_monospace: false,
        style: SpecimenStyle::Proportional,
    },
    BundledFontSpec {
        family: "Archivo Variable",
        role: "Display headings & Craft wordmarks",
        axes: "wght 100–900, wdth 62–125",
        license: "OFL-1.1",
        is_variable: true,
        is_cjk: false,
        is_monospace: false,
        style: SpecimenStyle::Display,
    },
    BundledFontSpec {
        family: "Geist Mono Variable",
        role: "HUD labels, badges & code",
        axes: "wght 100–900",
        license: "OFL-1.1",
        is_variable: true,
        is_cjk: false,
        is_monospace: true,
        style: SpecimenStyle::Monospace,
    },
];

/// Default Latin specimen pangram for the Fonts tab.
pub const SAMPLE_LATIN_PANGRAM: &str = "The quick brown fox jumps over the lazy dog 0123456789";
/// Default Korean specimen pangram for the Fonts tab.
pub const SAMPLE_KOREAN_PANGRAM: &str = "다람쥐 헌 쳇바퀴에 타고파 0123456789";

/// Recognized Creative Library / Swatch interchange format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibraryFormat {
    /// Adobe Swatch Exchange (`.ase`, magic `ASEF`).
    Ase,
    /// Adobe Color Swatch (`.aco`, v1 / v2).
    Aco,
    /// Creative Cloud Library archive or manifest (`.cclibs` / `.cclib`).
    CcLibs,
}

impl LibraryFormat {
    pub const fn badge(self) -> &'static str {
        match self {
            Self::Ase => "ASE",
            Self::Aco => "ACO",
            Self::CcLibs => "CCLIBS",
        }
    }

    pub const fn extension(self) -> &'static str {
        match self {
            Self::Ase => "ase",
            Self::Aco => "aco",
            Self::CcLibs => "cclibs",
        }
    }
}

/// One decoded color swatch from an `.ase`, `.aco`, or `.cclibs` library.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwatchColor {
    pub name: String,
    pub rgb: [u8; 3],
    pub model: &'static str,
}

impl SwatchColor {
    /// Uppercase `#RRGGBB` hex representation.
    pub fn hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.rgb[0], self.rgb[1], self.rgb[2])
    }
}

/// Parsed summary of a `.cclibs`, `.ase`, or `.aco` file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryInspection {
    pub format: LibraryFormat,
    pub version_label: String,
    pub library_name: String,
    pub swatches: Vec<SwatchColor>,
    pub element_count: usize,
    /// Craft Suite app IDs that accept this library/swatch handoff directly.
    pub target_apps: &'static [&'static str],
}

/// Craft Suite apps supporting color swatch & Creative Library handoff.
pub const LIBRARY_TARGET_APPS: [&str; 3] = ["photocraft", "vectorcraft", "designcraft"];

/// Starter studio palette used when creating a new `.cclibs` or `.ase` handoff library.
pub const STARTER_STUDIO_SWATCHES: [(&str, [u8; 3]); 8] = [
    ("ArtCraft Accent", [0x2D, 0x81, 0xFF]),
    ("PhotoCraft Cobalt", [0x2F, 0x7B, 0xF5]),
    ("VectorCraft Vermilion", [0xE8, 0x57, 0x3F]),
    ("FilmCraft Violet", [0x8B, 0x5C, 0xF6]),
    ("LightCraft Amber", [0xF2, 0xA5, 0x16]),
    ("DesignCraft Leaf", [0x7B, 0xB5, 0x1C]),
    ("Studio Obsidian", [0x12, 0x13, 0x16]),
    ("Studio Paper", [0xF2, 0xF1, 0xEE]),
];

fn read_u16_be(bytes: &[u8], offset: usize) -> Option<u16> {
    let slice = bytes.get(offset..offset.checked_add(2)?)?;
    Some(u16::from_be_bytes([*slice.first()?, *slice.get(1)?]))
}

fn read_u16_le(bytes: &[u8], offset: usize) -> Option<u16> {
    let slice = bytes.get(offset..offset.checked_add(2)?)?;
    Some(u16::from_le_bytes([*slice.first()?, *slice.get(1)?]))
}

fn read_u32_be(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset.checked_add(4)?)?;
    Some(u32::from_be_bytes([*slice.first()?, *slice.get(1)?, *slice.get(2)?, *slice.get(3)?]))
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset.checked_add(4)?)?;
    Some(u32::from_le_bytes([*slice.first()?, *slice.get(1)?, *slice.get(2)?, *slice.get(3)?]))
}

fn read_f32_be(bytes: &[u8], offset: usize) -> Option<f32> {
    read_u32_be(bytes, offset).map(f32::from_bits)
}

fn decode_utf16be(bytes: &[u8]) -> String {
    let units = bytes.as_chunks::<2>().0.iter().copied().map(u16::from_be_bytes);
    char::decode_utf16(units).map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER)).filter(|&c| c != '\0').collect::<String>().trim().to_string()
}

fn clamp_u8(v: f32) -> u8 {
    if !v.is_finite() {
        return 0;
    }
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn cmyk_to_rgb(c: f32, m: f32, y: f32, k: f32) -> [u8; 3] {
    let inv_k = 1.0 - k.clamp(0.0, 1.0);
    [clamp_u8((1.0 - c.clamp(0.0, 1.0)) * inv_k), clamp_u8((1.0 - m.clamp(0.0, 1.0)) * inv_k), clamp_u8((1.0 - y.clamp(0.0, 1.0)) * inv_k)]
}

fn lab_to_rgb(l_raw: f32, a: f32, b: f32) -> [u8; 3] {
    let l = if (0.0..=1.0).contains(&l_raw) { l_raw * 100.0 } else { l_raw.clamp(0.0, 100.0) };
    let fy = (l + 16.0) / 116.0;
    let fx = a / 500.0 + fy;
    let fz = fy - b / 200.0;
    let inv = |t: f32| {
        let t3 = t * t * t;
        if t3 > 0.008_856 { t3 } else { (t - 16.0 / 116.0) / 7.787 }
    };
    let x = 0.950_47 * inv(fx);
    let y = 1.000_00 * inv(fy);
    let z = 1.088_83 * inv(fz);
    let lin_r = 3.2406 * x - 1.5372 * y - 0.4986 * z;
    let lin_g = -0.9689 * x + 1.8758 * y + 0.0415 * z;
    let lin_b = 0.0557 * x - 0.2040 * y + 1.0570 * z;
    let gamma = |u: f32| {
        let u = u.clamp(0.0, 1.0);
        if u <= 0.003_130_8 { 12.92 * u } else { 1.055 * u.powf(1.0 / 2.4) - 0.055 }
    };
    [clamp_u8(gamma(lin_r)), clamp_u8(gamma(lin_g)), clamp_u8(gamma(lin_b))]
}

fn hsb_to_rgb(h_norm: f32, s_norm: f32, v_norm: f32) -> [u8; 3] {
    let s = s_norm.clamp(0.0, 1.0);
    let v = v_norm.clamp(0.0, 1.0);
    if s <= f32::EPSILON {
        let g = clamp_u8(v);
        return [g, g, g];
    }
    let h = (h_norm.rem_euclid(1.0)) * 6.0;
    let sector = h.floor();
    let f = h - sector;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match sector as i32 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    [clamp_u8(r), clamp_u8(g), clamp_u8(b)]
}

/// Parse an Adobe Swatch Exchange (`.ase`) binary buffer.
pub fn parse_ase(bytes: &[u8], fallback_name: &str) -> Result<LibraryInspection, String> {
    if bytes.len() > MAX_LIBRARY_BYTES {
        return Err("library file exceeds the 16 MB inspection limit".into());
    }
    if bytes.get(..4) != Some(b"ASEF") {
        return Err("invalid .ase header (expected ASEF magic)".into());
    }
    let major = read_u16_be(bytes, 4).ok_or_else(|| "truncated .ase version".to_string())?;
    let minor = read_u16_be(bytes, 6).ok_or_else(|| "truncated .ase version".to_string())?;
    let block_count = read_u32_be(bytes, 8).ok_or_else(|| "truncated .ase block count".to_string())?;
    let capped_blocks = usize::try_from(block_count).unwrap_or(MAX_SWATCHES).min(MAX_SWATCHES * 2);

    let mut pos = 12usize;
    let mut group_name: Option<String> = None;
    let mut swatches = Vec::new();

    for _ in 0..capped_blocks {
        if pos >= bytes.len() || swatches.len() >= MAX_SWATCHES {
            break;
        }
        let block_type = read_u16_be(bytes, pos).ok_or_else(|| "truncated .ase block header".to_string())?;
        let block_len = usize::try_from(read_u32_be(bytes, pos.saturating_add(2)).ok_or_else(|| "truncated .ase block length".to_string())?)
            .map_err(|_| "invalid .ase block length".to_string())?;
        let body_start = pos.checked_add(6).ok_or_else(|| "overflow in .ase block offset".to_string())?;
        let body_end = body_start.checked_add(block_len).ok_or_else(|| "overflow in .ase block size".to_string())?;
        let body = bytes.get(body_start..body_end).ok_or_else(|| "truncated .ase block payload".to_string())?;
        pos = body_end;

        match block_type {
            0xC001 => {
                if let Some(char_len) = read_u16_be(body, 0) {
                    let byte_len = usize::from(char_len).saturating_mul(2);
                    if let Some(name_bytes) = body.get(2..2usize.saturating_add(byte_len)) {
                        let decoded = decode_utf16be(name_bytes);
                        if !decoded.is_empty() && group_name.is_none() {
                            group_name = Some(decoded);
                        }
                    }
                }
            }
            0x0001 => {
                let Some(char_len) = read_u16_be(body, 0) else { continue };
                let name_bytes_len = usize::from(char_len).saturating_mul(2);
                let after_name = 2usize.saturating_add(name_bytes_len);
                let name = body.get(2..after_name).map(decode_utf16be).unwrap_or_default();
                let Some(mode) = body.get(after_name..after_name.saturating_add(4)) else { continue };
                let values_at = after_name.saturating_add(4);
                let parsed = match mode {
                    b"RGB " => {
                        let (Some(r), Some(g), Some(b)) =
                            (read_f32_be(body, values_at), read_f32_be(body, values_at + 4), read_f32_be(body, values_at + 8))
                        else {
                            continue;
                        };
                        Some(([clamp_u8(r), clamp_u8(g), clamp_u8(b)], "RGB"))
                    }
                    b"CMYK" => {
                        let (Some(c), Some(m), Some(y), Some(k)) = (
                            read_f32_be(body, values_at),
                            read_f32_be(body, values_at + 4),
                            read_f32_be(body, values_at + 8),
                            read_f32_be(body, values_at + 12),
                        ) else {
                            continue;
                        };
                        Some((cmyk_to_rgb(c, m, y, k), "CMYK"))
                    }
                    b"LAB " => {
                        let (Some(l), Some(a), Some(b)) =
                            (read_f32_be(body, values_at), read_f32_be(body, values_at + 4), read_f32_be(body, values_at + 8))
                        else {
                            continue;
                        };
                        Some((lab_to_rgb(l, a, b), "Lab"))
                    }
                    b"GRAY" => {
                        let Some(g) = read_f32_be(body, values_at) else { continue };
                        let v = clamp_u8(g);
                        Some(([v, v, v], "Gray"))
                    }
                    _ => None,
                };
                if let Some((rgb, model)) = parsed {
                    let index = swatches.len().saturating_add(1);
                    let label = if name.is_empty() { format!("Swatch {index}") } else { name };
                    swatches.push(SwatchColor { name: label, rgb, model });
                }
            }
            _ => {}
        }
    }

    let count = swatches.len();
    Ok(LibraryInspection {
        format: LibraryFormat::Ase,
        version_label: format!("ASE {major}.{minor}"),
        library_name: group_name.unwrap_or_else(|| fallback_name.to_string()),
        swatches,
        element_count: count,
        target_apps: &LIBRARY_TARGET_APPS,
    })
}

fn parse_aco_section(bytes: &[u8], start: usize, version: u16) -> Option<(usize, Vec<SwatchColor>)> {
    let count = usize::from(read_u16_be(bytes, start.checked_add(2)?)?).min(MAX_SWATCHES);
    let mut pos = start.checked_add(4)?;
    let mut swatches = Vec::with_capacity(count.min(64));

    for idx in 0..count {
        let space = read_u16_be(bytes, pos)?;
        let w = read_u16_be(bytes, pos.checked_add(2)?)?;
        let x = read_u16_be(bytes, pos.checked_add(4)?)?;
        let y = read_u16_be(bytes, pos.checked_add(6)?)?;
        let z = read_u16_be(bytes, pos.checked_add(8)?)?;
        pos = pos.checked_add(10)?;

        let name = if version == 2 {
            let _zero = read_u16_be(bytes, pos)?;
            let char_len = usize::from(read_u16_be(bytes, pos.checked_add(2)?)?);
            let byte_len = char_len.checked_mul(2)?;
            let name_start = pos.checked_add(4)?;
            let name_end = name_start.checked_add(byte_len)?;
            let raw = bytes.get(name_start..name_end)?;
            pos = name_end;
            let decoded = decode_utf16be(raw);
            if decoded.is_empty() { format!("Swatch {}", idx + 1) } else { decoded }
        } else {
            format!("Swatch {}", idx + 1)
        };

        let (rgb, model) = match space {
            0 => ([(w >> 8) as u8, (x >> 8) as u8, (y >> 8) as u8], "RGB"),
            1 => (hsb_to_rgb(f32::from(w) / 65535.0, f32::from(x) / 65535.0, f32::from(y) / 65535.0), "HSB"),
            2 => (
                cmyk_to_rgb(1.0 - f32::from(w) / 65535.0, 1.0 - f32::from(x) / 65535.0, 1.0 - f32::from(y) / 65535.0, 1.0 - f32::from(z) / 65535.0),
                "CMYK",
            ),
            7 => {
                let l = f32::from(w) / 100.0;
                let a = f32::from(x as i16) / 100.0;
                let b = f32::from(y as i16) / 100.0;
                (lab_to_rgb(l, a, b), "Lab")
            }
            8 => {
                let g = clamp_u8(1.0 - (f32::from(w) / 10_000.0).clamp(0.0, 1.0));
                ([g, g, g], "Gray")
            }
            _ => ([(w >> 8) as u8, (x >> 8) as u8, (y >> 8) as u8], "Spot"),
        };
        swatches.push(SwatchColor { name, rgb, model });
    }
    Some((pos, swatches))
}

/// Parse an Adobe Color Swatch (`.aco` v1 or v2) binary buffer.
pub fn parse_aco(bytes: &[u8], fallback_name: &str) -> Result<LibraryInspection, String> {
    if bytes.len() > MAX_LIBRARY_BYTES {
        return Err("swatch file exceeds the 16 MB inspection limit".into());
    }
    let version = read_u16_be(bytes, 0).ok_or_else(|| "truncated .aco header".to_string())?;
    if version != 1 && version != 2 {
        return Err(format!("unsupported .aco version {version} (expected 1 or 2)"));
    }
    let (next_pos, v1_swatches) = parse_aco_section(bytes, 0, version).ok_or_else(|| "truncated .aco swatch records".to_string())?;
    let (ver_label, swatches) = if version == 1 && read_u16_be(bytes, next_pos) == Some(2) {
        if let Some((_, v2_swatches)) = parse_aco_section(bytes, next_pos, 2) {
            ("ACO v2".to_string(), v2_swatches)
        } else {
            ("ACO v1".to_string(), v1_swatches)
        }
    } else {
        (format!("ACO v{version}"), v1_swatches)
    };
    let count = swatches.len();
    Ok(LibraryInspection {
        format: LibraryFormat::Aco,
        version_label: ver_label,
        library_name: fallback_name.to_string(),
        swatches,
        element_count: count,
        target_apps: &LIBRARY_TARGET_APPS,
    })
}

fn extract_json_swatch(elem: &serde_json::Value, idx: usize) -> Option<SwatchColor> {
    let name = elem.get("name").and_then(serde_json::Value::as_str).unwrap_or("").trim();
    let label = if name.is_empty() { format!("Library Swatch {}", idx + 1) } else { name.to_string() };

    // Format A: `{"value": {"r": 45, "g": 129, "b": 255}}` or `{"color": {"r": ..., "g": ..., "b": ...}}`
    for key in ["value", "color", "rgb"] {
        if let Some(obj) = elem.get(key)
            && let (Some(r), Some(g), Some(b)) = (
                obj.get("r").and_then(serde_json::Value::as_f64),
                obj.get("g").and_then(serde_json::Value::as_f64),
                obj.get("b").and_then(serde_json::Value::as_f64),
            )
        {
            let scale = if r <= 1.0 && g <= 1.0 && b <= 1.0 && (r.fract() != 0.0 || g.fract() != 0.0 || b.fract() != 0.0) { 255.0 } else { 1.0 };
            return Some(SwatchColor {
                name: label,
                rgb: [
                    (r * scale).clamp(0.0, 255.0).round() as u8,
                    (g * scale).clamp(0.0, 255.0).round() as u8,
                    (b * scale).clamp(0.0, 255.0).round() as u8,
                ],
                model: "RGB",
            });
        }
    }

    // Format B: Creative Cloud Library `representations` array with `color#data`
    if let Some(reps) = elem.get("representations").and_then(serde_json::Value::as_array) {
        for rep in reps {
            if let Some(data) = rep.get("color#data").or_else(|| rep.get("data"))
                && let Some(val) = data.get("value")
                && let (Some(r), Some(g), Some(b)) = (
                    val.get("r").and_then(serde_json::Value::as_f64),
                    val.get("g").and_then(serde_json::Value::as_f64),
                    val.get("b").and_then(serde_json::Value::as_f64),
                )
            {
                return Some(SwatchColor {
                    name: label,
                    rgb: [r.clamp(0.0, 255.0).round() as u8, g.clamp(0.0, 255.0).round() as u8, b.clamp(0.0, 255.0).round() as u8],
                    model: "RGB",
                });
            }
        }
    }

    // Format C: `"hex": "#RRGGBB"`
    if let Some(hex) = elem.get("hex").and_then(serde_json::Value::as_str) {
        let h = hex.trim().trim_start_matches('#');
        if h.len() == 6
            && let (Ok(r), Ok(g), Ok(b)) = (u8::from_str_radix(&h[..2], 16), u8::from_str_radix(&h[2..4], 16), u8::from_str_radix(&h[4..6], 16))
        {
            return Some(SwatchColor { name: label, rgb: [r, g, b], model: "RGB" });
        }
    }
    None
}

fn parse_cclibs_json(json_bytes: &[u8], fallback_name: &str, archive_entries: Option<usize>) -> Result<LibraryInspection, String> {
    let doc: serde_json::Value = serde_json::from_slice(json_bytes).map_err(|e| format!("invalid .cclibs JSON manifest: {e}"))?;
    let name = doc.get("name").and_then(serde_json::Value::as_str).filter(|s| !s.trim().is_empty()).unwrap_or(fallback_name).to_string();
    let version = doc.get("version").and_then(serde_json::Value::as_str).unwrap_or("1.0");
    let mut swatches = Vec::new();
    let mut elem_total = archive_entries.unwrap_or(0);

    for array_key in ["elements", "children", "swatches"] {
        if let Some(arr) = doc.get(array_key).and_then(serde_json::Value::as_array) {
            elem_total = elem_total.max(arr.len());
            for (idx, item) in arr.iter().take(MAX_SWATCHES).enumerate() {
                if let Some(swatch) = extract_json_swatch(item, idx) {
                    swatches.push(swatch);
                }
            }
        }
    }
    if elem_total == 0 {
        elem_total = swatches.len();
    }

    Ok(LibraryInspection {
        format: LibraryFormat::CcLibs,
        version_label: format!("CC Library {version}"),
        library_name: name,
        swatches,
        element_count: elem_total,
        target_apps: &LIBRARY_TARGET_APPS,
    })
}

/// Inspect a `.cclibs` / `.cclib` Creative Cloud Library (either a ZIP container or JSON manifest).
pub fn parse_cclibs(bytes: &[u8], fallback_name: &str) -> Result<LibraryInspection, String> {
    if bytes.len() > MAX_LIBRARY_BYTES {
        return Err("library file exceeds the 16 MB inspection limit".into());
    }
    let trimmed_start = bytes.iter().position(|b| !b.is_ascii_whitespace()).and_then(|i| bytes.get(i..)).unwrap_or(bytes);
    if trimmed_start.first() == Some(&b'{') {
        return parse_cclibs_json(trimmed_start, fallback_name, None);
    }

    // ZIP container (`PK\x03\x04` local file entries). Walk local headers safely without `unsafe` or new crates.
    if bytes.get(..4) == Some(b"PK\x03\x04") {
        let mut pos = 0usize;
        let mut entry_count = 0usize;
        let mut stored_manifest: Option<&[u8]> = None;

        while pos.saturating_add(30) <= bytes.len() && entry_count < MAX_SWATCHES {
            if bytes.get(pos..pos + 4) != Some(b"PK\x03\x04") {
                break;
            }
            let flags = read_u16_le(bytes, pos + 6).unwrap_or(0);
            let compression = read_u16_le(bytes, pos + 8).unwrap_or(0);
            let comp_size = usize::try_from(read_u32_le(bytes, pos + 18).unwrap_or(0)).unwrap_or(0);
            let name_len = usize::from(read_u16_le(bytes, pos + 26).unwrap_or(0));
            let extra_len = usize::from(read_u16_le(bytes, pos + 28).unwrap_or(0));
            let name_start = pos.saturating_add(30);
            let data_start = name_start.saturating_add(name_len).saturating_add(extra_len);
            let data_end = data_start.saturating_add(comp_size);
            if data_end > bytes.len() {
                break;
            }
            entry_count = entry_count.saturating_add(1);
            if let Some(raw_name) = bytes.get(name_start..name_start.saturating_add(name_len)) {
                let entry_name = String::from_utf8_lossy(raw_name);
                if (entry_name == "manifest" || entry_name.ends_with("manifest.json")) && compression == 0 && (flags & 0x08) == 0 {
                    stored_manifest = bytes.get(data_start..data_end);
                }
            }
            if (flags & 0x08) != 0 && comp_size == 0 {
                // Data descriptor follows compressed stream; stop linear header walk and count central directory if present.
                break;
            }
            pos = data_end;
        }

        if let Some(manifest_bytes) = stored_manifest
            && let Ok(inspected) = parse_cclibs_json(manifest_bytes, fallback_name, Some(entry_count))
        {
            return Ok(inspected);
        }

        return Ok(LibraryInspection {
            format: LibraryFormat::CcLibs,
            version_label: "CC Library (ZIP)".into(),
            library_name: fallback_name.to_string(),
            swatches: Vec::new(),
            element_count: entry_count.max(1),
            target_apps: &LIBRARY_TARGET_APPS,
        });
    }

    Err("unrecognized .cclibs container (expected JSON manifest or PKZIP header)".into())
}

/// Inspect a `.cclibs`, `.ase`, or `.aco` payload by magic header or filename extension.
pub fn inspect_library_bytes(file_name: &str, bytes: &[u8]) -> Result<LibraryInspection, String> {
    if bytes.is_empty() {
        return Err("file is empty".into());
    }
    let stem = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name).rsplit_once('.').map_or(file_name, |(s, _)| s);
    let fallback = if stem.trim().is_empty() { "Creative Library" } else { stem.trim() };
    let ext = file_name.rsplit_once('.').map(|(_, e)| e.trim().to_ascii_lowercase()).unwrap_or_default();

    if bytes.get(..4) == Some(b"ASEF") || ext == "ase" {
        parse_ase(bytes, fallback)
    } else if ext == "aco" {
        parse_aco(bytes, fallback)
    } else if matches!(ext.as_str(), "cclibs" | "cclib") || bytes.get(..4) == Some(b"PK\x03\x04") || bytes.first() == Some(&b'{') {
        parse_cclibs(bytes, fallback)
    } else {
        Err("unsupported library format (expected .cclibs, .ase, or .aco)".into())
    }
}

fn push_utf16be_null_terminated(out: &mut Vec<u8>, text: &str) {
    let units: Vec<u16> = text.encode_utf16().chain(std::iter::once(0u16)).collect();
    let count = u16::try_from(units.len()).unwrap_or(u16::MAX);
    out.extend_from_slice(&count.to_be_bytes());
    for unit in units.into_iter().take(usize::from(count)) {
        out.extend_from_slice(&unit.to_be_bytes());
    }
}

/// Build a valid binary Adobe Swatch Exchange (`.ase` v1.0) file from named RGB swatches.
pub fn build_starter_ase(palette_name: &str, swatches: &[(&str, [u8; 3])]) -> Vec<u8> {
    let clean_title = if palette_name.trim().is_empty() { "ArtCraft Studio Swatches" } else { palette_name.trim() };
    let block_count = u32::try_from(swatches.len().saturating_add(2)).unwrap_or(2);
    let mut out = Vec::with_capacity(64 + swatches.len() * 48);
    out.extend_from_slice(b"ASEF");
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&block_count.to_be_bytes());

    // Group start block (0xC001)
    let mut group_body = Vec::new();
    push_utf16be_null_terminated(&mut group_body, clean_title);
    out.extend_from_slice(&0xC001u16.to_be_bytes());
    out.extend_from_slice(&(u32::try_from(group_body.len()).unwrap_or(0)).to_be_bytes());
    out.extend_from_slice(&group_body);

    // Color entry blocks (0x0001)
    for (name, [r, g, b]) in swatches {
        let mut body = Vec::new();
        push_utf16be_null_terminated(&mut body, name);
        body.extend_from_slice(b"RGB ");
        body.extend_from_slice(&(f32::from(*r) / 255.0).to_bits().to_be_bytes());
        body.extend_from_slice(&(f32::from(*g) / 255.0).to_bits().to_be_bytes());
        body.extend_from_slice(&(f32::from(*b) / 255.0).to_bits().to_be_bytes());
        // Color type: 2 = Normal (process)
        body.extend_from_slice(&2u16.to_be_bytes());

        out.extend_from_slice(&0x0001u16.to_be_bytes());
        out.extend_from_slice(&(u32::try_from(body.len()).unwrap_or(0)).to_be_bytes());
        out.extend_from_slice(&body);
    }

    // Group end block (0xC002)
    out.extend_from_slice(&0xC002u16.to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());
    out
}

/// Build a valid Creative Library (`.cclibs`) JSON manifest package with named RGB color elements.
pub fn build_starter_cclibs(library_name: &str, swatches: &[(&str, [u8; 3])]) -> Vec<u8> {
    let clean_title = if library_name.trim().is_empty() { "ArtCraft Studio Library" } else { library_name.trim() };
    let elements: Vec<serde_json::Value> = swatches
        .iter()
        .enumerate()
        .map(|(idx, (name, [r, g, b]))| {
            serde_json::json!({
                "id": format!("swatch-{}", idx + 1),
                "name": name,
                "type": "application/vnd.adobe.element.color+dcx",
                "hex": format!("#{:02X}{:02X}{:02X}", r, g, b),
                "representations": [{
                    "type": "application/vnd.adobe.color+json",
                    "color#data": {
                        "mode": "RGB",
                        "value": { "r": r, "g": g, "b": b }
                    }
                }]
            })
        })
        .collect();

    let manifest = serde_json::json!({
        "version": "1.0",
        "type": "application/vnd.adobe.library+json",
        "name": clean_title,
        "elements": elements,
    });
    serde_json::to_vec_pretty(&manifest).unwrap_or_default()
}

/// Extension, plugin, and MCP automation profile for one Craft Suite application.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SuitePluginSpec {
    pub app_id: &'static str,
    pub formats: &'static [&'static str],
    pub runtime: &'static str,
    pub mcp_enabled: bool,
    pub summary: &'static str,
}

/// Plugin, preset, and MCP extension capabilities across the Craft App Suite.
pub const SUITE_PLUGINS: [SuitePluginSpec; 13] = [
    SuitePluginSpec {
        app_id: "photocraft",
        formats: &[".8bf", ".atn", ".abr", ".asl", "WASM", "UXP / MCP"],
        runtime: "Cross-Platform Zero-JIT C++ .8bf Plug-in VM (x86_64/AArch64 PE32+ & Mach-O) + WASM + MCP",
        mcp_enabled: true,
        summary: "Runs compiled C++ Photoshop .8bf filters (PE32+ & Mach-O on iOS, Android, macOS, Linux, Windows, Web), .atn/.abr/.asl, WASM filters, and MCP.",
    },
    SuitePluginSpec {
        app_id: "vectorcraft",
        formats: &[".aip", ".svg", ".ase", "WASM", "MCP"],
        runtime: "Cross-Platform Zero-JIT C++ .aip Plug-in VM (x86_64/AArch64 PE32+ & Mach-O) + WASM + MCP",
        mcp_enabled: true,
        summary: "Runs compiled C++ Illustrator .aip SweetPea/PICA plug-ins (PE32+ & Mach-O on iOS, Android, macOS, Linux, Windows, Web), WASM path effects, and MCP.",
    },
    SuitePluginSpec {
        app_id: "filmcraft",
        formats: &[".prm", ".aex", ".drfx", ".ograf", ".cube", ".3dl", "WASM / MCP"],
        runtime: "Cross-Platform Zero-JIT C++ .prm/.aex Plug-in VM (x86_64/AArch64 PE32+ & Mach-O) + LUT/Template + MCP",
        mcp_enabled: true,
        summary: "Runs compiled C++ Premiere .prm & After Effects .aex effects (PE32+ & Mach-O on iOS, Android, macOS, Linux, Windows, Web), .drfx/.ograf, 3D LUTs, and MCP.",
    },
    SuitePluginSpec {
        app_id: "effectcraft",
        formats: &[".aex", ".prm", ".drfx", ".ograf", "JS Expr", "WASM", "MCP"],
        runtime: "Cross-Platform Zero-JIT C++ .aex/.prm Plug-in VM (x86_64/AArch64 PE32+ & Mach-O) + Compositor + MCP",
        mcp_enabled: true,
        summary: "Runs compiled C++ After Effects .aex & Premiere .prm effects (PE32+ & Mach-O on iOS, Android, macOS, Linux, Windows, Web), .drfx/.ograf, expressions, WASM, and MCP.",
    },
    SuitePluginSpec {
        app_id: "designcraft",
        formats: &[".idml", ".icml", ".cclibs", "WASM", "MCP"],
        runtime: "Layout Automation & JSON/MCP Control Channel",
        mcp_enabled: true,
        summary: "Editorial interchange (.idml / .icml), swatch libraries, typographic layout scripts, and full MCP control.",
    },
    SuitePluginSpec {
        app_id: "lightcraft",
        formats: &[".xmp", ".dng", ".cube", "MCP"],
        runtime: "Non-Destructive Raw Preset & Profile Pipeline",
        mcp_enabled: true,
        summary: "Camera Raw develop presets (.xmp), dual-illuminant DNG profiles, creative LUTs, and catalog automation.",
    },
    SuitePluginSpec {
        app_id: "soundcraft",
        formats: &[".clap", "DSP Rack", "MCP"],
        runtime: "Native CLAP Audio Plug-in Host + MCP",
        mcp_enabled: true,
        summary: "Loads native .clap instruments and effects alongside built-in EQ, dynamics, synths, and MCP mix automation.",
    },
    SuitePluginSpec {
        app_id: "anicraft",
        formats: &[".xfl", "Lottie", "JSON-lines", "MCP"],
        runtime: "Timeline Command Engine + MCP Server",
        mcp_enabled: true,
        summary: "Symbol libraries (.xfl), Lottie animation exporters, sprite-sheet pipelines, and MCP timeline commands.",
    },
    SuitePluginSpec {
        app_id: "figcraft",
        formats: &[".figcraft", "Tokens", "WASM", "MCP"],
        runtime: "UI Variable & Codegen Handoff Engine",
        mcp_enabled: true,
        summary: "Design token variables, CSS/React/SwiftUI/Compose code generation, component libraries, and MCP inspection.",
    },
    SuitePluginSpec {
        app_id: "texturecraft",
        formats: &[".glb", ".usdz", "Smart Mat", "MCP"],
        runtime: "3D PBR Baker & Shader Brush Pipeline",
        mcp_enabled: true,
        summary: "Smart materials, mesh-map bakers, procedural PBR brush packs, and glTF/USDZ material export.",
    },
    SuitePluginSpec {
        app_id: "materialcraft",
        formats: &[".sbs", ".sbsar", ".mcmat", "MCP"],
        runtime: "Procedural Material Node Graph Engine",
        mcp_enabled: true,
        summary: "Parametric PBR node graphs (.mcmat / .sbs / .sbsar), custom filter nodes, and headless texture baking.",
    },
    SuitePluginSpec {
        app_id: "cadcraft",
        formats: &[".dxf", ".dwg", "Blocks", "MCP"],
        runtime: "Command-Line Drafting & MCP Server",
        mcp_enabled: true,
        summary: "Reusable CAD block libraries, paper-space plot styles, direct CLI command scripts, and MCP drafting.",
    },
    SuitePluginSpec {
        app_id: "craft-anything",
        formats: &["UXP / MCP", "AgentEnv", "CLI"],
        runtime: "UXP & Binary Inspection + MCP Toolset Host",
        mcp_enabled: true,
        summary: "Extension & bundle inspection, clean-room AgentEnv reconstruction workflows, and MCP toolsets.",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ase_starter_builder_round_trips_through_parser() {
        let raw = build_starter_ase("Brand Swatches", &STARTER_STUDIO_SWATCHES);
        let parsed = parse_ase(&raw, "fallback").expect("valid starter ASE");
        assert_eq!(parsed.format, LibraryFormat::Ase);
        assert_eq!(parsed.version_label, "ASE 1.0");
        assert_eq!(parsed.library_name, "Brand Swatches");
        assert_eq!(parsed.swatches.len(), STARTER_STUDIO_SWATCHES.len());
        assert_eq!(parsed.swatches[0].name, "ArtCraft Accent");
        assert_eq!(parsed.swatches[0].rgb, [0x2D, 0x81, 0xFF]);
        assert_eq!(parsed.swatches[0].hex(), "#2D81FF");
    }

    #[test]
    fn cclibs_starter_builder_round_trips_through_inspector() {
        let raw = build_starter_cclibs("Editorial Palette", &STARTER_STUDIO_SWATCHES[..4]);
        let parsed = inspect_library_bytes("editorial.cclibs", &raw).expect("valid starter cclibs");
        assert_eq!(parsed.format, LibraryFormat::CcLibs);
        assert_eq!(parsed.library_name, "Editorial Palette");
        assert_eq!(parsed.swatches.len(), 4);
        assert_eq!(parsed.swatches[2].name, "VectorCraft Vermilion");
        assert_eq!(parsed.swatches[2].rgb, [0xE8, 0x57, 0x3F]);
    }

    #[test]
    fn aco_v2_binary_parses_rgb_and_utf16_names() {
        // Build a minimal valid ACO v2 with 1 RGB swatch named "Sky"
        let mut aco = Vec::new();
        aco.extend_from_slice(&2u16.to_be_bytes()); // version 2
        aco.extend_from_slice(&1u16.to_be_bytes()); // 1 swatch
        aco.extend_from_slice(&0u16.to_be_bytes()); // color space 0 = RGB
        aco.extend_from_slice(&0x2D81u16.to_be_bytes()); // R
        aco.extend_from_slice(&0x81FFu16.to_be_bytes()); // G
        aco.extend_from_slice(&0xFF00u16.to_be_bytes()); // B
        aco.extend_from_slice(&0u16.to_be_bytes()); // Z
        aco.extend_from_slice(&0u16.to_be_bytes()); // v2 constant 0
        let utf16: Vec<u16> = "Sky".encode_utf16().chain(std::iter::once(0)).collect();
        aco.extend_from_slice(&(utf16.len() as u16).to_be_bytes());
        for u in utf16 {
            aco.extend_from_slice(&u.to_be_bytes());
        }

        let parsed = inspect_library_bytes("colors.aco", &aco).expect("valid ACO v2");
        assert_eq!(parsed.format, LibraryFormat::Aco);
        assert_eq!(parsed.version_label, "ACO v2");
        assert_eq!(parsed.swatches.len(), 1);
        assert_eq!(parsed.swatches[0].name, "Sky");
        assert_eq!(parsed.swatches[0].rgb, [0x2D, 0x81, 0xFF]);
    }

    #[test]
    fn zip_cclibs_with_stored_manifest_extracts_metadata() {
        let manifest = build_starter_cclibs("Zipped CC Lib", &[("Cyan", [0x00, 0xBC, 0xD4])]);
        let name = b"manifest";
        let mut zip = Vec::new();
        zip.extend_from_slice(b"PK\x03\x04");
        zip.extend_from_slice(&20u16.to_le_bytes()); // version needed
        zip.extend_from_slice(&0u16.to_le_bytes()); // flags
        zip.extend_from_slice(&0u16.to_le_bytes()); // compression 0 = stored
        zip.extend_from_slice(&0u16.to_le_bytes()); // mod time
        zip.extend_from_slice(&0u16.to_le_bytes()); // mod date
        zip.extend_from_slice(&0u32.to_le_bytes()); // crc32
        zip.extend_from_slice(&(manifest.len() as u32).to_le_bytes()); // comp size
        zip.extend_from_slice(&(manifest.len() as u32).to_le_bytes()); // uncomp size
        zip.extend_from_slice(&(name.len() as u16).to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes()); // extra len
        zip.extend_from_slice(name);
        zip.extend_from_slice(&manifest);

        let parsed = inspect_library_bytes("archive.cclibs", &zip).expect("valid zip cclibs");
        assert_eq!(parsed.format, LibraryFormat::CcLibs);
        assert_eq!(parsed.library_name, "Zipped CC Lib");
        assert_eq!(parsed.swatches.len(), 1);
        assert_eq!(parsed.swatches[0].rgb, [0x00, 0xBC, 0xD4]);
    }

    #[test]
    fn hostile_and_truncated_library_inputs_never_panic() {
        for bad in [&[][..], b"ASEF\x00", b"\x00\x02\x00\x05\xff", b"PK\x03\x04\xff\xff", b"{not json"] {
            assert!(inspect_library_bytes("broken.ase", bad).is_err() || inspect_library_bytes("broken.cclibs", bad).is_err());
        }
    }

    #[test]
    fn every_suite_plugin_entry_maps_to_a_catalog_app() {
        for spec in &SUITE_PLUGINS {
            assert!(crate::CraftApp::by_id(spec.app_id).is_some(), "unknown app_id {}", spec.app_id);
            assert!(!spec.formats.is_empty(), "{}", spec.app_id);
            assert!(!spec.summary.is_empty(), "{}", spec.app_id);
        }
    }

    #[test]
    fn suite_plugins_advertise_zero_jit_cpp_binary_plugin_formats_and_vm() {
        let find = |id: &str| SUITE_PLUGINS.iter().find(|s| s.app_id == id).unwrap_or_else(|| panic!("missing {id}"));

        let pc = find("photocraft");
        assert!(pc.formats.contains(&".8bf"));
        assert!(pc.runtime.contains("Zero-JIT") && pc.runtime.contains("x86_64/AArch64"));
        assert!(pc.summary.contains("iOS, Android, macOS, Linux, Windows, Web"));

        let vc = find("vectorcraft");
        assert!(vc.formats.contains(&".aip"));
        assert!(vc.runtime.contains("Zero-JIT") && vc.runtime.contains("PE32+ & Mach-O"));
        assert!(vc.summary.contains(".aip"));

        let fc = find("filmcraft");
        assert!(fc.formats.contains(&".prm") && fc.formats.contains(&".aex"));
        assert!(fc.runtime.contains("Zero-JIT"));

        let ec = find("effectcraft");
        assert!(ec.formats.contains(&".aex") && ec.formats.contains(&".prm"));
        assert!(ec.runtime.contains("Zero-JIT"));
    }
}
