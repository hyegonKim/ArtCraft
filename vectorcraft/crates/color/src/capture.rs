//! Adobe Capture (`FFC_capture`) Perceptual OKLab Color Theme Extractor.
//!
//! Extracts harmonious 5-swatch palettes in Björn Ottosson's perceptual OKLab / OKLCh color space
//! across the 5 Capture moods:
//! - [`CaptureMood::Colorful`] (`화려함`)
//! - [`CaptureMood::Bright`] (`밝음`)
//! - [`CaptureMood::Muted`] (`차분함`)
//! - [`CaptureMood::Deep`] (`깊음`)
//! - [`CaptureMood::Dark`] (`어두움`)

use serde::{Deserialize, Serialize};

use crate::Color;

/// Number of swatches in an extracted Capture color theme.
pub const CAPTURE_THEME_SWATCHES: usize = 5;

/// Mood profile governing perceptual lightness and chroma shaping in OKLab space.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CaptureMood {
    #[default]
    Colorful,
    Bright,
    Muted,
    Deep,
    Dark,
}

impl CaptureMood {
    pub const ALL: [Self; 5] = [Self::Colorful, Self::Bright, Self::Muted, Self::Deep, Self::Dark];

    pub fn id(self) -> &'static str {
        match self {
            Self::Colorful => "colorful",
            Self::Bright => "bright",
            Self::Muted => "muted",
            Self::Deep => "deep",
            Self::Dark => "dark",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Colorful => "Colorful",
            Self::Bright => "Bright",
            Self::Muted => "Muted",
            Self::Deep => "Deep",
            Self::Dark => "Dark",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "colorful" | "화려함" => Some(Self::Colorful),
            "bright" | "밝음" => Some(Self::Bright),
            "muted" | "차분함" => Some(Self::Muted),
            "deep" | "깊음" => Some(Self::Deep),
            "dark" | "어두움" => Some(Self::Dark),
            _ => None,
        }
    }
}

/// Perceptual OKLab coordinates (`L` in `0..=1`, `a` and `b` roughly `-0.4..=0.4`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OkLab {
    pub l: f32,
    pub a: f32,
    pub b: f32,
}

impl OkLab {
    #[inline]
    pub fn chroma(self) -> f32 {
        (self.a * self.a + self.b * self.b).sqrt()
    }

    #[inline]
    pub fn hue_rad(self) -> f32 {
        self.b.atan2(self.a)
    }

    #[inline]
    pub fn dist_sq(self, other: Self) -> f32 {
        let dl = self.l - other.l;
        let da = self.a - other.a;
        let db = self.b - other.b;
        dl * dl + da * da + db * db
    }

    pub fn from_lch(l: f32, c: f32, h_rad: f32) -> Self {
        let (sin_h, cos_h) = h_rad.sin_cos();
        Self { l, a: c * cos_h, b: c * sin_h }
    }
}

#[inline]
fn srgb_to_linear(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

#[inline]
fn linear_to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

/// Converts display sRGB `[r, g, b]` (`0..=1`) into perceptual [`OkLab`].
pub fn srgb_to_oklab([r, g, b]: [f32; 3]) -> OkLab {
    let lr = srgb_to_linear(r);
    let lg = srgb_to_linear(g);
    let lb = srgb_to_linear(b);

    let l = (0.412_221_46 * lr + 0.536_332_55 * lg + 0.051_445_995 * lb).max(0.0).cbrt();
    let m = (0.211_903_5 * lr + 0.680_699_5 * lg + 0.107_396_96 * lb).max(0.0).cbrt();
    let s = (0.088_302_46 * lr + 0.281_718_85 * lg + 0.629_978_7 * lb).max(0.0).cbrt();

    OkLab {
        l: (0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s).clamp(0.0, 1.0),
        a: 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        b: 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    }
}

fn oklab_to_linear_rgb_unclamped(lab: OkLab) -> [f32; 3] {
    let l_ = lab.l + 0.396_337_78 * lab.a + 0.215_803_76 * lab.b;
    let m_ = lab.l - 0.105_561_346 * lab.a - 0.063_854_17 * lab.b;
    let s_ = lab.l - 0.089_484_18 * lab.a - 1.291_485_5 * lab.b;

    let l = l_ * l_ * l_;
    let m = m_ * m_ * m_;
    let s = s_ * s_ * s_;

    [
        4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
        -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
        -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
    ]
}

/// Converts [`OkLab`] back to display sRGB `[r, g, b]` (`0..=1`), preserving hue via binary chroma
/// gamut reduction when out of sRGB gamut.
pub fn oklab_to_srgb(lab: OkLab) -> [f32; 3] {
    let l = lab.l.clamp(0.0, 1.0);
    let in_gamut = |lin: [f32; 3]| lin.iter().all(|&v| (-1e-4..=1.0001).contains(&v));
    let raw = oklab_to_linear_rgb_unclamped(OkLab { l, ..lab });
    if in_gamut(raw) {
        return raw.map(linear_to_srgb);
    }
    let hue = lab.hue_rad();
    let mut lo = 0.0f32;
    let mut hi = lab.chroma();
    for _ in 0..12 {
        let mid = 0.5 * (lo + hi);
        if in_gamut(oklab_to_linear_rgb_unclamped(OkLab::from_lch(l, mid, hue))) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    oklab_to_linear_rgb_unclamped(OkLab::from_lch(l, lo, hue)).map(linear_to_srgb)
}

/// Shapes a candidate [`OkLab`] swatch to conform to the target [`CaptureMood`] envelope.
pub fn shape_for_mood(lab: OkLab, mood: CaptureMood) -> OkLab {
    let c = lab.chroma();
    let h = lab.hue_rad();
    let (new_l, new_c) = match mood {
        CaptureMood::Colorful => {
            let l = lab.l.clamp(0.38, 0.84);
            let boosted = if c < 0.025 { c * 1.15 } else { (c * 1.28 + 0.02).clamp(0.07, 0.30) };
            (l, boosted)
        }
        CaptureMood::Bright => {
            let l = (0.68 + 0.24 * lab.l).clamp(0.68, 0.93);
            let bright_c = if c < 0.02 { c } else { (c * 1.12 + 0.015).clamp(0.05, 0.22) };
            (l, bright_c)
        }
        CaptureMood::Muted => {
            let l = (0.46 + 0.36 * lab.l).clamp(0.45, 0.82);
            let muted_c = (c * 0.46).min(0.085);
            (l, muted_c)
        }
        CaptureMood::Deep => {
            let l = (0.24 + 0.32 * lab.l).clamp(0.24, 0.56);
            let deep_c = if c < 0.02 { c * 0.8 } else { (c * 1.22 + 0.02).clamp(0.08, 0.26) };
            (l, deep_c)
        }
        CaptureMood::Dark => {
            let l = (0.14 + 0.26 * lab.l).clamp(0.14, 0.40);
            let dark_c = (c * 0.85).min(0.16);
            (l, dark_c)
        }
    };
    OkLab::from_lch(new_l, new_c, h)
}

#[derive(Clone, Copy)]
struct WeightedLab {
    lab: OkLab,
    weight: f32,
}

fn bin_rgba_samples(rgba_samples: &[[u8; 4]]) -> Vec<WeightedLab> {
    // 12x12x12 quantized RGB bins (1,728 max buckets) keep clustering deterministic and O(1) after binning.
    const SIDE: usize = 12;
    const BUCKETS: usize = SIDE * SIDE * SIDE;
    let mut sums = vec![[0.0f64; 3]; BUCKETS];
    let mut weights = vec![0.0f64; BUCKETS];

    for &[r, g, b, a] in rgba_samples {
        if a < 16 {
            continue;
        }
        let w = f64::from(a) / 255.0;
        let bi = ((usize::from(r) * SIDE) / 256).min(SIDE - 1);
        let bj = ((usize::from(g) * SIDE) / 256).min(SIDE - 1);
        let bk = ((usize::from(b) * SIDE) / 256).min(SIDE - 1);
        let idx = (bi * SIDE + bj) * SIDE + bk;
        if let (Some(s), Some(wt)) = (sums.get_mut(idx), weights.get_mut(idx)) {
            let lab = srgb_to_oklab([f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0]);
            s[0] += f64::from(lab.l) * w;
            s[1] += f64::from(lab.a) * w;
            s[2] += f64::from(lab.b) * w;
            *wt += w;
        }
    }

    let mut out = Vec::new();
    for (s, &w) in sums.iter().zip(&weights) {
        if w > 1e-5 {
            out.push(WeightedLab { lab: OkLab { l: (s[0] / w) as f32, a: (s[1] / w) as f32, b: (s[2] / w) as f32 }, weight: w as f32 });
        }
    }
    out
}

fn mood_affinity(lab: OkLab, mood: CaptureMood) -> f32 {
    let c = lab.chroma();
    match mood {
        CaptureMood::Colorful => 0.4 + 2.8 * c + 0.3 * (1.0 - (lab.l - 0.62).abs()),
        CaptureMood::Bright => 0.4 + 1.6 * lab.l + 1.2 * c,
        CaptureMood::Muted => 0.6 + 1.4 * (1.0 - (c * 3.5).min(1.0)) + 0.4 * (1.0 - (lab.l - 0.64).abs()),
        CaptureMood::Deep => 0.4 + 2.0 * c + 1.3 * (1.0 - (lab.l - 0.40).abs() * 1.8).max(0.0),
        CaptureMood::Dark => 0.5 + 1.9 * (1.0 - lab.l) + 0.6 * c,
    }
}

fn cluster_oklab(bins: &[WeightedLab], k: usize, mood: CaptureMood) -> Vec<WeightedLab> {
    if bins.is_empty() {
        return Vec::new();
    }
    let k = k.min(bins.len()).max(1);
    let mut centers = Vec::with_capacity(k);

    // First center: highest mood-weighted bin.
    let mut first_idx = 0usize;
    let mut best_score = -1.0f32;
    for (i, b) in bins.iter().enumerate() {
        let sc = b.weight.sqrt() * mood_affinity(b.lab, mood);
        if sc > best_score {
            best_score = sc;
            first_idx = i;
        }
    }
    if let Some(first) = bins.get(first_idx) {
        centers.push(first.lab);
    }

    // Remaining centers: deterministic farthest-first weighted by mood affinity & population.
    while centers.len() < k {
        let mut next_best = (0usize, -1.0f32);
        for (i, b) in bins.iter().enumerate() {
            let min_d2 = centers.iter().map(|c| c.dist_sq(b.lab)).fold(f32::MAX, f32::min);
            let score = min_d2 * b.weight.powf(0.35) * mood_affinity(b.lab, mood);
            if score > next_best.1 {
                next_best = (i, score);
            }
        }
        if next_best.1 <= 1e-8 {
            break;
        }
        if let Some(chosen) = bins.get(next_best.0) {
            centers.push(chosen.lab);
        } else {
            break;
        }
    }

    // 8 Lloyd refinement iterations in OKLab space.
    let m = centers.len();
    let mut acc = vec![[0.0f64; 3]; m];
    let mut w_sum = vec![0.0f64; m];
    for _ in 0..8 {
        acc.fill([0.0; 3]);
        w_sum.fill(0.0);
        for b in bins {
            let mut nearest = (0usize, f32::MAX);
            for (ci, &c) in centers.iter().enumerate() {
                let d2 = c.dist_sq(b.lab);
                if d2 < nearest.1 {
                    nearest = (ci, d2);
                }
            }
            let w = f64::from(b.weight);
            if let (Some(a), Some(ws)) = (acc.get_mut(nearest.0), w_sum.get_mut(nearest.0)) {
                a[0] += f64::from(b.lab.l) * w;
                a[1] += f64::from(b.lab.a) * w;
                a[2] += f64::from(b.lab.b) * w;
                *ws += w;
            }
        }
        for ci in 0..m {
            if let (Some(c), Some(a), Some(&ws)) = (centers.get_mut(ci), acc.get(ci), w_sum.get(ci))
                && ws > 1e-6
            {
                *c = OkLab { l: (a[0] / ws) as f32, a: (a[1] / ws) as f32, b: (a[2] / ws) as f32 };
            }
        }
    }

    centers.into_iter().enumerate().map(|(i, lab)| WeightedLab { lab, weight: w_sum.get(i).copied().unwrap_or(1.0) as f32 }).collect()
}

fn synthesize_harmonious_completion(seed_labs: &mut Vec<OkLab>, mood: CaptureMood) {
    let base = seed_labs.first().copied().unwrap_or(OkLab { l: 0.62, a: 0.11, b: 0.06 });
    let base_c = base.chroma().max(0.09);
    let base_h = base.hue_rad();
    // Golden-angle / split-complementary harmonic offsets in OKLCh
    let offsets: [(f32, f32, f32); 5] = [(0.0, 1.0, 0.0), (0.62, 0.92, 0.08), (-0.58, 0.88, -0.07), (2.65, 0.95, 0.04), (3.45, 0.78, -0.11)];
    let mut idx = 0usize;
    while seed_labs.len() < CAPTURE_THEME_SWATCHES {
        let (dh, cs, dl) = offsets.get(idx % offsets.len()).copied().unwrap_or((1.1, 0.9, 0.0));
        idx += 1;
        let cand = shape_for_mood(OkLab::from_lch((base.l + dl).clamp(0.12, 0.94), base_c * cs, base_h + dh), mood);
        if seed_labs.iter().all(|existing| existing.dist_sq(cand) > 0.0012) || idx > 10 {
            seed_labs.push(cand);
        }
    }
}

/// Extracts a harmonious 5-swatch [`Color`] theme in OKLab space from straight RGBA8 samples.
pub fn extract_color_theme_rgba(rgba_samples: &[[u8; 4]], mood: CaptureMood) -> [Color; CAPTURE_THEME_SWATCHES] {
    let bins = bin_rgba_samples(rgba_samples);
    extract_from_weighted_bins(&bins, mood)
}

/// Extracts a harmonious 5-swatch [`Color`] theme in OKLab space from existing vector [`Color`]s.
pub fn extract_color_theme_colors(colors: &[Color], mood: CaptureMood) -> [Color; CAPTURE_THEME_SWATCHES] {
    let bins: Vec<WeightedLab> = colors.iter().map(|c| WeightedLab { lab: srgb_to_oklab(c.to_rgb()), weight: 1.0 }).collect();
    extract_from_weighted_bins(&bins, mood)
}

fn extract_from_weighted_bins(bins: &[WeightedLab], mood: CaptureMood) -> [Color; CAPTURE_THEME_SWATCHES] {
    let clusters = cluster_oklab(bins, 8, mood);
    let mut chosen: Vec<OkLab> = Vec::with_capacity(CAPTURE_THEME_SWATCHES);

    // Greedily pick up to 5 shaped clusters maximizing mood fitness + minimum OKLab separation.
    let shaped: Vec<(OkLab, f32)> = clusters.iter().map(|c| (shape_for_mood(c.lab, mood), c.weight.powf(0.3) * mood_affinity(c.lab, mood))).collect();

    for _ in 0..CAPTURE_THEME_SWATCHES {
        let mut best: Option<(OkLab, f32)> = None;
        for &(cand, base_sc) in &shaped {
            if chosen.iter().any(|e| e.dist_sq(cand) < 0.0018) {
                continue;
            }
            let diversity = if chosen.is_empty() { 1.0 } else { chosen.iter().map(|e| e.dist_sq(cand).sqrt()).fold(f32::MAX, f32::min) };
            let score = base_sc * (0.35 + diversity * 2.4);
            if best.is_none_or(|(_, bsc)| score > bsc) {
                best = Some((cand, score));
            }
        }
        match best {
            Some((c, _)) => chosen.push(c),
            None => break,
        }
    }

    synthesize_harmonious_completion(&mut chosen, mood);
    chosen.truncate(CAPTURE_THEME_SWATCHES);
    // Sort swatches smoothly by perceptual hue angle then lightness.
    chosen.sort_by(|a, b| a.hue_rad().total_cmp(&b.hue_rad()).then_with(|| a.l.total_cmp(&b.l)));

    let mut out = [Color::BLACK; CAPTURE_THEME_SWATCHES];
    for (dst, lab) in out.iter_mut().zip(chosen) {
        let [r, g, b] = oklab_to_srgb(lab);
        *dst = Color::rgb(r, g, b);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oklab_roundtrip_is_accurate_across_srgb_gamut() {
        for hex in ["#ff3b30", "#34c759", "#007aff", "#ffcc00", "#af52de", "#ffffff", "#1c1c1e"] {
            let c = Color::from_hex(hex).unwrap();
            let lab = srgb_to_oklab(c.to_rgb());
            let [r, g, b] = oklab_to_srgb(lab);
            let back = Color::rgb(r, g, b);
            assert_eq!(back.to_hex(), hex, "roundtrip mismatch for {hex}");
        }
    }

    #[test]
    fn extract_color_theme_differentiates_all_five_moods() {
        let mut samples = Vec::new();
        for &(r, g, b) in &[(230u8, 62u8, 48u8), (42, 148, 218), (244, 196, 48), (46, 178, 106), (142, 68, 173), (38, 44, 58)] {
            samples.extend(std::iter::repeat_n([r, g, b, 255], 32));
        }

        let bright = extract_color_theme_rgba(&samples, CaptureMood::Bright);
        let dark = extract_color_theme_rgba(&samples, CaptureMood::Dark);
        let colorful = extract_color_theme_rgba(&samples, CaptureMood::Colorful);
        let muted = extract_color_theme_rgba(&samples, CaptureMood::Muted);
        let deep = extract_color_theme_rgba(&samples, CaptureMood::Deep);

        let mean_l = |pal: &[Color; 5]| -> f32 { pal.iter().map(|c| srgb_to_oklab(c.to_rgb()).l).sum::<f32>() / 5.0 };
        let mean_c = |pal: &[Color; 5]| -> f32 { pal.iter().map(|c| srgb_to_oklab(c.to_rgb()).chroma()).sum::<f32>() / 5.0 };

        assert!(mean_l(&bright) > mean_l(&deep) && mean_l(&deep) > mean_l(&dark));
        assert!(mean_c(&colorful) > mean_c(&muted) * 1.4);
    }
}
