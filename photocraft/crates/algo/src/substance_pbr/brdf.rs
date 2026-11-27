//! Physically based Cook-Torrance GGX microfacet BRDF, 3D vector/matrix math, and spherical
//! equirectangular IBL sampling helpers shared across Substance 3D Sampler, Stager, and Designer.

use std::f32::consts::{FRAC_1_PI, PI, TAU};

use crate::photo_util::{bilinear, linear_to_srgb, srgb_to_linear};

#[inline]
pub(crate) fn sanitize_f32(v: f32, fallback: f32, lo: f32, hi: f32) -> f32 {
    if v.is_finite() { v.clamp(lo, hi) } else { fallback.clamp(lo, hi) }
}

#[inline]
pub(crate) fn v3_add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[inline]
pub(crate) fn v3_sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[inline]
pub(crate) fn v3_scale(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

#[inline]
pub(crate) fn v3_dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
pub(crate) fn v3_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

#[inline]
pub(crate) fn v3_len(a: [f32; 3]) -> f32 {
    v3_dot(a, a).max(0.0).sqrt()
}

#[inline]
pub(crate) fn v3_norm(a: [f32; 3]) -> [f32; 3] {
    let len2 = v3_dot(a, a);
    if len2 > 1e-12 && len2.is_finite() {
        let inv = len2.sqrt().recip();
        [a[0] * inv, a[1] * inv, a[2] * inv]
    } else {
        [0.0, 0.0, 1.0]
    }
}

#[inline]
pub(crate) fn v3_lerp(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let k = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, a[2] + (b[2] - a[2]) * k]
}

#[inline]
pub(crate) fn v3_reflect(incident: [f32; 3], normal: [f32; 3]) -> [f32; 3] {
    let d = 2.0 * v3_dot(incident, normal);
    v3_sub(incident, v3_scale(normal, d))
}

#[inline]
pub(crate) fn srgb3_to_linear(c: [f32; 3]) -> [f32; 3] {
    [srgb_to_linear(c[0].clamp(0.0, 1.0)), srgb_to_linear(c[1].clamp(0.0, 1.0)), srgb_to_linear(c[2].clamp(0.0, 1.0))]
}

#[inline]
pub(crate) fn linear3_to_srgb(c: [f32; 3]) -> [f32; 3] {
    [linear_to_srgb(c[0].clamp(0.0, 1.0)), linear_to_srgb(c[1].clamp(0.0, 1.0)), linear_to_srgb(c[2].clamp(0.0, 1.0))]
}

/// Soft ACES-inspired filmic highlight roll-off in linear RGB `[0, inf) -> [0, 1]`.
#[inline]
pub(crate) fn tone_map_filmic(lin: [f32; 3]) -> [f32; 3] {
    let map_one = |x: f32| -> f32 {
        let v = x.max(0.0);
        if !v.is_finite() {
            return 0.0;
        }
        // Shoulder roll-off that stays near-linear below 0.65 and smoothly approaches 1.0.
        let num = v * (1.0 + 0.32 * v);
        let den = 1.0 + 0.85 * v + 0.28 * v * v;
        (num / den.max(1e-6)).clamp(0.0, 1.0)
    };
    [map_one(lin[0]), map_one(lin[1]), map_one(lin[2])]
}

/// 3×3 rotation matrix for Euler angles `(rx, ry, rz)` in degrees.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Mat3 {
    pub rows: [[f32; 3]; 3],
}

impl Mat3 {
    pub fn from_euler_deg(rot_deg: [f32; 3]) -> Self {
        let rx = sanitize_f32(rot_deg[0], 0.0, -3600.0, 3600.0).to_radians();
        let ry = sanitize_f32(rot_deg[1], 0.0, -3600.0, 3600.0).to_radians();
        let rz = sanitize_f32(rot_deg[2], 0.0, -3600.0, 3600.0).to_radians();
        let (sx, cx) = rx.sin_cos();
        let (sy, cy) = ry.sin_cos();
        let (sz, cz) = rz.sin_cos();

        // R = Rz * Ry * Rx
        Self {
            rows: [
                [cz * cy, cz * sy * sx - sz * cx, cz * sy * cx + sz * sx],
                [sz * cy, sz * sy * sx + cz * cx, sz * sy * cx - cz * sx],
                [-sy, cy * sx, cy * cx],
            ],
        }
    }

    #[inline]
    pub fn mul_vec(&self, v: [f32; 3]) -> [f32; 3] {
        [v3_dot(self.rows[0], v), v3_dot(self.rows[1], v), v3_dot(self.rows[2], v)]
    }

    #[inline]
    pub fn transpose(&self) -> Self {
        Self {
            rows: [
                [self.rows[0][0], self.rows[1][0], self.rows[2][0]],
                [self.rows[0][1], self.rows[1][1], self.rows[2][1]],
                [self.rows[0][2], self.rows[1][2], self.rows[2][2]],
            ],
        }
    }
}

/// Trowbridge-Reitz GGX normal distribution function $D_{\text{GGX}}(N \cdot H, \alpha)$.
#[inline]
pub fn d_ggx(ndoth: f32, roughness: f32) -> f32 {
    let a = roughness.clamp(0.04, 1.0);
    let a2 = a * a;
    let a4 = a2 * a2;
    let nh = ndoth.clamp(0.0, 1.0);
    let denom = nh * nh * (a4 - 1.0) + 1.0;
    a4 / (PI * denom * denom).max(1e-6)
}

/// Schlick-GGX single-direction masking/shadowing term $G_1(N \cdot X, k)$.
#[inline]
fn g_schlick_ggx(ndotx: f32, k: f32) -> f32 {
    let nx = ndotx.clamp(0.0, 1.0);
    nx / (nx * (1.0 - k) + k).max(1e-6)
}

/// Smith separable height-correlated-style geometry function $G_{\text{Smith}}(N \cdot V, N \cdot L, \text{roughness})$.
#[inline]
pub fn g_smith(ndotv: f32, ndotl: f32, roughness: f32) -> f32 {
    let r = roughness.clamp(0.04, 1.0) + 1.0;
    let k = (r * r) * 0.125;
    g_schlick_ggx(ndotv, k) * g_schlick_ggx(ndotl, k)
}

/// Schlick Fresnel approximation $F_{\text{Schlick}}(V \cdot H, F_0)$.
#[inline]
pub fn f_schlick(cos_theta: f32, f0: [f32; 3]) -> [f32; 3] {
    let m = (1.0 - cos_theta.clamp(0.0, 1.0)).powi(5);
    [f0[0] + (1.0 - f0[0]) * m, f0[1] + (1.0 - f0[1]) * m, f0[2] + (1.0 - f0[2]) * m]
}

/// Roughness-aware Schlick Fresnel for split-sum ambient IBL reflections.
#[inline]
pub(crate) fn f_schlick_roughness(cos_theta: f32, f0: [f32; 3], roughness: f32) -> [f32; 3] {
    let m = (1.0 - cos_theta.clamp(0.0, 1.0)).powi(5);
    let gloss = 1.0 - roughness.clamp(0.04, 1.0);
    [f0[0] + (gloss.max(f0[0]) - f0[0]) * m, f0[1] + (gloss.max(f0[1]) - f0[1]) * m, f0[2] + (gloss.max(f0[2]) - f0[2]) * m]
}

/// Surface material point properties evaluated in linear RGB.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PbrSurfacePoint {
    pub albedo_lin: [f32; 3],
    pub normal: [f32; 3],
    pub roughness: f32,
    pub metallic: f32,
    pub ao: f32,
}

/// Evaluates direct Cook-Torrance GGX radiance for a directional light `(light_dir, light_radiance)`.
pub(crate) fn shade_ggx_direct(surf: &PbrSurfacePoint, view_dir: [f32; 3], light_dir: [f32; 3], light_radiance: [f32; 3]) -> [f32; 3] {
    let n = surf.normal;
    let v = view_dir;
    let l = light_dir;
    let ndotl = v3_dot(n, l).max(0.0);
    let ndotv = v3_dot(n, v).max(1e-4);
    if ndotl <= 0.0 {
        return [0.0; 3];
    }
    let h = v3_norm(v3_add(v, l));
    let ndoth = v3_dot(n, h).max(0.0);
    let vdoth = v3_dot(v, h).max(0.0);

    let f0 = v3_lerp([0.04, 0.04, 0.04], surf.albedo_lin, surf.metallic);
    let ndf = d_ggx(ndoth, surf.roughness);
    let geom = g_smith(ndotv, ndotl, surf.roughness);
    let fres = f_schlick(vdoth, f0);

    let spec_denom = (4.0 * ndotv * ndotl).max(1e-4);
    let spec_scale = (ndf * geom) / spec_denom;
    let kd_metal = 1.0 - surf.metallic.clamp(0.0, 1.0);

    let mut out = [0.0f32; 3];
    for c in 0..3 {
        let ks = fres[c];
        let kd = (1.0 - ks) * kd_metal;
        let diffuse = kd * surf.albedo_lin[c] * FRAC_1_PI;
        let specular = ks * spec_scale;
        out[c] = (diffuse + specular) * light_radiance[c] * ndotl;
    }
    out
}

/// Precomputed flattened linear-RGB equirectangular IBL probe (diffuse + sharp levels).
pub(crate) struct EquirectProbe {
    pub w: usize,
    pub h: usize,
    pub sharp_lin: Vec<f32>,
    pub diffuse_lin: Vec<f32>,
}

impl EquirectProbe {
    /// Builds a studio neutral sky/ground fallback probe (`64 × 32`).
    pub fn neutral_studio() -> Self {
        let (w, h) = (64usize, 32usize);
        let mut sharp_lin = vec![0.0f32; w * h * 3];
        let mut diffuse_lin = vec![0.0f32; w * h * 3];
        for y in 0..h {
            let v = (y as f32 + 0.5) / h as f32;
            let elev = (0.5 - v) * PI;
            let sky_t = (elev.sin() * 0.5 + 0.5).clamp(0.0, 1.0);
            for x in 0..w {
                let u = (x as f32 + 0.5) / w as f32;
                let phi = (u - 0.5) * TAU;
                // Subtle studio softbox highlights at +-55 deg azimuth
                let box_a = ((phi - 0.95).cos().max(0.0) * elev.cos().max(0.0)).powi(6) * 0.85;
                let box_b = ((phi + 1.15).cos().max(0.0) * elev.cos().max(0.0)).powi(8) * 0.45;
                let base = v3_lerp([0.10, 0.11, 0.13], [0.46, 0.50, 0.58], sky_t);
                let idx = (y * w + x) * 3;
                sharp_lin[idx] = base[0] + box_a * 1.0 + box_b * 0.75;
                sharp_lin[idx + 1] = base[1] + box_a * 0.96 + box_b * 0.86;
                sharp_lin[idx + 2] = base[2] + box_a * 0.90 + box_b * 1.0;
                diffuse_lin[idx] = base[0] * 0.88 + 0.08;
                diffuse_lin[idx + 1] = base[1] * 0.88 + 0.08;
                diffuse_lin[idx + 2] = base[2] * 0.88 + 0.08;
            }
        }
        Self { w, h, sharp_lin, diffuse_lin }
    }

    /// Builds a 2-level (glossy + diffuse irradiance) probe from straight sRGB equirectangular RGBA.
    pub fn from_equirect_rgba(env_rgba: &[[f32; 4]], w: usize, h: usize) -> Self {
        if w == 0 || h == 0 || env_rgba.len() < w.saturating_mul(h) {
            return Self::neutral_studio();
        }
        let n = w * h;
        let mut sharp_lin = vec![0.0f32; n * 3];
        for (i, q) in env_rgba.iter().take(n).enumerate() {
            let lin = srgb3_to_linear([q[0], q[1], q[2]]);
            sharp_lin[i * 3] = lin[0];
            sharp_lin[i * 3 + 1] = lin[1];
            sharp_lin[i * 3 + 2] = lin[2];
        }
        let mut diffuse_lin = sharp_lin.clone();
        let sigma = (h as f32 * 0.18).clamp(2.0, 24.0);
        crate::fxutil::gauss_blur_n(&mut diffuse_lin, w, h, 3, sigma);
        Self { w, h, sharp_lin, diffuse_lin }
    }

    /// Samples direction `dir` (unit 3D vector, Y-up) with `blur_mix` (`0` = sharp glossy, `1` = diffuse).
    pub fn sample_dir(&self, dir: [f32; 3], blur_mix: f32) -> [f32; 3] {
        let d = v3_norm(dir);
        let phi = d[0].atan2(d[2]); // [-pi, pi]
        let theta = d[1].clamp(-1.0, 1.0).asin(); // [-pi/2, pi/2]
        let u = (phi / TAU + 0.5).rem_euclid(1.0) * (self.w.saturating_sub(1) as f32);
        let v = (0.5 - theta / PI).clamp(0.0, 1.0) * (self.h.saturating_sub(1) as f32);
        let k = blur_mix.clamp(0.0, 1.0);
        let mut out = [0.0f32; 3];
        for (c, item) in out.iter_mut().enumerate() {
            let s = bilinear(&self.sharp_lin, self.w, self.h, 3, c, u, v);
            let b = bilinear(&self.diffuse_lin, self.w, self.h, 3, c, u, v);
            *item = s + (b - s) * k;
        }
        out
    }

    /// Evaluates ambient diffuse + split-sum glossy IBL contribution for `surf`.
    pub fn shade_ibl(&self, surf: &PbrSurfacePoint, view_dir: [f32; 3]) -> [f32; 3] {
        let ndotv = v3_dot(surf.normal, view_dir).max(1e-4);
        let f0 = v3_lerp([0.04, 0.04, 0.04], surf.albedo_lin, surf.metallic);
        let ks = f_schlick_roughness(ndotv, f0, surf.roughness);
        let kd_metal = 1.0 - surf.metallic.clamp(0.0, 1.0);

        let irradiance = self.sample_dir(surf.normal, 1.0);
        let refl_dir = v3_reflect(v3_scale(view_dir, -1.0), surf.normal);
        let rough_lod = (surf.roughness * surf.roughness).clamp(0.0, 1.0);
        let prefiltered = self.sample_dir(refl_dir, rough_lod);

        // Energy-conserving split-sum scale & bias approximation (Karis 2013 mobile/analytic fit)
        let gloss = 1.0 - surf.roughness.clamp(0.04, 1.0);
        let env_scale = (gloss * gloss * 0.72 + 0.22).clamp(0.05, 1.0);
        let env_bias = (1.0 - env_scale) * 0.04;

        let mut out = [0.0f32; 3];
        for c in 0..3 {
            let kd = (1.0 - ks[c]) * kd_metal;
            let diff = kd * surf.albedo_lin[c] * irradiance[c];
            let spec = prefiltered[c] * (ks[c] * env_scale + env_bias);
            out[c] = (diff + spec) * surf.ao;
        }
        out
    }
}
