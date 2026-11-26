//! Substance-style deterministic procedural PBR material synthesis for 3D models, parametric
//! primitives, and extruded 3D text/shape layers.
//!
//! Synthesizes seamless tileable (`Wrap::Repeat`) physical PBR map sets ready for Cook-Torrance
//! GGX shading:
//! - `base_color`: sRGB straight RGBA `[0..=1]`
//! - `metallic_roughness`: linear RGBA with Roughness in `G` and Metallic in `B`
//! - `normal`: tangent-space unit normal map encoded in `[0..=1]` (`nz > 0`)
//! - `occlusion`: linear ambient occlusion in `R` (`[0..=1]`)

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{AlphaMode, Material, Model, TexRef, Texture, Wrap};

/// Built-in Substance-style procedural PBR material presets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubstancePreset {
    #[default]
    BrushedSteel,
    PolishedGold,
    CarbonHexWeave,
    TerrazzoMarble,
    CeramicTiles,
    WeatheredCopper,
}

impl SubstancePreset {
    pub const ALL: [SubstancePreset; 6] = [
        SubstancePreset::BrushedSteel,
        SubstancePreset::PolishedGold,
        SubstancePreset::CarbonHexWeave,
        SubstancePreset::TerrazzoMarble,
        SubstancePreset::CeramicTiles,
        SubstancePreset::WeatheredCopper,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::BrushedSteel => "brushedSteel",
            Self::PolishedGold => "polishedGold",
            Self::CarbonHexWeave => "carbonHexWeave",
            Self::TerrazzoMarble => "terrazzoMarble",
            Self::CeramicTiles => "ceramicTiles",
            Self::WeatheredCopper => "weatheredCopper",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::BrushedSteel => "Brushed Steel",
            Self::PolishedGold => "Polished Gold",
            Self::CarbonHexWeave => "Carbon Hex Weave",
            Self::TerrazzoMarble => "Terrazzo Marble",
            Self::CeramicTiles => "Ceramic Tiles",
            Self::WeatheredCopper => "Weathered Copper",
        }
    }

    pub fn parse(s: &str) -> Option<SubstancePreset> {
        let norm = s.trim().to_ascii_lowercase().replace([' ', '_', '-'], "");
        match norm.as_str() {
            "brushedsteel" | "steel" => Some(Self::BrushedSteel),
            "polishedgold" | "gold" => Some(Self::PolishedGold),
            "carbonhexweave" | "carbonweave" | "carbonhex" => Some(Self::CarbonHexWeave),
            "terrazzomarble" | "terrazzo" | "marble" => Some(Self::TerrazzoMarble),
            "ceramictiles" | "ceramic" | "tiles" => Some(Self::CeramicTiles),
            "weatheredcopper" | "copper" | "patinacopper" => Some(Self::WeatheredCopper),
            _ => None,
        }
    }
}

/// Synthesis parameters for a procedural PBR map set.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubstanceParams {
    /// Square texture map width/height in texels (clamped to `16..=1024`).
    pub resolution: u32,
    /// Deterministic PRNG seed.
    pub seed: u32,
    /// Surface wear / oxidation / micro-scratch level in `0.0..=1.0`.
    pub wear: f32,
    /// Normal map bump intensity multiplier (`0.0..=4.0`).
    pub normal_scale: f32,
}

impl Default for SubstanceParams {
    fn default() -> Self {
        Self {
            resolution: 128,
            seed: 1,
            wear: 0.25,
            normal_scale: 1.0,
        }
    }
}

/// Complete Cook-Torrance GGX PBR texture set synthesized from a [`SubstancePreset`].
#[derive(Clone, Debug, PartialEq)]
pub struct SubstancePbrMaps {
    pub preset: SubstancePreset,
    /// sRGB straight-alpha base colour `[0..=1]`.
    pub base_color: Texture,
    /// Linear metallic-roughness map: Roughness in `G`, Metallic in `B`.
    pub metallic_roughness: Texture,
    /// Tangent-space normal map encoded as `[nx * 0.5 + 0.5, ny * 0.5 + 0.5, nz * 0.5 + 0.5, 1.0]`.
    pub normal: Texture,
    /// Linear ambient occlusion map in `R` (`[0..=1]`).
    pub occlusion: Texture,
    pub normal_scale: f32,
    pub occlusion_strength: f32,
}

impl SubstancePbrMaps {
    /// Append the 4 PBR textures to `textures` and return a [`Material`] referencing them.
    pub fn to_material(&self, textures: &mut Vec<Arc<Texture>>) -> Material {
        let base_idx = textures.len();
        textures.push(Arc::new(self.base_color.clone()));
        let mr_idx = textures.len();
        textures.push(Arc::new(self.metallic_roughness.clone()));
        let norm_idx = textures.len();
        textures.push(Arc::new(self.normal.clone()));
        let ao_idx = textures.len();
        textures.push(Arc::new(self.occlusion.clone()));

        Material {
            name: self.preset.label().into(),
            base_color: [1.0, 1.0, 1.0, 1.0],
            base_color_tex: Some(TexRef { texture: base_idx, uv_set: 0 }),
            metallic: 1.0,
            roughness: 1.0,
            metallic_roughness_tex: Some(TexRef { texture: mr_idx, uv_set: 0 }),
            normal_tex: Some(TexRef { texture: norm_idx, uv_set: 0 }),
            normal_scale: self.normal_scale,
            occlusion_tex: Some(TexRef { texture: ao_idx, uv_set: 0 }),
            occlusion_strength: self.occlusion_strength,
            emissive: [0.0; 3],
            emissive_tex: None,
            alpha_mode: AlphaMode::Opaque,
            alpha_cutoff: 0.5,
            double_sided: false,
            unlit: false,
        }
    }

    /// Replace all materials on `model` with this procedural PBR material, computing tangents as needed.
    pub fn apply_to_model(&self, model: &mut Model) {
        let mat = self.to_material(&mut model.textures);
        let mat_idx = model.materials.len();
        model.materials.push(mat);
        for mesh in &mut model.meshes {
            for prim in &mut mesh.primitives {
                prim.material = Some(mat_idx);
                if !prim.uvs.is_empty() && prim.tangents.is_empty() {
                    prim.compute_tangents();
                }
            }
        }
    }
}

struct Sample {
    albedo: [f32; 3],
    roughness: f32,
    metallic: f32,
    height: f32,
    ao: f32,
}

/// Synthesize a complete deterministic PBR texture set for `preset` and `params`.
pub fn generate(preset: SubstancePreset, params: &SubstanceParams) -> SubstancePbrMaps {
    let n = params.resolution.clamp(16, 1024);
    let wear = params.wear.clamp(0.0, 1.0);
    let normal_scale = params.normal_scale.clamp(0.0, 4.0);
    let count = (n * n) as usize;

    let mut base_data = Vec::with_capacity(count);
    let mut mr_data = Vec::with_capacity(count);
    let mut ao_data = Vec::with_capacity(count);
    let mut heights = Vec::with_capacity(count);

    let inv_n = 1.0 / (n as f32);
    for y in 0..n {
        let v = (y as f32 + 0.5) * inv_n;
        for x in 0..n {
            let u = (x as f32 + 0.5) * inv_n;
            let s = eval_preset(preset, u, v, params.seed, wear);
            base_data.push([
                s.albedo[0].clamp(0.0, 1.0),
                s.albedo[1].clamp(0.0, 1.0),
                s.albedo[2].clamp(0.0, 1.0),
                1.0,
            ]);
            mr_data.push([1.0, s.roughness.clamp(0.04, 1.0), s.metallic.clamp(0.0, 1.0), 1.0]);
            let ao = s.ao.clamp(0.0, 1.0);
            ao_data.push([ao, ao, ao, 1.0]);
            heights.push(s.height);
        }
    }

    let slope = bump_strength_of(preset) * (n as f32 / 128.0);
    let normal_data = normals_from_heightfield(&heights, n, slope);
    let wrap = [Wrap::Repeat; 2];

    SubstancePbrMaps {
        preset,
        base_color: Texture { width: n, height: n, data: base_data, wrap },
        metallic_roughness: Texture { width: n, height: n, data: mr_data, wrap },
        normal: Texture { width: n, height: n, data: normal_data, wrap },
        occlusion: Texture { width: n, height: n, data: ao_data, wrap },
        normal_scale,
        occlusion_strength: 1.0,
    }
}

fn bump_strength_of(preset: SubstancePreset) -> f32 {
    match preset {
        SubstancePreset::BrushedSteel => 2.2,
        SubstancePreset::PolishedGold => 1.1,
        SubstancePreset::CarbonHexWeave => 4.2,
        SubstancePreset::TerrazzoMarble => 1.8,
        SubstancePreset::CeramicTiles => 4.8,
        SubstancePreset::WeatheredCopper => 2.8,
    }
}

fn normals_from_heightfield(heights: &[f32], n: u32, slope: f32) -> Vec<[f32; 4]> {
    let mut out = Vec::with_capacity((n * n) as usize);
    for y in 0..n {
        let ym = (y + n - 1) % n;
        let yp = (y + 1) % n;
        for x in 0..n {
            let xm = (x + n - 1) % n;
            let xp = (x + 1) % n;
            let dx = (heights[(y * n + xp) as usize] - heights[(y * n + xm) as usize]) * 0.5 * slope;
            let dy = (heights[(yp * n + x) as usize] - heights[(ym * n + x) as usize]) * 0.5 * slope;
            let raw = [-dx, -dy, 1.0f32];
            let len = (raw[0] * raw[0] + raw[1] * raw[1] + raw[2] * raw[2]).sqrt().max(1e-12);
            let nx = (raw[0] / len).clamp(-1.0, 1.0);
            let ny = (raw[1] / len).clamp(-1.0, 1.0);
            let nz = (raw[2] / len).clamp(1e-4, 1.0);
            out.push([nx * 0.5 + 0.5, ny * 0.5 + 0.5, nz * 0.5 + 0.5, 1.0]);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Deterministic Periodic Noise & Cellular Primitives
// ---------------------------------------------------------------------------

fn hash3(seed: u32, x: i32, y: i32) -> u32 {
    let mut h = seed
        ^ (x as u32).wrapping_mul(0x9e37_79b9)
        ^ (y as u32).wrapping_mul(0x85eb_ca6b);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^= h >> 16;
    h
}

fn rand01(seed: u32, x: i32, y: i32) -> f32 {
    (hash3(seed, x, y) as f32) * (1.0 / 4_294_967_296.0)
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0).max(1e-6)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [lerp(a[0], b[0], t), lerp(a[1], b[1], t), lerp(a[2], b[2], t)]
}

fn periodic_value_noise(u: f32, v: f32, period_x: i32, period_y: i32, seed: u32) -> f32 {
    let px = period_x.max(1);
    let py = period_y.max(1);
    let sx = u.rem_euclid(1.0) * px as f32;
    let sy = v.rem_euclid(1.0) * py as f32;
    let ix = sx.floor() as i32;
    let iy = sy.floor() as i32;
    let fx = smoothstep(0.0, 1.0, sx - ix as f32);
    let fy = smoothstep(0.0, 1.0, sy - iy as f32);

    let x0 = ix.rem_euclid(px);
    let x1 = (ix + 1).rem_euclid(px);
    let y0 = iy.rem_euclid(py);
    let y1 = (iy + 1).rem_euclid(py);

    let c00 = rand01(seed, x0, y0);
    let c10 = rand01(seed, x1, y0);
    let c01 = rand01(seed, x0, y1);
    let c11 = rand01(seed, x1, y1);
    lerp(lerp(c00, c10, fx), lerp(c01, c11, fx), fy)
}

fn periodic_fbm(u: f32, v: f32, fx: i32, fy: i32, octaves: u32, seed: u32) -> f32 {
    let mut sum = 0.0f32;
    let mut amp = 0.5f32;
    let mut norm = 0.0f32;
    let (mut px, mut py) = (fx.max(1), fy.max(1));
    for oct in 0..octaves {
        sum += amp * periodic_value_noise(u, v, px, py, seed.wrapping_add(oct * 0x1b87_3593));
        norm += amp;
        amp *= 0.5;
        px = (px * 2).min(512);
        py = (py * 2).min(512);
    }
    if norm > 0.0 { sum / norm } else { 0.5 }
}

/// Periodic Voronoi returning `(nearest_dist, edge_dist, cell_hash)`.
fn periodic_voronoi(u: f32, v: f32, cells: i32, seed: u32) -> (f32, f32, u32) {
    let n = cells.max(2);
    let su = u.rem_euclid(1.0) * n as f32;
    let sv = v.rem_euclid(1.0) * n as f32;
    let gx = su.floor() as i32;
    let gy = sv.floor() as i32;
    let mut d1 = f32::INFINITY;
    let mut d2 = f32::INFINITY;
    let mut best_id = 0u32;

    for oy in -1..=1 {
        for ox in -1..=1 {
            let cx = (gx + ox).rem_euclid(n);
            let cy = (gy + oy).rem_euclid(n);
            let jx = rand01(seed, cx, cy) * 0.82 + 0.09;
            let jy = rand01(seed ^ 0x55aa_1234, cx, cy) * 0.82 + 0.09;
            let dx = (gx + ox) as f32 + jx - su;
            let dy = (gy + oy) as f32 + jy - sv;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < d1 {
                d2 = d1;
                d1 = dist;
                best_id = hash3(seed ^ 0xa3c5_9ac3, cx, cy);
            } else if dist < d2 {
                d2 = dist;
            }
        }
    }
    (d1, (d2 - d1).max(0.0), best_id)
}

// ---------------------------------------------------------------------------
// Preset Surface Evaluators
// ---------------------------------------------------------------------------

fn eval_preset(preset: SubstancePreset, u: f32, v: f32, seed: u32, wear: f32) -> Sample {
    match preset {
        SubstancePreset::BrushedSteel => sample_brushed_steel(u, v, seed, wear),
        SubstancePreset::PolishedGold => sample_polished_gold(u, v, seed, wear),
        SubstancePreset::CarbonHexWeave => sample_carbon_hex_weave(u, v, seed, wear),
        SubstancePreset::TerrazzoMarble => sample_terrazzo_marble(u, v, seed, wear),
        SubstancePreset::CeramicTiles => sample_ceramic_tiles(u, v, seed, wear),
        SubstancePreset::WeatheredCopper => sample_weathered_copper(u, v, seed, wear),
    }
}

fn sample_brushed_steel(u: f32, v: f32, seed: u32, wear: f32) -> Sample {
    // Long horizontal brushed grooves + fine cross-grain micro-scratches.
    let brush = periodic_fbm(u, v, 4, 64, 3, seed);
    let fine_brush = periodic_value_noise(u, v, 8, 128, seed ^ 0x1111);
    let scratch_field = periodic_value_noise(u + v * 0.35, v - u * 0.25, 24, 24, seed ^ 0x2222);
    let scratch = smoothstep(0.92 - wear * 0.14, 0.99, scratch_field);

    let groove = 0.65 * brush + 0.35 * fine_brush;
    let lum = 0.74 + 0.14 * (groove - 0.5) - 0.12 * scratch;
    let albedo = [lum * 0.97, lum * 0.99, (lum * 1.03).clamp(0.0, 1.0)];
    let roughness = 0.20 + 0.14 * (1.0 - groove) + 0.22 * wear + 0.18 * scratch;
    let metallic = 0.98 - 0.08 * scratch * wear;
    let height = groove * 0.22 - scratch * 0.35;
    let ao = 0.90 + 0.10 * groove - 0.12 * scratch;
    Sample { albedo, roughness, metallic, height, ao }
}

fn sample_polished_gold(u: f32, v: f32, seed: u32, wear: f32) -> Sample {
    let broad = periodic_fbm(u, v, 4, 4, 3, seed);
    let micro = periodic_fbm(u, v, 24, 24, 2, seed ^ 0x7777);
    let scuff = smoothstep(0.86 - wear * 0.22, 0.98, micro) * wear;

    let k = 0.94 + 0.08 * (broad - 0.5) - 0.06 * scuff;
    let albedo = [(1.00 * k).clamp(0.0, 1.0), (0.84 * k).clamp(0.0, 1.0), (0.35 * k).clamp(0.0, 1.0)];
    let roughness = 0.06 + 0.05 * broad + 0.28 * scuff;
    let height = broad * 0.12 - scuff * 0.20;
    let ao = 0.97 - 0.06 * scuff;
    Sample { albedo, roughness, metallic: 1.0, height, ao }
}

fn sample_carbon_hex_weave(u: f32, v: f32, seed: u32, wear: f32) -> Sample {
    // Tri-axial hexagonal interlocking carbon weave on a periodic hex lattice (6x6 cells).
    let tau = std::f32::consts::TAU;
    let freq = 6.0;
    let p0 = u * freq;
    let p1 = (u * 0.5 + v * 0.866_025_4) * freq;
    let p2 = (-u * 0.5 + v * 0.866_025_4) * freq;

    let w0 = (p0 * tau).cos();
    let w1 = (p1 * tau).cos();
    let w2 = (p2 * tau).cos();

    // Pick dominant strand family around each hex dome; pockets between strands hold epoxy resin.
    let dome = (w0 + w1 + w2) / 3.0;
    let strand_arch = w0.max(w1).max(w2);
    let in_strand = smoothstep(-0.15, 0.35, strand_arch);
    let strand_grain = periodic_value_noise(u, v, 48, 48, seed ^ 0x3333);

    let fiber_col = [0.11 + 0.05 * strand_grain, 0.12 + 0.05 * strand_grain, 0.14 + 0.06 * strand_grain];
    let resin_col = [0.05, 0.055, 0.065];
    let albedo = lerp3(resin_col, fiber_col, in_strand);

    let roughness = lerp(0.11, 0.28, in_strand) + wear * 0.18 * strand_grain;
    let metallic = lerp(0.04, 0.22, in_strand);
    let height = 0.35 * in_strand * (0.5 + 0.5 * dome) + 0.04 * strand_grain;
    let ao = 0.62 + 0.38 * in_strand;
    Sample { albedo, roughness, metallic, height, ao }
}

fn sample_terrazzo_marble(u: f32, v: f32, seed: u32, wear: f32) -> Sample {
    let (d_big, edge_big, id_big) = periodic_voronoi(u, v, 7, seed);
    let (d_small, edge_small, id_small) = periodic_voronoi(u, v, 16, seed ^ 0x9999);
    let vein = periodic_fbm(u, v, 6, 6, 4, seed ^ 0x4444);

    let chip_big = (id_big & 0xff) < 145 && d_big < 0.36 + 0.08 * vein;
    let chip_small = !chip_big && (id_small & 0xff) < 110 && d_small < 0.28;

    let binder = {
        let base = 0.86 + 0.06 * (vein - 0.5);
        [base * 1.01, base * 0.99, base * 0.95]
    };
    let palette = [
        [0.78, 0.46, 0.40], // terracotta rose marble
        [0.24, 0.27, 0.30], // charcoal basalt
        [0.74, 0.78, 0.72], // sage serpentine
        [0.92, 0.85, 0.68], // warm ochre calcite
    ];

    let (albedo, is_chip, edge) = if chip_big {
        let c = palette[(id_big as usize >> 8) & 3];
        let v_tint = 0.92 + 0.16 * vein;
        ([c[0] * v_tint, c[1] * v_tint, c[2] * v_tint], true, edge_big)
    } else if chip_small {
        let c = palette[(id_small as usize >> 8) & 3];
        (c, true, edge_small)
    } else {
        (binder, false, 1.0)
    };

    let rim = smoothstep(0.0, 0.06, edge);
    let roughness = (if is_chip { 0.14 } else { 0.32 }) + wear * 0.24 * (1.0 - vein);
    let height = if is_chip { 0.16 * rim } else { 0.05 * vein } - wear * 0.08 * (1.0 - rim);
    let ao = 0.88 + 0.12 * rim;
    Sample { albedo, roughness, metallic: 0.0, height, ao }
}

fn sample_ceramic_tiles(u: f32, v: f32, seed: u32, wear: f32) -> Sample {
    let tiles = 4.0f32;
    let row = (v * tiles).floor() as i32;
    // Slight running-bond offset on odd rows? Use clean architectural grid with subtle per-tile variation.
    let tu = (u * tiles).fract();
    let tv = (v * tiles).fract();
    let col = (u * tiles).floor() as i32;

    let border = tu.min(1.0 - tu).min(tv.min(1.0 - tv));
    let grout_w = 0.045;
    let bevel_w = 0.11;
    let noise = periodic_fbm(u, v, 16, 16, 3, seed ^ 0x5555);
    let edge_chip = wear * 0.03 * noise;

    let tile_mask = smoothstep(grout_w + edge_chip, grout_w + 0.015 + edge_chip, border);
    let bevel = smoothstep(grout_w, bevel_w, border);

    let tile_tint = 0.94 + 0.10 * (rand01(seed, col.rem_euclid(4), row.rem_euclid(4)) - 0.5);
    let glaze_col = [0.80 * tile_tint, 0.86 * tile_tint, 0.89 * tile_tint];
    let grout_col = [0.44 + 0.08 * noise, 0.43 + 0.08 * noise, 0.41 + 0.07 * noise];

    let albedo = lerp3(grout_col, glaze_col, tile_mask);
    let roughness = lerp(0.86, 0.11 + 0.16 * wear * noise, tile_mask);
    let height = tile_mask * (0.15 + 0.35 * bevel) + (1.0 - tile_mask) * 0.05 * noise;
    let ao = lerp(0.56, 0.92 + 0.08 * bevel, tile_mask);
    Sample { albedo, roughness, metallic: 0.0, height, ao }
}

fn sample_weathered_copper(u: f32, v: f32, seed: u32, wear: f32) -> Sample {
    let base_fbm = periodic_fbm(u, v, 6, 6, 4, seed);
    let detail = periodic_fbm(u, v, 20, 20, 3, seed ^ 0x6666);
    let combined = 0.65 * base_fbm + 0.35 * detail;

    // Patina grows with `wear`: at wear=0 minimal crevice tarnish, at wear=1 heavy verdigris crust.
    let threshold = 0.82 - 0.68 * wear;
    let patina = smoothstep(threshold - 0.12, threshold + 0.12, combined);
    let tarnish = smoothstep(threshold - 0.25, threshold, combined) * (1.0 - patina);

    let copper = [0.95, 0.64, 0.53];
    let dark_oxide = [0.36, 0.22, 0.18];
    let verdigris = [0.30 + 0.10 * detail, 0.66 + 0.08 * base_fbm, 0.55 + 0.07 * detail];

    let metal_surface = lerp3(copper, dark_oxide, tarnish * 0.75);
    let albedo = lerp3(metal_surface, verdigris, patina);
    let metallic = lerp(0.98 - 0.25 * tarnish, 0.03, patina);
    let roughness = lerp(0.20 + 0.25 * tarnish, 0.78 + 0.12 * detail, patina);
    let height = 0.10 * detail + 0.26 * patina * combined;
    let ao = 0.96 - 0.18 * tarnish - 0.12 * patina * (1.0 - detail);
    Sample { albedo, roughness, metallic, height, ao }
}
