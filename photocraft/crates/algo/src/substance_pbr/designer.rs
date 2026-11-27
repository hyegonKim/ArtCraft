//! Suite C — Substance 3D Designer (`Procedural PBR Material Generator`):
//! Synthesizes tileable procedural height, albedo, roughness, metallic, ambient occlusion, and
//! tangent-space normal maps for 6 studio material graphs (`BrushedSteel`, `CarbonHexWeave`,
//! `TerrazzoMarble`, `CeramicTiles`, `WeatheredCopper`, `WovenFabric`) and shades them with
//! Cook-Torrance GGX + studio hemisphere IBL across 8-bit, 16-bit, and 32-bit float `Surface`s.

use std::f32::consts::{PI, TAU};

use photocraft_geom::Rect;
use photocraft_raster::Surface;
use serde::{Deserialize, Serialize};

use super::brdf::{
    EquirectProbe, PbrSurfacePoint, linear3_to_srgb, sanitize_f32, shade_ggx_direct, srgb3_to_linear, tone_map_filmic, v3_add, v3_lerp, v3_norm, v3_scale,
};
use super::sampler::write_rgba_to_surface;
use crate::photo_util::{hash01, par_rows};

/// Procedural Substance 3D Designer material presets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ProceduralPbrPreset {
    #[default]
    BrushedSteel,
    CarbonHexWeave,
    TerrazzoMarble,
    CeramicTiles,
    WeatheredCopper,
    WovenFabric,
}

impl ProceduralPbrPreset {
    pub const ALL: [Self; 6] = [Self::BrushedSteel, Self::CarbonHexWeave, Self::TerrazzoMarble, Self::CeramicTiles, Self::WeatheredCopper, Self::WovenFabric];

    pub fn id(self) -> &'static str {
        match self {
            Self::BrushedSteel => "brushedSteel",
            Self::CarbonHexWeave => "carbonHexWeave",
            Self::TerrazzoMarble => "terrazzoMarble",
            Self::CeramicTiles => "ceramicTiles",
            Self::WeatheredCopper => "weatheredCopper",
            Self::WovenFabric => "wovenFabric",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::BrushedSteel => "Brushed Steel",
            Self::CarbonHexWeave => "Carbon Hex Weave",
            Self::TerrazzoMarble => "Terrazzo Marble",
            Self::CeramicTiles => "Ceramic Tiles",
            Self::WeatheredCopper => "Weathered Copper",
            Self::WovenFabric => "Woven Fabric",
        }
    }

    pub fn from_id(s: &str) -> Option<Self> {
        match s.trim() {
            "brushedSteel" | "steel" | "BrushedSteel" => Some(Self::BrushedSteel),
            "carbonHexWeave" | "carbon" | "CarbonHexWeave" => Some(Self::CarbonHexWeave),
            "terrazzoMarble" | "terrazzo" | "TerrazzoMarble" => Some(Self::TerrazzoMarble),
            "ceramicTiles" | "tiles" | "CeramicTiles" => Some(Self::CeramicTiles),
            "weatheredCopper" | "copper" | "WeatheredCopper" => Some(Self::WeatheredCopper),
            "wovenFabric" | "fabric" | "WovenFabric" => Some(Self::WovenFabric),
            _ => None,
        }
    }
}

/// Parameters for the Procedural PBR Material Generator (`filter.material.proceduralPbr`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ProceduralPbrParams {
    pub preset: ProceduralPbrPreset,
    pub scale: f32,
    pub roughness: f32,
    pub normal_strength: f32,
    pub wear: f32,
    pub light_angle_deg: f32,
    pub light_elevation_deg: f32,
    pub seed: u32,
}

impl Default for ProceduralPbrParams {
    fn default() -> Self {
        Self {
            preset: ProceduralPbrPreset::BrushedSteel,
            scale: 4.0,
            roughness: 0.32,
            normal_strength: 1.6,
            wear: 0.25,
            light_angle_deg: 42.0,
            light_elevation_deg: 54.0,
            seed: 1337,
        }
    }
}

impl ProceduralPbrParams {
    pub fn sanitized(&self) -> Self {
        Self {
            preset: self.preset,
            scale: sanitize_f32(self.scale, 4.0, 0.25, 16.0),
            roughness: sanitize_f32(self.roughness, 0.32, 0.04, 1.0),
            normal_strength: sanitize_f32(self.normal_strength, 1.6, 0.1, 6.0),
            wear: sanitize_f32(self.wear, 0.25, 0.0, 1.0),
            light_angle_deg: sanitize_f32(self.light_angle_deg, 42.0, -360.0, 360.0),
            light_elevation_deg: sanitize_f32(self.light_elevation_deg, 54.0, 10.0, 85.0),
            seed: self.seed,
        }
    }
}

#[derive(Clone, Copy)]
struct ProcSample {
    height: f32,
    albedo_srgb: [f32; 3],
    roughness: f32,
    metallic: f32,
    ao: f32,
}

/// Periodic 2D value noise with integer period `(px, py)` so procedural graphs tile seamlessly!
fn periodic_noise(u: f32, v: f32, period: i64, seed: u64) -> f32 {
    let p = period.max(1);
    let x = u * p as f32;
    let y = v * p as f32;
    let ix0 = (x.floor() as i64).rem_euclid(p);
    let iy0 = (y.floor() as i64).rem_euclid(p);
    let ix1 = (ix0 + 1).rem_euclid(p);
    let iy1 = (iy0 + 1).rem_euclid(p);
    let fx = x - x.floor();
    let fy = y - y.floor();
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);

    let c00 = hash01(ix0, iy0, seed);
    let c10 = hash01(ix1, iy0, seed);
    let c01 = hash01(ix0, iy1, seed);
    let c11 = hash01(ix1, iy1, seed);
    let top = c00 + (c10 - c00) * sx;
    let bot = c01 + (c11 - c01) * sx;
    top + (bot - top) * sy
}

fn periodic_fbm(u: f32, v: f32, base_period: i64, seed: u64) -> f32 {
    let p = base_period.max(2);
    0.55 * periodic_noise(u, v, p, seed) + 0.30 * periodic_noise(u, v, p * 2, seed.wrapping_add(17)) + 0.15 * periodic_noise(u, v, p * 4, seed.wrapping_add(53))
}

fn eval_brushed_steel(u: f32, v: f32, p: &ProceduralPbrParams, seed: u64) -> ProcSample {
    let freq = (p.scale * 4.0).round().max(4.0) as i64;
    // Stretched horizontal anisotropic scratches
    let streak1 = periodic_noise(u, v * 12.0, freq, seed);
    let streak2 = periodic_noise(u * 0.5, v * 28.0, freq * 2, seed + 11);
    let cross_scratch = (periodic_noise(u * 6.0 + v * 1.5, v * 6.0, freq * 3, seed + 29) - 0.82).max(0.0) * p.wear * 2.5;
    let h = (0.58 * streak1 + 0.42 * streak2 - cross_scratch).clamp(0.0, 1.0);
    let base = 0.72 + 0.12 * (h - 0.5) - 0.18 * cross_scratch;
    ProcSample {
        height: h,
        albedo_srgb: [base * 0.96, base * 0.98, (base * 1.02).clamp(0.0, 1.0)],
        roughness: (p.roughness + 0.14 * (1.0 - h) + cross_scratch * 0.4).clamp(0.06, 0.95),
        metallic: (0.94 - cross_scratch * 0.25).clamp(0.5, 1.0),
        ao: (0.92 + 0.08 * h - cross_scratch * 0.3).clamp(0.3, 1.0),
    }
}

fn eval_carbon_hex_weave(u: f32, v: f32, p: &ProceduralPbrParams, seed: u64) -> ProcSample {
    let cells = p.scale.round().max(2.0);
    let gx = u * cells * 2.0;
    let gy = v * cells * 2.0;
    let ix = gx.floor() as i64;
    let iy = gy.floor() as i64;
    let fx = gx - gx.floor();
    let fy = gy - gy.floor();

    // 2x2 twill carbon-fiber ribbon weave: warp vs weft arch profile
    let warp_over = (ix + iy).rem_euclid(2) == 0;
    let (along, across) = if warp_over { (fy, fx) } else { (fx, fy) };
    let ribbon_edge = (1.0 - (across * 2.0 - 1.0).abs().powf(2.4)).clamp(0.0, 1.0);
    let arch = (along * PI).sin().powf(0.45);
    let micro_strands = ((across * 14.0 * PI).sin() * 0.5 + 0.5) * 0.08;
    let h = (ribbon_edge * (0.35 + 0.57 * arch) + micro_strands).clamp(0.0, 1.0);

    let scuff = periodic_fbm(u, v, cells as i64 * 2, seed) * p.wear;
    let shade = if warp_over { 0.16 } else { 0.12 } + 0.08 * arch + 0.10 * scuff;
    ProcSample {
        height: h,
        albedo_srgb: [shade, shade * 1.02, (shade * 1.07).clamp(0.0, 1.0)],
        roughness: (p.roughness * 0.75 + (1.0 - ribbon_edge) * 0.38 + scuff * 0.22).clamp(0.06, 0.95),
        metallic: 0.28,
        ao: (0.55 + 0.45 * ribbon_edge).clamp(0.25, 1.0),
    }
}

fn eval_terrazzo_marble(u: f32, v: f32, p: &ProceduralPbrParams, seed: u64) -> ProcSample {
    let cells = (p.scale * 2.0).round().max(3.0) as i64;
    let gx = u * cells as f32;
    let gy = v * cells as f32;
    let cx = gx.floor() as i64;
    let cy = gy.floor() as i64;

    let mut best_d = 10.0f32;
    let mut second_d = 10.0f32;
    let mut best_id = 0u64;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let nx = (cx + dx).rem_euclid(cells);
            let ny = (cy + dy).rem_euclid(cells);
            let jx = (cx + dx) as f32 + 0.15 + 0.70 * hash01(nx, ny, seed);
            let jy = (cy + dy) as f32 + 0.15 + 0.70 * hash01(nx, ny, seed + 7);
            // Angular polygonal mineral aggregate metric
            let ddx = gx - jx;
            let ddy = gy - jy;
            let ang = hash01(nx, ny, seed + 19) * TAU;
            let (sa, ca) = ang.sin_cos();
            let rx = (ddx * ca - ddy * sa).abs();
            let ry = (ddx * sa + ddy * ca).abs();
            let d = 0.65 * rx.hypot(ry) + 0.35 * rx.max(ry);
            if d < best_d {
                second_d = best_d;
                best_d = d;
                best_id = (nx as u64).wrapping_mul(131) ^ (ny as u64).wrapping_mul(977);
            } else if d < second_d {
                second_d = d;
            }
        }
    }

    let chip_radius = 0.28 + 0.14 * hash01(best_id as i64, 3, seed + 41);
    let in_chip = best_d < chip_radius && (second_d - best_d) > 0.06;
    let vein = periodic_fbm(u, v, cells, seed + 83);

    if in_chip {
        let palette = [
            [0.78f32, 0.44, 0.34], // terracotta
            [0.24, 0.30, 0.34],    // slate
            [0.88, 0.84, 0.76],    // quartz
            [0.42, 0.56, 0.50],    // sage serpentine
        ];
        let c = palette[(best_id as usize) % palette.len()];
        let bevel = ((chip_radius - best_d) / 0.04).clamp(0.0, 1.0);
        ProcSample {
            height: 0.54 + 0.12 * bevel - 0.08 * p.wear * vein,
            albedo_srgb: c,
            roughness: (p.roughness * 0.7 + 0.12 * p.wear).clamp(0.06, 0.9),
            metallic: 0.0,
            ao: 0.92 + 0.08 * bevel,
        }
    } else {
        let base = 0.86 - 0.10 * vein - 0.08 * p.wear;
        ProcSample {
            height: 0.46 + 0.06 * vein,
            albedo_srgb: [base, base * 0.99, base * 0.96],
            roughness: (p.roughness + 0.14 * vein).clamp(0.08, 0.95),
            metallic: 0.0,
            ao: 0.95,
        }
    }
}

fn eval_ceramic_tiles(u: f32, v: f32, p: &ProceduralPbrParams, seed: u64) -> ProcSample {
    let tiles = p.scale.round().max(2.0) as i64;
    let gx = u * tiles as f32;
    let gy = v * tiles as f32;
    let tx = (gx.floor() as i64).rem_euclid(tiles);
    let ty = (gy.floor() as i64).rem_euclid(tiles);
    let fx = gx - gx.floor();
    let fy = gy - gy.floor();

    let grout = 0.055;
    let edge_d = fx.min(1.0 - fx).min(fy.min(1.0 - fy));
    let n_fine = periodic_fbm(u, v, tiles * 3, seed + 61);

    if edge_d < grout {
        let depth = edge_d / grout;
        let g_col = 0.52 - 0.14 * p.wear + 0.08 * n_fine;
        ProcSample {
            height: 0.12 + 0.25 * depth,
            albedo_srgb: [g_col, g_col * 0.98, g_col * 0.95],
            roughness: 0.88,
            metallic: 0.0,
            ao: (0.48 + 0.42 * depth).clamp(0.3, 1.0),
        }
    } else {
        let bevel = ((edge_d - grout) / 0.05).clamp(0.0, 1.0);
        let smooth_bevel = bevel * bevel * (3.0 - 2.0 * bevel);
        let jitter = (hash01(tx, ty, seed) - 0.5) * 0.08;
        let chip = ((1.0 - smooth_bevel) * n_fine * p.wear * 1.4).clamp(0.0, 0.6);
        let tint = 0.82 + jitter - chip * 0.25;
        ProcSample {
            height: (0.45 + 0.45 * smooth_bevel + jitter * 0.4 - chip * 0.3).clamp(0.0, 1.0),
            albedo_srgb: [(tint * 0.92).clamp(0.0, 1.0), (tint * 0.96).clamp(0.0, 1.0), tint.clamp(0.0, 1.0)],
            roughness: (p.roughness * 0.55 + chip * 0.65).clamp(0.05, 0.95),
            metallic: 0.0,
            ao: (0.78 + 0.22 * smooth_bevel).clamp(0.4, 1.0),
        }
    }
}

fn eval_weathered_copper(u: f32, v: f32, p: &ProceduralPbrParams, seed: u64) -> ProcSample {
    let period = (p.scale * 2.0).round().max(2.0) as i64;
    let hammer = periodic_fbm(u, v, period, seed);
    let detail = periodic_fbm(u, v, period * 3, seed + 37);
    let patina_raw = (0.65 * hammer + 0.35 * detail + (p.wear - 0.35) * 0.85).clamp(0.0, 1.0);
    let patina = ((patina_raw - 0.32) / 0.42).clamp(0.0, 1.0);

    let copper = [0.93f32, 0.56, 0.42];
    let verdigris = [0.28f32, 0.66, 0.55];
    let albedo = v3_lerp(copper, verdigris, patina);
    let h = (0.50 + 0.30 * (hammer - 0.5) - 0.16 * patina + 0.12 * detail).clamp(0.0, 1.0);
    ProcSample {
        height: h,
        albedo_srgb: albedo,
        roughness: (p.roughness * 0.7 + patina * 0.58).clamp(0.08, 0.96),
        metallic: (0.96 * (1.0 - patina * 0.92)).clamp(0.04, 1.0),
        ao: (0.82 + 0.18 * h - patina * 0.14).clamp(0.4, 1.0),
    }
}

fn eval_woven_fabric(u: f32, v: f32, p: &ProceduralPbrParams, seed: u64) -> ProcSample {
    let threads = (p.scale * 4.0).round().max(4.0);
    let gx = u * threads;
    let gy = v * threads;
    let ix = gx.floor() as i64;
    let iy = gy.floor() as i64;
    let fx = gx - gx.floor();
    let fy = gy - gy.floor();

    let warp = (ix + iy).rem_euclid(2) == 0;
    let (along, across) = if warp { (fy, fx) } else { (fx, fy) };
    let yarn = (1.0 - (across * 2.0 - 1.0).abs().powf(1.8)).clamp(0.0, 1.0);
    let wave = (along * PI).sin().max(0.0);
    let fuzz = periodic_noise(u * 4.0, v * 4.0, (threads as i64).max(4) * 2, seed) * 0.14;
    let h = (yarn * (0.30 + 0.55 * wave) + fuzz).clamp(0.0, 1.0);

    let base_col = if warp { [0.26f32, 0.36, 0.52] } else { [0.32f32, 0.42, 0.58] };
    let fade = p.wear * 0.18 * fuzz;
    ProcSample {
        height: h,
        albedo_srgb: [(base_col[0] + fade).clamp(0.0, 1.0), (base_col[1] + fade).clamp(0.0, 1.0), (base_col[2] + fade).clamp(0.0, 1.0)],
        roughness: (0.68 + 0.24 * (1.0 - yarn) + p.roughness * 0.2).clamp(0.35, 1.0),
        metallic: 0.0,
        ao: (0.50 + 0.50 * yarn).clamp(0.25, 1.0),
    }
}

fn eval_preset(u: f32, v: f32, p: &ProceduralPbrParams) -> ProcSample {
    let seed = u64::from(p.seed);
    match p.preset {
        ProceduralPbrPreset::BrushedSteel => eval_brushed_steel(u, v, p, seed),
        ProceduralPbrPreset::CarbonHexWeave => eval_carbon_hex_weave(u, v, p, seed),
        ProceduralPbrPreset::TerrazzoMarble => eval_terrazzo_marble(u, v, p, seed),
        ProceduralPbrPreset::CeramicTiles => eval_ceramic_tiles(u, v, p, seed),
        ProceduralPbrPreset::WeatheredCopper => eval_weathered_copper(u, v, p, seed),
        ProceduralPbrPreset::WovenFabric => eval_woven_fabric(u, v, p, seed),
    }
}

/// Evaluates and shades the procedural Substance 3D Designer material graph into straight sRGB RGBA (`w × h`).
pub fn render_procedural_pbr_rgba(w: usize, h: usize, params: &ProceduralPbrParams) -> Vec<[f32; 4]> {
    let n = w.saturating_mul(h);
    if n == 0 {
        return Vec::new();
    }
    let p = params.sanitized();
    let wf = w as f32;
    let hf = h as f32;

    // First pass: evaluate procedural material channels across all pixels
    let mut samples = vec![ProcSample { height: 0.5, albedo_srgb: [0.5; 3], roughness: 0.5, metallic: 0.0, ao: 1.0 }; n];
    par_rows(&mut samples, w, 1, |y, row| {
        let v = (y as f32 + 0.5) / hf;
        for (x, dst) in row.iter_mut().enumerate() {
            let u = (x as f32 + 0.5) / wf;
            *dst = eval_preset(u, v, &p);
        }
    });

    // Second pass: toroidal central-difference tangent-space normals + Cook-Torrance GGX shading
    let az = p.light_angle_deg.to_radians();
    let el = p.light_elevation_deg.to_radians();
    let light_dir = v3_norm([az.cos() * el.cos(), -az.sin() * el.cos(), el.sin()]);
    let view_dir = [0.0f32, 0.0, 1.0];
    let key_radiance = [2.35f32, 2.30, 2.22];
    let probe = EquirectProbe::neutral_studio();

    let mut out = vec![[0.0f32; 4]; n];
    par_rows(&mut out, w, 1, |y, row| {
        let ym = (y + h - 1) % h;
        let yp = (y + 1) % h;
        for (x, dst) in row.iter_mut().enumerate() {
            let xm = (x + w - 1) % w;
            let xp = (x + 1) % w;
            let s = samples[y * w + x];

            let h_l = samples[y * w + xm].height;
            let h_r = samples[y * w + xp].height;
            let h_u = samples[ym * w + x].height;
            let h_d = samples[yp * w + x].height;

            let gx = (h_r - h_l) * p.normal_strength * 3.0;
            let gy = (h_d - h_u) * p.normal_strength * 3.0;
            let normal = v3_norm([-gx, -gy, 1.0]);

            let surf = PbrSurfacePoint { albedo_lin: srgb3_to_linear(s.albedo_srgb), normal, roughness: s.roughness, metallic: s.metallic, ao: s.ao };
            let direct = shade_ggx_direct(&surf, view_dir, light_dir, key_radiance);
            let ibl = probe.shade_ibl(&surf, view_dir);
            let lit = linear3_to_srgb(tone_map_filmic(v3_add(direct, v3_scale(ibl, 0.95))));
            *dst = [lit[0], lit[1], lit[2], 1.0];
        }
    });
    out
}

/// Depth-agnostic `Surface` wrapper (`8`/`16`/`32f`) for `render_procedural_pbr_rgba`.
pub fn render_procedural_pbr_surface(src: &Surface, rect: Rect, params: &ProceduralPbrParams) -> Surface {
    if rect.is_empty() {
        return Surface::new(src.format());
    }
    let w = rect.width() as usize;
    let h = rect.height() as usize;
    let rgba = render_procedural_pbr_rgba(w, h, params);
    write_rgba_to_surface(&rgba, src, rect)
}
