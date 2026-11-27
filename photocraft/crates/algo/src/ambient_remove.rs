//! Photoshop 2027 `Edit > Remove Ambient Effects…` (`주변 효과 제거…`):
//! Four physics-grounded intrinsic restoration modes:
//! - `Shadows` (`그림자`): illumination-invariant log-chromaticity shadow mask + soft Retinex shadow re-illumination.
//! - `Reflections` (`반사`): low-frequency additive glass/surface reflection veil separation (`I = T + R`).
//! - `LensFlares` (`렌즈 플레어`): saturated radial/blob glare & purple/cyan bloom detection + surrounding chroma-structure recovery.
//! - `Atmospheric` (`분위기 있는`): He-Sun-Tang dark-channel prior dehaze / fog / smoke transmission recovery.
//!   Plus `remove_related_elements` (`관련 요소 제거`: secondary penumbra / ghost / halo cleanup)
//!   and `enhance` (`향상`: local micro-contrast + chroma restoration).

use photocraft_geom::Rect;
use photocraft_raster::{Surface, from_rgba_into, to_rgba};
use serde::{Deserialize, Serialize};

use crate::photo_util::{linear_to_srgb, par_rows, srgb_to_linear};

/// Mode in `Edit > Remove Ambient Effects…` (`주변 효과 제거…`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AmbientMode {
    #[default]
    Shadows,
    Reflections,
    LensFlares,
    Atmospheric,
}

impl AmbientMode {
    pub const ALL: [Self; 4] = [Self::Shadows, Self::Reflections, Self::LensFlares, Self::Atmospheric];

    pub fn id(self) -> &'static str {
        match self {
            Self::Shadows => "shadows",
            Self::Reflections => "reflections",
            Self::LensFlares => "lensFlares",
            Self::Atmospheric => "atmospheric",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Shadows => "Shadows",
            Self::Reflections => "Reflections",
            Self::LensFlares => "Lens flares",
            Self::Atmospheric => "Atmospheric",
        }
    }

    pub fn from_id(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "shadows" | "shadow" => Some(Self::Shadows),
            "reflections" | "reflection" => Some(Self::Reflections),
            "lensflares" | "lens_flares" | "lens flares" | "flare" => Some(Self::LensFlares),
            "atmospheric" | "haze" | "fog" => Some(Self::Atmospheric),
            _ => None,
        }
    }
}

/// Parameters for `Remove Ambient Effects…`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AmbientRemoveParams {
    pub mode: AmbientMode,
    /// `Remove Related Elements` (`관련 요소 제거`): expands cleanup to penumbra, ghosting, or veil halos.
    pub remove_related_elements: bool,
    /// `Enhance` (`향상`): restores local clarity & color vibrancy after ambient removal.
    pub enhance: bool,
    /// Effect strength (`0.0..=100.0`).
    pub strength: f32,
}

impl Default for AmbientRemoveParams {
    fn default() -> Self {
        Self { mode: AmbientMode::Shadows, remove_related_elements: true, enhance: true, strength: 80.0 }
    }
}

fn blur_1ch(src: &[f32], w: usize, h: usize, sigma: f32) -> Vec<f32> {
    let mut out = src.to_vec();
    crate::fxutil::gauss_blur_n(&mut out, w, h, 1, sigma.max(0.5));
    out
}

fn blur_3ch(src: &[f32], w: usize, h: usize, sigma: f32) -> Vec<f32> {
    let mut out = src.to_vec();
    crate::fxutil::gauss_blur_n(&mut out, w, h, 3, sigma.max(0.5));
    out
}

/// Runs `Remove Ambient Effects` in-place on straight sRGB RGBA pixels (`w × h`).
pub fn remove_ambient_effects_rgba(px: &mut [[f32; 4]], w: usize, h: usize, params: &AmbientRemoveParams) {
    let n = w * h;
    if n == 0 || px.len() < n {
        return;
    }
    let amt = (if params.strength.is_finite() { params.strength } else { 80.0 }).clamp(0.0, 100.0) / 100.0;
    if amt <= 1e-4 {
        return;
    }
    let min_side = (w.min(h) as f32).max(16.0);

    match params.mode {
        AmbientMode::Shadows => remove_shadows(px, w, h, amt, params.remove_related_elements, min_side),
        AmbientMode::Reflections => remove_reflections(px, w, h, amt, params.remove_related_elements, min_side),
        AmbientMode::LensFlares => remove_lens_flares(px, w, h, amt, params.remove_related_elements, min_side),
        AmbientMode::Atmospheric => remove_atmospheric(px, w, h, amt, params.remove_related_elements, min_side),
    }

    if params.enhance {
        apply_post_enhance(px, w, h, amt * 0.65, min_side);
    }
}

fn remove_shadows(px: &mut [[f32; 4]], w: usize, h: usize, amt: f32, related: bool, min_side: f32) {
    let n = w * h;
    let mut luma = vec![0.0f32; n];
    for (i, q) in px.iter().take(n).enumerate() {
        let lr = srgb_to_linear(q[0].max(0.0));
        let lg = srgb_to_linear(q[1].max(0.0));
        let lb = srgb_to_linear(q[2].max(0.0));
        luma[i] = (0.2126 * lr + 0.7152 * lg + 0.0722 * lb).max(1e-4);
    }
    // Surround illumination envelope: broad Gaussian captures sunlit neighborhood around cast shadows.
    let radius = (min_side * if related { 0.14 } else { 0.08 }).clamp(4.0, 72.0);
    let illum = blur_1ch(&luma, w, h, radius);
    let mut sorted = luma.clone();
    sorted.sort_by(f32::total_cmp);
    let bright_ref = sorted[(n.saturating_sub(1) * 85) / 100].max(0.12);

    par_rows(px, w, 1, |y, row| {
        for (x, q) in row.iter_mut().enumerate() {
            let idx = y * w + x;
            let l = luma[idx];
            let env = illum[idx].max(bright_ref * 0.65);
            // Cast shadows have lower luminance than their broad neighborhood while retaining chromaticity.
            if l < env * 0.92 {
                let deficit = ((env - l) / env.max(1e-4)).clamp(0.0, 0.92);
                let penumbra_boost = if related { deficit.sqrt() } else { deficit };
                let gain = 1.0 + amt * penumbra_boost * ((env / l.max(0.025)).min(4.2) - 1.0);
                // Cast outdoor shadows carry cool sky-blue bias; gently warm recovered shadow toward neutral.
                let warm = 0.06 * amt * deficit;
                let lr = srgb_to_linear(q[0].max(0.0)) * gain * (1.0 + warm);
                let lg = srgb_to_linear(q[1].max(0.0)) * gain;
                let lb = srgb_to_linear(q[2].max(0.0)) * gain * (1.0 - warm * 0.8);
                q[0] = linear_to_srgb(lr.clamp(0.0, 1.0));
                q[1] = linear_to_srgb(lg.clamp(0.0, 1.0));
                q[2] = linear_to_srgb(lb.clamp(0.0, 1.0));
            }
        }
    });
}

fn remove_reflections(px: &mut [[f32; 4]], w: usize, h: usize, amt: f32, related: bool, min_side: f32) {
    let n = w * h;
    // Additive reflection layer R tends to be spatially smoother & desaturated relative to sharp transmitted edges T.
    let mut lin3 = vec![0.0f32; n * 3];
    let mut min_ch = vec![0.0f32; n];
    for (i, q) in px.iter().take(n).enumerate() {
        let r = srgb_to_linear(q[0].max(0.0));
        let g = srgb_to_linear(q[1].max(0.0));
        let b = srgb_to_linear(q[2].max(0.0));
        lin3[i * 3] = r;
        lin3[i * 3 + 1] = g;
        lin3[i * 3 + 2] = b;
        min_ch[i] = r.min(g).min(b);
    }
    let veil_sigma = (min_side * if related { 0.09 } else { 0.055 }).clamp(3.0, 48.0);
    let smooth_rgb = blur_3ch(&lin3, w, h, veil_sigma);
    let smooth_min = blur_1ch(&min_ch, w, h, veil_sigma * 0.7);

    par_rows(px, w, 1, |y, row| {
        for (x, q) in row.iter_mut().enumerate() {
            let idx = y * w + x;
            let o = idx * 3;
            let r = lin3[o];
            let g = lin3[o + 1];
            let b = lin3[o + 2];
            let mx = r.max(g).max(b);
            let mn = r.min(g).min(b);
            let sat = if mx > 1e-4 { (mx - mn) / mx } else { 0.0 };
            // Glass/window reflection veil lifts minimum channel & smooths low-saturation glare patches.
            let veil_w = (1.0 - sat * 0.85).clamp(0.15, 1.0) * amt * if related { 0.72 } else { 0.56 };
            let veil_base = smooth_min[idx] * veil_w;
            for (c, orig_lin) in [r, g, b].into_iter().enumerate() {
                let refl_est = 0.55 * veil_base + 0.45 * (smooth_rgb[o + c] * veil_w * 0.65);
                let transmitted = ((orig_lin - refl_est).max(0.0) / (1.0 - refl_est * 0.72).max(0.25)).clamp(0.0, 1.0);
                q[c] = linear_to_srgb(transmitted);
            }
        }
    });
}

fn remove_lens_flares(px: &mut [[f32; 4]], w: usize, h: usize, amt: f32, related: bool, min_side: f32) {
    let n = w * h;
    let mut rgb = vec![0.0f32; n * 3];
    let mut flare_mask = vec![0.0f32; n];
    for (i, q) in px.iter().take(n).enumerate() {
        let r = q[0].clamp(0.0, 1.0);
        let g = q[1].clamp(0.0, 1.0);
        let b = q[2].clamp(0.0, 1.0);
        rgb[i * 3] = r;
        rgb[i * 3 + 1] = g;
        rgb[i * 3 + 2] = b;
        let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        let mx = r.max(g).max(b);
        let mn = r.min(g).min(b);
        let sat = if mx > 1e-4 { (mx - mn) / mx } else { 0.0 };
        // Detect bright blown flare cores AND purple/cyan/warm secondary aperture ghosts.
        let blown = ((lum - 0.74) / 0.24).clamp(0.0, 1.0) * (1.0 - sat * 0.65);
        let chroma_ghost = if related { ((lum - 0.55) / 0.35).clamp(0.0, 1.0) * ((b.max(r) - g).max(0.0) * 1.8).clamp(0.0, 1.0) } else { 0.0 };
        flare_mask[i] = blown.max(chroma_ghost);
    }
    let mask_sigma = (min_side * if related { 0.045 } else { 0.025 }).clamp(1.5, 24.0);
    let soft_mask = blur_1ch(&flare_mask, w, h, mask_sigma);
    // Weighted background fill excluding flare hotspots!
    let mut weighted_bg = vec![0.0f32; n * 3];
    let mut weights = vec![0.0f32; n];
    for i in 0..n {
        let keep = (1.0 - soft_mask[i] * 0.95).clamp(0.03, 1.0);
        weights[i] = keep;
        weighted_bg[i * 3] = rgb[i * 3] * keep;
        weighted_bg[i * 3 + 1] = rgb[i * 3 + 1] * keep;
        weighted_bg[i * 3 + 2] = rgb[i * 3 + 2] * keep;
    }
    let fill_sigma = (min_side * 0.085).clamp(4.0, 48.0);
    let bg_num = blur_3ch(&weighted_bg, w, h, fill_sigma);
    let bg_den = blur_1ch(&weights, w, h, fill_sigma);

    par_rows(px, w, 1, |y, row| {
        for (x, q) in row.iter_mut().enumerate() {
            let idx = y * w + x;
            let m = (soft_mask[idx] * 1.35 * amt).clamp(0.0, 1.0);
            if m <= 1e-4 {
                continue;
            }
            let den = bg_den[idx].max(1e-4);
            for c in 0..3 {
                let clean = (bg_num[idx * 3 + c] / den).clamp(0.0, 1.0);
                // Keep fine underlying detail variations where not completely clipped.
                let detail = (q[c] - clean).clamp(-0.08, 0.08) * (1.0 - m);
                q[c] = (q[c] * (1.0 - m) + (clean + detail) * m).clamp(0.0, 1.0);
            }
        }
    });
}

fn remove_atmospheric(px: &mut [[f32; 4]], w: usize, h: usize, amt: f32, related: bool, min_side: f32) {
    // Dark-Channel Prior dehaze / fog / smoke transmission recovery (He-Sun-Tang).
    let n = w * h;
    let mut dark = vec![0.0f32; n];
    for (i, q) in px.iter().take(n).enumerate() {
        dark[i] = q[0].min(q[1]).min(q[2]).clamp(0.0, 1.0);
    }
    // Estimate atmospheric light A from brightest 0.5% of dark channel.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| dark[b].total_cmp(&dark[a]));
    let top_k = (n / 200).clamp(1, n);
    let mut atmos = [0.0f64; 3];
    for &idx in &order[..top_k] {
        for c in 0..3 {
            atmos[c] += f64::from(px[idx][c].clamp(0.0, 1.0));
        }
    }
    let a_rgb = [
        (atmos[0] / top_k as f64).clamp(0.35, 0.99) as f32,
        (atmos[1] / top_k as f64).clamp(0.35, 0.99) as f32,
        (atmos[2] / top_k as f64).clamp(0.35, 0.99) as f32,
    ];

    let mut norm_min = vec![0.0f32; n];
    for (i, q) in px.iter().take(n).enumerate() {
        let r = q[0] / a_rgb[0];
        let g = q[1] / a_rgb[1];
        let b = q[2] / a_rgb[2];
        norm_min[i] = r.min(g).min(b).clamp(0.0, 1.0);
    }
    let patch_sigma = (min_side * 0.03).clamp(1.5, 20.0);
    let smooth_dark = blur_1ch(&norm_min, w, h, patch_sigma);
    let omega = (amt * if related { 0.92 } else { 0.80 }).clamp(0.0, 0.95);

    par_rows(px, w, 1, |y, row| {
        for (x, q) in row.iter_mut().enumerate() {
            let idx = y * w + x;
            let t = (1.0 - omega * smooth_dark[idx]).clamp(0.14, 1.0);
            for c in 0..3 {
                q[c] = ((q[c] - a_rgb[c]) / t + a_rgb[c]).clamp(0.0, 1.0);
            }
        }
    });
}

fn apply_post_enhance(px: &mut [[f32; 4]], w: usize, h: usize, strength: f32, min_side: f32) {
    let n = w * h;
    let mut luma = vec![0.0f32; n];
    for (i, q) in px.iter().take(n).enumerate() {
        luma[i] = 0.2126 * q[0] + 0.7152 * q[1] + 0.0722 * q[2];
    }
    let base = blur_1ch(&luma, w, h, (min_side * 0.018).clamp(1.2, 8.0));
    par_rows(px, w, 1, |y, row| {
        for (x, q) in row.iter_mut().enumerate() {
            let idx = y * w + x;
            let l = luma[idx];
            let detail = (l - base[idx]) * 0.45 * strength;
            let target_l = (l + detail).clamp(0.0, 1.0);
            let ratio = if l > 1e-4 { target_l / l } else { 1.0 };
            let vib = 1.0 + 0.14 * strength;
            for ch_val in q.iter_mut().take(3) {
                let v = (*ch_val * ratio).clamp(0.0, 1.0);
                *ch_val = (target_l + (v - target_l) * vib).clamp(0.0, 1.0);
            }
        }
    });
}

/// Surface wrapper for `Remove Ambient Effects…`.
pub fn remove_ambient_effects_surface(src: &Surface, rect: Rect, params: &AmbientRemoveParams) -> Surface {
    if rect.is_empty() {
        return Surface::new(src.format());
    }
    let fmt = src.format();
    let ch = fmt.channels();
    let w = rect.width() as usize;
    let h = rect.height() as usize;
    let raw = src.read_region(rect);
    let mut rgba_px: Vec<[f32; 4]> = raw.chunks_exact(ch).map(|p| to_rgba(&fmt, p)).collect();
    remove_ambient_effects_rgba(&mut rgba_px, w, h, params);
    let mut out_buf = vec![0.0f32; w * h * ch];
    for (p, dst) in rgba_px.iter().zip(out_buf.chunks_exact_mut(ch)) {
        from_rgba_into(&fmt, *p, dst);
    }
    let mut out = Surface::with_default(fmt, &src.default_pixel());
    out.write_region(rect, &out_buf);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_four_ambient_modes_restore_degraded_regions() {
        let (w, h) = (48, 48);
        // 1. Cast shadow patch inside sunlit field
        let mut sh = vec![[0.75f32, 0.72, 0.68, 1.0]; w * h];
        for y in 16..32 {
            for x in 16..32 {
                sh[y * w + x] = [0.20, 0.22, 0.26, 1.0];
            }
        }
        remove_ambient_effects_rgba(
            &mut sh,
            w,
            h,
            &AmbientRemoveParams { mode: AmbientMode::Shadows, remove_related_elements: true, enhance: false, strength: 90.0 },
        );
        assert!(sh[24 * w + 24][0] > 0.36, "shadow center should lift significantly, got {}", sh[24 * w + 24][0]);

        // 2. Reflection veil
        let mut rf = vec![[0.55f32, 0.60, 0.65, 1.0]; w * h];
        remove_ambient_effects_rgba(
            &mut rf,
            w,
            h,
            &AmbientRemoveParams { mode: AmbientMode::Reflections, remove_related_elements: true, enhance: false, strength: 85.0 },
        );
        assert!(rf[24 * w + 24][0] < 0.50, "reflection veil removal lowers milky floor, got {}", rf[24 * w + 24][0]);

        // 3. Lens flare hotspot
        let mut lf = vec![[0.25f32, 0.40, 0.30, 1.0]; w * h];
        for y in 20..28 {
            for x in 20..28 {
                lf[y * w + x] = [0.98, 0.96, 0.97, 1.0];
            }
        }
        remove_ambient_effects_rgba(
            &mut lf,
            w,
            h,
            &AmbientRemoveParams { mode: AmbientMode::LensFlares, remove_related_elements: true, enhance: false, strength: 95.0 },
        );
        assert!(lf[24 * w + 24][0] < 0.72, "flare hotspot should be attenuated toward background, got {}", lf[24 * w + 24][0]);

        // 4. Atmospheric fog/haze
        let mut hz = vec![[0.0f32; 4]; w * h];
        for (i, q) in hz.iter_mut().enumerate() {
            let v = if i % 2 == 0 { 0.62 } else { 0.86 };
            *q = [v, v + 0.02, v + 0.04, 1.0];
        }
        remove_ambient_effects_rgba(
            &mut hz,
            w,
            h,
            &AmbientRemoveParams { mode: AmbientMode::Atmospheric, remove_related_elements: true, enhance: true, strength: 90.0 },
        );
        assert!(hz[0][0] < 0.55, "dehaze restores dark contrast through haze, got {}", hz[0][0]);
    }
}
