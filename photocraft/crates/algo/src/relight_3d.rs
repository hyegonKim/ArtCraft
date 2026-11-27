//! Photoshop 2027 3D Relight (`Lightshop` / `재조명`) & Mockup Environmental Lighting (`환경 조명`):
//! - Estimates smooth 3D surface relief + normal field `(nx, ny, nz)` from multi-scale luminance
//!   plus intrinsic albedo separation (multi-scale Retinex).
//! - Evaluates up to 3 interactive 3D point/spot lamps (`x`, `y`, `z`, `intensity`, `color`, `spread`)
//!   with diffuse Lambertian + Blinn-Phong micro-specular shading.
//! - Provides `apply_mockup_lighting_rgba` for Mockup groups: warps decal graphics by the underlying
//!   object's Sobel surface gradient and blends realistic diffuse/specular environmental lighting.

use photocraft_geom::Rect;
use photocraft_raster::{Surface, from_rgba_into, to_rgba};
use serde::{Deserialize, Serialize};

use crate::photo_util::{bilinear, linear_to_srgb, par_rows, srgb_to_linear};

/// One 3D lamp in `Relight3dParams`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Lamp3d {
    pub enabled: bool,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub intensity: f32,
    pub color: [f32; 3],
    pub spread: f32,
}

impl Default for Lamp3d {
    fn default() -> Self {
        Self { enabled: true, x: 0.32, y: 0.28, z: 0.55, intensity: 1.15, color: [1.0, 0.90, 0.76], spread: 0.68 }
    }
}

/// Parameters for 3-lamp 3D Relight (`새 재조명 레이어…`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Relight3dParams {
    pub ambient: f32,
    pub surface_relief: f32,
    pub lamps: [Lamp3d; 3],
}

impl Default for Relight3dParams {
    fn default() -> Self {
        Self {
            ambient: 0.55,
            surface_relief: 1.0,
            lamps: [
                Lamp3d { enabled: true, x: 0.30, y: 0.28, z: 0.55, intensity: 1.20, color: [1.0, 0.90, 0.76], spread: 0.68 },
                Lamp3d { enabled: true, x: 0.74, y: 0.62, z: 0.65, intensity: 0.55, color: [0.72, 0.86, 1.0], spread: 0.80 },
                Lamp3d { enabled: false, x: 0.50, y: 0.14, z: 0.42, intensity: 0.85, color: [1.0, 0.58, 0.82], spread: 0.50 },
            ],
        }
    }
}

fn blur_gray(src: &[f32], w: usize, h: usize, sigma: f32) -> Vec<f32> {
    let mut buf = src.to_vec();
    crate::fxutil::gauss_blur_n(&mut buf, w, h, 1, sigma.max(0.5));
    buf
}

/// Applies 3-lamp 3D depth-normal relighting to straight sRGB RGBA pixels (`w × h`).
pub fn apply_relight_3d_rgba(px: &mut [[f32; 4]], w: usize, h: usize, params: &Relight3dParams) {
    if px.is_empty() || w == 0 || h == 0 {
        return;
    }
    let n = w * h;
    let min_side = (w.min(h) as f32).max(16.0);
    let sig_fine = (min_side * 0.018).clamp(1.5, 18.0);
    let sig_coarse = (min_side * 0.075).clamp(4.0, 64.0);

    let mut luma = vec![0.0f32; n];
    for (i, q) in px.iter().enumerate() {
        let lr = srgb_to_linear(q[0].max(0.0));
        let lg = srgb_to_linear(q[1].max(0.0));
        let lb = srgb_to_linear(q[2].max(0.0));
        luma[i] = (0.2126 * lr + 0.7152 * lg + 0.0722 * lb).max(1e-4);
    }
    let fine = blur_gray(&luma, w, h, sig_fine);
    let coarse = blur_gray(&luma, w, h, sig_coarse);

    let ambient = (if params.ambient.is_finite() { params.ambient } else { 0.55 }).clamp(0.0, 2.0);
    let relief = (if params.surface_relief.is_finite() { params.surface_relief } else { 1.0 }).clamp(0.0, 3.0);
    let step = (sig_fine.round() as isize).clamp(1, 16);

    let active_lamps: Vec<Lamp3d> = params.lamps.iter().copied().filter(|l| l.enabled && l.intensity.is_finite() && l.intensity > 1e-4).collect();

    let wf = w as f32;
    let hf = h as f32;
    let aspect_x = wf / min_side;
    let aspect_y = hf / min_side;

    par_rows(px, w, 1, |y, row| {
        let yi = y as isize;
        let ym = (yi - step).clamp(0, h as isize - 1) as usize;
        let yp = (yi + step).clamp(0, h as isize - 1) as usize;
        let py_norm = (y as f32 + 0.5) / hf;

        for (x, q) in row.iter_mut().enumerate() {
            if q[3] <= 1e-5 {
                continue;
            }
            let xi = x as isize;
            let xm = (xi - step).clamp(0, w as isize - 1) as usize;
            let xp = (xi + step).clamp(0, w as isize - 1) as usize;
            let idx = y * w + x;

            // Multi-scale depth-from-shading height field gradient -> unit surface normal.
            let h_xm = 0.6 * fine[y * w + xm] + 0.4 * coarse[y * w + xm];
            let h_xp = 0.6 * fine[y * w + xp] + 0.4 * coarse[y * w + xp];
            let h_ym = 0.6 * fine[ym * w + x] + 0.4 * coarse[ym * w + x];
            let h_yp = 0.6 * fine[yp * w + x] + 0.4 * coarse[yp * w + x];

            let gx = -(h_xp - h_xm) * relief * 3.2;
            let gy = -(h_yp - h_ym) * relief * 3.2;
            let inv_n = (gx * gx + gy * gy + 1.0).sqrt().recip();
            let normal = [gx * inv_n, gy * inv_n, inv_n];

            // Intrinsic shading normalization (Retinex): remove soft existing illumination envelope while keeping texture.
            let shade_env = coarse[idx].clamp(0.06, 1.0);
            let lin = [srgb_to_linear(q[0].max(0.0)), srgb_to_linear(q[1].max(0.0)), srgb_to_linear(q[2].max(0.0))];
            let albedo_boost = (0.38 / shade_env).clamp(0.45, 2.25);

            let px_norm = (x as f32 + 0.5) / wf;
            let mut lit = [ambient; 3];
            let mut spec_accum = [0.0f32; 3];

            for lamp in &active_lamps {
                let lx = (lamp.x - px_norm) * aspect_x;
                let ly = (lamp.y - py_norm) * aspect_y;
                let lz = lamp.z.clamp(0.05, 2.5);
                let dist2 = lx * lx + ly * ly + lz * lz;
                let dist = dist2.sqrt().max(1e-4);
                let ldir = [lx / dist, ly / dist, lz / dist];

                let ndotl = (normal[0] * ldir[0] + normal[1] * ldir[1] + normal[2] * ldir[2]).max(0.0);
                let spread = lamp.spread.clamp(0.08, 3.0);
                let falloff = (-(lx * lx + ly * ly) / (2.0 * spread * spread)).exp();
                let gain = lamp.intensity * ndotl * falloff;

                // Subtle Blinn-Phong view-aligned highlight (view = [0, 0, 1]).
                let hx = ldir[0];
                let hy = ldir[1];
                let hz = ldir[2] + 1.0;
                let hlen = (hx * hx + hy * hy + hz * hz).sqrt().max(1e-4);
                let ndoth = ((normal[0] * hx + normal[1] * hy + normal[2] * hz) / hlen).max(0.0);
                let spec = ndoth.powi(20) * lamp.intensity * falloff * 0.14 * relief.min(1.5);

                for c in 0..3 {
                    let lc = srgb_to_linear(lamp.color[c].clamp(0.0, 1.0));
                    lit[c] += gain * lc;
                    spec_accum[c] += spec * lc;
                }
            }

            for c in 0..3 {
                let base_albedo = lin[c] * (1.0 + 0.35 * (albedo_boost - 1.0));
                let shaded = base_albedo * lit[c] + spec_accum[c];
                let rolled = shaded / (1.0 + 0.18 * (shaded - 1.0).max(0.0));
                q[c] = linear_to_srgb(rolled.clamp(0.0, 1.0));
            }
        }
    });
}

/// Surface wrapper for `Relight3d`.
pub fn apply_relight_3d_surface(src: &Surface, rect: Rect, params: &Relight3dParams) -> Surface {
    if rect.is_empty() {
        return Surface::new(src.format());
    }
    let fmt = src.format();
    let ch = fmt.channels();
    let w = rect.width() as usize;
    let h = rect.height() as usize;
    let raw = src.read_region(rect);
    let mut rgba_px: Vec<[f32; 4]> = raw.chunks_exact(ch).map(|p| to_rgba(&fmt, p)).collect();
    apply_relight_3d_rgba(&mut rgba_px, w, h, params);
    let mut out_buf = vec![0.0f32; w * h * ch];
    for (p, dst) in rgba_px.iter().zip(out_buf.chunks_exact_mut(ch)) {
        from_rgba_into(&fmt, *p, dst);
    }
    let mut out = Surface::with_default(fmt, &src.default_pixel());
    out.write_region(rect, &out_buf);
    out
}

/// Applies Photoshop 2027 Mockup surface displacement + `Environmental Lighting` (`환경 조명`)
/// when compositing `decal_px` onto `base_px` (`w × h`, straight sRGB RGBA) and writes the
/// combined Mockup result into `out_px`.
pub fn composite_mockup_rgba(base_px: &[[f32; 4]], decal_px: &[[f32; 4]], out_px: &mut [[f32; 4]], w: usize, h: usize, lighting_strength: f32) {
    let n = w * h;
    if n == 0 || base_px.len() < n || decal_px.len() < n || out_px.len() < n {
        return;
    }
    let strength = (if lighting_strength.is_finite() { lighting_strength } else { 0.8 }).clamp(0.0, 2.0);
    let mut base_luma = vec![0.5f32; n];
    for i in 0..n {
        let b = base_px[i];
        base_luma[i] = (0.2126 * b[0] + 0.7152 * b[1] + 0.0722 * b[2]) * b[3];
    }
    let min_side = (w.min(h) as f32).max(16.0);
    let smooth_luma = blur_gray(&base_luma, w, h, (min_side * 0.02).clamp(1.2, 12.0));
    let mean_luma = {
        let mut sum = 0.0f64;
        let mut wsum = 0.0f64;
        for i in 0..n {
            let a = f64::from(base_px[i][3]);
            sum += f64::from(smooth_luma[i]) * a;
            wsum += a;
        }
        if wsum > 1e-6 { (sum / wsum) as f32 } else { 0.5 }
    }
    .clamp(0.15, 0.85);

    let mut decal_flat = vec![0.0f32; n * 4];
    for (i, d) in decal_px.iter().take(n).enumerate() {
        decal_flat[i * 4..i * 4 + 4].copy_from_slice(d);
    }
    let disp_scale = strength * (min_side * 0.022).clamp(1.0, 14.0);

    par_rows(out_px, w, 1, |y, row| {
        let ym = y.saturating_sub(1);
        let yp = (y + 1).min(h - 1);
        for (x, dst) in row.iter_mut().enumerate() {
            let idx = y * w + x;
            let base = base_px[idx];
            let xm = x.saturating_sub(1);
            let xp = (x + 1).min(w - 1);

            // Sobel-like surface slope from smoothed object luminance.
            let gx = smooth_luma[y * w + xp] - smooth_luma[y * w + xm];
            let gy = smooth_luma[yp * w + x] - smooth_luma[ym * w + x];

            let sx = (x as f32 - gx * disp_scale).clamp(0.0, (w - 1) as f32);
            let sy = (y as f32 - gy * disp_scale).clamp(0.0, (h - 1) as f32);
            let mut dec = [0.0f32; 4];
            for (c, item) in dec.iter_mut().enumerate() {
                *item = bilinear(&decal_flat, w, h, 4, c, sx, sy);
            }

            // Clip decal to object silhouette (`base[3]`) and transfer environmental diffuse + specular!
            let dec_a = (dec[3] * base[3]).clamp(0.0, 1.0);
            if dec_a <= 1e-5 {
                *dst = base;
                continue;
            }
            let local_l = base_luma[idx].clamp(0.0, 1.0);
            let shade_ratio = (local_l / mean_luma).clamp(0.15, 2.2);
            let spec_glint = ((local_l - 0.82).max(0.0) / 0.18).powi(2) * 0.35 * strength;

            let mut lit_decal = [0.0f32; 3];
            for c in 0..3 {
                let shaded = dec[c] * shade_ratio + spec_glint;
                lit_decal[c] = (dec[c] * (1.0 - strength.min(1.0)) + shaded * strength.min(1.0)).clamp(0.0, 1.0);
            }
            let out_a = base[3] + dec_a * (1.0 - base[3]);
            if out_a > 1e-6 {
                for c in 0..3 {
                    dst[c] = ((lit_decal[c] * dec_a + base[c] * base[3] * (1.0 - dec_a)) / out_a).clamp(0.0, 1.0);
                }
            }
            dst[3] = out_a.clamp(0.0, 1.0);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relight_3d_shifts_brightness_toward_lamp_position() {
        let (w, h) = (64, 64);
        let mut px = vec![[0.45f32, 0.45, 0.45, 1.0]; w * h];
        // Add a subtle sphere bump in the middle so normals respond too.
        for y in 0..h {
            for x in 0..w {
                let dx = (x as f32 - 32.0) / 24.0;
                let dy = (y as f32 - 32.0) / 24.0;
                let dome = (1.0 - (dx * dx + dy * dy)).max(0.0).sqrt() * 0.25;
                for ch in px[y * w + x].iter_mut().take(3) {
                    *ch = (0.35 + dome).clamp(0.0, 1.0);
                }
            }
        }
        let params = Relight3dParams {
            ambient: 0.20,
            surface_relief: 1.2,
            lamps: [
                Lamp3d { enabled: true, x: 0.15, y: 0.50, z: 0.45, intensity: 1.8, color: [1.0, 0.9, 0.8], spread: 0.45 },
                Lamp3d { enabled: false, ..Default::default() },
                Lamp3d { enabled: false, ..Default::default() },
            ],
        };
        apply_relight_3d_rgba(&mut px, w, h, &params);
        let left = px[32 * w + 16][0];
        let right = px[32 * w + 48][0];
        assert!(left > right + 0.12, "left side under lamp ({left}) should be brighter than right side ({right})");
    }

    #[test]
    fn mockup_lighting_transfers_base_shading_onto_decal() {
        let (w, h) = (48, 48);
        let mut base = vec![[0.0f32; 4]; w * h];
        let mut decal = vec![[0.0f32; 4]; w * h];
        for y in 0..h {
            for x in 0..w {
                let shade = 0.25 + 0.65 * (x as f32 / w as f32);
                base[y * w + x] = [shade, shade, shade, 1.0];
                if (12..36).contains(&x) && (12..36).contains(&y) {
                    decal[y * w + x] = [0.85, 0.20, 0.20, 1.0];
                }
            }
        }
        let mut out_lit = vec![[0.0f32; 4]; w * h];
        composite_mockup_rgba(&base, &decal, &mut out_lit, w, h, 1.0);
        // Decal on darker left side of base object receives darker environmental shading than right side!
        assert!(out_lit[24 * w + 32][0] > out_lit[24 * w + 15][0] + 0.15);
    }
}
