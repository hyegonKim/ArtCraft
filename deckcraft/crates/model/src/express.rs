//! Express Suite models and algorithms for DeckCraft:
//! - [`BrandKit`]: primary/secondary/accent hex colors, heading/body font families, optional logo media.
//! - [`SocialChannelPreset`]: smart social aspect-ratio targets with channel-aware safe margins.
//! - [`QrMatrix`] & [`encode_qr_matrix`]: pure-Rust ISO/IEC 18004 QR Code encoder (Byte mode,
//!   GF(256) `0x11D` Reed-Solomon EC, BCH `(15, 5)` format info, 8-mask penalty evaluation) and
//!   vector [`CustomPath`] builder.

use deckcraft_color::Rgba;
use deckcraft_geom::preset::FillMode;
use deckcraft_geom::{Rect, Size};
use serde::{Deserialize, Serialize};

use crate::{CustomPath, Geom, MediaId};

/// Brand identity kit (palette, typography, optional logo media) applied to a slide or entire deck.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BrandKit {
    pub name: String,
    pub primary_hex: String,
    pub secondary_hex: String,
    pub accent_hex: String,
    pub heading_font: String,
    pub body_font: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo_asset: Option<MediaId>,
}

impl Default for BrandKit {
    fn default() -> Self {
        Self {
            name: "Studio Indigo".into(),
            primary_hex: "#0F172A".into(),
            secondary_hex: "#F8FAFC".into(),
            accent_hex: "#4F46E5".into(),
            heading_font: "Montserrat".into(),
            body_font: "Inter".into(),
            logo_asset: None,
        }
    }
}

impl BrandKit {
    pub fn parse_hex_rgb(hex_str: &str, field: &str) -> Result<(Rgba, String), String> {
        let trimmed = hex_str.trim();
        let raw = trimmed.strip_prefix('#').unwrap_or(trimmed);
        if raw.len() != 6 || !raw.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("`{field}` must be a 6-digit hex color like `#1E293B` (got `{hex_str}`)"));
        }
        let rgba = Rgba::from_hex(raw).ok_or_else(|| format!("invalid hex color `{hex_str}` for `{field}`"))?;
        Ok((rgba, format!("#{}", raw.to_ascii_uppercase())))
    }

    pub fn validated_colors(&self) -> Result<(Rgba, Rgba, Rgba), String> {
        let (p, _) = Self::parse_hex_rgb(&self.primary_hex, "primaryHex")?;
        let (s, _) = Self::parse_hex_rgb(&self.secondary_hex, "secondaryHex")?;
        let (a, _) = Self::parse_hex_rgb(&self.accent_hex, "accentHex")?;
        if self.heading_font.trim().is_empty() || self.body_font.trim().is_empty() {
            return Err("headingFont and bodyFont must be non-empty font family names".into());
        }
        Ok((p, s, a))
    }
}

pub fn builtin_brand_kits() -> Vec<BrandKit> {
    vec![
        BrandKit::default(),
        BrandKit {
            name: "Editorial Warm".into(),
            primary_hex: "#1C1917".into(),
            secondary_hex: "#FFFBEB".into(),
            accent_hex: "#EA580C".into(),
            heading_font: "Playfair Display".into(),
            body_font: "Source Sans 3".into(),
            logo_asset: None,
        },
        BrandKit {
            name: "Cyber Neon".into(),
            primary_hex: "#F1F5F9".into(),
            secondary_hex: "#090D16".into(),
            accent_hex: "#06B6D4".into(),
            heading_font: "Space Grotesk".into(),
            body_font: "IBM Plex Sans".into(),
            logo_asset: None,
        },
    ]
}

/// Social & presentation smart-resize targets (`0.5 pt = 1 px`, matching DeckCraft's `960 × 540 pt` 1080p convention).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SocialChannelPreset {
    InstagramPost,
    InstagramReel,
    YouTubeThumbnail,
    LinkedInBanner,
    Presentation16x9,
}

impl SocialChannelPreset {
    pub const ALL: [SocialChannelPreset; 5] = [
        SocialChannelPreset::InstagramPost,
        SocialChannelPreset::InstagramReel,
        SocialChannelPreset::YouTubeThumbnail,
        SocialChannelPreset::LinkedInBanner,
        SocialChannelPreset::Presentation16x9,
    ];

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().replace([' ', '_', '-', ':'], "").as_str() {
            "instagrampost" | "igpost" | "square" | "1080x1080" => Some(Self::InstagramPost),
            "instagramreel" | "igreel" | "reel" | "story" | "1080x1920" => Some(Self::InstagramReel),
            "youtubethumbnail" | "ytthumbnail" | "youtube" | "thumbnail" => Some(Self::YouTubeThumbnail),
            "linkedinbanner" | "linkedin" | "1200x627" => Some(Self::LinkedInBanner),
            "presentation16x9" | "presentation169" | "16x9" | "169" | "widescreen" => Some(Self::Presentation16x9),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::InstagramPost => "instagramPost",
            Self::InstagramReel => "instagramReel",
            Self::YouTubeThumbnail => "youtubeThumbnail",
            Self::LinkedInBanner => "linkedinBanner",
            Self::Presentation16x9 => "presentation16x9",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::InstagramPost => "Instagram Post (1080×1080)",
            Self::InstagramReel => "Instagram Reel (1080×1920)",
            Self::YouTubeThumbnail => "YouTube Thumbnail (1920×1080)",
            Self::LinkedInBanner => "LinkedIn Banner (1200×627)",
            Self::Presentation16x9 => "Presentation 16:9 (1920×1080)",
        }
    }

    pub fn pixels(self) -> (u32, u32) {
        match self {
            Self::InstagramPost => (1080, 1080),
            Self::InstagramReel => (1080, 1920),
            Self::YouTubeThumbnail | Self::Presentation16x9 => (1920, 1080),
            Self::LinkedInBanner => (1200, 627),
        }
    }

    pub fn size_points(self) -> Size {
        let (px_w, px_h) = self.pixels();
        Size::new(f64::from(px_w) * 0.5, f64::from(px_h) * 0.5)
    }

    /// Fractional `(horizontal, vertical)` safe-margin ratios protecting social UI chrome.
    pub fn default_safe_margins(self) -> (f64, f64) {
        match self {
            Self::InstagramPost => (0.06, 0.06),
            Self::InstagramReel => (0.07, 0.12),
            Self::YouTubeThumbnail => (0.06, 0.07),
            Self::LinkedInBanner => (0.08, 0.10),
            Self::Presentation16x9 => (0.05, 0.05),
        }
    }

    pub fn safe_rect(self, override_margin: Option<f64>) -> Rect {
        let sz = self.size_points();
        let (mx, my) = match override_margin {
            Some(m) => {
                let c = m.clamp(0.0, 0.25);
                (c, c)
            }
            None => self.default_safe_margins(),
        };
        let pad_x = sz.width * mx;
        let pad_y = sz.height * my;
        Rect::from_origin_size((pad_x, pad_y), ((sz.width - 2.0 * pad_x).max(36.0), (sz.height - 2.0 * pad_y).max(36.0)))
    }
}

// ============================================================================
// Pure-Rust ISO/IEC 18004 QR Code Generator (Versions 1..=5, EC Level L, Byte Mode)
// ============================================================================

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QrMatrix {
    pub version: u8,
    pub size: usize,
    /// Row-major `size * size` dark modules (`true` = dark).
    pub modules: Vec<bool>,
}

impl QrMatrix {
    pub fn get(&self, x: usize, y: usize) -> bool {
        x < self.size && y < self.size && self.modules.get(y * self.size + x).copied().unwrap_or(false)
    }
}

/// Version 1..=5 single-block ISO 18004 Level-L specs: `(version, size, data_codewords, ec_codewords, alignment_second_coord)`.
const QR_VERSIONS_L: [(u8, usize, usize, usize, Option<usize>); 5] =
    [(1, 21, 19, 7, None), (2, 25, 34, 10, Some(18)), (3, 29, 55, 15, Some(22)), (4, 33, 80, 20, Some(26)), (5, 37, 108, 26, Some(30))];

fn gf_mul(mut a: u8, mut b: u8) -> u8 {
    let mut p = 0_u8;
    for _ in 0..8 {
        if (b & 1) != 0 {
            p ^= a;
        }
        let hi = (a & 0x80) != 0;
        a <<= 1;
        if hi {
            a ^= 0x1D; // 0x11D mod 256
        }
        b >>= 1;
    }
    p
}

fn gf_pow(x: u8, power: usize) -> u8 {
    let mut acc = 1_u8;
    for _ in 0..power {
        acc = gf_mul(acc, x);
    }
    acc
}

fn rs_generator_poly(degree: usize) -> Vec<u8> {
    let mut poly = vec![1_u8];
    for i in 0..degree {
        let root = gf_pow(2, i);
        let mut next = vec![0_u8; poly.len() + 1];
        for (j, &c) in poly.iter().enumerate() {
            if let Some(d0) = next.get_mut(j) {
                *d0 ^= c;
            }
            if let Some(d1) = next.get_mut(j + 1) {
                *d1 ^= gf_mul(c, root);
            }
        }
        poly = next;
    }
    // Drop leading 1 so remainder loop shifts cleanly
    poly.into_iter().skip(1).collect()
}

fn rs_encode(data: &[u8], ec_len: usize) -> Vec<u8> {
    let gen_poly = rs_generator_poly(ec_len);
    let mut rem = vec![0_u8; ec_len];
    for &byte in data {
        let factor = byte ^ rem.first().copied().unwrap_or(0);
        rem.rotate_left(1);
        if let Some(last) = rem.last_mut() {
            *last = 0;
        }
        for (r, &g) in rem.iter_mut().zip(&gen_poly) {
            *r ^= gf_mul(g, factor);
        }
    }
    rem
}

fn push_bits(bits: &mut Vec<bool>, val: u32, count: usize) {
    for shift in (0..count).rev() {
        bits.push(((val >> shift) & 1) != 0);
    }
}

fn encode_byte_payload(bytes: &[u8], data_codewords: usize) -> Vec<u8> {
    let mut bits = Vec::with_capacity(data_codewords * 8);
    // Mode indicator: Byte mode = 0100; character count is 8 bits for Versions 1..=9
    push_bits(&mut bits, 0b0100, 4);
    push_bits(&mut bits, bytes.len() as u32, 8);
    for &b in bytes {
        push_bits(&mut bits, u32::from(b), 8);
    }
    let capacity_bits = data_codewords * 8;
    let terminator = (capacity_bits.saturating_sub(bits.len())).min(4);
    push_bits(&mut bits, 0, terminator);
    while !bits.len().is_multiple_of(8) {
        bits.push(false);
    }
    let mut out = Vec::with_capacity(data_codewords);
    for chunk in bits.chunks(8) {
        let mut b = 0_u8;
        for &bit in chunk {
            b = (b << 1) | u8::from(bit);
        }
        out.push(b);
    }
    let pads = [0xEC_u8, 0x11_u8];
    let mut pad_idx = 0_usize;
    while out.len() < data_codewords {
        out.push(pads[pad_idx % 2]);
        pad_idx += 1;
    }
    out
}

#[derive(Clone)]
struct QrGrid {
    size: usize,
    modules: Vec<bool>,
    is_func: Vec<bool>,
}

impl QrGrid {
    fn new(size: usize) -> Self {
        Self { size, modules: vec![false; size * size], is_func: vec![false; size * size] }
    }

    fn set_func(&mut self, x: isize, y: isize, dark: bool) {
        if x >= 0 && y >= 0 {
            let (ux, uy) = (x as usize, y as usize);
            if ux < self.size && uy < self.size {
                let idx = uy * self.size + ux;
                if let (Some(m), Some(f)) = (self.modules.get_mut(idx), self.is_func.get_mut(idx)) {
                    *m = dark;
                    *f = true;
                }
            }
        }
    }

    fn place_finder(&mut self, cx: isize, cy: isize) {
        for dy in -4_isize..=4 {
            for dx in -4_isize..=4 {
                let dist = dx.abs().max(dy.abs());
                let dark = dist != 2 && dist != 4;
                self.set_func(cx + dx, cy + dy, dark);
            }
        }
    }

    fn place_alignment(&mut self, cx: isize, cy: isize) {
        for dy in -2_isize..=2 {
            for dx in -2_isize..=2 {
                let dist = dx.abs().max(dy.abs());
                self.set_func(cx + dx, cy + dy, dist != 1);
            }
        }
    }

    fn reserve_format_areas(&mut self) {
        let n = self.size as isize;
        for i in 0..=8 {
            if i != 6 {
                self.set_func(8, i, false);
                self.set_func(i, 8, false);
            }
        }
        for i in 0..8 {
            self.set_func(n - 1 - i, 8, false);
        }
        for i in 0..7 {
            self.set_func(8, n - 1 - i, false);
        }
    }

    fn write_format_bits(&mut self, mask: u8) {
        // EC Level L = 01, 5-bit data = (0b01 << 3) | mask
        let data = (0b01_u16 << 3) | u16::from(mask & 7);
        let mut rem = data << 10;
        for i in (0..5).rev() {
            if ((rem >> (i + 10)) & 1) != 0 {
                rem ^= 0x537 << i;
            }
        }
        let bits = ((data << 10) | rem) ^ 0x5412;
        let bit = |i: u8| ((bits >> i) & 1) != 0;

        // Top-left copy
        for i in 0..=5 {
            self.set_func(8, isize::from(i), bit(i));
        }
        self.set_func(8, 7, bit(6));
        self.set_func(8, 8, bit(7));
        self.set_func(7, 8, bit(8));
        for i in 9..15 {
            self.set_func(14 - isize::from(i), 8, bit(i));
        }

        // Split top-right & bottom-left copy
        let n = self.size as isize;
        for i in 0..8 {
            self.set_func(n - 1 - isize::from(i), 8, bit(i));
        }
        for i in 8..15 {
            self.set_func(8, n - 15 + isize::from(i), bit(i));
        }
        self.set_func(8, n - 8, true); // Dark module
    }

    fn place_codewords(&mut self, codewords: &[u8]) {
        let mut bit_idx = 0_usize;
        let total_bits = codewords.len() * 8;
        let n = self.size as isize;
        let mut right = n - 1;
        let mut upward = true;
        while right >= 1 {
            if right == 6 {
                right -= 1;
            }
            for step in 0..n {
                let y = if upward { n - 1 - step } else { step };
                for dx in 0..2 {
                    let x = right - dx;
                    let idx = (y as usize) * self.size + (x as usize);
                    if !self.is_func.get(idx).copied().unwrap_or(true) {
                        let dark = if bit_idx < total_bits {
                            let cw = codewords.get(bit_idx / 8).copied().unwrap_or(0);
                            ((cw >> (7 - (bit_idx % 8))) & 1) != 0
                        } else {
                            false
                        };
                        if let Some(m) = self.modules.get_mut(idx) {
                            *m = dark;
                        }
                        bit_idx += 1;
                    }
                }
            }
            upward = !upward;
            right -= 2;
        }
    }

    fn apply_mask(&mut self, mask: u8) {
        for y in 0..self.size {
            for x in 0..self.size {
                let idx = y * self.size + x;
                if self.is_func.get(idx).copied().unwrap_or(true) {
                    continue;
                }
                let invert = match mask {
                    0 => (x + y).is_multiple_of(2),
                    1 => y.is_multiple_of(2),
                    2 => x.is_multiple_of(3),
                    3 => (x + y).is_multiple_of(3),
                    4 => (y / 2 + x / 3).is_multiple_of(2),
                    5 => ((x * y) % 2) + ((x * y) % 3) == 0,
                    6 => (((x * y) % 2) + ((x * y) % 3)).is_multiple_of(2),
                    _ => (((x + y) % 2) + ((x * y) % 3)).is_multiple_of(2),
                };
                if invert && let Some(m) = self.modules.get_mut(idx) {
                    *m = !*m;
                }
            }
        }
    }

    fn penalty_score(&self) -> u32 {
        let mut score = 0_u32;
        let get = |x: usize, y: usize| self.modules.get(y * self.size + x).copied().unwrap_or(false);
        // N1: adjacent run penalty along rows and columns
        for y in 0..self.size {
            let mut run = 1_u32;
            for x in 1..self.size {
                if get(x, y) == get(x - 1, y) {
                    run += 1;
                } else {
                    if run >= 5 {
                        score += 3 + (run - 5);
                    }
                    run = 1;
                }
            }
            if run >= 5 {
                score += 3 + (run - 5);
            }
        }
        for x in 0..self.size {
            let mut run = 1_u32;
            for y in 1..self.size {
                if get(x, y) == get(x, y - 1) {
                    run += 1;
                } else {
                    if run >= 5 {
                        score += 3 + (run - 5);
                    }
                    run = 1;
                }
            }
            if run >= 5 {
                score += 3 + (run - 5);
            }
        }
        // N2: 2x2 solid blocks
        for y in 0..self.size.saturating_sub(1) {
            for x in 0..self.size.saturating_sub(1) {
                let c = get(x, y);
                if get(x + 1, y) == c && get(x, y + 1) == c && get(x + 1, y + 1) == c {
                    score += 3;
                }
            }
        }
        // N4: dark/light balance
        let dark_count = self.modules.iter().filter(|&&b| b).count() as u32;
        let total = (self.size * self.size).max(1) as u32;
        let pct = (dark_count * 100) / total;
        let steps = pct.abs_diff(50) / 5;
        score + steps * 10
    }
}

/// Encode `payload` (1..=106 UTF-8 bytes) into a scannable ISO/IEC 18004 QR Code matrix.
pub fn encode_qr_matrix(payload: &str) -> Result<QrMatrix, String> {
    let bytes = payload.as_bytes();
    if bytes.is_empty() {
        return Err("QR code text must not be empty".into());
    }
    // Byte mode header uses 4 + 8 = 12 bits -> max payload is `data_codewords - 2` bytes
    let &(version, size, data_cw, ec_cw, align_c) = QR_VERSIONS_L
        .iter()
        .find(|&&(_, _, dcw, _, _)| bytes.len() + 2 <= dcw)
        .ok_or_else(|| format!("QR payload too long ({} bytes; maximum 106 UTF-8 bytes)", bytes.len()))?;

    let data = encode_byte_payload(bytes, data_cw);
    let ec = rs_encode(&data, ec_cw);
    let mut full = Vec::with_capacity(data_cw + ec_cw);
    full.extend_from_slice(&data);
    full.extend_from_slice(&ec);

    let mut base = QrGrid::new(size);
    let n = size as isize;
    base.place_finder(3, 3);
    base.place_finder(n - 4, 3);
    base.place_finder(3, n - 4);
    for i in 8..(size - 8) {
        let dark = i.is_multiple_of(2);
        base.set_func(i as isize, 6, dark);
        base.set_func(6, i as isize, dark);
    }
    if let Some(ac) = align_c {
        base.place_alignment(ac as isize, ac as isize);
    }
    base.reserve_format_areas();
    base.set_func(8, n - 8, true);
    base.place_codewords(&full);

    let mut best_mask = 0_u8;
    let mut best_score = u32::MAX;
    let mut best_modules = base.modules.clone();
    for mask in 0..8_u8 {
        let mut candidate = base.clone();
        candidate.apply_mask(mask);
        candidate.write_format_bits(mask);
        let sc = candidate.penalty_score();
        if sc < best_score {
            best_score = sc;
            best_mask = mask;
            best_modules = candidate.modules;
        }
    }
    let _ = best_mask;

    Ok(QrMatrix { version, size, modules: best_modules })
}

/// Convert a [`QrMatrix`] into a crisp horizontal-run-merged vector [`Geom::Custom`] shape.
pub fn qr_custom_geom(matrix: &QrMatrix, quiet_zone: usize) -> Geom {
    let qz = quiet_zone.min(8);
    let total = (matrix.size + qz * 2) as f64;
    let mut d = String::with_capacity(matrix.size * matrix.size * 12);
    for y in 0..matrix.size {
        let mut x = 0_usize;
        while x < matrix.size {
            if !matrix.get(x, y) {
                x += 1;
                continue;
            }
            let x0 = x;
            while x < matrix.size && matrix.get(x, y) {
                x += 1;
            }
            let x1 = x;
            let px0 = (x0 + qz) as f64;
            let px1 = (x1 + qz) as f64;
            let py0 = (y + qz) as f64;
            let py1 = (y + 1 + qz) as f64;
            if !d.is_empty() {
                d.push(' ');
            }
            use std::fmt::Write as _;
            let _ = write!(d, "M {px0} {py0} L {px1} {py0} L {px1} {py1} L {px0} {py1} Z");
        }
    }
    Geom::Custom { paths: vec![CustomPath { w: total, h: total, d, fill: FillMode::Norm, stroke: false }] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_encoder_builds_valid_finder_patterns_and_vector_paths() {
        let qr = encode_qr_matrix("https://getartcraft.com/express").expect("encode qr");
        assert_eq!(qr.version, 2);
        assert_eq!(qr.size, 25);
        // Top-left, top-right, and bottom-left finder centers (3,3), (21,3), (3,21) are dark
        assert!(qr.get(3, 3));
        assert!(qr.get(21, 3));
        assert!(qr.get(3, 21));
        // Separator ring at (7, 0..=7) is light
        for y in 0..=7 {
            assert!(!qr.get(7, y));
        }
        let geom = qr_custom_geom(&qr, 2);
        let Geom::Custom { paths } = geom else { panic!("expected custom geom") };
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].w, 29.0);
        assert!(paths[0].d.contains("M "));
    }

    #[test]
    fn social_presets_and_brand_kit_validation_cover_all_targets() {
        for preset in SocialChannelPreset::ALL {
            assert_eq!(SocialChannelPreset::parse(preset.id()), Some(preset));
            let safe = preset.safe_rect(None);
            let full = preset.size_points();
            assert!(safe.width() > 0.0 && safe.width() < full.width);
            assert!(safe.height() > 0.0 && safe.height() < full.height);
        }
        assert!(BrandKit::default().validated_colors().is_ok());
        assert!(BrandKit { primary_hex: "not-hex".into(), ..Default::default() }.validated_colors().is_err());
    }
}
