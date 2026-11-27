//! Gouraud lighting represented by an ordinary vector linear gradient on each triangle, and the
//! flat surface shading of the classic 3D effects.

use vectorcraft_geom::{Point, Vec2};

/// The surface of a classic 3D effect: how its faces are drawn and lit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ClassicSurface {
    /// Only the outline of every face, hidden ones included; the faces are transparent.
    Wireframe,
    /// Every face in the art's own paint, unlit.
    NoShading,
    /// Soft light: each face as bright as it faces the light.
    Diffuse,
    /// Diffuse light plus a glossy highlight where a face mirrors the light toward the viewer.
    #[default]
    Plastic,
}

impl ClassicSurface {
    pub const ALL: [ClassicSurface; 4] = [Self::Wireframe, Self::NoShading, Self::Diffuse, Self::Plastic];

    pub fn id(self) -> &'static str {
        match self {
            Self::Wireframe => "wireframe",
            Self::NoShading => "none",
            Self::Diffuse => "diffuse",
            Self::Plastic => "plastic",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.id() == s)
    }
}

/// The surface settings of a classic 3D effect. Shares are 0..1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClassicShading {
    pub surface: ClassicSurface,
    /// Light every face gets, whichever way it faces.
    pub ambient: f64,
    /// How bright the highlight is at its center (Plastic only).
    pub highlight_intensity: f64,
    /// How wide the highlight spreads: 1 broad, 0 a pinpoint (Plastic only).
    pub highlight_size: f64,
    /// How many levels the shading and the highlight are rounded to, 1..256.
    pub blend_steps: u32,
}

impl Default for ClassicShading {
    fn default() -> Self {
        Self { surface: ClassicSurface::Plastic, ambient: 0.5, highlight_intensity: 0.6, highlight_size: 0.9, blend_steps: 25 }
    }
}

impl ClassicShading {
    /// The shine exponent of the highlight: 2 at the largest size, 256 at the smallest.
    fn shininess(self) -> f64 {
        2f64.powf(1.0 + 7.0 * (1.0 - self.highlight_size.clamp(0.0, 1.0)))
    }

    /// `x` (0..1) rounded to the nearest of the blend steps' levels.
    fn stepped(self, x: f64) -> f64 {
        let steps = f64::from(self.blend_steps.clamp(1, 256));
        ((x.clamp(0.0, 1.0) * steps).round() / steps).clamp(0.0, 1.0)
    }

    /// The tone (0 in full shadow, 1 fully lit) and the highlight (0 none, 1 white) of a face with
    /// view-space unit normal `normal`, seen from the front and lit by `lights`: each a unit
    /// direction toward the light and its intensity (0..1). The lights add up. Wireframe and No
    /// Shading are unlit: tone 1, no highlight.
    pub fn tone(self, normal: [f64; 3], lights: &[([f64; 3], f64)]) -> (f64, f64) {
        if matches!(self.surface, ClassicSurface::Wireframe | ClassicSurface::NoShading) {
            return (1.0, 0.0);
        }
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let (mut tone, mut spec) = (self.ambient.clamp(0.0, 1.0), 0.0);
        for &(light, intensity) in lights {
            let intensity = if intensity.is_finite() { intensity.clamp(0.0, 1.0) } else { 0.0 };
            let lit = dot(normal, light).max(0.0);
            tone += intensity * lit;
            // The halfway direction between the light and the eye in front of the page.
            let half = [light[0], light[1], light[2] + 1.0];
            let length = dot(half, half).sqrt();
            if self.surface == ClassicSurface::Plastic && length > 1e-12 && lit > 0.0 {
                spec += intensity * (dot(normal, half) / length).max(0.0).powf(self.shininess()) * self.highlight_intensity.clamp(0.0, 1.0);
            }
        }
        if !tone.is_finite() || !spec.is_finite() {
            return (1.0, 0.0);
        }
        (self.stepped(tone), self.stepped(spec))
    }
}

/// Document-space gradient placement and the brightness at its two ends. Clipping a
/// face must keep this placement unchanged, so newly cut edges retain their lighting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadeGradient {
    pub start: Point,
    pub end: Point,
    pub brightness: [f64; 2],
}

impl ShadeGradient {
    pub(crate) fn triangle(points: &[Point], values: &[f64]) -> Option<Self> {
        let [p0, p1, p2] = points else { return None };
        let [b0, b1, b2] = values else { return None };
        let u = *p1 - *p0;
        let v = *p2 - *p0;
        let det = u.cross(v);
        if det.abs() < 1e-10 {
            return None;
        }
        // Solve the brightness plane in screen space. Its perpendicular is the gradient
        // vector, so all three corner values and shared triangle edges agree exactly.
        let g = Vec2::new(((b1 - b0) * v.y - (b2 - b0) * u.y) / det, (u.x * (b2 - b0) - v.x * (b1 - b0)) / det);
        let lo = b0.min(*b1).min(*b2);
        let hi = b0.max(*b1).max(*b2);
        if hi - lo < 1e-10 || g.hypot2() < 1e-30 {
            return None;
        }
        let start = *p0 + g * ((lo - b0) / g.hypot2());
        let end = *p0 + g * ((hi - b0) / g.hypot2());
        [start.x, start.y, end.x, end.y].iter().all(|n| n.is_finite()).then_some(Self { start, end, brightness: [lo, hi] })
    }

    /// Brightness at a projected point, including a point introduced by visibility clipping.
    pub fn at(self, p: Point) -> f64 {
        let v = self.end - self.start;
        let t = if v.hypot2() > 0.0 { ((p - self.start).dot(v) / v.hypot2()).clamp(0.0, 1.0) } else { 0.0 };
        self.brightness[0] + (self.brightness[1] - self.brightness[0]) * t
    }
}

/// A directional-light ramp before channel clipping. Keep ambient light neutral and add
/// stops where individual RGB channels saturate, so colored highlights retain their hue.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightGradient {
    pub diffuse: ShadeGradient,
    pub ambient: f64,
    pub color: [f64; 3],
}

impl LightGradient {
    pub fn at(self, p: Point) -> [f64; 3] {
        self.factors(self.diffuse.at(p))
    }

    fn factors(self, direct: f64) -> [f64; 3] {
        self.color.map(|c| (self.ambient + direct * c).clamp(0.0, 1.0))
    }

    /// At most five ordinary linear-gradient stops, including channel clipping points.
    pub fn stops(self) -> Vec<(f32, [f64; 3])> {
        let [lo, hi] = self.diffuse.brightness;
        let span = hi - lo;
        let mut positions = vec![0.0f64, 1.0];
        for c in self.color {
            if c > 0.0 && span > 0.0 {
                let t = ((1.0 - self.ambient) / c - lo) / span;
                if t > 0.0 && t < 1.0 {
                    positions.push(t);
                }
            }
        }
        positions.sort_by(f64::total_cmp);
        positions.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
        positions.into_iter().map(|t| (t as f32, self.factors(lo + span * t))).collect()
    }

    pub(crate) fn is_finite(self) -> bool {
        [
            self.diffuse.start.x,
            self.diffuse.start.y,
            self.diffuse.end.x,
            self.diffuse.end.y,
            self.diffuse.brightness[0],
            self.diffuse.brightness[1],
            self.ambient,
        ]
        .into_iter()
        .chain(self.color)
        .all(f64::is_finite)
    }
}

/// Independent affine RGB channels. Different light directions need different gradient axes;
/// interpolating a single luminance ramp would erase their color across a smooth surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RgbGradient {
    pub channels: [ShadeGradient; 3],
}

impl RgbGradient {
    pub(crate) fn triangle(points: &[Point], values: &[[f64; 3]]) -> Option<Self> {
        let [p, _, _] = points else { return None };
        let [first, _, _] = values else { return None };
        Some(Self {
            channels: std::array::from_fn(|channel| {
                let values: Vec<_> = values.iter().map(|v| v[channel]).collect();
                ShadeGradient::triangle(points, &values).unwrap_or(ShadeGradient { start: *p, end: *p, brightness: [first[channel]; 2] })
            }),
        })
    }

    pub fn at(self, p: Point) -> [f64; 3] {
        self.channels.map(|g| g.at(p).clamp(0.0, 1.0))
    }

    pub(crate) fn is_finite(self) -> bool {
        self.channels.iter().all(|g| [g.start.x, g.start.y, g.end.x, g.end.y, g.brightness[0], g.brightness[1]].into_iter().all(f64::is_finite))
    }
}

/// Physically based metallic-roughness material parameters (`0.0..=1.0`) for 3D & Materials and
/// Turntable vector facet shading.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PbrMaterial {
    pub metallic: f64,
    pub roughness: f64,
}

impl Default for PbrMaterial {
    fn default() -> Self {
        Self { metallic: 0.0, roughness: 0.5 }
    }
}

impl PbrMaterial {
    pub fn new(metallic: f64, roughness: f64) -> Self {
        Self {
            metallic: if metallic.is_finite() { metallic.clamp(0.0, 1.0) } else { 0.0 },
            roughness: if roughness.is_finite() { roughness.clamp(0.0, 1.0) } else { 0.5 },
        }
    }

    pub fn is_default(self) -> bool {
        self.metallic.abs() < 1e-6 && (self.roughness - 0.5).abs() < 1e-6
    }

    /// Evaluate Cook-Torrance / Trowbridge-Reitz GGX microfacet BRDF for a view-space unit
    /// `normal` (viewer at `[0, 0, 1]`) and directional `lights` (`(unit_dir, color_rgb, intensity)`).
    pub fn shade_normal_rgb(self, base_rgb: [f64; 3], normal: [f64; 3], lights: &[([f64; 3], [f64; 3], f64)], ambient: f64) -> [f64; 3] {
        let m = self.metallic.clamp(0.0, 1.0);
        let rough = self.roughness.clamp(0.045, 1.0);
        let alpha = rough * rough;
        let alpha2 = alpha * alpha;
        let k_geom = ((rough + 1.0) * (rough + 1.0)) / 8.0;
        let f0 = [0.04 * (1.0 - m) + base_rgb[0] * m, 0.04 * (1.0 - m) + base_rgb[1] * m, 0.04 * (1.0 - m) + base_rgb[2] * m];
        let n_dot_v = normal[2].abs().max(1e-4);
        let g_v = n_dot_v / (n_dot_v * (1.0 - k_geom) + k_geom);
        let amb = ambient.clamp(0.0, 2.0);
        let amb_kd = 1.0 - 0.65 * m;
        let mut out = [
            base_rgb[0] * amb * amb_kd + f0[0] * amb * 0.35 * m,
            base_rgb[1] * amb * amb_kd + f0[1] * amb * 0.35 * m,
            base_rgb[2] * amb * amb_kd + f0[2] * amb * 0.35 * m,
        ];
        let dot3 = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        for &(l_dir, l_col, intensity) in lights {
            let inten = if intensity.is_finite() { intensity.clamp(0.0, 4.0) } else { 0.0 };
            let n_dot_l = dot3(normal, l_dir).max(0.0);
            if n_dot_l <= 0.0 || inten <= 0.0 {
                continue;
            }
            let hx_raw = [l_dir[0], l_dir[1], l_dir[2] + 1.0];
            let h_len = dot3(hx_raw, hx_raw).sqrt().max(1e-9);
            let h = [hx_raw[0] / h_len, hx_raw[1] / h_len, hx_raw[2] / h_len];
            let n_dot_h = dot3(normal, h).max(0.0);
            let v_dot_h = h[2].max(0.0);

            let denom_d = n_dot_h * n_dot_h * (alpha2 - 1.0) + 1.0;
            let d_ggx = alpha2 / (std::f64::consts::PI * denom_d * denom_d).max(1e-9);
            let g_l = n_dot_l / (n_dot_l * (1.0 - k_geom) + k_geom);
            let g_smith = g_v * g_l;
            let fresnel_pow = (1.0 - v_dot_h).clamp(0.0, 1.0).powi(5);

            for ch in 0..3 {
                let f_ch = f0[ch] + (1.0 - f0[ch]) * fresnel_pow;
                let spec = (d_ggx * g_smith * f_ch) / (4.0 * n_dot_v * n_dot_l).max(1e-6);
                let kd = (1.0 - f_ch) * (1.0 - m);
                let diffuse = kd * base_rgb[ch] / std::f64::consts::PI;
                out[ch] += (diffuse * std::f64::consts::PI + spec) * l_col[ch] * inten * n_dot_l;
            }
        }
        out.map(|v| if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 })
    }

    /// Modulate pre-computed irradiance `light_rgb` on `base_rgb` with metallic reflection and
    /// roughness highlight response (used when a facet stores scalar/RGB irradiance ramps).
    pub fn shade_lit_rgb(self, base_rgb: [f64; 3], light_rgb: [f64; 3], ambient: f64) -> [f64; 3] {
        if self.is_default() {
            return [
                (base_rgb[0] * light_rgb[0]).clamp(0.0, 1.0),
                (base_rgb[1] * light_rgb[1]).clamp(0.0, 1.0),
                (base_rgb[2] * light_rgb[2]).clamp(0.0, 1.0),
            ];
        }
        let m = self.metallic.clamp(0.0, 1.0);
        let gloss = 1.0 - self.roughness.clamp(0.0, 1.0);
        let lum = (light_rgb[0] * 0.2126 + light_rgb[1] * 0.7152 + light_rgb[2] * 0.0722).clamp(0.0, 2.0);
        let spec_exp = 2.0 + gloss * 22.0;
        let spec_lobe = lum.min(1.0).powf(spec_exp) * (0.15 + 0.85 * gloss);
        let amb = ambient.clamp(0.0, 1.0);
        std::array::from_fn(|ch| {
            let f0 = 0.04 * (1.0 - m) + base_rgb[ch] * m;
            let diff = base_rgb[ch] * light_rgb[ch] * (1.0 - 0.7 * m);
            let metal_env = base_rgb[ch] * (amb * 0.4 + light_rgb[ch] * 0.85) * m;
            let spec = (f0 + (1.0 - f0) * 0.25 * gloss) * spec_lobe * light_rgb[ch].max(0.2);
            (diff + metal_env * 0.5 + spec).clamp(0.0, 1.0)
        })
    }
}

#[cfg(test)]
mod pbr_tests {
    use super::PbrMaterial;

    #[test]
    fn pbr_material_distinguishes_rough_matte_from_glossy_metal() {
        let base = [0.8, 0.5, 0.2];
        let normal = [0.0, 0.0, 1.0];
        let lights = [([0.0, 0.0, 1.0], [1.0, 1.0, 1.0], 0.9)];
        let matte = PbrMaterial::new(0.0, 0.95).shade_normal_rgb(base, normal, &lights, 0.2);
        let glossy_metal = PbrMaterial::new(1.0, 0.1).shade_normal_rgb(base, normal, &lights, 0.2);
        assert!(glossy_metal[0] > matte[0], "glossy metal specular peak exceeds rough matte: {glossy_metal:?} vs {matte:?}");
        assert!(PbrMaterial::default().is_default());
    }
}
