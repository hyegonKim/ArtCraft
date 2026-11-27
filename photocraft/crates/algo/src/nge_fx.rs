//! Photoshop 2027 NextGen Filters / Adjustments:
//! - `Fireflies` (`반딧불이`): depth-aware glowing bokeh/firefly light particle synthesis.
//! - `Water Ripple` (`물결`): horizon-aware planar water reflection with perspective wave distortion.

use photocraft_geom::Rect;
use photocraft_raster::{Surface, from_rgba_into, to_rgba};
use serde::{Deserialize, Serialize};

use crate::photo_util::{bilinear, hash01, linear_to_srgb, par_rows, srgb_to_linear};

/// Parameters for NextGen `Fireflies` (`반딧불이`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FirefliesParams {
    /// Particle count (`1..=300`).
    pub quantity: u32,
    /// Near/far depth spread (`0.0..=100.0`).
    pub depth_variation: f32,
    /// Optical defocus / bokeh softness (`0.0..=100.0`).
    pub blurriness: f32,
    /// Base glow RGB tint (`0.0..=1.0`).
    pub color: [f32; 3],
    /// Drift phase (`0.0..=100.0`).
    pub progress: f32,
    /// Deterministic seed.
    pub seed: u32,
}

impl Default for FirefliesParams {
    fn default() -> Self {
        Self { quantity: 48, depth_variation: 65.0, blurriness: 45.0, color: [1.0, 0.84, 0.32], progress: 25.0, seed: 2027 }
    }
}

#[derive(Clone, Copy)]
struct Particle {
    cx: f32,
    cy: f32,
    radius: f32,
    core_radius: f32,
    softness: f32,
    ring_gain: f32,
    tint: [f32; 3],
    strength: f32,
}

/// Synthesizes depth-varied glowing bokeh fireflies over straight sRGB RGBA pixels (`w × h`).
pub fn apply_fireflies_rgba(px: &mut [[f32; 4]], w: usize, h: usize, params: &FirefliesParams) {
    if px.is_empty() || w == 0 || h == 0 || params.quantity == 0 {
        return;
    }
    let count = params.quantity.clamp(1, 300) as usize;
    let depth_var = (if params.depth_variation.is_finite() { params.depth_variation } else { 50.0 }).clamp(0.0, 100.0) / 100.0;
    let blur = (if params.blurriness.is_finite() { params.blurriness } else { 45.0 }).clamp(0.0, 100.0) / 100.0;
    let phase = (if params.progress.is_finite() { params.progress } else { 0.0 }).clamp(0.0, 100.0) / 100.0 * std::f32::consts::TAU;
    let base_col = [params.color[0].clamp(0.05, 1.0), params.color[1].clamp(0.05, 1.0), params.color[2].clamp(0.05, 1.0)];
    let min_side = (w.min(h) as f32).max(16.0);
    let seed = u64::from(params.seed);

    let mut particles = Vec::with_capacity(count);
    for i in 0..count {
        let idx = i as i64;
        let rx = hash01(idx, 11, seed);
        let ry = hash01(idx, 23, seed);
        let rz = hash01(idx, 37, seed);
        let rphase = hash01(idx, 53, seed) * std::f32::consts::TAU;
        let rhue = hash01(idx, 71, seed) - 0.5;

        // Orbital drift with `progress` so animating `progress` glides particles naturally.
        let drift_amp = 0.035 + 0.045 * rz;
        let u = (rx + drift_amp * (phase + rphase).cos()).rem_euclid(1.0);
        let v = (ry + drift_amp * 0.75 * (phase * 1.3 + rphase * 1.7).sin()).rem_euclid(1.0);
        let cx = u * w as f32;
        let cy = v * h as f32;

        // Sample underlying scene brightness so dark foliage/shadows attract slightly brighter glow contrast.
        let sx = (cx as usize).min(w.saturating_sub(1));
        let sy = (cy as usize).min(h.saturating_sub(1));
        let bg = px[sy * w + sx];
        let bg_luma = (0.2126 * bg[0] + 0.7152 * bg[1] + 0.0722 * bg[2]).clamp(0.0, 1.0);

        // Depth z in 0..1: z=0 far pinpoint firefly, z=1 foreground out-of-focus bokeh orb.
        let depth = 0.5 + (rz - 0.5) * depth_var;
        let radius_frac = 0.008 + 0.075 * depth * depth * (0.35 + 0.95 * blur);
        let radius = (radius_frac * min_side).clamp(0.45, min_side * 0.24);
        let core_radius = (radius * (0.42 - 0.26 * blur).clamp(0.12, 0.45)).max(0.20);
        let softness = 0.25 + 0.75 * blur;
        let ring_gain = depth * blur * 0.55;
        // Subtle warm/cool per-particle variation around user tint.
        let tint = [
            srgb_to_linear((base_col[0] + 0.12 * rhue).clamp(0.0, 1.0)),
            srgb_to_linear((base_col[1] + 0.05 * rhue).clamp(0.0, 1.0)),
            srgb_to_linear((base_col[2] - 0.10 * rhue).clamp(0.0, 1.0)),
        ];
        // Twinkle modulation with progress + depth energy conservation.
        let twinkle = 0.72 + 0.28 * (phase * 2.0 + rphase * 2.3).sin();
        let energy_scale = (1.15 - 0.55 * depth * blur) * (1.12 - 0.30 * bg_luma);
        let strength = (1.35 * twinkle * energy_scale).clamp(0.15, 2.2);

        particles.push(Particle { cx, cy, radius, core_radius, softness, ring_gain, tint, strength });
    }

    par_rows(px, w, 1, |y, row| {
        let fy = y as f32 + 0.5;
        // Pre-filter particles overlapping this scanline!
        let mut active: Vec<&Particle> = Vec::new();
        for p in &particles {
            if (fy - p.cy).abs() <= p.radius * 1.35 {
                active.push(p);
            }
        }
        if active.is_empty() {
            return;
        }
        for (x, q) in row.iter_mut().enumerate() {
            let fx = x as f32 + 0.5;
            let mut lin = [srgb_to_linear(q[0].max(0.0)), srgb_to_linear(q[1].max(0.0)), srgb_to_linear(q[2].max(0.0))];
            let mut glow_alpha = 0.0f32;
            for p in &active {
                let dx = fx - p.cx;
                let reach = p.radius * 1.35;
                if dx.abs() > reach {
                    continue;
                }
                let dy = fy - p.cy;
                let d2 = dx * dx + dy * dy;
                if d2 > reach * reach {
                    continue;
                }
                let d = d2.sqrt();
                let u = d / p.radius.max(1e-3);
                // Incandescent hot core + soft bokeh disc + subtle aperture edge ring.
                let core = (-(d / p.core_radius.max(0.18)).powi(2) * 1.8).exp();
                let disc_edge = ((1.08 - u) / (0.18 + 0.45 * p.softness)).clamp(0.0, 1.0);
                let disc = disc_edge * disc_edge * (3.0 - 2.0 * disc_edge);
                let ring = (-((u - 0.84) / 0.14).powi(2)).exp() * disc * p.ring_gain;
                let wgt = p.strength * (0.62 * core + 0.38 * disc + ring);
                if wgt <= 1e-5 {
                    continue;
                }
                // Hot center shifts slightly toward white like real bioluminescence/bokeh.
                let white_mix = (core * 0.55).clamp(0.0, 0.75);
                for (c, l_ch) in lin.iter_mut().enumerate() {
                    let emit = p.tint[c] * (1.0 - white_mix) + white_mix;
                    *l_ch += wgt * emit;
                }
                glow_alpha = (glow_alpha + (wgt * 0.45).clamp(0.0, 1.0)).min(1.0);
            }
            for c in 0..3 {
                // Soft highlight roll-off so overlapping fireflies bloom smoothly instead of harsh clipping.
                let mapped = lin[c] / (1.0 + 0.22 * (lin[c] - 1.0).max(0.0));
                q[c] = linear_to_srgb(mapped.clamp(0.0, 1.0));
            }
            q[3] = q[3].max(glow_alpha);
        }
    });
}

/// Parameters for NextGen `Water Ripple` (`물결`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WaterRippleParams {
    /// Normalized horizon line (`0.05..=0.95`, fraction from top).
    pub horizon: f32,
    /// Wave distortion amplitude (`0.0..=100.0`).
    pub amplitude: f32,
    /// Wave spatial frequency (`1.0..=100.0`).
    pub frequency: f32,
    /// Wave phase progress (`0.0..=100.0`).
    pub progress: f32,
}

impl Default for WaterRippleParams {
    fn default() -> Self {
        Self { horizon: 0.58, amplitude: 42.0, frequency: 36.0, progress: 20.0 }
    }
}

/// Applies horizon-aware planar water reflection and perspective sinusoidal wave ripples to `px` (`w × h`).
pub fn apply_water_ripple_rgba(px: &mut [[f32; 4]], w: usize, h: usize, params: &WaterRippleParams) {
    if px.is_empty() || w < 2 || h < 2 {
        return;
    }
    let horizon_t = (if params.horizon.is_finite() { params.horizon } else { 0.58 }).clamp(0.05, 0.95);
    let amp = (if params.amplitude.is_finite() { params.amplitude } else { 42.0 }).clamp(0.0, 100.0) / 100.0;
    let freq = (if params.frequency.is_finite() { params.frequency } else { 36.0 }).clamp(1.0, 100.0);
    let phase = (if params.progress.is_finite() { params.progress } else { 0.0 }).clamp(0.0, 100.0) / 100.0 * std::f32::consts::TAU;

    let y_h = horizon_t * h as f32;
    let water_span = (h as f32 - y_h).max(1.0);
    let y_start = (y_h.floor() as usize).min(h);
    if y_start >= h {
        return;
    }

    // Flat copy of source RGBA for bilinear reflection + refraction sampling.
    let mut src = vec![0.0f32; w * h * 4];
    for (i, q) in px.iter().enumerate() {
        let o = i * 4;
        src[o..o + 4].copy_from_slice(q);
    }

    let max_disp_x = amp * (w as f32 * 0.065).max(2.0);
    let max_disp_y = amp * (water_span * 0.14).max(2.0);

    par_rows(&mut px[y_start * w..], w, 1, |rel_y, row| {
        let y = y_start + rel_y;
        let fy = y as f32 + 0.5;
        let v = ((fy - y_h) / water_span).clamp(0.0, 1.0);
        // Perspective depth coordinate: compressed near horizon (v -> 0), wide ripples in foreground (v -> 1).
        let inv_z = 1.0 / (v + 0.065);
        let wave_envelope = v.sqrt() * (0.25 + 0.75 * v);
        // Schlick-like Fresnel: grazing angle near horizon reflects ~92%, steeper angle in foreground ~68%.
        let fresnel = (0.64 + 0.30 * (1.0 - v).powi(2)).clamp(0.0, 0.96);
        let shoreline_blend = (v / 0.025).clamp(0.0, 1.0);

        for (x, q) in row.iter_mut().enumerate() {
            let fx = x as f32 + 0.5;
            let u = fx / w as f32 - 0.5;

            // Multi-component capillary + swell wave field in perspective water coordinates.
            let k1 = inv_z * freq * 0.85 + phase;
            let k2 = (inv_z * 0.55 + u * 3.4) * freq * 0.62 - phase * 1.35;
            let k3 = (inv_z * 0.32 - u * 4.1) * freq * 0.48 + phase * 0.8;

            let nx = (k2.cos() * 0.55 - k3.cos() * 0.45 + (u * freq * 1.4 + phase).sin() * 0.25) * wave_envelope;
            let ny = (k1.sin() * 0.65 + k2.sin() * 0.22 + k3.sin() * 0.13) * wave_envelope;

            // Planar reflection coordinate above horizon + normal perturbation.
            let ref_x = (fx + nx * max_disp_x).clamp(0.0, (w - 1) as f32);
            let mirror_y = y_h - (fy - y_h);
            let ref_y = (mirror_y + ny * max_disp_y).clamp(0.0, y_h.max(0.0).min((h - 1) as f32));

            // Refracted submerged bed sample for water depth realism.
            let bed_x = (fx - nx * max_disp_x * 0.35).clamp(0.0, (w - 1) as f32);
            let bed_y = (fy + ny * max_disp_y * 0.25).clamp(0.0, (h - 1) as f32);

            let mut refl = [0.0f32; 4];
            let mut bed = [0.0f32; 4];
            for c in 0..4 {
                refl[c] = bilinear(&src, w, h, 4, c, ref_x, ref_y);
                bed[c] = bilinear(&src, w, h, 4, c, bed_x, bed_y);
            }

            // Cool-teal deep water absorption + wave crest specular glint.
            let water_tint = [0.78f32, 0.89, 0.96];
            let crest = (ny * amp * 1.6).max(0.0).powi(2) * 0.22;
            let orig = *q;
            for c in 0..3 {
                let tinted_bed = bed[c] * water_tint[c] * (0.82 - 0.28 * v);
                let mixed = refl[c] * fresnel * (0.94 + 0.06 * water_tint[c]) + tinted_bed * (1.0 - fresnel) + crest;
                q[c] = (orig[c] * (1.0 - shoreline_blend) + mixed * shoreline_blend).clamp(0.0, 1.0);
            }
            q[3] = orig[3].max(refl[3] * shoreline_blend);
        }
    });
}

/// Surface wrapper for `Fireflies`.
pub fn apply_fireflies_surface(src: &Surface, rect: Rect, params: &FirefliesParams) -> Surface {
    if rect.is_empty() {
        return Surface::new(src.format());
    }
    let fmt = src.format();
    let ch = fmt.channels();
    let w = rect.width() as usize;
    let h = rect.height() as usize;
    let raw = src.read_region(rect);
    let mut rgba_px: Vec<[f32; 4]> = raw.chunks_exact(ch).map(|p| to_rgba(&fmt, p)).collect();
    apply_fireflies_rgba(&mut rgba_px, w, h, params);
    let mut out_buf = vec![0.0f32; w * h * ch];
    for (p, dst) in rgba_px.iter().zip(out_buf.chunks_exact_mut(ch)) {
        from_rgba_into(&fmt, *p, dst);
    }
    let mut out = Surface::with_default(fmt, &src.default_pixel());
    out.write_region(rect, &out_buf);
    out
}

/// Surface wrapper for `Water Ripple`.
pub fn apply_water_ripple_surface(src: &Surface, rect: Rect, params: &WaterRippleParams) -> Surface {
    if rect.is_empty() {
        return Surface::new(src.format());
    }
    let fmt = src.format();
    let ch = fmt.channels();
    let w = rect.width() as usize;
    let h = rect.height() as usize;
    let raw = src.read_region(rect);
    let mut rgba_px: Vec<[f32; 4]> = raw.chunks_exact(ch).map(|p| to_rgba(&fmt, p)).collect();
    apply_water_ripple_rgba(&mut rgba_px, w, h, params);
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
    fn fireflies_adds_warm_bokeh_particles_deterministically() {
        let (w, h) = (96, 72);
        let base = vec![[0.08f32, 0.10, 0.14, 1.0]; w * h];
        let mut a = base.clone();
        let mut b = base.clone();
        let p = FirefliesParams { quantity: 30, depth_variation: 75.0, blurriness: 55.0, color: [1.0, 0.85, 0.30], progress: 15.0, seed: 42 };
        apply_fireflies_rgba(&mut a, w, h, &p);
        apply_fireflies_rgba(&mut b, w, h, &p);
        assert_eq!(a, b, "fireflies synthesis must be deterministic");
        let lit = a.iter().filter(|q| q[0] > 0.18).count();
        assert!(lit > 40, "expected glowing firefly pixels, got {lit}");
    }

    #[test]
    fn water_ripple_preserves_sky_and_reflects_below_horizon() {
        let (w, h) = (80, 80);
        let mut px = vec![[0.0f32; 4]; w * h];
        for y in 0..h {
            for x in 0..w {
                // Bright sun/sky in upper half, dark ground in lower half.
                px[y * w + x] = if y < 40 { [0.92, 0.70, 0.35, 1.0] } else { [0.05, 0.06, 0.08, 1.0] };
            }
        }
        let sky_row = px[10 * w..11 * w].to_vec();
        apply_water_ripple_rgba(&mut px, w, h, &WaterRippleParams { horizon: 0.5, amplitude: 50.0, frequency: 40.0, progress: 25.0 });
        assert_eq!(&px[10 * w..11 * w], sky_row.as_slice(), "sky above horizon stays intact");
        let avg_water_r: f32 = px[60 * w..61 * w].iter().map(|q| q[0]).sum::<f32>() / w as f32;
        assert!(avg_water_r > 0.45, "lower water half should reflect bright upper sky, got {avg_water_r}");
    }
}
