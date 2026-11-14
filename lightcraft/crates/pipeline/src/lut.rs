//! 3D LUT profiles: `.cube` files (the plain-text Cube LUT format: `LUT_3D_SIZE`, optional
//! `DOMAIN_MIN` / `DOMAIN_MAX`, then size³ "r g b" lines with red changing fastest), applied to
//! the display-encoded output colour and blended by the profile amount. Profiles are registered
//! here by id (`lut:…`) and found by the finishing stage; LUT profiles render on the CPU.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

/// A 3D colour lookup table.
#[derive(Clone, Debug, PartialEq)]
pub struct Lut3d {
    pub size: usize,
    pub data: Vec<[f32; 3]>,
    pub domain_min: [f32; 3],
    pub domain_max: [f32; 3],
    pub title: Option<String>,
}

impl Lut3d {
    /// Parse a `.cube` file.
    pub fn parse_cube(text: &str) -> Result<Lut3d, String> {
        let (mut size, mut title) = (0usize, None);
        let (mut min, mut max) = ([0.0f32; 3], [1.0f32; 3]);
        let mut data = Vec::new();
        let three = |rest: &str| -> Option<[f32; 3]> {
            let v: Vec<f32> = rest.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            (v.len() == 3).then(|| [v[0], v[1], v[2]])
        };
        for line in text.lines() {
            let l = line.trim();
            if l.is_empty() || l.starts_with('#') {
                continue;
            }
            if let Some(r) = l.strip_prefix("TITLE") {
                title = Some(r.trim().trim_matches('"').to_string());
            } else if let Some(r) = l.strip_prefix("LUT_3D_SIZE") {
                size = r.trim().parse().map_err(|_| format!("bad LUT_3D_SIZE `{}`", r.trim()))?;
                if !(2..=256).contains(&size) {
                    return Err(format!("LUT_3D_SIZE {size} out of range"));
                }
            } else if l.starts_with("LUT_1D_SIZE") {
                return Err("1D LUTs are not supported (only 3D .cube files)".into());
            } else if let Some(r) = l.strip_prefix("DOMAIN_MIN") {
                min = three(r).ok_or("bad DOMAIN_MIN")?;
            } else if let Some(r) = l.strip_prefix("DOMAIN_MAX") {
                max = three(r).ok_or("bad DOMAIN_MAX")?;
            } else if l.starts_with(|c: char| c.is_ascii_alphabetic()) {
                // other keywords (LUT_IN_VIDEO_RANGE…) are ignored
            } else {
                data.push(three(l).ok_or_else(|| format!("bad line `{l}`"))?);
            }
        }
        if size == 0 {
            return Err("not a 3D .cube LUT (no LUT_3D_SIZE)".into());
        }
        if data.len() != size * size * size {
            return Err(format!("expected {} entries, found {}", size * size * size, data.len()));
        }
        Ok(Lut3d { size, data, domain_min: min, domain_max: max, title })
    }

    /// Look up `c` (trilinear).
    pub fn apply(&self, c: [f32; 3]) -> [f32; 3] {
        let n = self.size;
        let s = (n - 1) as f32;
        let mut idx = [0usize; 3];
        let mut frac = [0.0f32; 3];
        for k in 0..3 {
            let span = (self.domain_max[k] - self.domain_min[k]).max(1e-6);
            let x = (((c[k] - self.domain_min[k]) / span).clamp(0.0, 1.0)) * s;
            let i = (x.floor() as usize).min(n - 2);
            idx[k] = i;
            frac[k] = x - i as f32;
        }
        let at = |r: usize, g: usize, b: usize| self.data[r + n * (g + n * b)];
        let lerp = |a: [f32; 3], b: [f32; 3], t: f32| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
        let (r, g, b) = (idx[0], idx[1], idx[2]);
        let c00 = lerp(at(r, g, b), at(r + 1, g, b), frac[0]);
        let c10 = lerp(at(r, g + 1, b), at(r + 1, g + 1, b), frac[0]);
        let c01 = lerp(at(r, g, b + 1), at(r + 1, g, b + 1), frac[0]);
        let c11 = lerp(at(r, g + 1, b + 1), at(r + 1, g + 1, b + 1), frac[0]);
        lerp(lerp(c00, c10, frac[1]), lerp(c01, c11, frac[1]), frac[2])
    }

    /// Build an identity 3D LUT of edge `size` (clamped to `2..=129`).
    pub fn identity(size: usize) -> Self {
        Self::from_fn(size, Some("Identity".into()), |c| c)
    }

    /// Sample `f` across a `size × size × size` grid (red fastest).
    pub fn from_fn(size: usize, title: Option<String>, f: impl Fn([f32; 3]) -> [f32; 3]) -> Self {
        let size = size.clamp(2, 129);
        let denom = (size - 1) as f32;
        let mut data = Vec::with_capacity(size * size * size);
        for b in 0..size {
            for g in 0..size {
                for r in 0..size {
                    let c = f([r as f32 / denom, g as f32 / denom, b as f32 / denom]);
                    data.push(c.map(|v| if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 }));
                }
            }
        }
        Lut3d { size, data, domain_min: [0.0; 3], domain_max: [1.0; 3], title }
    }

    /// Serialize this 3D LUT to the standard Adobe / Iridas `.cube` format (compatible with
    /// Final Cut Pro, FilmCraft, Premiere Pro, DaVinci Resolve, and PhotoCraft).
    pub fn to_cube(&self) -> String {
        use std::fmt::Write;
        let mut out = String::with_capacity(self.data.len() * 28 + 128);
        if let Some(t) = &self.title
            && !t.is_empty()
        {
            let _ = writeln!(out, "TITLE \"{}\"", t.replace('"', "'"));
        }
        let _ = writeln!(out, "LUT_3D_SIZE {}", self.size);
        if self.domain_min != [0.0; 3] || self.domain_max != [1.0; 3] {
            let _ = writeln!(out, "DOMAIN_MIN {:.6} {:.6} {:.6}", self.domain_min[0], self.domain_min[1], self.domain_min[2]);
            let _ = writeln!(out, "DOMAIN_MAX {:.6} {:.6} {:.6}", self.domain_max[0], self.domain_max[1], self.domain_max[2]);
        }
        for c in &self.data {
            let _ = writeln!(out, "{:.6} {:.6} {:.6}", c[0], c[1], c[2]);
        }
        out
    }
}

/// Bake the global colour and tone adjustments of `settings` into a 3D [`Lut3d`] of edge `size`
/// (`2..=65`) in `space` (typically [`crate::OutputSpace::Srgb`] or [`crate::OutputSpace::Rec2020`]).
///
/// Spatial-only operations (crop, vignette, grain, spot removal, red-eye, local masks, clarity,
/// texture, dehaze, sharpening, noise reduction, lens optics) are neutralized so the exported
/// `.cube` LUT captures a clean, position-independent 3D colour grade suitable for Final Cut Pro,
/// FilmCraft, EffectCraft, and PhotoCraft.
pub fn bake_develop_lut(
    settings: &lightcraft_develop::DevelopSettings,
    size: usize,
    space: crate::OutputSpace,
    title: Option<String>,
) -> Result<Lut3d, String> {
    if !(2..=65).contains(&size) {
        return Err(format!("LUT_3D_SIZE {size} out of range (expected 2..=65)"));
    }
    let mut s = settings.effective().into_owned();
    s.crop = Default::default();
    s.orientation = Default::default();
    s.vignette = Default::default();
    s.grain = Default::default();
    s.spots.clear();
    s.red_eye.clear();
    s.masks.clear();
    s.effects.clarity = 0.0;
    s.effects.texture = 0.0;
    s.effects.dehaze = 0.0;
    s.detail = Default::default();
    s.optics = Default::default();
    s.hdr.enabled = false;

    let trc = space.trc();
    let to_working = space.rgb_space().to_space(&lightcraft_color::REC2020);
    let denom = (size - 1) as f32;
    let w = size * size;
    let h = size;
    let mut pixels = Vec::with_capacity(w * h);
    for b in 0..size {
        for g in 0..size {
            for r in 0..size {
                let enc = [r as f32 / denom, g as f32 / denom, b as f32 / denom];
                let lin = enc.map(|v| trc.decode(v));
                pixels.push(to_working.apply_f32(lin));
            }
        }
    }
    let src = lightcraft_raster::Rgb32f { width: w, height: h, data: pixels };
    let req = crate::RenderRequest {
        max_w: w,
        max_h: h,
        quality: crate::Quality::Full,
        apply_crop: false,
        overlay: crate::Overlay::None,
        space,
        depth: crate::OutputDepth::U16,
        proof: None,
        window: None,
        display: None,
    };
    let rendered = crate::render(&src, &crate::SourceInfo::default(), &s, &req);
    let data = match rendered.deep.map(|d| d.samples) {
        Some(crate::DeepSamples::U16(v)) if v.len() == w * h * 3 => v
            .as_chunks::<3>()
            .0
            .iter()
            .map(|px| [px[0] as f32 / 65535.0, px[1] as f32 / 65535.0, px[2] as f32 / 65535.0])
            .collect(),
        _ => return Err("failed to render 16-bit LUT lattice".into()),
    };
    Ok(Lut3d { size, data, domain_min: [0.0; 3], domain_max: [1.0; 3], title })
}

fn registry() -> &'static RwLock<HashMap<String, Arc<Lut3d>>> {
    static R: OnceLock<RwLock<HashMap<String, Arc<Lut3d>>>> = OnceLock::new();
    R.get_or_init(Default::default)
}

/// Make a LUT available to renders under profile id `id` (`lut:…`).
pub fn register(id: &str, lut: Lut3d) {
    registry().write().unwrap_or_else(|e| e.into_inner()).insert(id.to_string(), Arc::new(lut));
}

/// The LUT registered for profile `id`.
pub fn get(id: &str) -> Option<Arc<Lut3d>> {
    if !id.starts_with("lut:") {
        return None;
    }
    registry().read().unwrap_or_else(|e| e.into_inner()).get(id).cloned()
}

/// Forget a LUT profile.
pub fn unregister(id: &str) {
    registry().write().unwrap_or_else(|e| e.into_inner()).remove(id);
}

/// Profile ids that render with a LUT (and so on the CPU).
pub fn is_lut_profile(id: &str) -> bool {
    id.starts_with("lut:")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(f: impl Fn([f32; 3]) -> [f32; 3], n: usize) -> String {
        let mut s = format!("TITLE \"test\"\nLUT_3D_SIZE {n}\n");
        for b in 0..n {
            for g in 0..n {
                for r in 0..n {
                    let c = f([r as f32 / (n - 1) as f32, g as f32 / (n - 1) as f32, b as f32 / (n - 1) as f32]);
                    s.push_str(&format!("{} {} {}\n", c[0], c[1], c[2]));
                }
            }
        }
        s
    }

    #[test]
    fn identity_and_swap() {
        let id = Lut3d::parse_cube(&cube(|c| c, 17)).unwrap();
        assert_eq!(id.title.as_deref(), Some("test"));
        let v = id.apply([0.2, 0.55, 0.9]);
        assert!(v.iter().zip([0.2, 0.55, 0.9]).all(|(a, b)| (a - b).abs() < 1e-5), "{v:?}");
        let swap = Lut3d::parse_cube(&cube(|c| [c[2], c[1], c[0]], 9)).unwrap();
        let v = swap.apply([0.1, 0.5, 0.8]);
        assert!((v[0] - 0.8).abs() < 1e-5 && (v[2] - 0.1).abs() < 1e-5, "{v:?}");
        assert!(Lut3d::parse_cube("LUT_1D_SIZE 4\n0 0 0").is_err());
        assert!(Lut3d::parse_cube("LUT_3D_SIZE 2\n0 0 0\n").is_err(), "too few entries");
    }

    #[test]
    fn bake_develop_lut_and_cube_roundtrip() {
        let neutral = lightcraft_develop::DevelopSettings::default();
        let id_lut = bake_develop_lut(&neutral, 9, crate::OutputSpace::Srgb, Some("Neutral Grade".into())).unwrap();
        let cube_text = id_lut.to_cube();
        assert!(cube_text.contains("TITLE \"Neutral Grade\""));
        assert!(cube_text.contains("LUT_3D_SIZE 9"));
        let parsed = Lut3d::parse_cube(&cube_text).unwrap();
        let mid = parsed.apply([0.5, 0.5, 0.5]);
        for ch in mid {
            assert!((ch - 0.5).abs() < 0.02, "expected neutral midgrey ~0.5, got {mid:?}");
        }

        let mut warm = lightcraft_develop::DevelopSettings::default();
        warm.light.exposure = 0.7;
        warm.color.saturation = 25.0;
        let warm_lut = bake_develop_lut(&warm, 9, crate::OutputSpace::Srgb, Some("Brighter".into())).unwrap();
        let lifted = warm_lut.apply([0.4, 0.3, 0.2]);
        assert!(lifted[0] > 0.45 && lifted[1] > 0.33, "exposure +0.7 EV should lift midtones: {lifted:?}");
    }
}
