//! Suite A — Substance 3D Sampler (`Photo → PBR Material`, `Make It Tile`, and `Photo → IBL`):
//! - `make_seamless_rgba`: 4-quadrant toroidal `(w/2, h/2)` offset + smooth cosine cross-fade
//!   synthesis with low-frequency shade equalization so horizontal and vertical wraparound seams vanish.
//! - `sample_pbr_maps_rgba`: Multi-scale Retinex delighting (`BaseColor`), band-pass relief (`Height`),
//!   Sobel + fine-detail unit-normalized tangent-space `Normal`, local micro-variance `Roughness`,
//!   specular-chroma `Metallic`, and multi-radius cavity `AmbientOcclusion`.
//! - `extract_ibl_environment_rgba`: Converts a backdrop/photo into a smooth `2:1` equirectangular
//!   spherical lighting map (`w_env × h_env`).

use std::f32::consts::{PI, TAU};

use photocraft_geom::Rect;
use photocraft_raster::{Surface, from_rgba_into, to_rgba};
use serde::{Deserialize, Serialize};

use super::brdf::{linear3_to_srgb, sanitize_f32, srgb3_to_linear, tone_map_filmic, v3_norm};
use crate::photo_util::{bilinear, par_rows};

/// PBR texture channels extracted by Substance 3D Sampler.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum PbrChannel {
    #[default]
    BaseColor,
    Normal,
    Roughness,
    Metallic,
    Height,
    AmbientOcclusion,
}

impl PbrChannel {
    pub const ALL: [Self; 6] = [
        Self::BaseColor,
        Self::Normal,
        Self::Roughness,
        Self::Metallic,
        Self::Height,
        Self::AmbientOcclusion,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::BaseColor => "baseColor",
            Self::Normal => "normal",
            Self::Roughness => "roughness",
            Self::Metallic => "metallic",
            Self::Height => "height",
            Self::AmbientOcclusion => "ambientOcclusion",
        }
    }

    pub fn layer_name(self) -> &'static str {
        match self {
            Self::BaseColor => "BaseColor",
            Self::Normal => "Normal",
            Self::Roughness => "Roughness",
            Self::Metallic => "Metallic",
            Self::Height => "Height",
            Self::AmbientOcclusion => "AO",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::BaseColor => "Base Color (Albedo)",
            Self::Normal => "Normal (Tangent)",
            Self::Roughness => "Roughness",
            Self::Metallic => "Metallic",
            Self::Height => "Height (Displacement)",
            Self::AmbientOcclusion => "Ambient Occlusion",
        }
    }

    pub fn from_id(s: &str) -> Option<Self> {
        match s.trim() {
            "baseColor" | "albedo" | "BaseColor" => Some(Self::BaseColor),
            "normal" | "Normal" => Some(Self::Normal),
            "roughness" | "Roughness" => Some(Self::Roughness),
            "metallic" | "metalness" | "Metallic" => Some(Self::Metallic),
            "height" | "displacement" | "Height" => Some(Self::Height),
            "ambientOcclusion" | "ao" | "AO" => Some(Self::AmbientOcclusion),
            _ => None,
        }
    }
}

/// Parameters for `Photo → PBR Material` (`MaterialSampler`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MaterialSamplerParams {
    pub delight_strength: f32,
    pub make_tileable: bool,
    pub tile_feather: f32,
    pub normal_strength: f32,
    pub roughness_base: f32,
    pub roughness_contrast: f32,
    pub metallic_level: f32,
    pub ao_strength: f32,
}

impl Default for MaterialSamplerParams {
    fn default() -> Self {
        Self {
            delight_strength: 0.75,
            make_tileable: false,
            tile_feather: 0.22,
            normal_strength: 2.0,
            roughness_base: 0.45,
            roughness_contrast: 1.0,
            metallic_level: 0.0,
            ao_strength: 0.85,
        }
    }
}

impl MaterialSamplerParams {
    pub fn sanitized(&self) -> Self {
        Self {
            delight_strength: sanitize_f32(self.delight_strength, 0.75, 0.0, 1.0),
            make_tileable: self.make_tileable,
            tile_feather: sanitize_f32(self.tile_feather, 0.22, 0.05, 0.5),
            normal_strength: sanitize_f32(self.normal_strength, 2.0, 0.1, 8.0),
            roughness_base: sanitize_f32(self.roughness_base, 0.45, 0.04, 1.0),
            roughness_contrast: sanitize_f32(self.roughness_contrast, 1.0, 0.0, 2.0),
            metallic_level: sanitize_f32(self.metallic_level, 0.0, 0.0, 1.0),
            ao_strength: sanitize_f32(self.ao_strength, 0.85, 0.0, 2.0),
        }
    }
}

/// Full 6-map PBR material set generated from a single photograph (`w × h`, straight sRGB RGBA).
#[derive(Clone, Debug)]
pub struct PbrMaterialMaps {
    pub width: usize,
    pub height: usize,
    pub base_color: Vec<[f32; 4]>,
    pub normal: Vec<[f32; 4]>,
    pub roughness: Vec<[f32; 4]>,
    pub metallic: Vec<[f32; 4]>,
    pub height_map: Vec<[f32; 4]>,
    pub ambient_occlusion: Vec<[f32; 4]>,
}

impl PbrMaterialMaps {
    pub fn channel(&self, ch: PbrChannel) -> &[[f32; 4]] {
        match ch {
            PbrChannel::BaseColor => &self.base_color,
            PbrChannel::Normal => &self.normal,
            PbrChannel::Roughness => &self.roughness,
            PbrChannel::Metallic => &self.metallic,
            PbrChannel::Height => &self.height_map,
            PbrChannel::AmbientOcclusion => &self.ambient_occlusion,
        }
    }

    pub fn into_channel(self, ch: PbrChannel) -> Vec<[f32; 4]> {
        match ch {
            PbrChannel::BaseColor => self.base_color,
            PbrChannel::Normal => self.normal,
            PbrChannel::Roughness => self.roughness,
            PbrChannel::Metallic => self.metallic,
            PbrChannel::Height => self.height_map,
            PbrChannel::AmbientOcclusion => self.ambient_occlusion,
        }
    }
}

fn blur_scalar(src: &[f32], w: usize, h: usize, sigma: f32) -> Vec<f32> {
    let mut out = src.to_vec();
    crate::fxutil::gauss_blur_n(&mut out, w, h, 1, sigma.max(0.4));
    out
}

/// Smooth cosine border weight: `0.0` at toroidal border `u = 0` or `u = 1`, `1.0` inside `[feather, 1 - feather]`.
#[inline]
fn seam_cosine_weight(coord: usize, size: usize, feather: f32) -> f32 {
    if size <= 2 {
        return 0.5;
    }
    let u = coord as f32 / size as f32;
    let dist_border = u.min(1.0 - u);
    let t = (dist_border / feather.clamp(0.05, 0.5)).clamp(0.0, 1.0);
    0.5 - 0.5 * (PI * t).cos()
}

/// Converts an arbitrary RGBA texture (`w × h`) into a seamlessly tileable texture via
/// 4-quadrant toroidal offset `(w/2, h/2)` + smooth cosine/diamond cross-fade synthesis
/// and low-frequency illumination leveling.
pub fn make_seamless_rgba(px: &[[f32; 4]], w: usize, h: usize, feather: f32) -> Vec<[f32; 4]> {
    let n = w.saturating_mul(h);
    if n == 0 || px.len() < n {
        return Vec::new();
    }
    if w <= 2 || h <= 2 {
        return px[..n].to_vec();
    }
    let f = sanitize_f32(feather, 0.22, 0.05, 0.5);

    // Pre-level broad illumination gradient lightly so shifted quadrants blend without tonal patches.
    let mut luma = vec![0.0f32; n];
    let mut sum_l = 0.0f64;
    for (i, q) in px.iter().take(n).enumerate() {
        let l = (0.2126 * q[0] + 0.7152 * q[1] + 0.0722 * q[2]).clamp(0.0, 1.0);
        luma[i] = l;
        sum_l += f64::from(l);
    }
    let mean_l = ((sum_l / n as f64) as f32).clamp(0.05, 0.95);
    let coarse_sigma = ((w.min(h) as f32) * 0.22).clamp(4.0, 72.0);
    let env = blur_scalar(&luma, w, h, coarse_sigma);

    let mut leveled = vec![[0.0f32; 4]; n];
    for i in 0..n {
        let q = px[i];
        let e = env.get(i).copied().unwrap_or(mean_l).max(0.05);
        let gain = (1.0 + 0.55 * (mean_l / e - 1.0)).clamp(0.45, 2.2);
        leveled[i] = [
            (q[0] * gain).clamp(0.0, 1.0),
            (q[1] * gain).clamp(0.0, 1.0),
            (q[2] * gain).clamp(0.0, 1.0),
            q[3].clamp(0.0, 1.0),
        ];
    }

    let hx = w / 2;
    let hy = h / 2;
    let wx_lut: Vec<f32> = (0..w).map(|x| seam_cosine_weight(x, w, f)).collect();
    let wy_lut: Vec<f32> = (0..h).map(|y| seam_cosine_weight(y, h, f)).collect();

    let mut out = vec![[0.0f32; 4]; n];
    par_rows(&mut out, w, 1, |y, row| {
        let wy = wy_lut.get(y).copied().unwrap_or(0.5);
        let ys = (y + hy) % h;
        for (x, dst) in row.iter_mut().enumerate() {
            let wx = wx_lut.get(x).copied().unwrap_or(0.5);
            let xs = (x + hx) % w;
            let p00 = leveled.get(y * w + x).copied().unwrap_or([0.0; 4]);
            let p10 = leveled.get(y * w + xs).copied().unwrap_or([0.0; 4]);
            let p01 = leveled.get(ys * w + x).copied().unwrap_or([0.0; 4]);
            let p11 = leveled.get(ys * w + xs).copied().unwrap_or([0.0; 4]);

            // Smooth cosine/diamond product weights: zero seam across x=0, y=0 AND zero weight at shifted seam!
            let w00 = wx * wy;
            let w10 = (1.0 - wx) * wy;
            let w01 = wx * (1.0 - wy);
            let w11 = (1.0 - wx) * (1.0 - wy);

            for c in 0..4 {
                dst[c] = (w00 * p00[c] + w10 * p10[c] + w01 * p01[c] + w11 * p11[c]).clamp(0.0, 1.0);
            }
        }
    });
    out
}

/// Depth-agnostic `Surface` wrapper for `make_seamless_rgba`.
pub fn make_seamless_surface(src: &Surface, rect: Rect, feather: f32) -> Surface {
    if rect.is_empty() {
        return Surface::new(src.format());
    }
    let fmt = src.format();
    let ch = fmt.channels();
    let w = rect.width() as usize;
    let h = rect.height() as usize;
    let raw = src.read_region(rect);
    let rgba_in: Vec<[f32; 4]> = raw.chunks_exact(ch).map(|p| to_rgba(&fmt, p)).collect();
    let rgba_out = make_seamless_rgba(&rgba_in, w, h, feather);
    write_rgba_to_surface(&rgba_out, src, rect)
}

#[inline]
fn neighbor_idx(coord: usize, delta: isize, size: usize, wrap: bool) -> usize {
    if size == 0 {
        return 0;
    }
    let next = coord as isize + delta;
    if wrap {
        next.rem_euclid(size as isize) as usize
    } else {
        next.clamp(0, size as isize - 1) as usize
    }
}

fn compute_delighted_albedo_and_luma(
    src_px: &[[f32; 4]],
    w: usize,
    h: usize,
    delight_strength: f32,
) -> (Vec<[f32; 4]>, Vec<f32>, Vec<f32>) {
    let n = w * h;
    let mut lin_rgb = vec![[0.0f32; 3]; n];
    let mut luma = vec![0.0f32; n];
    let mut sum_luma = 0.0f64;
    let mut sum_w = 0.0f64;

    for (i, q) in src_px.iter().take(n).enumerate() {
        let lin = srgb3_to_linear([q[0], q[1], q[2]]);
        lin_rgb[i] = lin;
        let l = (0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2]).max(1e-4);
        luma[i] = l;
        let a = f64::from(q[3].clamp(0.0, 1.0));
        sum_luma += f64::from(l) * a;
        sum_w += a;
    }
    let mean_luma = if sum_w > 1e-5 { (sum_luma / sum_w) as f32 } else { 0.22 }.clamp(0.03, 0.85);

    let min_side = (w.min(h) as f32).max(8.0);
    let env_mid = blur_scalar(&luma, w, h, (min_side * 0.055).clamp(2.5, 28.0));
    let env_coarse = blur_scalar(&luma, w, h, (min_side * 0.19).clamp(6.0, 88.0));

    let mut base_color = vec![[0.0f32; 4]; n];
    let mut env_combined = vec![mean_luma; n];

    for i in 0..n {
        let env = (0.42 * env_mid[i] + 0.58 * env_coarse[i]).max(0.025);
        env_combined[i] = env;
        let ratio = (mean_luma / env).clamp(0.22, 3.8);
        let gain = 1.0 + delight_strength * (ratio - 1.0);
        let lin = lin_rgb[i];
        let delighted = tone_map_filmic([lin[0] * gain, lin[1] * gain, lin[2] * gain]);
        let srgb = linear3_to_srgb(delighted);
        let alpha = src_px[i][3].clamp(0.0, 1.0);
        base_color[i] = [srgb[0], srgb[1], srgb[2], alpha];
    }
    (base_color, luma, env_combined)
}

fn compute_bandpass_height(luma: &[f32], w: usize, h: usize) -> Vec<f32> {
    let n = w * h;
    let min_side = (w.min(h) as f32).max(8.0);
    let g1 = blur_scalar(luma, w, h, (min_side * 0.015).clamp(1.0, 6.0));
    let g2 = blur_scalar(luma, w, h, (min_side * 0.05).clamp(3.0, 22.0));
    let g3 = blur_scalar(luma, w, h, (min_side * 0.14).clamp(7.0, 56.0));

    let mut raw = vec![0.0f32; n];
    let mut sum = 0.0f64;
    for i in 0..n {
        let fine = luma[i] - g1[i];
        let mid = g1[i] - g2[i];
        let broad = g2[i] - g3[i];
        let v = 0.50 * fine + 0.35 * mid + 0.15 * broad;
        raw[i] = v;
        sum += f64::from(v);
    }
    let mean = (sum / n.max(1) as f64) as f32;
    let var: f64 = raw.iter().map(|&v| f64::from(v - mean).powi(2)).sum::<f64>() / n.max(1) as f64;
    let std_dev = (var.sqrt() as f32).max(1e-4);
    let scale = if std_dev < 5e-4 { 0.0 } else { 0.22 / std_dev };

    raw.into_iter().map(|v| (0.5 + (v - mean) * scale).clamp(0.0, 1.0)).collect()
}

/// Extracts all 6 PBR material maps (`BaseColor`, `Normal`, `Roughness`, `Metallic`, `Height`, `AO`)
/// from straight sRGB RGBA pixels (`w × h`).
pub fn sample_pbr_maps_rgba(
    px: &[[f32; 4]],
    w: usize,
    h: usize,
    params: &MaterialSamplerParams,
) -> PbrMaterialMaps {
    let n = w.saturating_mul(h);
    if n == 0 || px.len() < n {
        return PbrMaterialMaps {
            width: 0,
            height: 0,
            base_color: Vec::new(),
            normal: Vec::new(),
            roughness: Vec::new(),
            metallic: Vec::new(),
            height_map: Vec::new(),
            ambient_occlusion: Vec::new(),
        };
    }
    let p = params.sanitized();
    let work_owned = p.make_tileable.then(|| make_seamless_rgba(px, w, h, p.tile_feather));
    let work_px: &[[f32; 4]] = work_owned.as_deref().unwrap_or(&px[..n]);

    let (base_color, luma, env) = compute_delighted_albedo_and_luma(work_px, w, h, p.delight_strength);
    let height_scalar = compute_bandpass_height(&luma, w, h);

    // Local micro-variance for Roughness & multi-radius blurred height for Cavity AO.
    let luma_sq: Vec<f32> = luma.iter().map(|&l| l * l).collect();
    let mean_l = blur_scalar(&luma, w, h, 1.8);
    let mean_l2 = blur_scalar(&luma_sq, w, h, 1.8);

    let min_side = (w.min(h) as f32).max(8.0);
    let h_blur_s = blur_scalar(&height_scalar, w, h, 1.6);
    let h_blur_m = blur_scalar(&height_scalar, w, h, (min_side * 0.04).clamp(3.0, 16.0));
    let h_blur_l = blur_scalar(&height_scalar, w, h, (min_side * 0.10).clamp(6.0, 36.0));

    let mut normal = vec![[0.5f32, 0.5, 1.0, 1.0]; n];
    let mut height_map = vec![[0.5f32, 0.5, 0.5, 1.0]; n];
    let mut ambient_occlusion = vec![[1.0f32; 4]; n];
    let mut metallic = vec![[0.0f32, 0.0, 0.0, 1.0]; n];
    let mut micro_energy = vec![0.0f32; n];

    let wrap = p.make_tileable;
    for y in 0..h {
        let ym = neighbor_idx(y, -1, h, wrap);
        let yp = neighbor_idx(y, 1, h, wrap);
        for x in 0..w {
            let xm = neighbor_idx(x, -1, w, wrap);
            let xp = neighbor_idx(x, 1, w, wrap);
            let idx = y * w + x;
            let alpha = work_px[idx][3].clamp(0.0, 1.0);

            // 3x3 Sobel operator on blended band-pass height + fine luma detail
            let sample_h = |xx: usize, yy: usize| -> f32 {
                let k = yy * w + xx;
                0.78 * height_scalar[k] + 0.22 * luma[k]
            };
            let tl = sample_h(xm, ym);
            let tc = sample_h(x, ym);
            let tr = sample_h(xp, ym);
            let ml = sample_h(xm, y);
            let mr = sample_h(xp, y);
            let bl = sample_h(xm, yp);
            let bc = sample_h(x, yp);
            let br = sample_h(xp, yp);

            let gx = (tr + 2.0 * mr + br) - (tl + 2.0 * ml + bl);
            let gy = (bl + 2.0 * bc + br) - (tl + 2.0 * tc + tr);
            let n_vec = v3_norm([-gx * p.normal_strength, -gy * p.normal_strength, 1.0]);
            normal[idx] = [n_vec[0] * 0.5 + 0.5, n_vec[1] * 0.5 + 0.5, n_vec[2] * 0.5 + 0.5, alpha];

            let hv = height_scalar[idx];
            height_map[idx] = [hv, hv, hv, alpha];

            // Multi-radius cavity occlusion
            let cav_s = (h_blur_s[idx] - hv).max(0.0);
            let cav_m = (h_blur_m[idx] - hv).max(0.0);
            let cav_l = (h_blur_l[idx] - hv).max(0.0);
            let cavity = 1.9 * cav_s + 1.4 * cav_m + 0.9 * cav_l;
            let ao = (1.0 - p.ao_strength * cavity).clamp(0.0, 1.0);
            ambient_occlusion[idx] = [ao, ao, ao, alpha];

            // Micro-variance + gradient slope for Roughness
            let local_std = (mean_l2[idx] - mean_l[idx] * mean_l[idx]).max(0.0).sqrt();
            let grad_mag = (gx * gx + gy * gy).sqrt();
            micro_energy[idx] = 0.65 * local_std + 0.35 * grad_mag;

            // Specular-chroma metallic reflectance estimator
            let bc_rgb = base_color[idx];
            let max_c = bc_rgb[0].max(bc_rgb[1]).max(bc_rgb[2]);
            let min_c = bc_rgb[0].min(bc_rgb[1]).min(bc_rgb[2]);
            let chroma = max_c - min_c;
            let spec_ratio = (luma[idx] / env[idx].max(0.04)).clamp(0.0, 2.0) * 0.5;
            let conductor_hint = (0.55 * spec_ratio + 0.45 * (1.0 - 0.5 * chroma)).clamp(0.0, 1.0);
            let met = (p.metallic_level * (0.35 + 0.65 * conductor_hint)).clamp(0.0, 1.0);
            metallic[idx] = [met, met, met, alpha];
        }
    }

    let mean_micro = (micro_energy.iter().map(|&v| f64::from(v)).sum::<f64>() / n.max(1) as f64) as f32;
    let mut roughness = vec![[p.roughness_base, p.roughness_base, p.roughness_base, 1.0]; n];
    for i in 0..n {
        let delta = (micro_energy[i] - mean_micro) * 2.4 * p.roughness_contrast;
        let r = (p.roughness_base + delta).clamp(0.04, 1.0);
        roughness[i] = [r, r, r, work_px[i][3].clamp(0.0, 1.0)];
    }

    PbrMaterialMaps {
        width: w,
        height: h,
        base_color,
        normal,
        roughness,
        metallic,
        height_map,
        ambient_occlusion,
    }
}

/// Extracts a single PBR channel as RGBA (`w × h`).
pub fn sample_pbr_channel_rgba(
    px: &[[f32; 4]],
    w: usize,
    h: usize,
    channel: PbrChannel,
    params: &MaterialSamplerParams,
) -> Vec<[f32; 4]> {
    sample_pbr_maps_rgba(px, w, h, params).into_channel(channel)
}

/// Depth-agnostic `Surface` wrapper (`8`/`16`/`32f`) for `sample_pbr_channel_rgba`.
pub fn sample_pbr_channel_surface(
    src: &Surface,
    rect: Rect,
    channel: PbrChannel,
    params: &MaterialSamplerParams,
) -> Surface {
    if rect.is_empty() {
        return Surface::new(src.format());
    }
    let fmt = src.format();
    let ch = fmt.channels();
    let w = rect.width() as usize;
    let h = rect.height() as usize;
    let raw = src.read_region(rect);
    let rgba_in: Vec<[f32; 4]> = raw.chunks_exact(ch).map(|p| to_rgba(&fmt, p)).collect();
    let rgba_out = sample_pbr_channel_rgba(&rgba_in, w, h, channel, params);
    write_rgba_to_surface(&rgba_out, src, rect)
}

/// Converts a backdrop/photo (`w × h` straight sRGB RGBA) into a smooth `2:1` spherical
/// equirectangular environment lighting map (`w_env × h_env`) with mirror-seamless azimuth wrap.
pub fn extract_ibl_environment_rgba(
    backdrop_px: &[[f32; 4]],
    w: usize,
    h: usize,
    w_env: usize,
    h_env: usize,
) -> Vec<[f32; 4]> {
    let out_n = w_env.saturating_mul(h_env);
    if out_n == 0 {
        return Vec::new();
    }
    let in_n = w.saturating_mul(h);
    if in_n == 0 || backdrop_px.len() < in_n {
        return vec![[0.5, 0.52, 0.56, 1.0]; out_n];
    }

    let mut flat = vec![0.0f32; in_n * 3];
    for (i, q) in backdrop_px.iter().take(in_n).enumerate() {
        let a = q[3].clamp(0.0, 1.0);
        // Composite transparent backdrop areas over neutral mid-gray studio bounce.
        flat[i * 3] = q[0].clamp(0.0, 1.0) * a + 0.45 * (1.0 - a);
        flat[i * 3 + 1] = q[1].clamp(0.0, 1.0) * a + 0.46 * (1.0 - a);
        flat[i * 3 + 2] = q[2].clamp(0.0, 1.0) * a + 0.48 * (1.0 - a);
    }

    let mut env_rgb = vec![0.0f32; out_n * 3];
    let wf = w.saturating_sub(1) as f32;
    let hf = h.saturating_sub(1) as f32;

    for ey in 0..h_env {
        let v = (ey as f32 + 0.5) / h_env as f32;
        let theta = (0.5 - v) * PI;
        let sy = (0.5 - theta.sin() * 0.5).clamp(0.0, 1.0) * hf;
        // Pole convergence damping toward zenith/nadir average
        let pole_blend = theta.cos().abs().powf(0.65);

        for ex in 0..w_env {
            let u = (ex as f32 + 0.5) / w_env as f32;
            let phi = (u - 0.5) * TAU;
            // Mirror-periodic horizontal fold: 100% C^1 continuous across phi = +-pi!
            let fold_u = 0.5 + 0.5 * phi.cos() * pole_blend;
            let sx = fold_u.clamp(0.0, 1.0) * wf;
            let dst = (ey * w_env + ex) * 3;
            for c in 0..3 {
                env_rgb[dst + c] = bilinear(&flat, w, h, 3, c, sx, sy);
            }
        }
    }

    let smooth_sigma = ((h_env as f32) * 0.045).clamp(1.0, 8.0);
    crate::fxutil::gauss_blur_n(&mut env_rgb, w_env, h_env, 3, smooth_sigma);

    let mut out = Vec::with_capacity(out_n);
    for &px in env_rgb.as_chunks::<3>().0 {
        out.push([px[0].clamp(0.0, 1.0), px[1].clamp(0.0, 1.0), px[2].clamp(0.0, 1.0), 1.0]);
    }
    out
}

pub(crate) fn write_rgba_to_surface(rgba_px: &[[f32; 4]], template: &Surface, rect: Rect) -> Surface {
    let fmt = template.format();
    let ch = fmt.channels();
    let n = (rect.width() as usize).saturating_mul(rect.height() as usize);
    let mut out_buf = vec![0.0f32; n * ch];
    for (p, dst) in rgba_px.iter().take(n).zip(out_buf.chunks_exact_mut(ch)) {
        from_rgba_into(&fmt, *p, dst);
    }
    let mut out = Surface::with_default(fmt, &template.default_pixel());
    out.write_region(rect, &out_buf);
    out
}
