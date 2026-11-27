//! Adobe Fresco (`FRSC v5.7.1`) Live Watercolor Wet-Canvas Capillary Diffusion Engine.
//!
//! Simulates physical watercolor dynamics on cold-press cotton paper:
//! 1. Per-pixel wetness map (`WetCanvasMap`) with suspended mobile pigment concentration.
//! 2. Porous-media capillary diffusion gated by multi-octave paper-fiber permeability, producing
//!    organic fractal backruns rather than isotropic Gaussian blur.
//! 3. Semi-Lagrangian / upwind pigment advection along wetness gradients (`-grad(w)`) and outward
//!    contact-line capillary flow.
//! 4. Coffee-ring / cauliflower edge-darkening pigment ring deposition at the wet-dry boundary
//!    with Beer-Lambert optical density darkening and paper-tooth granulation settling.
//! 5. Rapid evaporation & final pigment precipitation (`dry_all` for `fresco.dryLayer`).

use photocraft_geom::Rect;
use photocraft_raster::{Surface, from_rgba_into, to_rgba};
use serde::{Deserialize, Serialize};

use crate::StrokePoint;

/// Maximum working area (64 MP) for a single wet-canvas simulation pass.
pub const MAX_WATERCOLOR_PIXELS: usize = 64_000_000;

/// Built-in Adobe Fresco Live Watercolor brush & wash presets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FrescoWatercolorPreset {
    #[default]
    WashSoft,
    WetSpatter,
    DetailInk,
    Granulating,
}

impl FrescoWatercolorPreset {
    pub const ALL: [Self; 4] = [Self::WashSoft, Self::WetSpatter, Self::DetailInk, Self::Granulating];

    pub fn id(self) -> &'static str {
        match self {
            Self::WashSoft => "washSoft",
            Self::WetSpatter => "wetSpatter",
            Self::DetailInk => "detailInk",
            Self::Granulating => "granulating",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::WashSoft => "Wash Soft",
            Self::WetSpatter => "Wet Spatter",
            Self::DetailInk => "Detail Ink",
            Self::Granulating => "Granulating",
        }
    }

    pub fn from_id(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "washsoft" | "wash_soft" | "wash" | "부드러운 워시" => Some(Self::WashSoft),
            "wetspatter" | "wet_spatter" | "spatter" | "젖은 스패터" => Some(Self::WetSpatter),
            "detailink" | "detail_ink" | "ink" | "디테일 잉크" => Some(Self::DetailInk),
            "granulating" | "granulation" | "그래뉼레이션" => Some(Self::Granulating),
            _ => None,
        }
    }

    fn diffusion_rate(self) -> f32 {
        match self {
            Self::WashSoft => 0.22,
            Self::WetSpatter => 0.20,
            Self::DetailInk => 0.11,
            Self::Granulating => 0.16,
        }
    }

    fn capillary_threshold(self) -> f32 {
        match self {
            Self::WashSoft => 0.04,
            Self::WetSpatter => 0.07,
            Self::DetailInk => 0.12,
            Self::Granulating => 0.08,
        }
    }
}

/// Parameters controlling Live Watercolor capillary diffusion, pigment advection, and edge rings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FrescoWatercolorParams {
    pub preset: FrescoWatercolorPreset,
    /// Water volume injected into wet regions (`0.0..=1.0`).
    pub water_flow: f32,
    /// Number of porous-media PDE solver iterations (`1..=24`).
    pub diffusion_steps: u32,
    /// Pigment advection strength along wetness gradients (`0.0..=2.0`).
    pub advection_strength: f32,
    /// Cauliflower / coffee-ring edge-darkening strength (`0.0..=1.5`).
    pub cauliflower_edge: f32,
    /// Cold-press paper tooth granulation & permeability modulation (`0.0..=1.0`).
    pub paper_grain: f32,
    /// Water evaporation rate per step (`0.0..=1.0`).
    pub evaporation: f32,
    /// Deterministic paper-fiber & spatter seed.
    pub seed: u32,
}

impl Default for FrescoWatercolorParams {
    fn default() -> Self {
        Self {
            preset: FrescoWatercolorPreset::WashSoft,
            water_flow: 0.65,
            diffusion_steps: 8,
            advection_strength: 0.85,
            cauliflower_edge: 0.55,
            paper_grain: 0.35,
            evaporation: 0.12,
            seed: 42,
        }
    }
}

impl FrescoWatercolorParams {
    pub fn sanitized(&self) -> Self {
        let finite = |v: f32, d: f32| if v.is_finite() { v } else { d };
        Self {
            preset: self.preset,
            water_flow: finite(self.water_flow, 0.65).clamp(0.0, 1.0),
            diffusion_steps: self.diffusion_steps.clamp(1, 24),
            advection_strength: finite(self.advection_strength, 0.85).clamp(0.0, 2.0),
            cauliflower_edge: finite(self.cauliflower_edge, 0.55).clamp(0.0, 1.5),
            paper_grain: finite(self.paper_grain, 0.35).clamp(0.0, 1.0),
            evaporation: finite(self.evaporation, 0.12).clamp(0.0, 1.0),
            seed: self.seed,
        }
    }
}

/// Persistent per-layer wet-canvas state holding per-pixel water saturation and mobile pigment.
#[derive(Clone, Debug, PartialEq)]
pub struct WetCanvasMap {
    origin_x: i32,
    origin_y: i32,
    width: usize,
    height: usize,
    wetness: Vec<f32>,
    suspended_rgba: Vec<[f32; 4]>,
}

impl WetCanvasMap {
    /// Allocates an empty (dry) wet-canvas map covering `bounds`, returning `None` if empty or oversized.
    pub fn new(bounds: Rect) -> Option<Self> {
        if bounds.is_empty() {
            return None;
        }
        let width = usize::try_from(bounds.width()).ok()?;
        let height = usize::try_from(bounds.height()).ok()?;
        let len = width.checked_mul(height)?;
        if len == 0 || len > MAX_WATERCOLOR_PIXELS {
            return None;
        }
        Some(Self { origin_x: bounds.x0, origin_y: bounds.y0, width, height, wetness: vec![0.0; len], suspended_rgba: vec![[0.0; 4]; len] })
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn wetness_slice(&self) -> &[f32] {
        &self.wetness
    }

    /// Ensures the map matches `bounds`, preserving overlapping wet cells on resize.
    pub fn ensure_bounds(&mut self, bounds: Rect) -> bool {
        let Ok(nw) = usize::try_from(bounds.width()) else { return false };
        let Ok(nh) = usize::try_from(bounds.height()) else { return false };
        if self.origin_x == bounds.x0 && self.origin_y == bounds.y0 && self.width == nw && self.height == nh {
            return true;
        }
        let Some(mut next) = Self::new(bounds) else { return false };
        let overlap =
            Rect::new(self.origin_x, self.origin_y, self.origin_x.saturating_add(self.width as i32), self.origin_y.saturating_add(self.height as i32))
                .intersect(&bounds);
        for wy in overlap.y0..overlap.y1 {
            let oy = (wy - self.origin_y) as usize;
            let ny = (wy - next.origin_y) as usize;
            for wx in overlap.x0..overlap.x1 {
                let ox = (wx - self.origin_x) as usize;
                let nx = (wx - next.origin_x) as usize;
                let oi = oy * self.width + ox;
                let ni = ny * next.width + nx;
                if let (Some(&w), Some(dw)) = (self.wetness.get(oi), next.wetness.get_mut(ni)) {
                    *dw = w;
                }
                if let (Some(&s), Some(ds)) = (self.suspended_rgba.get(oi), next.suspended_rgba.get_mut(ni)) {
                    *ds = s;
                }
            }
        }
        *self = next;
        true
    }

    /// Fraction of canvas pixels currently holding active moisture (`wetness > 0.005`).
    pub fn wet_fraction(&self) -> f32 {
        if self.wetness.is_empty() {
            return 0.0;
        }
        let wet_count = self.wetness.iter().filter(|&&w| w > 0.005).count();
        wet_count as f32 / self.wetness.len() as f32
    }

    /// Peak pixel wetness (`0.0..=1.0`).
    pub fn max_wetness(&self) -> f32 {
        self.wetness.iter().copied().fold(0.0f32, f32::max)
    }

    ///Wetness at world coordinate `(wx, wy)`.
    pub fn wetness_at(&self, wx: i32, wy: i32) -> f32 {
        let Some(lx) = wx.checked_sub(self.origin_x).and_then(|v| usize::try_from(v).ok()) else {
            return 0.0;
        };
        let Some(ly) = wy.checked_sub(self.origin_y).and_then(|v| usize::try_from(v).ok()) else {
            return 0.0;
        };
        if lx >= self.width || ly >= self.height {
            return 0.0;
        }
        self.wetness.get(ly * self.width + lx).copied().unwrap_or(0.0)
    }

    /// Seeds moisture and mobilizes existing canvas pigment where pixels have non-white/opaque ink.
    pub fn seed_from_canvas(&mut self, canvas_rgba: &[[f32; 4]], params: &FrescoWatercolorParams) {
        let flow = params.water_flow.clamp(0.0, 1.0);
        if flow <= 1e-5 {
            return;
        }
        for y in 0..self.height {
            for x in 0..self.width {
                let idx = y * self.width + x;
                let Some(&px) = canvas_rgba.get(idx) else { continue };
                let lum = 0.2126 * px[0] + 0.7152 * px[1] + 0.0722 * px[2];
                let min_ch = px[0].min(px[1]).min(px[2]);
                let max_ch = px[0].max(px[1]).max(px[2]);
                let chroma = max_ch - min_ch;
                // Pure white background paper carries no watercolor pigment; colored or dark strokes do.
                let ink_density = ((1.0 - lum) * 0.85 + chroma * 0.65).clamp(0.0, 1.0);
                let pigment_mass = px[3] * ink_density;
                if px[3] <= 0.01 || pigment_mass <= 0.015 {
                    continue;
                }
                let spatter_mod = if params.preset == FrescoWatercolorPreset::WetSpatter {
                    0.70 + 0.60 * paper_hash(x as i32, y as i32, params.seed ^ 0x9E37_79B9)
                } else {
                    1.0
                };
                let added_wet = (flow * (0.55 + 0.45 * pigment_mass) * spatter_mod).clamp(0.0, 1.0);
                if let Some(w_slot) = self.wetness.get_mut(idx) {
                    *w_slot = (*w_slot + added_wet * (1.0 - *w_slot * 0.5)).clamp(0.0, 1.0);
                }
                let lift = (0.52 * flow * (0.4 + 0.6 * ink_density)).clamp(0.05, 0.78);
                if let Some(susp) = self.suspended_rgba.get_mut(idx) {
                    let add_a = px[3] * lift;
                    let new_c = (susp[3] + add_a * (1.0 - susp[3])).clamp(0.0, 1.0);
                    if new_c > 1e-5 {
                        for c in 0..3 {
                            susp[c] = ((susp[c] * susp[3] * (1.0 - add_a) + px[c] * add_a) / new_c).clamp(0.0, 1.0);
                        }
                    }
                    susp[3] = new_c;
                }
            }
        }
    }

    /// Injects a live wet watercolor brush stroke into both `canvas_rgba` and this wetness map.
    pub fn inject_stroke(&mut self, canvas_rgba: &mut [[f32; 4]], points: &[StrokePoint], brush_size: f32, color: [f32; 4], water_flow: f32, seed: u32) {
        if points.is_empty() || self.width == 0 || self.height == 0 {
            return;
        }
        let radius = (if brush_size.is_finite() { brush_size * 0.5 } else { 12.0 }).clamp(1.0, 200.0);
        let flow = (if water_flow.is_finite() { water_flow } else { 0.65 }).clamp(0.05, 1.0);
        let dabs = interpolate_stroke_dabs(points, (radius * 0.25).max(0.75));
        for (cx, cy, pressure) in dabs {
            let r = (radius * (0.40 + 0.60 * pressure)).max(1.0);
            let reach = (r * 1.25).ceil() as i32;
            let x0 = (cx.floor() as i32 - reach).max(self.origin_x);
            let y0 = (cy.floor() as i32 - reach).max(self.origin_y);
            let x1 = (cx.ceil() as i32 + reach + 1).min(self.origin_x.saturating_add(self.width as i32));
            let y1 = (cy.ceil() as i32 + reach + 1).min(self.origin_y.saturating_add(self.height as i32));
            for wy in y0..y1 {
                let ly = (wy - self.origin_y) as usize;
                let dy = (wy as f32 + 0.5) - cy;
                for wx in x0..x1 {
                    let lx = (wx - self.origin_x) as usize;
                    let dx = (wx as f32 + 0.5) - cx;
                    let dist = (dx * dx + dy * dy).sqrt() / r;
                    let jitter = 0.08 * (paper_tooth(wx, wy, seed) - 0.5);
                    let falloff = (1.0 - (dist + jitter).max(0.0)).clamp(0.0, 1.0);
                    if falloff <= 1e-4 {
                        continue;
                    }
                    let idx = ly * self.width + lx;
                    let wet_gain = (falloff.sqrt() * flow * 0.85).clamp(0.0, 1.0);
                    let pig_gain = (falloff * color[3] * (0.35 + 0.45 * (1.0 - flow * 0.3))).clamp(0.0, 1.0);
                    if let Some(w_slot) = self.wetness.get_mut(idx) {
                        *w_slot = (*w_slot + wet_gain * (1.0 - *w_slot * 0.35)).clamp(0.0, 1.0);
                    }
                    if let Some(susp) = self.suspended_rgba.get_mut(idx) {
                        let out_a = (susp[3] + pig_gain * (1.0 - susp[3])).clamp(0.0, 1.0);
                        if out_a > 1e-5 {
                            for c in 0..3 {
                                susp[c] = ((color[c] * pig_gain + susp[c] * susp[3] * (1.0 - pig_gain)) / out_a).clamp(0.0, 1.0);
                            }
                        }
                        susp[3] = out_a;
                    }
                    if let Some(dst) = canvas_rgba.get_mut(idx) {
                        blend_pigment_onto_paper(dst, color, pig_gain * 0.55);
                    }
                }
            }
        }
    }

    /// Runs `params.diffusion_steps` iterations of capillary diffusion, pigment advection,
    /// cauliflower contact-line ring deposition, and paper granulation on `canvas_rgba`.
    pub fn step_capillary_diffusion(&mut self, canvas_rgba: &mut [[f32; 4]], params: &FrescoWatercolorParams) {
        let p = params.sanitized();
        let len = self.width.saturating_mul(self.height);
        if len == 0 || canvas_rgba.len() < len {
            return;
        }
        let paper = build_paper_field(self.origin_x, self.origin_y, self.width, self.height, p.seed);
        let mut next_wet = vec![0.0f32; len];
        let mut next_susp = vec![[0.0f32; 4]; len];

        for step in 0..p.diffusion_steps {
            diffuse_wetness_step(self, &paper, &p, step, &mut next_wet);
            advect_suspended_pigment(self, &next_wet, &paper, &p, &mut next_susp);
            self.wetness.copy_from_slice(&next_wet);
            self.suspended_rgba.copy_from_slice(&next_susp);
            deposit_cauliflower_and_granulation(self, canvas_rgba, &paper, &p);
        }
    }

    /// Rapidly evaporates all remaining moisture (`fresco.dryLayer`), precipitating mobile pigment
    /// with a contact-line edge ring onto `surface` and resetting wetness to `0.0`.
    pub fn dry_all(&mut self, surface: &mut Surface, area: Rect, cauliflower_boost: f32, selection: Option<&Surface>, lock_alpha: bool) -> f32 {
        let peak = self.max_wetness();
        if peak <= 1e-5 || area.is_empty() || !self.ensure_bounds(area) {
            self.wetness.fill(0.0);
            self.suspended_rgba.fill([0.0; 4]);
            return 0.0;
        }
        let fmt = surface.format();
        let ch = fmt.channels();
        let raw = surface.read_region(area);
        let mut canvas: Vec<[f32; 4]> = raw.chunks_exact(ch).map(|px| to_rgba(&fmt, px)).collect();
        let orig = canvas.clone();

        let boost = (if cauliflower_boost.is_finite() { cauliflower_boost } else { 0.35 }).clamp(0.0, 1.5);
        let paper = build_paper_field(self.origin_x, self.origin_y, self.width, self.height, 0xA5A5_5A5A);
        precipitate_final_drying(self, &mut canvas, &paper, boost);

        self.wetness.fill(0.0);
        self.suspended_rgba.fill([0.0; 4]);

        apply_selection_and_alpha_lock(&mut canvas, &orig, area, selection, lock_alpha);
        let mut out_buf = vec![0.0f32; canvas.len() * ch];
        for (rgba, dst) in canvas.iter().zip(out_buf.chunks_exact_mut(ch)) {
            from_rgba_into(&fmt, *rgba, dst);
        }
        surface.write_region(area, &out_buf);
        surface.prune();
        peak
    }
}

/// High-level helper: applies Live Watercolor capillary diffusion (and optional wet stroke points)
/// to `surface` over `area`, updating `wet_map` in place and returning the resulting [`Surface`].
pub fn apply_live_watercolor_surface(
    surface: &Surface,
    area: Rect,
    wet_map: &mut WetCanvasMap,
    params: &FrescoWatercolorParams,
    stroke_points: Option<&[StrokePoint]>,
    brush_size: f32,
    brush_color: [f32; 4],
) -> Surface {
    if area.is_empty() || !wet_map.ensure_bounds(area) {
        return surface.clone();
    }
    let p = params.sanitized();
    let fmt = surface.format();
    let ch = fmt.channels();
    let raw = surface.read_region(area);
    let mut canvas: Vec<[f32; 4]> = raw.chunks_exact(ch).map(|px| to_rgba(&fmt, px)).collect();

    if let Some(pts) = stroke_points.filter(|pts| !pts.is_empty()) {
        wet_map.inject_stroke(&mut canvas, pts, brush_size, brush_color, p.water_flow, p.seed);
    } else {
        wet_map.seed_from_canvas(&canvas, &p);
    }

    wet_map.step_capillary_diffusion(&mut canvas, &p);

    let mut out_buf = vec![0.0f32; canvas.len() * ch];
    for (rgba, dst) in canvas.iter().zip(out_buf.chunks_exact_mut(ch)) {
        from_rgba_into(&fmt, *rgba, dst);
    }
    let mut out = surface.clone();
    out.write_region(area, &out_buf);
    out.prune();
    out
}

// ------------------------------------------------------------------ Internal Physics Kernels

#[inline]
fn paper_hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x27D4_EB2D) ^ (y as u32).wrapping_mul(0x1656_67B1) ^ seed.wrapping_mul(0x9E37_79B9);
    h ^= h >> 15;
    h = h.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    h = h.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 16;
    (h & 0x00FF_FFFF) as f32 / 16_777_215.0
}

#[inline]
fn value_noise(fx: f32, fy: f32, seed: u32) -> f32 {
    let ix = fx.floor() as i32;
    let iy = fy.floor() as i32;
    let tx = fx - ix as f32;
    let ty = fy - iy as f32;
    let ux = tx * tx * (3.0 - 2.0 * tx);
    let uy = ty * ty * (3.0 - 2.0 * ty);
    let a = paper_hash(ix, iy, seed);
    let b = paper_hash(ix + 1, iy, seed);
    let c = paper_hash(ix, iy + 1, seed);
    let d = paper_hash(ix + 1, iy + 1, seed);
    let ab = a + (b - a) * ux;
    let cd = c + (d - c) * ux;
    ab + (cd - ab) * uy
}

/// Multi-octave cold-press cotton paper tooth height in `0.0..=1.0`.
#[inline]
fn paper_tooth(wx: i32, wy: i32, seed: u32) -> f32 {
    let x = wx as f32;
    let y = wy as f32;
    let coarse = value_noise(x * 0.11, y * 0.11, seed);
    let medium = value_noise(x * 0.29, y * 0.29, seed ^ 0x517C_C1B7);
    let fine = paper_hash(wx, wy, seed ^ 0xB529_7A4D);
    (0.52 * coarse + 0.33 * medium + 0.15 * fine).clamp(0.0, 1.0)
}

fn build_paper_field(ox: i32, oy: i32, w: usize, h: usize, seed: u32) -> Vec<f32> {
    let mut out = vec![0.5f32; w.saturating_mul(h)];
    for y in 0..h {
        let wy = oy.saturating_add(y as i32);
        for x in 0..w {
            let wx = ox.saturating_add(x as i32);
            if let Some(slot) = out.get_mut(y * w + x) {
                *slot = paper_tooth(wx, wy, seed);
            }
        }
    }
    out
}

fn diffuse_wetness_step(map: &WetCanvasMap, paper: &[f32], p: &FrescoWatercolorParams, step: u32, next_wet: &mut [f32]) {
    let (w, h) = (map.width, map.height);
    let rate = p.preset.diffusion_rate() * (0.45 + 0.75 * p.water_flow);
    let base_thresh = p.preset.capillary_threshold();
    let evap = p.evaporation * 0.045;
    let neighbors: [(isize, isize, f32); 8] =
        [(-1, 0, 0.16), (1, 0, 0.16), (0, -1, 0.16), (0, 1, 0.16), (-1, -1, 0.09), (1, -1, 0.09), (-1, 1, 0.09), (1, 1, 0.09)];

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let cur_w = map.wetness.get(idx).copied().unwrap_or(0.0);
            let tooth = paper.get(idx).copied().unwrap_or(0.5);
            let perm_here = (1.0 - p.paper_grain * 0.55 * (tooth - 0.35)).clamp(0.25, 1.25);
            // Organic cauliflower lobes arise because dry paper fibers resist wetting below threshold.
            let local_thresh = base_thresh * (0.65 + 0.85 * tooth * (0.4 + p.paper_grain));

            let mut flux = 0.0f32;
            for &(dx, dy, weight) in &neighbors {
                let nx = x as isize + dx;
                let ny = y as isize + dy;
                if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize {
                    continue;
                }
                let nidx = ny as usize * w + nx as usize;
                let nw = map.wetness.get(nidx).copied().unwrap_or(0.0);
                let ntooth = paper.get(nidx).copied().unwrap_or(0.5);
                let perm_mid = 0.5 * (perm_here + (1.0 - p.paper_grain * 0.55 * (ntooth - 0.35)).clamp(0.25, 1.25));
                let diff = nw - cur_w;
                if cur_w <= 0.005 && nw < local_thresh {
                    continue;
                }
                flux += weight * perm_mid * diff;
            }
            // Spatter micro-blooms on the first step for WetSpatter
            let spatter = if step == 0 && p.preset == FrescoWatercolorPreset::WetSpatter && cur_w > 0.25 { 0.04 * (tooth - 0.5) } else { 0.0 };
            if let Some(dst) = next_wet.get_mut(idx) {
                *dst = (cur_w + rate * flux + spatter - evap).clamp(0.0, 1.0);
            }
        }
    }
}

fn advect_suspended_pigment(map: &WetCanvasMap, wet: &[f32], paper: &[f32], p: &FrescoWatercolorParams, next_susp: &mut [[f32; 4]]) {
    let (w, h) = (map.width, map.height);
    let adv = p.advection_strength;
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let cw = wet.get(idx).copied().unwrap_or(0.0);
            let cur_s = map.suspended_rgba.get(idx).copied().unwrap_or([0.0; 4]);
            if cw <= 0.005 || adv <= 1e-4 {
                if let Some(dst) = next_susp.get_mut(idx) {
                    *dst = cur_s;
                }
                continue;
            }
            let wl = if x > 0 { wet.get(idx - 1).copied().unwrap_or(cw) } else { cw };
            let wr = if x + 1 < w { wet.get(idx + 1).copied().unwrap_or(cw) } else { cw };
            let wu = if y > 0 { wet.get(idx - w).copied().unwrap_or(cw) } else { cw };
            let wd = if y + 1 < h { wet.get(idx + w).copied().unwrap_or(cw) } else { cw };

            // Wetness gradient: pigment flows from wetter interior down `-grad(w)` toward the
            // evaporating contact line (coffee-ring outward capillary advection).
            let gx = 0.5 * (wr - wl);
            let gy = 0.5 * (wd - wu);
            let tooth = paper.get(idx).copied().unwrap_or(0.5);
            let speed = adv * (1.35 + 0.45 * p.cauliflower_edge) * (0.75 + 0.5 * (1.0 - tooth * p.paper_grain));
            let vx = (-gx * speed * 4.5).clamp(-1.5, 1.5);
            let vy = (-gy * speed * 4.5).clamp(-1.5, 1.5);

            // Semi-Lagrangian backtrace to upstream wet position `(sx, sy)`, gated by upstream wetness.
            let sx = (x as f32 - vx).clamp(0.0, (w.saturating_sub(1)) as f32);
            let sy = (y as f32 - vy).clamp(0.0, (h.saturating_sub(1)) as f32);
            let advected = sample_bilinear_rgba(&map.suspended_rgba, wet, w, h, sx, sy);

            // Also include isotropic wet-in-wet premultiplied pigment dispersion among wet neighbors.
            let neighbor_avg = average_wet_neighbors(&map.suspended_rgba, wet, w, h, x, y, cur_s);
            let mix_disp = (0.42 * cw).clamp(0.0, 0.52);
            let out_a = (advected[3] * (1.0 - mix_disp) + neighbor_avg[3] * mix_disp).clamp(0.0, 1.0);
            let mut out = [0.0f32; 4];
            if out_a > 1e-5 {
                for c in 0..3 {
                    let pm = advected[c] * advected[3] * (1.0 - mix_disp) + neighbor_avg[c] * neighbor_avg[3] * mix_disp;
                    out[c] = (pm / out_a).clamp(0.0, 1.0);
                }
            }
            out[3] = out_a;
            if let Some(dst) = next_susp.get_mut(idx) {
                *dst = out;
            }
        }
    }
}

fn sample_bilinear_rgba(susp: &[[f32; 4]], wet: &[f32], w: usize, h: usize, sx: f32, sy: f32) -> [f32; 4] {
    let x0 = (sx.floor() as usize).min(w.saturating_sub(1));
    let y0 = (sy.floor() as usize).min(h.saturating_sub(1));
    let x1 = (x0 + 1).min(w.saturating_sub(1));
    let y1 = (y0 + 1).min(h.saturating_sub(1));
    let tx = sx - x0 as f32;
    let ty = sy - y0 as f32;

    let weights = [((1.0 - tx) * (1.0 - ty), y0 * w + x0), (tx * (1.0 - ty), y0 * w + x1), ((1.0 - tx) * ty, y1 * w + x0), (tx * ty, y1 * w + x1)];
    let mut sum_pm = [0.0f32; 3];
    let mut sum_a = 0.0f32;
    let mut w_tot = 0.0f32;
    for (bw, idx) in weights {
        let cell_w = wet.get(idx).copied().unwrap_or(0.0);
        if cell_w <= 0.002 {
            continue;
        }
        let s = susp.get(idx).copied().unwrap_or([0.0; 4]);
        let eff = bw * (0.25 + 0.75 * cell_w);
        w_tot += eff;
        sum_a += s[3] * eff;
        for (slot, &ch) in sum_pm.iter_mut().zip(s.iter().take(3)) {
            *slot += ch * s[3] * eff;
        }
    }
    if w_tot <= 1e-6 || sum_a <= 1e-6 {
        return [0.0; 4];
    }
    let out_a = (sum_a / w_tot).clamp(0.0, 1.0);
    [(sum_pm[0] / sum_a).clamp(0.0, 1.0), (sum_pm[1] / sum_a).clamp(0.0, 1.0), (sum_pm[2] / sum_a).clamp(0.0, 1.0), out_a]
}

fn average_wet_neighbors(susp: &[[f32; 4]], wet: &[f32], w: usize, h: usize, x: usize, y: usize, fallback: [f32; 4]) -> [f32; 4] {
    let offsets: [(isize, isize); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
    let mut acc_pm = [0.0f32; 3];
    let mut acc_a = 0.0f32;
    let mut w_sum = 0.0f32;
    for (dx, dy) in offsets {
        let nx = x as isize + dx;
        let ny = y as isize + dy;
        if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize {
            continue;
        }
        let nidx = ny as usize * w + nx as usize;
        let nw = wet.get(nidx).copied().unwrap_or(0.0);
        if nw <= 0.005 {
            continue;
        }
        let ns = susp.get(nidx).copied().unwrap_or([0.0; 4]);
        w_sum += nw;
        acc_a += ns[3] * nw;
        for (slot, &ch) in acc_pm.iter_mut().zip(ns.iter().take(3)) {
            *slot += ch * ns[3] * nw;
        }
    }
    if w_sum <= 1e-6 {
        return fallback;
    }
    if acc_a <= 1e-6 {
        return [0.0; 4];
    }
    let out_a = (acc_a / w_sum).clamp(0.0, 1.0);
    [(acc_pm[0] / acc_a).clamp(0.0, 1.0), (acc_pm[1] / acc_a).clamp(0.0, 1.0), (acc_pm[2] / acc_a).clamp(0.0, 1.0), out_a]
}

fn deposit_cauliflower_and_granulation(map: &mut WetCanvasMap, canvas_rgba: &mut [[f32; 4]], paper: &[f32], p: &FrescoWatercolorParams) {
    let (w, h) = (map.width, map.height);
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let cw = map.wetness.get(idx).copied().unwrap_or(0.0);
            if cw <= 0.003 {
                continue;
            }
            let susp = map.suspended_rgba.get(idx).copied().unwrap_or([0.0; 4]);
            if susp[3] <= 1e-4 {
                continue;
            }
            let wl = if x > 0 { map.wetness.get(idx - 1).copied().unwrap_or(0.0) } else { 0.0 };
            let wr = if x + 1 < w { map.wetness.get(idx + 1).copied().unwrap_or(0.0) } else { 0.0 };
            let wu = if y > 0 { map.wetness.get(idx - w).copied().unwrap_or(0.0) } else { 0.0 };
            let wd = if y + 1 < h { map.wetness.get(idx + w).copied().unwrap_or(0.0) } else { 0.0 };
            let grad_mag = ((wr - wl).powi(2) + (wd - wu).powi(2)).sqrt() * 0.5;

            let tooth = paper.get(idx).copied().unwrap_or(0.5);
            let valley = (1.0 - tooth).clamp(0.0, 1.0);

            // Contact-line indicator: high wetness gradient near moderate/low wetness perimeter.
            let perimeter_band = (1.0 - ((cw - 0.18).abs() / 0.24)).clamp(0.0, 1.0);
            let ring_factor = (grad_mag * 3.4 * perimeter_band * p.cauliflower_edge * (0.7 + 0.6 * tooth)).clamp(0.0, 0.85);
            let gran_factor =
                if p.preset == FrescoWatercolorPreset::Granulating { 0.18 * valley * (0.4 + p.paper_grain) } else { 0.07 * valley * p.paper_grain };
            let deposit_rate = (0.075 + ring_factor * 0.44 + gran_factor).clamp(0.0, 0.72);
            let dep_alpha = (susp[3] * deposit_rate).clamp(0.0, 1.0);

            // Beer-Lambert optical edge darkening at the cauliflower ring (`gamma > 1`).
            let gamma = 1.0 + ring_factor * 0.85;
            let ring_color = [susp[0].clamp(0.0, 1.0).powf(gamma), susp[1].clamp(0.0, 1.0).powf(gamma), susp[2].clamp(0.0, 1.0).powf(gamma), 1.0];

            if let Some(dst) = canvas_rgba.get_mut(idx) {
                // Slight backrun wash-out in deep wet interior (`cw > 0.65` and low gradient)
                if cw > 0.65 && grad_mag < 0.03 && p.cauliflower_edge > 0.2 && dst[3] > 0.1 {
                    let lift = 0.018 * p.cauliflower_edge;
                    for ch in dst.iter_mut().take(3) {
                        *ch = (*ch + (1.0 - *ch) * lift).clamp(0.0, 1.0);
                    }
                }
                blend_pigment_onto_paper(dst, ring_color, dep_alpha);
            }
            if let Some(s_slot) = map.suspended_rgba.get_mut(idx) {
                s_slot[3] = (s_slot[3] * (1.0 - deposit_rate * 0.75)).clamp(0.0, 1.0);
            }
        }
    }
}

fn precipitate_final_drying(map: &WetCanvasMap, canvas_rgba: &mut [[f32; 4]], paper: &[f32], cauliflower_boost: f32) {
    let (w, h) = (map.width, map.height);
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let cw = map.wetness.get(idx).copied().unwrap_or(0.0);
            let susp = map.suspended_rgba.get(idx).copied().unwrap_or([0.0; 4]);
            if cw <= 1e-4 || susp[3] <= 1e-4 {
                continue;
            }
            let wl = if x > 0 { map.wetness.get(idx - 1).copied().unwrap_or(0.0) } else { 0.0 };
            let wr = if x + 1 < w { map.wetness.get(idx + 1).copied().unwrap_or(0.0) } else { 0.0 };
            let wu = if y > 0 { map.wetness.get(idx - w).copied().unwrap_or(0.0) } else { 0.0 };
            let wd = if y + 1 < h { map.wetness.get(idx + w).copied().unwrap_or(0.0) } else { 0.0 };
            let grad = ((wr - wl).powi(2) + (wd - wu).powi(2)).sqrt() * 0.5;
            let valley = 1.0 - paper.get(idx).copied().unwrap_or(0.5);
            let ring = (grad * 3.2 * cauliflower_boost).clamp(0.0, 0.9);
            let gamma = 1.0 + ring * 0.9;
            let color = [susp[0].clamp(0.0, 1.0).powf(gamma), susp[1].clamp(0.0, 1.0).powf(gamma), susp[2].clamp(0.0, 1.0).powf(gamma), 1.0];
            let dep_a = (susp[3] * (0.45 + 0.40 * ring + 0.15 * valley)).clamp(0.0, 1.0);
            if let Some(dst) = canvas_rgba.get_mut(idx) {
                blend_pigment_onto_paper(dst, color, dep_a);
            }
        }
    }
}

#[inline]
fn blend_pigment_onto_paper(dst: &mut [f32; 4], paint: [f32; 4], alpha: f32) {
    let a = alpha.clamp(0.0, 1.0);
    if a <= 1e-5 {
        return;
    }
    let out_a = (a + dst[3] * (1.0 - a)).clamp(0.0, 1.0);
    if out_a > 1e-6 {
        for c in 0..3 {
            // Subtractive glaze tendency when painting over opaque bright paper, standard source-over on transparent
            let glaze = dst[c] * paint[c];
            let target_c = paint[c] * (1.0 - 0.35 * dst[3]) + glaze * (0.35 * dst[3]);
            dst[c] = ((target_c * a + dst[c] * dst[3] * (1.0 - a)) / out_a).clamp(0.0, 1.0);
        }
    }
    dst[3] = out_a;
}

fn apply_selection_and_alpha_lock(canvas: &mut [[f32; 4]], orig: &[[f32; 4]], area: Rect, selection: Option<&Surface>, lock_alpha: bool) {
    let w = area.width() as usize;
    let h = area.height() as usize;
    for ly in 0..h {
        let wy = area.y0 + ly as i32;
        for lx in 0..w {
            let wx = area.x0 + lx as i32;
            let idx = ly * w + lx;
            let cov = selection.map_or(1.0, |s| s.sample_channel(wx, wy, 0).clamp(0.0, 1.0));
            if let (Some(dst), Some(&src)) = (canvas.get_mut(idx), orig.get(idx)) {
                if cov < 1.0 {
                    for c in 0..4 {
                        dst[c] = src[c] * (1.0 - cov) + dst[c] * cov;
                    }
                }
                if lock_alpha {
                    dst[3] = src[3];
                }
            }
        }
    }
}

fn interpolate_stroke_dabs(points: &[StrokePoint], spacing: f32) -> Vec<(f32, f32, f32)> {
    let mut out = Vec::new();
    let Some(first) = points.first() else { return out };
    if first.x.is_finite() && first.y.is_finite() {
        out.push((first.x as f32, first.y as f32, first.pressure.clamp(0.05, 1.0)));
    }
    let step = spacing.max(0.5);
    let mut carry = 0.0f32;
    for seg in points.windows(2) {
        let (Some(a), Some(b)) = (seg.first(), seg.get(1)) else { continue };
        if !a.x.is_finite() || !a.y.is_finite() || !b.x.is_finite() || !b.y.is_finite() {
            continue;
        }
        let (ax, ay) = (a.x as f32, a.y as f32);
        let (bx, by) = (b.x as f32, b.y as f32);
        let (dx, dy) = (bx - ax, by - ay);
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < 1e-4 {
            continue;
        }
        let mut d = step - carry;
        while d <= dist {
            let t = d / dist;
            let pr = a.pressure + (b.pressure - a.pressure) * t;
            out.push((ax + dx * t, ay + dy * t, pr.clamp(0.05, 1.0)));
            d += step;
        }
        carry = (dist + carry) % step;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use photocraft_color::PixelFormat;

    #[test]
    fn capillary_diffusion_expands_wet_region_and_forms_cauliflower_edge_ring() {
        let area = Rect::new(0, 0, 64, 64);
        let mut surf = Surface::new(PixelFormat::RGBA8);
        surf.fill_rect(area, &[1.0, 1.0, 1.0, 1.0]);
        // Paint a concentrated cobalt-blue spot in the center (26..38, 26..38 => 12x12 = 144 px)
        surf.fill_rect(Rect::new(26, 26, 38, 38), &[0.12, 0.32, 0.88, 1.0]);

        let mut wet_map = WetCanvasMap::new(area).unwrap();
        let params = FrescoWatercolorParams {
            preset: FrescoWatercolorPreset::WashSoft,
            water_flow: 0.85,
            diffusion_steps: 14,
            advection_strength: 1.1,
            cauliflower_edge: 0.9,
            paper_grain: 0.4,
            evaporation: 0.08,
            seed: 77,
        };

        let out = apply_live_watercolor_surface(&surf, area, &mut wet_map, &params, None, 16.0, [0.12, 0.32, 0.88, 1.0]);
        let initial_frac = (12.0 * 12.0) / (64.0 * 64.0);
        assert!(
            wet_map.wet_fraction() > initial_frac * 1.5,
            "wetness map must diffuse outward beyond initial {} fraction, got {}",
            initial_frac,
            wet_map.wet_fraction()
        );
        // Previously white pixel outside the original 26..38 square (x=24, y=32) receives diffused blue pigment!
        let bloom_px = out.rgba(24, 32);
        assert!(bloom_px[0] < 0.96 && bloom_px[2] > bloom_px[0], "expected diffused blue bloom outside original square, got {bloom_px:?}");

        // Drying precipitates remaining suspended pigment and zeroes wetness!
        let mut dry_surf = out.clone();
        let dried_peak = wet_map.dry_all(&mut dry_surf, area, 0.6, None, false);
        assert!(dried_peak > 0.1);
        assert_eq!(wet_map.max_wetness(), 0.0);
        assert_eq!(wet_map.wet_fraction(), 0.0);
    }

    #[test]
    fn wet_stroke_injection_works_across_bit_depths() {
        let area = Rect::new(0, 0, 48, 32);
        let pts = vec![StrokePoint::new(8.0, 16.0, 0.8), StrokePoint::new(40.0, 16.0, 1.0)];
        for fmt in [PixelFormat::RGBA8, PixelFormat::RGBA16, PixelFormat::RGBA32F] {
            let surf = Surface::new(fmt);
            let mut wet = WetCanvasMap::new(area).unwrap();
            let out = apply_live_watercolor_surface(&surf, area, &mut wet, &FrescoWatercolorParams::default(), Some(&pts), 14.0, [0.85, 0.18, 0.25, 1.0]);
            let mid = out.rgba(24, 16);
            assert!(mid[3] > 0.2 && mid[0] > mid[1], "{fmt:?} expected crimson watercolor pigment, got {mid:?}");
            assert!(wet.wetness_at(24, 16) > 0.1);
        }
    }
}
