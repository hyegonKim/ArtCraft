//! Distraction Removal — Reflections (Camera Raw 19.0 / Lightroom 2027).
//!
//! Two scene-linear reflection-suppression stages operating after white balance and spot removal
//! and before noise reduction:
//! 1. **Eyeglass Reflections** (`reflections.eyeglass`, 0..100): detects localized specular glare
//!    patches and anti-reflective (AR) coating green/cyan/magenta colour casts on spectacle lenses
//!    relative to the surrounding skin/eye context, reconstructing the underlying chromaticity and
//!    attenuating the additive glare component while preserving fine dark structures (eyelashes,
//!    frames, pupils).
//! 2. **Window Reflections** (`reflections.window`, 0..100): decomposes the scene into a smooth
//!    foreground glass reflection veil and the sharper transmitted background layer using
//!    multi-scale O(1) Gaussian envelopes, subtracting the additive reflection layer and restoring
//!    transmission contrast without clipping or haloing.

use lightcraft_color::luminance_2020;
use lightcraft_develop::DevelopSettings;
use lightcraft_raster::blur::gaussian;
use lightcraft_raster::{Plane, Rgb32f};

use crate::for_rows;

#[inline]
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let d = (e1 - e0).max(1e-6);
    let t = ((x - e0) / d).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Whether `s` applies any reflection removal.
pub fn renders(s: &DevelopSettings) -> bool {
    s.section_enabled("remove") && s.reflections.active()
}

/// Cache key of the reflection removal settings (`[0, 0]` when inactive).
pub fn key(s: &DevelopSettings) -> [u64; 2] {
    if !renders(s) {
        return [0, 0];
    }
    let e = if s.reflections.eyeglass.is_finite() { s.reflections.eyeglass.clamp(0.0, 100.0) } else { 0.0 };
    let w = if s.reflections.window.is_finite() { s.reflections.window.clamp(0.0, 100.0) } else { 0.0 };
    [e.to_bits(), w.to_bits()]
}

/// Apply eyeglass and window reflection suppression in place on the scene-linear Rec.2020 image.
pub fn apply(img: &mut Rgb32f, s: &DevelopSettings, px_per_long: f64) {
    if !renders(s) || img.width == 0 || img.height == 0 {
        return;
    }
    let scale = if px_per_long.is_finite() && px_per_long > 0.0 { px_per_long as f32 } else { img.width.max(img.height) as f32 };
    let eye_amt = if s.reflections.eyeglass.is_finite() { (s.reflections.eyeglass / 100.0).clamp(0.0, 1.0) as f32 } else { 0.0 };
    let win_amt = if s.reflections.window.is_finite() { (s.reflections.window / 100.0).clamp(0.0, 1.0) as f32 } else { 0.0 };

    if eye_amt > 0.0 {
        apply_eyeglass(img, eye_amt, scale);
    }
    if win_amt > 0.0 {
        apply_window(img, win_amt, scale);
    }
}

/// Separable 2D spatial minimum filter on a single-channel plane.
fn erode_plane(src: &Plane, radius: usize) -> Plane {
    let (w, h) = (src.width, src.height);
    if radius == 0 || w == 0 || h == 0 {
        return src.clone();
    }
    let mut tmp = vec![0.0f32; w * h];
    for_rows(&mut tmp, w, |y, row| {
        let srow = &src.data[y * w..(y + 1) * w];
        for (x, out) in row.iter_mut().enumerate() {
            let x0 = x.saturating_sub(radius);
            let x1 = (x + radius + 1).min(w);
            let mut m = f32::INFINITY;
            for &v in &srow[x0..x1] {
                if v.is_finite() && v < m {
                    m = v;
                }
            }
            *out = if m.is_finite() { m } else { 0.0 };
        }
    });
    let mut dst = vec![0.0f32; w * h];
    for_rows(&mut dst, w, |y, row| {
        let y0 = y.saturating_sub(radius);
        let y1 = (y + radius + 1).min(h);
        for (x, out) in row.iter_mut().enumerate() {
            let mut m = f32::INFINITY;
            for yy in y0..y1 {
                let v = tmp[yy * w + x];
                if v < m {
                    m = v;
                }
            }
            *out = if m.is_finite() { m } else { 0.0 };
        }
    });
    Plane { width: w, height: h, data: dst }
}

/// Separable per-channel 2D spatial minimum filter on linear RGB.
fn erode_rgb(src: &Rgb32f, radius: usize) -> Rgb32f {
    let (w, h) = (src.width, src.height);
    if radius == 0 || w == 0 || h == 0 {
        return src.clone();
    }
    let mut tmp = vec![[0.0f32; 3]; w * h];
    for_rows(&mut tmp, w, |y, row| {
        let srow = &src.data[y * w..(y + 1) * w];
        for (x, out) in row.iter_mut().enumerate() {
            let x0 = x.saturating_sub(radius);
            let x1 = (x + radius + 1).min(w);
            let mut m = [f32::INFINITY; 3];
            for px in &srow[x0..x1] {
                for c in 0..3 {
                    if px[c].is_finite() && px[c] < m[c] {
                        m[c] = px[c];
                    }
                }
            }
            *out = [
                if m[0].is_finite() { m[0].max(0.0) } else { 0.0 },
                if m[1].is_finite() { m[1].max(0.0) } else { 0.0 },
                if m[2].is_finite() { m[2].max(0.0) } else { 0.0 },
            ];
        }
    });
    let mut dst = vec![[0.0f32; 3]; w * h];
    for_rows(&mut dst, w, |y, row| {
        let y0 = y.saturating_sub(radius);
        let y1 = (y + radius + 1).min(h);
        for (x, out) in row.iter_mut().enumerate() {
            let mut m = [f32::INFINITY; 3];
            for yy in y0..y1 {
                let px = tmp[yy * w + x];
                for c in 0..3 {
                    if px[c] < m[c] {
                        m[c] = px[c];
                    }
                }
            }
            *out = m;
        }
    });
    Rgb32f { width: w, height: h, data: dst }
}

/// Eyeglass glare & AR-coating reflection suppression.
fn apply_eyeglass(img: &mut Rgb32f, amount: f32, px_per_long: f32) {
    let (w, h) = (img.width, img.height);
    let sigma_fine = (0.0028 * px_per_long).clamp(0.8, 32.0);
    let sigma_mid = (0.035 * px_per_long).clamp(4.0, 140.0);
    let sigma_wide = (0.11 * px_per_long).clamp(9.0, 280.0);
    let erode_r = (0.055 * px_per_long).round().clamp(4.0, 96.0) as usize;

    let fine = gaussian(img, sigma_fine);
    let mid = gaussian(img, sigma_mid);
    // Estimate glare-free surrounding skin/iris base using spatial erosion + wide Gaussian smoothing:
    let eroded = erode_rgb(img, erode_r);
    let surround = gaussian(&eroded, sigma_mid);
    let wide = gaussian(img, sigma_wide);

    for_rows(&mut img.data, w, |y, row| {
        let base = y * w;
        for (x, px) in row.iter_mut().enumerate() {
            let idx = base + x;
            let c = *px;
            if !c[0].is_finite() || !c[1].is_finite() || !c[2].is_finite() {
                continue;
            }
            let f = fine.data[idx];
            let m = mid.data[idx];
            let sur = surround.data[idx];
            let bg = [sur[0] * 0.65 + wide.data[idx][0] * 0.35, sur[1] * 0.65 + wide.data[idx][1] * 0.35, sur[2] * 0.65 + wide.data[idx][2] * 0.35];

            let y_c = luminance_2020(c).max(1e-5);
            let y_f = luminance_2020(f).max(1e-5);
            let y_m = luminance_2020(m).max(1e-5);
            let y_bg = luminance_2020(bg).max(1e-5);

            // Preserve dark structures (frames, eyelashes, pupils) where pixel luminance is below surround.
            let dark_guard = smoothstep(0.78, 1.05, y_c / y_bg.min(y_m));
            if dark_guard <= 0.0 {
                continue;
            }

            // 1. Localized bright glare patch relative to glare-free eye/face surround:
            let ratio_bg = y_f / y_bg;
            let ratio_mid = y_f / y_m;
            let glare_w = (smoothstep(1.12, 2.05, ratio_bg) * 0.8 + smoothstep(1.04, 1.65, ratio_mid) * 0.2) * dark_guard;

            // 2. Anti-reflective lens coating tint anomaly (green/cyan or magenta/violet excess over local surround):
            let chr_c = [c[0] / y_c, c[1] / y_c, c[2] / y_c];
            let chr_bg = [bg[0] / y_bg, bg[1] / y_bg, bg[2] / y_bg];
            let green_excess = (chr_c[1] - chr_bg[1]).max(0.0) - 0.5 * ((chr_c[0] - chr_bg[0]).max(0.0));
            let magenta_excess = 0.5 * ((chr_c[0] - chr_bg[0]).max(0.0) + (chr_c[2] - chr_bg[2]).max(0.0)) - (chr_c[1] - chr_bg[1]).max(0.0);
            let cyan_excess = 0.5 * ((chr_c[1] - chr_bg[1]).max(0.0) + (chr_c[2] - chr_bg[2]).max(0.0)) - (chr_c[0] - chr_bg[0]).max(0.0);
            let ar_anom = green_excess.max(magenta_excess).max(cyan_excess);
            let ar_w = smoothstep(0.04, 0.26, ar_anom) * smoothstep(0.92, 1.25, y_f / y_bg) * dark_guard;

            let mask = (glare_w.max(ar_w) * amount).clamp(0.0, 1.0);
            if mask <= 1e-4 {
                continue;
            }

            // Reconstruct underlying eye/skin base from glare-free surround while retaining fine high-frequency texture:
            let detail_ratio = (y_c / y_f).clamp(0.68, 1.32);
            let target_base_y = y_bg.min(y_f);
            let target_y = (target_base_y * detail_ratio).max(0.0);

            // Neutralize AR coating colour cast toward surrounding skin/eye chromaticity:
            let chroma_blend = (ar_w * amount * 0.94 + glare_w * amount * 0.72).clamp(0.0, 0.96);
            let target_chr = [
                chr_c[0] + (chr_bg[0] - chr_c[0]) * chroma_blend,
                chr_c[1] + (chr_bg[1] - chr_c[1]) * chroma_blend,
                chr_c[2] + (chr_bg[2] - chr_c[2]) * chroma_blend,
            ];
            let chr_y = (target_chr[0] * 0.2627 + target_chr[1] * 0.6780 + target_chr[2] * 0.0593).max(1e-5);
            let norm_chr = [target_chr[0] / chr_y, target_chr[1] / chr_y, target_chr[2] / chr_y];

            let out_y = y_c + (target_y - y_c) * (glare_w.max(ar_w * 0.55) * amount).clamp(0.0, 0.94);
            let reconstructed = [norm_chr[0] * out_y, norm_chr[1] * out_y, norm_chr[2] * out_y];
            *px = [
                (c[0] + (reconstructed[0] - c[0]) * mask.max(chroma_blend)).max(0.0),
                (c[1] + (reconstructed[1] - c[1]) * mask.max(chroma_blend)).max(0.0),
                (c[2] + (reconstructed[2] - c[2]) * mask.max(chroma_blend)).max(0.0),
            ];
        }
    });
    let _ = h;
}

/// Window / glass sheet reflection separation & transmission contrast restoration.
fn apply_window(img: &mut Rgb32f, amount: f32, px_per_long: f32) {
    let (w, h) = (img.width, img.height);
    // Window reflections form a smooth additive sheet (`I = T + R`) whose local spatial minimum
    // envelope rises above true background shadows. Estimate the smooth reflection sheet via
    // morphological per-channel erosion + Gaussian smoothing so sharp transmitted edges are untouched.
    let erode_r = (0.045 * px_per_long).round().clamp(4.0, 128.0) as usize;
    let sigma_veil = (0.045 * px_per_long).clamp(5.0, 240.0);

    let eroded_rgb = erode_rgb(img, erode_r);
    let veil_rgb = gaussian(&eroded_rgb, sigma_veil);

    let dark_plane = Plane { width: w, height: h, data: img.data.iter().map(|c| c[0].min(c[1]).min(c[2]).max(0.0)).collect() };
    let eroded_dark = erode_plane(&dark_plane, erode_r);
    let smooth_dark = gaussian(&eroded_dark, sigma_veil);

    for_rows(&mut img.data, w, |y, row| {
        let base = y * w;
        for (x, px) in row.iter_mut().enumerate() {
            let idx = base + x;
            let c = *px;
            if !c[0].is_finite() || !c[1].is_finite() || !c[2].is_finite() {
                continue;
            }
            let v = veil_rgb.data[idx];
            let d_veil = smooth_dark.data[idx];
            let y_c = luminance_2020(c).max(1e-5);

            // Blend per-channel smooth minimum envelope with dark-channel floor:
            let sheet = [
                (v[0] * 0.72 + d_veil * 0.28) * 0.82 * amount,
                (v[1] * 0.72 + d_veil * 0.28) * 0.82 * amount,
                (v[2] * 0.72 + d_veil * 0.28) * 0.82 * amount,
            ];
            let sub = [sheet[0].min(c[0] * 0.88), sheet[1].min(c[1] * 0.88), sheet[2].min(c[2] * 0.88)];

            // Restore transmission gain moderately so subtracting the reflection sheet preserves midtone brightness:
            let removed_y = luminance_2020(sub);
            let trans_gain = 1.0 + (removed_y / (y_c + 0.15)).clamp(0.0, 0.55) * (0.45 * amount);
            let highlight_tame = 1.0 / (1.0 + 0.22 * amount * (y_c - 0.85).max(0.0));
            let g = trans_gain * highlight_tame;

            *px = [((c[0] - sub[0]).max(0.0) * g).max(0.0), ((c[1] - sub[1]).max(0.0) * g).max(0.0), ((c[2] - sub[2]).max(0.0) * g).max(0.0)];
        }
    });
    let _ = h;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_reflections_is_inactive_noop() {
        let s = DevelopSettings::default();
        assert!(!renders(&s));
        assert_eq!(key(&s), [0, 0]);
        let mut img = Rgb32f::from_fn(32, 32, |x, y| [(x as f32) / 32.0, (y as f32) / 32.0, 0.25]);
        let orig = img.clone();
        apply(&mut img, &s, 32.0);
        assert_eq!(img.data, orig.data);
    }

    #[test]
    fn eyeglass_reflection_suppresses_glare_and_ar_green_tint() {
        let mut img = Rgb32f::from_fn(64, 64, |x, y| {
            // Neutral skin/eye background around 0.18 with a bright green-tinted AR reflection patch in the centre
            if (26..38).contains(&x) && (26..38).contains(&y) { [0.42, 0.68, 0.46] } else { [0.20, 0.18, 0.16] }
        });
        let before_center = img.data[32 * 64 + 32];
        let before_corner = img.data[4 * 64 + 4];
        let mut s = DevelopSettings::default();
        s.reflections.eyeglass = 85.0;
        assert!(renders(&s));
        apply(&mut img, &s, 64.0);

        let after_center = img.data[32 * 64 + 32];
        let after_corner = img.data[4 * 64 + 4];
        // Glare luminance and green excess are both reduced substantially:
        assert!(luminance_2020(after_center) < luminance_2020(before_center) * 0.78);
        let green_ratio_before = before_center[1] / before_center[0];
        let green_ratio_after = after_center[1] / after_center[0].max(1e-5);
        assert!(green_ratio_after < green_ratio_before);
        // Non-glare background stays nearly untouched:
        assert!((after_corner[0] - before_corner[0]).abs() < 0.02);
    }

    #[test]
    fn window_reflection_removes_additive_veil_and_improves_contrast() {
        let mut img = Rgb32f::from_fn(64, 64, |x, _y| {
            // Sharp vertical subject bars behind a uniform +0.22 window reflection veil
            let bg = if (x / 8) % 2 == 0 { 0.06 } else { 0.36 };
            [bg + 0.22, bg + 0.23, bg + 0.25]
        });
        let before_dark = luminance_2020(img.data[32 * 64 + 4]);
        let before_bright = luminance_2020(img.data[32 * 64 + 12]);
        let before_ratio = before_bright / before_dark;

        let mut s = DevelopSettings::default();
        s.reflections.window = 80.0;
        apply(&mut img, &s, 64.0);

        let after_dark = luminance_2020(img.data[32 * 64 + 4]);
        let after_bright = luminance_2020(img.data[32 * 64 + 12]);
        let after_ratio = after_bright / after_dark.max(1e-5);
        assert!(after_dark < before_dark, "additive reflection veil lowered in darks");
        assert!(after_ratio > before_ratio * 1.15, "transmitted contrast ratio increased: {before_ratio} -> {after_ratio}");
        assert!(img.data.iter().flatten().all(|v| v.is_finite() && *v >= 0.0));
    }
}
