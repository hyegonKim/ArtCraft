//! Clean-room Substance-style 3D procedural PBR materials and analytical studio environment IBL.
//!
//! Evaluates deterministic triplanar / 3D procedural textures (anisotropic brushed micro-grooves,
//! domain-warped FBM + Worley marble veins, 2x2 twill/hex carbon weave, cavity/curvature patina
//! wear, glazed ceramic micro-crackle, frosted acrylic, and sculpted clay grain) into per-sample
//! `(albedo, metallic, roughness, normal, tangent_normal)` without external textures or `unsafe`.

use crate::PbrMaterial;

/// Procedural PBR material preset for 3D surfaces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SubstanceMaterialPreset {
    #[default]
    None,
    BrushedAluminum,
    PolishedGold,
    SatinCopper,
    WeatheredPatina,
    FrostedAcrylic,
    CarraraMarble,
    CarbonWeave,
    GlazedCeramic,
    MatteClay,
}

impl SubstanceMaterialPreset {
    pub const ALL: [Self; 10] = [
        Self::None,
        Self::BrushedAluminum,
        Self::PolishedGold,
        Self::SatinCopper,
        Self::WeatheredPatina,
        Self::FrostedAcrylic,
        Self::CarraraMarble,
        Self::CarbonWeave,
        Self::GlazedCeramic,
        Self::MatteClay,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::BrushedAluminum => "brushedAluminum",
            Self::PolishedGold => "polishedGold",
            Self::SatinCopper => "satinCopper",
            Self::WeatheredPatina => "weatheredPatina",
            Self::FrostedAcrylic => "frostedAcrylic",
            Self::CarraraMarble => "carraraMarble",
            Self::CarbonWeave => "carbonWeave",
            Self::GlazedCeramic => "glazedCeramic",
            Self::MatteClay => "matteClay",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::BrushedAluminum => "Brushed Aluminum",
            Self::PolishedGold => "Polished Gold",
            Self::SatinCopper => "Satin Copper",
            Self::WeatheredPatina => "Weathered Patina",
            Self::FrostedAcrylic => "Frosted Acrylic",
            Self::CarraraMarble => "Carrara Marble",
            Self::CarbonWeave => "Carbon Weave",
            Self::GlazedCeramic => "Glazed Ceramic",
            Self::MatteClay => "Matte Clay",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.id() == s)
    }

    /// Physical baseline `(metallic, roughness)` (`0.0..=1.0`) for each preset.
    pub fn baseline_metallic_roughness(self) -> (f64, f64) {
        match self {
            Self::None => (0.0, 0.50),
            Self::BrushedAluminum => (0.95, 0.28),
            Self::PolishedGold => (1.00, 0.11),
            Self::SatinCopper => (0.96, 0.30),
            Self::WeatheredPatina => (0.68, 0.46),
            Self::FrostedAcrylic => (0.00, 0.42),
            Self::CarraraMarble => (0.00, 0.15),
            Self::CarbonWeave => (0.24, 0.22),
            Self::GlazedCeramic => (0.03, 0.08),
            Self::MatteClay => (0.00, 0.86),
        }
    }
}

/// Parameters controlling procedural PBR evaluation on a 3D surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SubstanceMaterial {
    pub preset: SubstanceMaterialPreset,
    /// User roughness slider (`0.0..=1.0`, default `0.5`).
    pub roughness: f64,
    /// User metallic slider (`0.0..=1.0`, default `0.0`).
    pub metallic: f64,
    /// Procedural pattern frequency multiplier (`0.1..=5.0`, default `1.0`).
    pub scale: f64,
    /// Edge/cavity wear and patina amount (`0.0..=1.0`, default `0.0`).
    pub wear: f64,
    /// Bump/normal perturbation strength (`0.0..=2.0`, default `1.0`).
    pub normal_strength: f64,
    /// Deterministic procedural noise seed.
    pub seed: u32,
}

impl Default for SubstanceMaterial {
    fn default() -> Self {
        Self { preset: SubstanceMaterialPreset::None, roughness: 0.5, metallic: 0.0, scale: 1.0, wear: 0.0, normal_strength: 1.0, seed: 0 }
    }
}

/// Evaluated PBR material channels at one 3D surface sample.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PbrSample {
    pub albedo: [f64; 3],
    pub metallic: f64,
    pub roughness: f64,
    /// Perturbed unit normal in the caller's coordinate frame.
    pub normal: [f64; 3],
    /// Tangent-space normal perturbation (`[0, 0, 1]` when unperturbed).
    pub tangent_normal: [f64; 3],
}

fn clamp_finite(v: f64, fallback: f64, lo: f64, hi: f64) -> f64 {
    if v.is_finite() { v.clamp(lo, hi) } else { fallback }
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn lerp3(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn normalize3(v: [f64; 3], fallback: [f64; 3]) -> [f64; 3] {
    let l2 = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    if l2.is_finite() && l2 > 1e-20 {
        let inv = 1.0 / l2.sqrt();
        [v[0] * inv, v[1] * inv, v[2] * inv]
    } else {
        fallback
    }
}

fn mix_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^ (x >> 16)
}

fn hash3(ix: i32, iy: i32, iz: i32, seed: u32) -> u32 {
    let h = seed
        .wrapping_add((ix as u32).wrapping_mul(0x9e37_79b1))
        .wrapping_add((iy as u32).wrapping_mul(0x85eb_ca77))
        .wrapping_add((iz as u32).wrapping_mul(0xc2b2_ae3d));
    mix_u32(h)
}

fn hash_unit(h: u32) -> f64 {
    f64::from(h & 0x00ff_ffff) / 16_777_215.0
}

fn smooth5(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn safe_grid(v: f64) -> (i32, f64) {
    let v = if v.is_finite() { v.clamp(-1e6, 1e6) } else { 0.0 };
    let fl = v.floor();
    (fl as i32, v - fl)
}

/// Deterministic C2-continuous 3D value noise in `0.0..=1.0`.
pub fn value_noise3(p: [f64; 3], seed: u32) -> f64 {
    let (ix, fx) = safe_grid(p[0]);
    let (iy, fy) = safe_grid(p[1]);
    let (iz, fz) = safe_grid(p[2]);
    let (ux, uy, uz) = (smooth5(fx), smooth5(fy), smooth5(fz));
    let c = |dx: i32, dy: i32, dz: i32| hash_unit(hash3(ix.wrapping_add(dx), iy.wrapping_add(dy), iz.wrapping_add(dz), seed));
    let x00 = lerp(c(0, 0, 0), c(1, 0, 0), ux);
    let x10 = lerp(c(0, 1, 0), c(1, 1, 0), ux);
    let x01 = lerp(c(0, 0, 1), c(1, 0, 1), ux);
    let x11 = lerp(c(0, 1, 1), c(1, 1, 1), ux);
    lerp(lerp(x00, x10, uy), lerp(x01, x11, uy), uz)
}

/// Multi-octave 3D fractional Brownian motion in `0.0..=1.0`.
pub fn fbm3(mut p: [f64; 3], octaves: u32, seed: u32) -> f64 {
    let octaves = octaves.clamp(1, 6);
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut norm = 0.0;
    for i in 0..octaves {
        sum += amp * value_noise3(p, seed.wrapping_add(i.wrapping_mul(1013)));
        norm += amp;
        p = [p[0] * 2.03 + 13.7, p[1] * 2.03 - 7.1, p[2] * 2.03 + 5.3];
        amp *= 0.5;
    }
    if norm > 0.0 { (sum / norm).clamp(0.0, 1.0) } else { 0.5 }
}

/// Deterministic 3D Worley cellular distances `(f1, f2 - f1)`.
pub fn worley3(p: [f64; 3], seed: u32) -> (f64, f64) {
    let (ix, fx) = safe_grid(p[0]);
    let (iy, fy) = safe_grid(p[1]);
    let (iz, fz) = safe_grid(p[2]);
    let mut d1 = 8.0f64;
    let mut d2 = 8.0f64;
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let h = hash3(ix.wrapping_add(dx), iy.wrapping_add(dy), iz.wrapping_add(dz), seed);
                let jx = f64::from(dx) + hash_unit(h) * 0.85 - fx;
                let jy = f64::from(dy) + hash_unit(mix_u32(h.wrapping_add(1))) * 0.85 - fy;
                let jz = f64::from(dz) + hash_unit(mix_u32(h.wrapping_add(2))) * 0.85 - fz;
                let dist = (jx * jx + jy * jy + jz * jz).sqrt();
                if dist < d1 {
                    d2 = d1;
                    d1 = dist;
                } else if dist < d2 {
                    d2 = dist;
                }
            }
        }
    }
    (d1.clamp(0.0, 2.0), (d2 - d1).clamp(0.0, 2.0))
}

/// Branchless orthonormal tangent frame `(tangent, bitangent)` around unit normal `n` (Duff et al.).
pub(crate) fn tangent_basis(n: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let sign = if n[2] >= 0.0 { 1.0 } else { -1.0 };
    let a = -1.0 / (sign + n[2]);
    let b = n[0] * n[1] * a;
    let t = normalize3([1.0 + sign * n[0] * n[0] * a, sign * b, -sign * n[0]], [1.0, 0.0, 0.0]);
    let bt = normalize3([b, sign + n[1] * n[1] * a, -n[1]], [0.0, 1.0, 0.0]);
    (t, bt)
}

/// Blended triplanar UV coordinates from normalized object-space position `p` and unit normal `n`.
fn triplanar_uv(p: [f64; 3], n: [f64; 3]) -> [f64; 2] {
    let wx = n[0].abs().powi(4);
    let wy = n[1].abs().powi(4);
    let wz = n[2].abs().powi(4);
    let sum = (wx + wy + wz).max(1e-9);
    let (wx, wy, wz) = (wx / sum, wy / sum, wz / sum);
    let u = p[2] * wx + p[0] * wy + p[0] * wz;
    let v = p[1] * wx + p[2] * wy + p[1] * wz;
    [u, v]
}

struct EvalCtx {
    p: [f64; 3],
    uv: [f64; 2],
    base_rgb: [f64; 3],
    wear_mask: f64,
    seed: u32,
}

struct RawSurface {
    albedo: [f64; 3],
    metallic: f64,
    roughness: f64,
    bump_u: f64,
    bump_v: f64,
}

impl SubstanceMaterial {
    pub fn sanitized(self) -> Self {
        Self {
            preset: self.preset,
            roughness: clamp_finite(self.roughness, 0.5, 0.0, 1.0),
            metallic: clamp_finite(self.metallic, 0.0, 0.0, 1.0),
            scale: clamp_finite(self.scale, 1.0, 0.1, 5.0),
            wear: clamp_finite(self.wear, 0.0, 0.0, 1.0),
            normal_strength: clamp_finite(self.normal_strength, 1.0, 0.0, 2.0),
            seed: self.seed,
        }
    }

    pub fn is_none(self) -> bool {
        self.preset == SubstanceMaterialPreset::None
    }

    /// Evaluate procedural PBR surface properties at normalized 3D position `object_pos`
    /// (`~ -1..1` across the object) with `shading_normal` in the caller's shading frame,
    /// `object_normal` in object-local coordinates, `base_rgb` (`0..1`), and `curvature` (`0..1`,
    /// higher near sharp bevels/edges or concavities).
    pub fn sample_3d(self, object_pos: [f64; 3], object_normal: [f64; 3], shading_normal: [f64; 3], base_rgb: [f64; 3], curvature: f64) -> PbrSample {
        let s = self.sanitized();
        let n_shade = normalize3(shading_normal, [0.0, 0.0, 1.0]);
        let base = base_rgb.map(|c| clamp_finite(c, 0.5, 0.0, 1.0));
        if s.preset == SubstanceMaterialPreset::None {
            return PbrSample {
                albedo: base,
                metallic: s.metallic,
                roughness: s.roughness.clamp(0.045, 1.0),
                normal: n_shade,
                tangent_normal: [0.0, 0.0, 1.0],
            };
        }

        let n_obj = normalize3(object_normal, n_shade);
        let freq = 3.5 * s.scale;
        let p = [object_pos[0] * freq, object_pos[1] * freq, object_pos[2] * freq];
        let uv = triplanar_uv(p, n_obj);
        let wear_noise = fbm3([p[0] * 1.4, p[1] * 1.4, p[2] * 1.4], 3, s.seed.wrapping_add(7919));
        let wear_mask = (s.wear * (0.45 + 0.55 * curvature.clamp(0.0, 1.0)) * (0.55 + 0.9 * wear_noise)).clamp(0.0, 1.0);

        let ctx = EvalCtx { p, uv, base_rgb: base, wear_mask, seed: s.seed };
        let raw = eval_preset(s.preset, &ctx);

        // User roughness/metallic sliders smoothly offset around their default (0.5 / 0.0) so
        // presets immediately exhibit physical materials while remaining fully customizable.
        let rough_delta = (s.roughness - 0.5) * 0.85;
        let metal_blend = if s.metallic > 1e-4 { lerp(raw.metallic, s.metallic, 0.65) } else { raw.metallic };
        let roughness = (raw.roughness + rough_delta).clamp(0.045, 1.0);
        let metallic = metal_blend.clamp(0.0, 1.0);

        let strength = s.normal_strength * 0.42;
        let tn = normalize3([-raw.bump_u * strength, -raw.bump_v * strength, 1.0], [0.0, 0.0, 1.0]);
        let (t, bt) = tangent_basis(n_shade);
        let world_n = normalize3(
            [
                t[0] * tn[0] + bt[0] * tn[1] + n_shade[0] * tn[2],
                t[1] * tn[0] + bt[1] * tn[1] + n_shade[1] * tn[2],
                t[2] * tn[0] + bt[2] * tn[1] + n_shade[2] * tn[2],
            ],
            n_shade,
        );

        PbrSample { albedo: raw.albedo.map(|c| c.clamp(0.0, 1.0)), metallic, roughness, normal: world_n, tangent_normal: tn }
    }
}

fn tint_toward(preset_rgb: [f64; 3], base_rgb: [f64; 3], base_share: f64) -> [f64; 3] {
    let lum = base_rgb[0] * 0.2126 + base_rgb[1] * 0.7152 + base_rgb[2] * 0.0722;
    if (base_rgb[0] - base_rgb[1]).abs() < 0.02 && (base_rgb[1] - base_rgb[2]).abs() < 0.02 && lum > 0.92 {
        return preset_rgb;
    }
    lerp3(preset_rgb, base_rgb, base_share)
}

fn eval_preset(preset: SubstanceMaterialPreset, c: &EvalCtx) -> RawSurface {
    match preset {
        SubstanceMaterialPreset::None => RawSurface { albedo: c.base_rgb, metallic: 0.0, roughness: 0.5, bump_u: 0.0, bump_v: 0.0 },
        SubstanceMaterialPreset::BrushedAluminum => eval_brushed_aluminum(c),
        SubstanceMaterialPreset::PolishedGold => eval_polished_gold(c),
        SubstanceMaterialPreset::SatinCopper => eval_satin_copper(c),
        SubstanceMaterialPreset::WeatheredPatina => eval_weathered_patina(c),
        SubstanceMaterialPreset::FrostedAcrylic => eval_frosted_acrylic(c),
        SubstanceMaterialPreset::CarraraMarble => eval_carrara_marble(c),
        SubstanceMaterialPreset::CarbonWeave => eval_carbon_weave(c),
        SubstanceMaterialPreset::GlazedCeramic => eval_glazed_ceramic(c),
        SubstanceMaterialPreset::MatteClay => eval_matte_clay(c),
    }
}

fn eval_brushed_aluminum(c: &EvalCtx) -> RawSurface {
    // High-frequency anisotropic directional grooves stretched along u, fine across v.
    let groove_a = fbm3([c.uv[0] * 0.9, c.uv[1] * 26.0, c.p[2] * 0.7], 4, c.seed.wrapping_add(101));
    let groove_b = fbm3([c.uv[0] * 1.1, (c.uv[1] + 0.02) * 26.0, c.p[2] * 0.7], 4, c.seed.wrapping_add(101));
    let micro = (groove_a - 0.5) * 0.14;
    let base_al = tint_toward([0.88, 0.90, 0.93], c.base_rgb, 0.18);
    let oxide = [0.62, 0.64, 0.67];
    let albedo = lerp3([base_al[0] + micro, base_al[1] + micro, base_al[2] + micro], oxide, c.wear_mask * 0.65);
    RawSurface {
        albedo,
        metallic: lerp(0.96, 0.72, c.wear_mask),
        roughness: lerp(0.24 + (groove_a - 0.5).abs() * 0.16, 0.58, c.wear_mask),
        bump_u: micro * 0.25,
        bump_v: (groove_b - groove_a) * 4.2,
    }
}

fn eval_polished_gold(c: &EvalCtx) -> RawSurface {
    let scratch = fbm3([c.uv[0] * 12.0, c.uv[1] * 12.0, c.p[2] * 4.0], 3, c.seed.wrapping_add(211));
    let fine = value_noise3([c.p[0] * 7.5, c.p[1] * 7.5, c.p[2] * 7.5], c.seed.wrapping_add(223));
    let gold = tint_toward([1.00, 0.79, 0.34], c.base_rgb, 0.12);
    let tarnish = [0.46, 0.35, 0.16];
    RawSurface {
        albedo: lerp3(gold, tarnish, c.wear_mask * 0.7),
        metallic: lerp(1.0, 0.78, c.wear_mask * 0.5),
        roughness: lerp(0.09 + (scratch - 0.5).abs() * 0.08, 0.44, c.wear_mask),
        bump_u: (scratch - 0.5) * 0.35 + c.wear_mask * (fine - 0.5) * 0.8,
        bump_v: (fine - 0.5) * 0.35,
    }
}

fn eval_satin_copper(c: &EvalCtx) -> RawSurface {
    let grain = fbm3([c.uv[0] * 4.0, c.uv[1] * 16.0, c.p[2] * 2.5], 4, c.seed.wrapping_add(307));
    let (_, edge) = worley3([c.p[0] * 2.2, c.p[1] * 2.2, c.p[2] * 2.2], c.seed.wrapping_add(313));
    let copper = tint_toward([0.95, 0.62, 0.50], c.base_rgb, 0.15);
    let verdigris = [0.30, 0.62, 0.53];
    let patina_t = (c.wear_mask * (1.1 - edge * 0.5)).clamp(0.0, 1.0);
    RawSurface {
        albedo: lerp3(copper, verdigris, patina_t * 0.85),
        metallic: lerp(0.95, 0.12, patina_t),
        roughness: lerp(0.28 + (grain - 0.5) * 0.10, 0.76, patina_t),
        bump_u: (grain - 0.5) * 0.6,
        bump_v: (0.5 - edge.min(1.0)) * patina_t * 1.1 + (grain - 0.5) * 0.9,
    }
}

fn eval_weathered_patina(c: &EvalCtx) -> RawSurface {
    let warp = fbm3([c.p[0] * 1.3, c.p[1] * 1.3, c.p[2] * 1.3], 4, c.seed.wrapping_add(401));
    let (cell_d, cell_edge) = worley3([c.p[0] * 2.1 + warp * 0.9, c.p[1] * 2.1 - warp * 0.7, c.p[2] * 2.1], c.seed.wrapping_add(409));
    let base_patina = (0.42 + 0.58 * c.wear_mask) * (0.35 + 0.85 * warp) * (1.0 - 0.35 * cell_d);
    let mask = base_patina.clamp(0.0, 1.0);
    let aged_bronze = tint_toward([0.66, 0.50, 0.28], c.base_rgb, 0.20);
    let turquoise = [0.24, 0.64, 0.56];
    let dark_pit = [0.18, 0.24, 0.22];
    let pit = (1.0 - cell_edge * 3.0).clamp(0.0, 1.0) * 0.45;
    let col = lerp3(lerp3(aged_bronze, turquoise, mask), dark_pit, pit);
    RawSurface {
        albedo: col,
        metallic: lerp(0.88, 0.06, mask),
        roughness: lerp(0.32, 0.84, mask),
        bump_u: (warp - 0.5) * 1.4 - pit * 1.2,
        bump_v: (cell_d - 0.5) * 1.3 + pit * 1.1,
    }
}

fn eval_frosted_acrylic(c: &EvalCtx) -> RawSurface {
    let micro_u = value_noise3([c.p[0] * 18.0, c.p[1] * 18.0, c.p[2] * 18.0], c.seed.wrapping_add(503));
    let micro_v = value_noise3([c.p[0] * 18.0 + 4.1, c.p[1] * 18.0 - 2.3, c.p[2] * 18.0], c.seed.wrapping_add(509));
    let cool_frost = [0.88, 0.93, 0.98];
    let tinted = lerp3(c.base_rgb, cool_frost, 0.42 + 0.15 * c.wear_mask);
    RawSurface {
        albedo: tinted,
        metallic: 0.0,
        roughness: (0.42 + (micro_u - 0.5) * 0.12 + c.wear_mask * 0.28).clamp(0.12, 0.95),
        bump_u: (micro_u - 0.5) * 1.6,
        bump_v: (micro_v - 0.5) * 1.6,
    }
}

fn eval_carrara_marble(c: &EvalCtx) -> RawSurface {
    let w1 = fbm3([c.p[0] * 0.9, c.p[1] * 0.9, c.p[2] * 0.9], 5, c.seed.wrapping_add(601));
    let w2 = fbm3([c.p[0] * 1.8 + 3.1, c.p[1] * 1.8 - 1.7, c.p[2] * 1.8], 4, c.seed.wrapping_add(607));
    let (_, crack) = worley3([c.p[0] * 1.5 + w1 * 1.4, c.p[1] * 1.5 + w2 * 1.4, c.p[2] * 1.5], c.seed.wrapping_add(613));
    let phase = (c.p[0] * 1.1 + c.p[1] * 0.65 + c.p[2] * 0.45) + 3.6 * w1 + 1.2 * w2;
    let primary_vein = (1.0 - phase.sin().abs()).powi(7);
    let fine_vein = (1.0 - (crack * 4.5).clamp(0.0, 1.0)).powi(3) * 0.55;
    let vein = (primary_vein * 0.85 + fine_vein).clamp(0.0, 1.0);
    let stone = tint_toward([0.95, 0.94, 0.92], c.base_rgb, 0.25);
    let vein_col = [0.30, 0.33, 0.38];
    let albedo = lerp3(stone, vein_col, vein * 0.78);
    RawSurface {
        albedo: lerp3(albedo, [0.78, 0.76, 0.73], c.wear_mask * 0.5),
        metallic: 0.0,
        roughness: lerp(0.13 + vein * 0.18, 0.55, c.wear_mask),
        bump_u: -vein * 0.45,
        bump_v: (w2 - 0.5) * 0.25 - fine_vein * 0.5,
    }
}

fn eval_carbon_weave(c: &EvalCtx) -> RawSurface {
    // 2x2 twill interlocking carbon fiber weave in scaled triplanar coordinates.
    let su = c.uv[0] * 6.0;
    let sv = c.uv[1] * 6.0;
    let iu = su.floor() as i32;
    let iv = sv.floor() as i32;
    let fu = su - su.floor();
    let fv = sv - sv.floor();
    // 2x2 twill pattern: warp is on top for 2 cells, weft on top for 2 cells, shifted each row.
    let pattern = (iu.wrapping_add(iv)).rem_euclid(4);
    let warp_top = pattern < 2;
    let profile = |f: f64| ((f * std::f64::consts::PI).sin()).max(0.0);
    let strand = if warp_top { profile(fu) } else { profile(fv) };
    let slope = if warp_top { (fu * std::f64::consts::PI).cos() } else { (fv * std::f64::consts::PI).cos() };
    let dark_resin = [0.07, 0.08, 0.09];
    let fiber_sheen = tint_toward([0.22, 0.24, 0.27], c.base_rgb, 0.22);
    let base = lerp3(dark_resin, fiber_sheen, strand * 0.85);
    let scuff = [0.34, 0.35, 0.37];
    RawSurface {
        albedo: lerp3(base, scuff, c.wear_mask * 0.55),
        metallic: if warp_top { 0.30 } else { 0.18 },
        roughness: lerp(0.18 + (1.0 - strand) * 0.18, 0.62, c.wear_mask),
        bump_u: if warp_top { slope * 0.9 } else { 0.0 },
        bump_v: if warp_top { 0.0 } else { slope * 0.9 },
    }
}

fn eval_glazed_ceramic(c: &EvalCtx) -> RawSurface {
    let speckle = value_noise3([c.p[0] * 14.0, c.p[1] * 14.0, c.p[2] * 14.0], c.seed.wrapping_add(809));
    let (_, craze) = worley3([c.p[0] * 3.2, c.p[1] * 3.2, c.p[2] * 3.2], c.seed.wrapping_add(811));
    let crackle = (1.0 - (craze * 14.0).clamp(0.0, 1.0)) * (0.25 + 0.75 * c.wear_mask);
    let glaze = lerp3([0.92, 0.94, 0.95], c.base_rgb, 0.68);
    let speckle_col = [glaze[0] * 0.72, glaze[1] * 0.70, glaze[2] * 0.66];
    let raw_clay = [0.76, 0.66, 0.56];
    let speckled = if speckle > 0.83 { lerp3(glaze, speckle_col, 0.55) } else { glaze };
    let chipped = (c.wear_mask * 1.15 - 0.35).clamp(0.0, 1.0);
    let col = lerp3(lerp3(speckled, [0.42, 0.39, 0.36], crackle * 0.5), raw_clay, chipped);
    RawSurface {
        albedo: col,
        metallic: 0.02 * (1.0 - chipped),
        roughness: lerp(0.06 + crackle * 0.25, 0.80, chipped),
        bump_u: -crackle * 0.9 + chipped * (speckle - 0.5) * 1.2,
        bump_v: (speckle - 0.5) * 0.18 - crackle * 0.7,
    }
}

fn eval_matte_clay(c: &EvalCtx) -> RawSurface {
    let sculpt_u = fbm3([c.p[0] * 2.8, c.p[1] * 2.8, c.p[2] * 2.8], 4, c.seed.wrapping_add(907));
    let sculpt_v = fbm3([c.p[0] * 2.8 + 5.2, c.p[1] * 2.8 - 3.4, c.p[2] * 2.8], 4, c.seed.wrapping_add(911));
    let grain = value_noise3([c.p[0] * 16.0, c.p[1] * 16.0, c.p[2] * 16.0], c.seed.wrapping_add(919));
    let terracotta = tint_toward([0.78, 0.47, 0.35], c.base_rgb, 0.65);
    let dry_dust = [0.86, 0.76, 0.68];
    RawSurface {
        albedo: lerp3(terracotta, dry_dust, c.wear_mask * 0.6 + (grain - 0.5).max(0.0) * 0.15),
        metallic: 0.0,
        roughness: (0.85 + (grain - 0.5) * 0.10).clamp(0.65, 1.0),
        bump_u: (sculpt_u - 0.5) * 1.1 + (grain - 0.5) * 0.55,
        bump_v: (sculpt_v - 0.5) * 1.1 - (grain - 0.5) * 0.55,
    }
}

/// Evaluate an analytical studio HDRI environment probe in `y`-up coordinates (`x` right, `y` up,
/// `z` toward viewer), broadened smoothly by `roughness` (`0.0..=1.0`).
pub fn sample_studio_env(dir: [f64; 3], roughness: f64) -> [f64; 3] {
    let d = normalize3(dir, [0.0, 1.0, 0.0]);
    let r = clamp_finite(roughness, 0.5, 0.0, 1.0);
    let gloss = 1.0 - r;

    // Sky / ground base gradient
    let sky_t = (d[1] * 0.5 + 0.5).clamp(0.0, 1.0);
    let ground = [0.14, 0.15, 0.17];
    let horizon = [0.46, 0.49, 0.54];
    let zenith = [0.64, 0.74, 0.90];
    let dome = if d[1] >= 0.0 { lerp3(horizon, zenith, d[1].powf(0.65)) } else { lerp3(horizon, ground, (-d[1]).powf(0.55)) };

    // Overhead key softbox + warm right rim + soft front fill
    let dot3 = |a: [f64; 3], b: [f64; 3]| (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]).max(0.0);
    let key_dir = [-0.42, 0.74, 0.52];
    let rim_dir = [0.76, 0.28, 0.58];
    let fill_dir = [0.10, -0.15, 0.98];

    let exp_key = 2.0 + gloss * gloss * 42.0;
    let exp_rim = 2.0 + gloss * gloss * 24.0;
    let key_lobe = dot3(d, key_dir).powf(exp_key) * (0.28 + 1.35 * gloss);
    let rim_lobe = dot3(d, rim_dir).powf(exp_rim) * (0.18 + 0.75 * gloss);
    let fill_lobe = dot3(d, fill_dir).powf(2.0 + gloss * 8.0) * 0.22;

    let diffuse_avg = [0.44, 0.47, 0.52];
    let env = [
        dome[0] + key_lobe * 1.05 + rim_lobe * 1.08 + fill_lobe * 0.90,
        dome[1] + key_lobe * 1.02 + rim_lobe * 0.88 + fill_lobe * 0.94,
        dome[2] + key_lobe * 0.98 + rim_lobe * 0.70 + fill_lobe * 1.00,
    ];
    let _ = sky_t;
    lerp3(env, diffuse_avg, r * r * 0.65)
}

/// Shade a view-space (`y`-down, `z` toward viewer) facet using [`SubstanceMaterial`] + GGX +
/// studio environment IBL, for fast vector-mode preview and Turntable when `raytrace` is off.
pub fn shade_facet_substance(
    material: SubstanceMaterial,
    base_rgb: [f64; 3],
    object_pos: [f64; 3],
    object_normal: [f64; 3],
    view_normal: [f64; 3],
    lights: &[([f64; 3], [f64; 3], f64)],
    ambient: f64,
) -> [f64; 3] {
    let sample = material.sample_3d(object_pos, object_normal, view_normal, base_rgb, 0.35);
    let direct = PbrMaterial::new(sample.metallic, sample.roughness).shade_normal_rgb(
        sample.albedo,
        sample.normal,
        lights,
        ambient * (1.0 - 0.45 * sample.metallic),
    );
    // Add roughness-attenuated Schlick Fresnel studio IBL reflection (converting view y-down to y-up).
    let n_up = [sample.normal[0], -sample.normal[1], sample.normal[2]];
    let r_up = [2.0 * n_up[2] * n_up[0], 2.0 * n_up[2] * n_up[1], 2.0 * n_up[2] * n_up[2] - 1.0];
    let env = sample_studio_env(r_up, sample.roughness);
    let m = sample.metallic;
    let gloss = 1.0 - sample.roughness;
    let n_dot_v = sample.normal[2].abs().clamp(0.0, 1.0);
    let fresnel = (1.0 - n_dot_v).powi(5);
    std::array::from_fn(|ch| {
        let f0 = 0.04 * (1.0 - m) + sample.albedo[ch] * m;
        let f_ibl = f0 + (gloss.max(f0) - f0) * fresnel;
        let ibl = env[ch] * f_ibl * (0.28 + 0.42 * m);
        let combined = direct[ch] + ibl;
        // Soft shoulder tone map to preserve highlight hue without harsh clipping.
        (combined / (1.0 + 0.22 * combined.max(0.0))).clamp(0.0, 1.0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_substance_presets_produce_distinct_finite_samples() {
        let base = [0.75, 0.45, 0.25];
        let pos = [0.37, -0.24, 0.51];
        let n = normalize3([0.3, -0.4, 0.86], [0.0, 0.0, 1.0]);
        let mut samples = Vec::new();
        for preset in SubstanceMaterialPreset::ALL {
            let mat = SubstanceMaterial { preset, wear: 0.35, scale: 1.2, ..SubstanceMaterial::default() };
            let s = mat.sample_3d(pos, n, n, base, 0.5);
            assert!(s.albedo.iter().all(|c| c.is_finite() && (0.0..=1.0).contains(c)), "{preset:?}");
            assert!(s.metallic.is_finite() && (0.0..=1.0).contains(&s.metallic), "{preset:?}");
            assert!(s.roughness.is_finite() && (0.04..=1.0).contains(&s.roughness), "{preset:?}");
            let n_len = (s.normal[0] * s.normal[0] + s.normal[1] * s.normal[1] + s.normal[2] * s.normal[2]).sqrt();
            assert!((n_len - 1.0).abs() < 1e-6, "{preset:?} normal length {n_len}");
            samples.push((preset, s));
        }
        for i in 0..samples.len() {
            for j in (i + 1)..samples.len() {
                let (pa, a) = samples[i];
                let (pb, b) = samples[j];
                let diff = (a.albedo[0] - b.albedo[0]).abs()
                    + (a.albedo[1] - b.albedo[1]).abs()
                    + (a.albedo[2] - b.albedo[2]).abs()
                    + (a.metallic - b.metallic).abs()
                    + (a.roughness - b.roughness).abs()
                    + (a.tangent_normal[0] - b.tangent_normal[0]).abs()
                    + (a.tangent_normal[1] - b.tangent_normal[1]).abs();
                assert!(diff > 1e-3, "presets {pa:?} and {pb:?} produced nearly identical samples");
            }
        }
    }
}
