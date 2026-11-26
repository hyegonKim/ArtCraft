//! Stylize effects, batch 3: Block Reveal, Film Burn, Glass Relief, Hexagonal Tiles, Palette Map
//! and Plastic Shading.
//!
//! Written from the public descriptions of what these effects do (progressive block loading,
//! burning film, bump-mapped glass / plastic surfaces lit by an effect light, hexagonal tiling,
//! a colour map sampled from the image itself); the maths is our own.

use effectcraft_color::rgb_to_hsl;
use effectcraft_keyframe::Value;
use effectcraft_project::ParamUi;
use effectcraft_raster::{Image, Px};
use rayon::prelude::*;

use crate::noise::fbm;
use crate::util::{Plane, gauss_plane, layer_or_self, lerp4, premul, smoothstep, unpremul};
use crate::{Buf, EffectCtx, EffectSpec, col, num, p, popup, slider};

fn spec(id: &'static str, name: &'static str, params: Vec<crate::ParamSpec>, render: crate::RenderFn) -> EffectSpec {
    EffectSpec { id, name, category: "Stylize", params, render, gpu: false, float: true }
}

fn pct() -> ParamUi {
    slider(0.0, 100.0, 0.0, 100.0, 1)
}

fn pt(x: f64, y: f64) -> Value {
    Value::Vec2([x, y])
}

// ------------------------------------------------------------------ Block Reveal

/// Mean colour of every `s`×`s` block (grid of `ceil(w/s)`×`ceil(h/s)`).
fn block_means(img: &Image, s: usize) -> (usize, usize, Vec<Px>) {
    let (w, h) = (img.width as usize, img.height as usize);
    let gw = w.div_ceil(s).max(1);
    let gh = h.div_ceil(s).max(1);
    let mut out = vec![[0.0f32; 4]; gw * gh];
    out.par_chunks_mut(gw).enumerate().for_each(|(by, row)| {
        for (bx, o) in row.iter_mut().enumerate() {
            let mut acc = [0.0f32; 4];
            let mut n = 0.0f32;
            for y in by * s..((by + 1) * s).min(h) {
                for x in bx * s..((bx + 1) * s).min(w) {
                    let p = img.data[y * w + x];
                    for c in 0..4 {
                        acc[c] += p[c];
                    }
                    n += 1.0;
                }
            }
            if n > 0.0 {
                *o = acc.map(|v| v / n);
            }
        }
    });
    (gw, gh, out)
}

/// Value of pixel (x, y) at block size `s` (nearest block, or bilinear between block centres).
fn block_at(g: &(usize, usize, Vec<Px>), s: usize, x: usize, y: usize, bilinear: bool) -> Px {
    let (gw, gh, d) = g;
    if !bilinear || s == 1 {
        return d[(y / s).min(gh - 1) * gw + (x / s).min(gw - 1)];
    }
    let fx = (x as f32 + 0.5) / s as f32 - 0.5;
    let fy = (y as f32 + 0.5) / s as f32 - 0.5;
    let x0 = fx.floor();
    let y0 = fy.floor();
    let (tx, ty) = (fx - x0, fy - y0);
    let gx = |v: f32| (v as i64).clamp(0, *gw as i64 - 1) as usize;
    let gy = |v: f32| (v as i64).clamp(0, *gh as i64 - 1) as usize;
    let a = d[gy(y0) * gw + gx(x0)];
    let b = d[gy(y0) * gw + gx(x0 + 1.0)];
    let c = d[gy(y0 + 1.0) * gw + gx(x0)];
    let e = d[gy(y0 + 1.0) * gw + gx(x0 + 1.0)];
    lerp4(lerp4(a, b, tx), lerp4(c, e, tx), ty)
}

fn block_load(ctx: &EffectCtx, mut b: Buf) -> Buf {
    let c = (ctx.params.f("completion") / 100.0).clamp(0.0, 1.0);
    if c >= 1.0 {
        return b;
    }
    let scan = ctx.params.b("scanlines");
    let smooth = ctx.params.b("smoothing");
    let cleared = ctx.params.b("startCleared");
    let bilinear = ctx.params.b("bilinear");
    let (w, h) = (b.img.width as usize, b.img.height as usize);
    let nlev = (w.max(h).max(2) as f64).log2().ceil() as u32;
    // Levels 0..=nlev with block sizes 2^(nlev-k); level k is "loading" while p is in [k, k+1).
    let p = c * (nlev + 1) as f64;
    let done = p.floor() as u32;
    let frac = (p - done as f64) as f32;
    let size = |k: u32| 1usize << (nlev - k.min(nlev));
    let s_new = size(done);
    let new = block_means(&b.img, s_new);
    let old = if done > 0 { Some((size(done - 1), block_means(&b.img, size(done - 1)))) } else { None };
    let rows_new = h.div_ceil(s_new) as f32;
    let soft = if smooth { 1.5f32 } else { 0.0 };
    let mut out = Image::new(w as u32, h as u32);
    out.rows_mut().for_each(|(y, row)| {
        // Fraction of the new level present on this row.
        let t = if scan {
            let front = frac * (rows_new + soft) - soft * 0.5;
            let r = (y / s_new) as f32 + 0.5;
            if soft > 0.0 {
                smoothstep(r - soft * 0.5, r + soft * 0.5, front)
            } else if r < front {
                1.0
            } else {
                0.0
            }
        } else if smooth {
            frac
        } else {
            0.0
        };
        for (x, o) in row.iter_mut().enumerate() {
            let n = if t > 0.0 { block_at(&new, s_new, x, y, bilinear) } else { [0.0; 4] };
            let prev = match &old {
                Some((s, g)) => block_at(g, *s, x, y, bilinear),
                None if cleared => [0.0; 4],
                None => block_at(&new, s_new, x, y, bilinear),
            };
            *o = if t >= 1.0 {
                n
            } else if t <= 0.0 {
                prev
            } else {
                lerp4(prev, n, t)
            };
        }
    });
    b.img = out;
    b
}

// ------------------------------------------------------------------ Film Burn

fn burn_film(ctx: &EffectCtx, mut b: Buf) -> Buf {
    let burn = (ctx.params.f("burn") / 100.0).clamp(0.0, 1.0) as f32;
    if burn <= 0.0 {
        return b;
    }
    let (cx, cy) = b.to_px(ctx.params.v2("center"));
    let seed = ctx.params.f("randomSeed").max(0.0) as u32;
    let s = b.scale;
    let diag = (ctx.layer_size[0].hypot(ctx.layer_size[1]) * s).max(1.0);
    let feat = (60.0 * s).max(1e-3);
    let th = 1.0 - burn * 1.25;
    let band = 0.12 * (burn * 10.0).min(1.0);
    b.img.rows_mut().for_each(|(y, row)| {
        for (x, px) in row.iter_mut().enumerate() {
            let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
            let d = ((fx - cx).hypot(fy - cy) / diag) as f32;
            let n = fbm((fx / feat) as f32, (fy / feat) as f32, 0.0, seed.wrapping_add(17), 4.0);
            // High near the centre and where the noise is high; burning eats from the top down.
            let f = n * 0.65 + (1.0 - d.min(1.0)) * 0.35;
            if f > th + band {
                *px = [0.0; 4];
                continue;
            }
            if f > th - band * 2.0 {
                let (c, a) = unpremul(*px);
                // Charred rim (scorched dark) with a hot orange edge right next to the hole.
                let k = smoothstep(th - band * 2.0, th, f);
                let hot = smoothstep(th, th + band, f);
                let char = 1.0 - k * 0.85;
                let mut o = [c[0] * char, c[1] * char, c[2] * char];
                o = [o[0] + (1.0 - o[0]) * hot * 0.9, o[1] + (0.45 - o[1]) * hot * 0.9, o[2] * (1.0 - hot)];
                let na = a * (1.0 - hot * 0.6);
                *px = premul(o, na);
            }
        }
    });
    b
}

// ------------------------------------------------------------------ bump-map surfaces (Glass, Plastic)

const BUMP_PROPS: [&str; 6] = ["Red", "Green", "Blue", "Alpha", "Luminance", "Lightness"];

/// The bump-map layer parameter: Glass Relief's Bump Map, Plastic Shading's Bump Layer.
pub fn bump_layer_id(ctx: &EffectCtx) -> &'static str {
    if ctx.params.get("bumpLayer").is_some() { "bumpLayer" } else { "bumpMap" }
}

/// Height field (0..1) from the bump-map layer (or the layer itself), softened.
fn height_field(ctx: &EffectCtx, b: &Buf) -> Plane {
    let src = layer_or_self(ctx, b, bump_layer_id(ctx), true, false);
    let prop = ctx.params.e("property");
    let mut pl = Plane::from_image(&src, |p| {
        let (c, a) = unpremul(p);
        let v = match prop {
            0 => c[0],
            1 => c[1],
            2 => c[2],
            3 => return a,
            4 => effectcraft_color::luminance(c[0], c[1], c[2]),
            _ => rgb_to_hsl(c[0], c[1], c[2]).2,
        };
        v * a
    });
    let soft = ctx.params.f("softness") * b.scale * 0.5;
    if soft > 0.05 {
        pl = gauss_plane(&pl, soft, soft);
    }
    pl
}

/// The effect light and surface of Glass Relief / Plastic Shading (shared with the GPU compositor).
pub struct BumpLight {
    pub intensity: f32,
    pub color: [f32; 3],
    pub point: bool,
    pub pos: (f64, f64),
    /// Distant light direction (towards the light).
    pub dir: [f32; 3],
    pub height: f32,
    pub ambient: f32,
    pub diffuse: f32,
    pub specular: f32,
    pub shininess: f32,
    pub metal: f32,
}

impl BumpLight {
    pub fn from(ctx: &EffectCtx, b: &Buf) -> BumpLight {
        let pr = &ctx.params;
        let elev = (pr.f("lightHeight").clamp(-100.0, 100.0) / 100.0 * 90.0).to_radians();
        let az = pr.f("lightDirection").to_radians();
        let dir = [(az.sin() * elev.cos()) as f32, (-az.cos() * elev.cos()) as f32, elev.sin() as f32];
        let c = pr.color("lightColor");
        let rough = pr.f("roughness").clamp(0.001, 1.0) as f32;
        BumpLight {
            intensity: (pr.f("lightIntensity") / 100.0) as f32,
            color: [c[0], c[1], c[2]],
            point: pr.e("lightType") == 1,
            pos: b.to_px(pr.v2("lightPosition")),
            dir,
            height: (pr.f("lightHeight") / 100.0 * ctx.layer_size[0].max(ctx.layer_size[1]) * b.scale * 0.5) as f32,
            ambient: (pr.f("ambient") / 100.0) as f32,
            diffuse: (pr.f("diffuse") / 100.0) as f32,
            specular: (pr.f("specular") / 100.0) as f32,
            shininess: 1.0 / rough,
            metal: (pr.f("metal") / 100.0) as f32,
        }
    }

    fn to_light(&self, x: f64, y: f64) -> [f32; 3] {
        if self.point {
            let v = [(self.pos.0 - x) as f32, (self.pos.1 - y) as f32, self.height.max(1.0)];
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
            [v[0] / l, v[1] / l, v[2] / l]
        } else {
            self.dir
        }
    }

    /// Lit straight colour for surface normal `n`. Diffuse is normalised so that a flat surface
    /// keeps its colour; highlights add on top.
    fn shade(&self, c: [f32; 3], n: [f32; 3], l: [f32; 3]) -> [f32; 3] {
        let ndl = (n[0] * l[0] + n[1] * l[1] + n[2] * l[2]).max(0.0);
        let flat = l[2].max(0.0);
        let base = self.ambient + self.diffuse * flat;
        let lit = if base > 1e-4 { (self.ambient + self.diffuse * ndl) / base } else { 1.0 };
        let lit = 1.0 + (lit - 1.0) * self.intensity;
        // Blinn half vector with the viewer straight above.
        let hv = [l[0], l[1], l[2] + 1.0];
        let hl = (hv[0] * hv[0] + hv[1] * hv[1] + hv[2] * hv[2]).sqrt().max(1e-6);
        let ndh = ((n[0] * hv[0] + n[1] * hv[1] + n[2] * hv[2]) / hl).max(0.0);
        let spec = self.specular * self.intensity * ndh.powf(self.shininess);
        let mut o = [0.0f32; 3];
        for i in 0..3 {
            let hl_col = self.color[i] * (1.0 - self.metal) + c[i] * self.color[i] * self.metal;
            o[i] = c[i] * lit * (1.0 + (self.color[i] - 1.0) * self.intensity.min(1.0)) + spec * hl_col;
        }
        o
    }
}

/// Surface normal from a height field at pixel (x, y) with height scale `k`.
#[inline]
fn normal(h: &Plane, x: usize, y: usize, k: f32) -> [f32; 3] {
    let (xi, yi) = (x as i64, y as i64);
    let gx = (h.get_clamped(xi + 1, yi) - h.get_clamped(xi - 1, yi)) * 0.5 * k;
    let gy = (h.get_clamped(xi, yi + 1) - h.get_clamped(xi, yi - 1)) * 0.5 * k;
    let l = (gx * gx + gy * gy + 1.0).sqrt();
    [-gx / l, -gy / l, 1.0 / l]
}

fn glass(ctx: &EffectCtx, mut b: Buf) -> Buf {
    let hf = height_field(ctx, &b);
    let height = ctx.params.f("height") as f32 / 100.0;
    let k = height * 25.0;
    let disp = ctx.params.f("displacement") * b.scale * 0.25;
    let light = BumpLight::from(ctx, &b);
    let src = b.img.clone();
    b.img.rows_mut().for_each(|(y, row)| {
        for (x, px) in row.iter_mut().enumerate() {
            let n = normal(&hf, x, y, k);
            let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
            let s = if disp != 0.0 && height != 0.0 {
                src.sample_bilinear_clamped(fx + n[0] as f64 * disp, fy + n[1] as f64 * disp)
            } else {
                src.data[y * src.width as usize + x]
            };
            let a = src.data[y * src.width as usize + x][3];
            if a <= 0.0 {
                *px = [0.0; 4];
                continue;
            }
            let (c, sa) = unpremul(s);
            let c = if sa > 0.0 { c } else { unpremul(src.data[y * src.width as usize + x]).0 };
            let o = light.shade(c, n, light.to_light(fx, fy));
            *px = premul(o, a);
        }
    });
    b
}

fn plastic(ctx: &EffectCtx, mut b: Buf) -> Buf {
    let mut hf = height_field(ctx, &b);
    let lo = (ctx.params.f("cutMin") / 100.0) as f32;
    let hi = (ctx.params.f("cutMax") / 100.0) as f32;
    if lo > 0.0 || hi < 1.0 {
        hf = hf.map(|v| v.clamp(lo.min(hi), hi.max(lo)));
    }
    let k = ctx.params.f("height") as f32 / 100.0 * 25.0;
    let light = BumpLight::from(ctx, &b);
    b.img.rows_mut().for_each(|(y, row)| {
        for (x, px) in row.iter_mut().enumerate() {
            if px[3] <= 0.0 {
                continue;
            }
            let (c, a) = unpremul(*px);
            let n = normal(&hf, x, y, k);
            let o = light.shade(c, n, light.to_light(x as f64 + 0.5, y as f64 + 0.5));
            *px = premul(o, a);
        }
    });
    b
}

// ------------------------------------------------------------------ Hexagonal Tiles

/// Axial hex cell (pointy-top) containing (x, y) for circumradius `r`; returns the cell centre
/// and integer cell coordinates.
fn hex_cell(x: f64, y: f64, r: f64) -> ((f64, f64), (i64, i64)) {
    let q = (3f64.sqrt() / 3.0 * x - y / 3.0) / r;
    let rr = (2.0 / 3.0 * y) / r;
    // Cube rounding.
    let (cx, cz) = (q, rr);
    let cy = (-3f64.sqrt() / 3.0 * x - y / 3.0) / r;
    let (mut rx, ry, mut rz) = (cx.round(), cy.round(), cz.round());
    let (dx, dy, dz) = ((rx - cx).abs(), (ry - cy).abs(), (rz - cz).abs());
    if dx > dy && dx > dz {
        rx = -ry - rz;
    } else if dy <= dz {
        rz = -rx - ry;
    }
    let px = r * 3f64.sqrt() * (rx + rz / 2.0);
    let py = r * 1.5 * rz;
    ((px, py), (rx as i64, rz as i64))
}

/// Stable f64 hex-cell selection and source geometry for CPU and GPU renders.
pub struct HexTile {
    r: f64,
    c: (f64, f64),
    sn: f64,
    cs: f64,
    smear: f64,
    mode: u32,
}
impl HexTile {
    /// No image pixels are read; only buffer geometry and parameters are used.
    pub fn new(ctx: &EffectCtx, b: &Buf) -> Self {
        let r = (ctx.params.f("radius") * b.scale).max(0.5);
        let c = b.to_px(ctx.params.v2("center"));
        let rot = ctx.params.f("rotate").to_radians();
        let (sn, cs) = rot.sin_cos();
        let smear = (ctx.params.f("smearing") / 100.0).clamp(0.0, 1.0);
        let mode = ctx.params.e("render");
        Self { r, c, sn, cs, smear, mode }
    }
    /// Bilinear source position at a buffer pixel centre.
    pub fn source_at(&self, x: u32, y: u32) -> (f64, f64) {
        let Self { r, c: (cx, cy), sn, cs, smear, mode } = *self;
        let (dx, dy) = (x as f64 + 0.5 - cx, y as f64 + 0.5 - cy);
        // Into the rotated tiling frame.
        let (u, v) = (dx * cs + dy * sn, -dx * sn + dy * cs);
        let ((hx, hy), (qi, ri)) = hex_cell(u, v, r);
        let (mut lu, lv) = (u - hx, v - hy);
        let odd = (qi + ri).rem_euclid(2) == 1;
        match mode {
            1 if odd => lu = -lu,
            2 if odd => lu = -lu,
            _ => {}
        }
        let lv = if mode == 2 && odd { -lv } else { lv };
        // Every tile shows the hexagon around the centre point.
        let (ox, oy) = (lu * cs - lv * sn, lu * sn + lv * cs);
        let (tx, ty) = (cx + ox, cy + oy);
        let (sx, sy) = (tx + (x as f64 + 0.5 - tx) * smear, ty + (y as f64 + 0.5 - ty) * smear);
        (sx, sy)
    }
}
fn hextile(ctx: &EffectCtx, mut b: Buf) -> Buf {
    let geometry = HexTile::new(ctx, &b);
    let src = b.img.clone();
    b.img.rows_mut().for_each(|(y, row)| {
        for (x, px) in row.iter_mut().enumerate() {
            let (sx, sy) = geometry.source_at(x as u32, y as u32);
            *px = src.sample_bilinear_clamped(sx, sy);
        }
    });
    b
}

// ------------------------------------------------------------------ Palette Map

const SMOOTHIE_PROPS: [&str; 8] = ["Red", "Green", "Blue", "Alpha", "Luminance", "Lightness", "Hue", "Saturation"];

fn smoothie(ctx: &EffectCtx, mut b: Buf) -> Buf {
    let flow = layer_or_self(ctx, &b, "flowLayer", true, false);
    let prop = ctx.params.e("property");
    let mut pl = Plane::from_image(&flow, |p| {
        let (c, a) = unpremul(p);
        match prop {
            0 => c[0],
            1 => c[1],
            2 => c[2],
            3 => a,
            4 => effectcraft_color::luminance(c[0], c[1], c[2]),
            5 => rgb_to_hsl(c[0], c[1], c[2]).2,
            6 => rgb_to_hsl(c[0], c[1], c[2]).0,
            _ => rgb_to_hsl(c[0], c[1], c[2]).1,
        }
    });
    let sm = ctx.params.f("smoothness") * b.scale * 0.5;
    if sm > 0.05 {
        pl = gauss_plane(&pl, sm, sm);
    }
    let (ax, ay) = b.to_px(ctx.params.v2("sampleA"));
    let (bx, by) = b.to_px(ctx.params.v2("sampleB"));
    // Colour palette sampled from the image along A→B.
    const N: usize = 256;
    let pal: Vec<Px> = (0..N)
        .map(|i| {
            let t = i as f64 / (N - 1) as f64;
            let p = b.img.sample_bilinear_clamped(ax + (bx - ax) * t, ay + (by - ay) * t);
            let (c, _) = unpremul(p);
            [c[0], c[1], c[2], 1.0]
        })
        .collect();
    let phase = (ctx.params.f("phase") / 360.0) as f32;
    let loops = (ctx.params.e("colorLoop") + 1) as f32;
    b.img.data.par_iter_mut().zip(pl.data.par_iter()).for_each(|(px, &v)| {
        if px[3] <= 0.0 {
            return;
        }
        let t = (v * loops + phase).rem_euclid(1.0);
        // Ping-pong so the palette loops without seams.
        let t = if t < 0.5 { t * 2.0 } else { 2.0 - t * 2.0 };
        let f = t * (N - 1) as f32;
        let i = (f as usize).min(N - 2);
        let c = lerp4(pal[i], pal[i + 1], f - i as f32);
        *px = premul([c[0], c[1], c[2]], px[3]);
    });
    b
}

// ------------------------------------------------------------------ specs

fn light_params() -> Vec<crate::ParamSpec> {
    vec![
        p("using", "Using", Value::Enum(0), popup(&["Effect Light", "AE Lights"])),
        p("lightIntensity", "Light Intensity", num(100.0), slider(0.0, 400.0, 0.0, 200.0, 1)),
        p("lightColor", "Light Color", col(1.0, 1.0, 1.0), ParamUi::Color),
        p("lightType", "Light Type", Value::Enum(0), popup(&["Distant Light", "Point Light"])),
        p("lightHeight", "Light Height", num(50.0), slider(-100.0, 100.0, -100.0, 100.0, 1)),
        p("lightPosition", "Light Position", pt(0.25, 0.25), ParamUi::Point),
        p("lightDirection", "Light Direction", num(-45.0), ParamUi::Angle),
    ]
}

fn shading_params(ambient: f64, diffuse: f64, specular: f64, roughness: f64, metal: f64) -> Vec<crate::ParamSpec> {
    vec![
        p("ambient", "Ambient", num(ambient), slider(0.0, 200.0, 0.0, 100.0, 1)),
        p("diffuse", "Diffuse", num(diffuse), slider(0.0, 200.0, 0.0, 100.0, 1)),
        p("specular", "Specular", num(specular), slider(0.0, 200.0, 0.0, 100.0, 1)),
        p("roughness", "Roughness", num(roughness), slider(0.001, 0.5, 0.001, 0.5, 3)),
        p("metal", "Metal", num(metal), pct()),
    ]
}

pub fn specs() -> Vec<EffectSpec> {
    let mut glass_p = vec![
        p("bumpMap", "Bump Map", Value::Layer(None), ParamUi::Layer),
        p("property", "Property", Value::Enum(3), popup(&BUMP_PROPS)),
        p("softness", "Softness", num(10.0), slider(0.0, 200.0, 0.0, 100.0, 1)),
        p("height", "Height", num(50.0), slider(-100.0, 100.0, -100.0, 100.0, 1)),
        p("displacement", "Displacement", num(100.0), slider(-500.0, 500.0, -200.0, 200.0, 1)),
    ];
    glass_p.extend(light_params());
    glass_p.extend(shading_params(75.0, 32.0, 26.0, 0.05, 100.0));
    let mut plastic_p = vec![
        p("bumpLayer", "Bump Layer", Value::Layer(None), ParamUi::Layer),
        p("property", "Property", Value::Enum(3), popup(&BUMP_PROPS)),
        p("softness", "Softness", num(10.0), slider(0.0, 200.0, 0.0, 100.0, 1)),
        p("height", "Height", num(25.0), slider(-100.0, 100.0, -100.0, 100.0, 1)),
        p("cutMin", "Cut Min", num(0.0), pct()),
        p("cutMax", "Cut Max", num(100.0), pct()),
    ];
    plastic_p.extend(light_params());
    plastic_p.extend(shading_params(100.0, 70.0, 50.0, 0.05, 50.0));
    vec![
        spec(
            "ec.stylize.ccblockload",
            "Block Reveal",
            vec![
                p("completion", "Completion", num(0.0), pct()),
                p("scanlines", "Scanlines", Value::Bool(true), ParamUi::Checkbox),
                p("smoothing", "Smoothing", Value::Bool(true), ParamUi::Checkbox),
                p("startCleared", "Start Cleared", Value::Bool(true), ParamUi::Checkbox),
                p("bilinear", "Bilinear", Value::Bool(false), ParamUi::Checkbox),
            ],
            block_load,
        ),
        spec(
            "ec.stylize.ccburnfilm",
            "Film Burn",
            vec![
                p("burn", "Burn", num(0.0), pct()),
                p("center", "Center", pt(0.5, 0.5), ParamUi::Point),
                p("randomSeed", "Random Seed", num(0.0), slider(0.0, 10000.0, 0.0, 100.0, 0)),
            ],
            burn_film,
        ),
        spec("ec.stylize.ccglass", "Glass Relief", glass_p, glass),
        spec(
            "ec.stylize.cchextile",
            "Hexagonal Tiles",
            vec![
                p("render", "Render", Value::Enum(0), popup(&["Normal", "Mirrored", "Rotated"])),
                p("radius", "Radius", num(50.0), slider(1.0, 4000.0, 2.0, 500.0, 1)),
                p("center", "Center", pt(0.5, 0.5), ParamUi::Point),
                p("rotate", "Rotate", num(0.0), ParamUi::Angle),
                p("smearing", "Smearing", num(0.0), pct()),
            ],
            hextile,
        ),
        spec(
            "ec.stylize.ccmrsmoothie",
            "Palette Map",
            vec![
                p("flowLayer", "Flow Layer", Value::Layer(None), ParamUi::Layer),
                p("property", "Property", Value::Enum(4), popup(&SMOOTHIE_PROPS)),
                p("smoothness", "Smoothness", num(20.0), slider(0.0, 200.0, 0.0, 100.0, 1)),
                p("sampleA", "Sample A", pt(0.25, 0.5), ParamUi::Point),
                p("sampleB", "Sample B", pt(0.75, 0.5), ParamUi::Point),
                p("phase", "Phase", num(0.0), ParamUi::Angle),
                p("colorLoop", "Color Loop", Value::Enum(0), popup(&["1x", "2x", "3x", "4x"])),
            ],
            smoothie,
        ),
        spec("ec.stylize.ccplastic", "Plastic Shading", plastic_p, plastic),
        spec(
            "ec.stylize.outlinepaint",
            "Outline Paint",
            vec![
                p("smearRadius", "Smear Radius", num(6.0), slider(0.0, 100.0, 0.0, 40.0, 1)),
                p("smearAngle", "Smear Angle", num(35.0), ParamUi::Angle),
                p("paletteColors", "Palette Colors", num(5.0), slider(2.0, 8.0, 2.0, 8.0, 0)),
                p("sampleFromImage", "Sample from Image", Value::Bool(true), ParamUi::Checkbox),
                p("color1", "Color 1", col(0.11, 0.14, 0.24), ParamUi::Color),
                p("color2", "Color 2", col(0.84, 0.33, 0.24), ParamUi::Color),
                p("color3", "Color 3", col(0.24, 0.56, 0.58), ParamUi::Color),
                p("color4", "Color 4", col(0.92, 0.72, 0.28), ParamUi::Color),
                p("color5", "Color 5", col(0.96, 0.93, 0.86), ParamUi::Color),
                p("color6", "Color 6", col(0.42, 0.26, 0.20), ParamUi::Color),
                p("softness", "Posterize Softness", num(10.0), pct()),
                p("outlineWidth", "Outline Width", num(2.0), slider(0.0, 25.0, 0.0, 10.0, 1)),
                p("outlineThreshold", "Outline Threshold", num(22.0), pct()),
                p("outlineColor", "Outline Color", col(0.08, 0.07, 0.09), ParamUi::Color),
                p("blendWithOriginal", "Blend with Original", num(0.0), pct()),
            ],
            outline_paint,
        ),
    ]
}

// ------------------------------------------------------------------ Outline Paint (AE 27.0)

fn luma_straight(rgb: [f32; 3]) -> f32 {
    0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
}

fn sample_image_palette(straight: &[[f32; 4]], k: usize) -> Vec<[f32; 3]> {
    let stride = (straight.len() / 4096).max(1);
    let mut samples: Vec<[f32; 3]> = straight
        .iter()
        .step_by(stride)
        .filter(|p| p[3] > 0.05)
        .map(|p| [p[0].clamp(0.0, 1.0), p[1].clamp(0.0, 1.0), p[2].clamp(0.0, 1.0)])
        .collect();
    if samples.is_empty() {
        return (0..k).map(|i| [i as f32 / (k.saturating_sub(1).max(1)) as f32; 3]).collect();
    }
    samples.sort_by(|a, b| luma_straight(*a).total_cmp(&luma_straight(*b)));
    let mut centers: Vec<[f32; 3]> = (0..k)
        .map(|i| {
            let idx = ((i * 2 + 1) * samples.len()) / (k * 2);
            samples[idx.min(samples.len() - 1)]
        })
        .collect();
    for _ in 0..4 {
        let mut sums = vec![[0.0f64; 3]; k];
        let mut counts = vec![0usize; k];
        for &s in &samples {
            let mut best = 0usize;
            let mut best_d = f32::INFINITY;
            for (ci, c) in centers.iter().enumerate() {
                let d = (s[0] - c[0]).powi(2) + (s[1] - c[1]).powi(2) + (s[2] - c[2]).powi(2);
                if d < best_d {
                    best_d = d;
                    best = ci;
                }
            }
            sums[best][0] += f64::from(s[0]);
            sums[best][1] += f64::from(s[1]);
            sums[best][2] += f64::from(s[2]);
            counts[best] += 1;
        }
        for ci in 0..k {
            if counts[ci] > 0 {
                let inv = 1.0 / counts[ci] as f64;
                centers[ci] = [(sums[ci][0] * inv) as f32, (sums[ci][1] * inv) as f32, (sums[ci][2] * inv) as f32];
            }
        }
    }
    centers
}

fn custom_palette(ctx: &EffectCtx, k: usize) -> Vec<[f32; 3]> {
    let base = [
        ctx.params.color("color1"),
        ctx.params.color("color2"),
        ctx.params.color("color3"),
        ctx.params.color("color4"),
        ctx.params.color("color5"),
        ctx.params.color("color6"),
    ];
    let rgb = |c: [f32; 4]| [c[0], c[1], c[2]];
    if k <= 6 {
        base[..k].iter().copied().map(rgb).collect()
    } else {
        (0..k)
            .map(|i| {
                let t = i as f32 * 5.0 / (k - 1) as f32;
                let i0 = (t.floor() as usize).min(5);
                let i1 = (i0 + 1).min(5);
                let f = t - i0 as f32;
                let a = rgb(base[i0]);
                let b = rgb(base[i1]);
                [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f]
            })
            .collect()
    }
}

fn outline_paint(ctx: &EffectCtx, b: Buf) -> Buf {
    let blend_orig = (ctx.params.f("blendWithOriginal") / 100.0).clamp(0.0, 1.0) as f32;
    if blend_orig >= 1.0 - 1e-6 {
        return b;
    }
    let (w, h) = (b.img.width as usize, b.img.height as usize);
    if w == 0 || h == 0 {
        return b;
    }
    let scale = b.scale.max(0.05) as f32;
    let smear_r = (ctx.params.f("smearRadius") as f32 * scale).clamp(0.0, 80.0);
    let angle_rad = (ctx.params.f("smearAngle") as f32).to_radians();
    let (bias_dx, bias_dy) = (angle_rad.cos(), angle_rad.sin());
    let k = (ctx.params.f("paletteColors").round() as usize).clamp(2, 8);
    let sample_auto = ctx.params.b("sampleFromImage");
    let softness = (ctx.params.f("softness") / 100.0).clamp(0.0, 1.0) as f32;
    let out_w = (ctx.params.f("outlineWidth") as f32 * scale).clamp(0.0, 32.0);
    let out_thresh = (ctx.params.f("outlineThreshold") / 100.0).clamp(0.01, 0.99) as f32;
    let out_col = ctx.params.color("outlineColor");

    let straight: Vec<[f32; 4]> = b.img.data.iter().copied().map(|px| {
        let (c, a) = unpremul(px);
        [c[0], c[1], c[2], a]
    }).collect();
    let luma: Vec<f32> = straight.iter().map(|p| luma_straight([p[0], p[1], p[2]]) * p[3]).collect();
    let clamp_xy = |x: i32, y: i32| -> usize {
        (y.clamp(0, h as i32 - 1) as usize) * w + (x.clamp(0, w as i32 - 1) as usize)
    };

    // Pass 1: edge-guided directional brush smear.
    let smeared: Vec<[f32; 4]> = if smear_r < 0.25 {
        straight.clone()
    } else {
        let steps = (smear_r.ceil() as i32).clamp(1, 24);
        let step_len = smear_r / steps as f32;
        let mut out_s = vec![[0.0f32; 4]; w * h];
        out_s.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
            let yi = y as i32;
            for (x, dst) in row.iter_mut().enumerate() {
                let xi = x as i32;
                let gx = -luma[clamp_xy(xi - 1, yi - 1)] - 2.0 * luma[clamp_xy(xi - 1, yi)] - luma[clamp_xy(xi - 1, yi + 1)]
                    + luma[clamp_xy(xi + 1, yi - 1)] + 2.0 * luma[clamp_xy(xi + 1, yi)] + luma[clamp_xy(xi + 1, yi + 1)];
                let gy = -luma[clamp_xy(xi - 1, yi - 1)] - 2.0 * luma[clamp_xy(xi, yi - 1)] - luma[clamp_xy(xi + 1, yi - 1)]
                    + luma[clamp_xy(xi - 1, yi + 1)] + 2.0 * luma[clamp_xy(xi, yi + 1)] + luma[clamp_xy(xi + 1, yi + 1)];
                let gmag = gx.hypot(gy);
                // Tangent runs perpendicular to gradient; blend with user brush angle.
                let (tx, ty) = if gmag > 0.04 {
                    let (ux, uy) = (-gy / gmag, gx / gmag);
                    let sign = if ux * bias_dx + uy * bias_dy < 0.0 { -1.0 } else { 1.0 };
                    let edge_w = (gmag * 2.5).clamp(0.0, 0.85);
                    let mx = ux * sign * edge_w + bias_dx * (1.0 - edge_w);
                    let my = uy * sign * edge_w + bias_dy * (1.0 - edge_w);
                    let mlen = mx.hypot(my).max(1e-5);
                    (mx / mlen, my / mlen)
                } else {
                    (bias_dx, bias_dy)
                };
                let center = straight[y * w + x];
                let center_l = luma[y * w + x];
                let mut acc = [0.0f32; 4];
                let mut wsum = 0.0f32;
                for s in -steps..=steps {
                    let dist = s as f32 * step_len;
                    let sx = (xi as f32 + tx * dist).round() as i32;
                    let sy = (yi as f32 + ty * dist).round() as i32;
                    let idx = clamp_xy(sx, sy);
                    let sp = straight[idx];
                    let dl = (luma[idx] - center_l).abs();
                    let spatial = (-0.5 * (dist / (smear_r * 0.65).max(0.5)).powi(2)).exp();
                    let range = (-dl * dl * 18.0).exp();
                    let wt = spatial * range * sp[3].max(0.02);
                    for c in 0..3 {
                        acc[c] += sp[c] * wt;
                    }
                    acc[3] += sp[3] * spatial;
                    wsum += wt;
                }
                if wsum > 1e-6 {
                    let norm_a = (steps * 2 + 1) as f32;
                    *dst = [acc[0] / wsum, acc[1] / wsum, acc[2] / wsum, center[3].max(acc[3] / norm_a).min(1.0)];
                } else {
                    *dst = center;
                }
            }
        });
        out_s
    };

    // Pass 2: palette extraction / custom palette quantization + ink outline.
    let palette = if sample_auto { sample_image_palette(&smeared, k) } else { custom_palette(ctx, k) };
    let sme_luma: Vec<f32> = smeared.iter().map(|p| luma_straight([p[0], p[1], p[2]])).collect();
    let edge_step = out_w.max(1.0).round() as i32;

    let mut out = Image::new(w as u32, h as u32);
    out.rows_mut().for_each(|(y, row)| {
        let yi = y as i32;
        for (x, dst) in row.iter_mut().enumerate() {
            let sp = smeared[y * w + x];
            let orig_px = b.img.data[y * w + x];
            if sp[3] <= 1e-6 && out_w <= 0.0 {
                *dst = orig_px;
                continue;
            }
            let mut best0 = 0usize;
            let mut best1 = 0usize;
            let mut d0 = f32::INFINITY;
            let mut d1 = f32::INFINITY;
            for (ci, c) in palette.iter().enumerate() {
                let d = (sp[0] - c[0]).powi(2) + (sp[1] - c[1]).powi(2) + (sp[2] - c[2]).powi(2);
                if d < d0 {
                    d1 = d0;
                    best1 = best0;
                    d0 = d;
                    best0 = ci;
                } else if d < d1 {
                    d1 = d;
                    best1 = ci;
                }
            }
            let mut rgb = palette[best0];
            if softness > 1e-4 && best1 != best0 && d1.is_finite() {
                let ratio = (d0.sqrt() / (d0.sqrt() + d1.sqrt()).max(1e-6)).clamp(0.0, 0.5);
                let t = smoothstep(0.5 * (1.0 - softness), 0.5, ratio) * 0.5;
                let c1 = palette[best1];
                rgb = [rgb[0] + (c1[0] - rgb[0]) * t, rgb[1] + (c1[1] - rgb[1]) * t, rgb[2] + (c1[2] - rgb[2]) * t];
            }

            if out_w > 0.05 {
                let xi = x as i32;
                let r = edge_step.max(1);
                let gx = (sme_luma[clamp_xy(xi + r, yi)] - sme_luma[clamp_xy(xi - r, yi)])
                    + 0.5 * (sme_luma[clamp_xy(xi + r, yi - r)] - sme_luma[clamp_xy(xi - r, yi - r)])
                    + 0.5 * (sme_luma[clamp_xy(xi + r, yi + r)] - sme_luma[clamp_xy(xi - r, yi + r)]);
                let gy = (sme_luma[clamp_xy(xi, yi + r)] - sme_luma[clamp_xy(xi, yi - r)])
                    + 0.5 * (sme_luma[clamp_xy(xi - r, yi + r)] - sme_luma[clamp_xy(xi - r, yi - r)])
                    + 0.5 * (sme_luma[clamp_xy(xi + r, yi + r)] - sme_luma[clamp_xy(xi + r, yi - r)]);
                let edge = (gx.hypot(gy) * 0.5).clamp(0.0, 1.0);
                let ink = smoothstep(out_thresh * 0.7, (out_thresh * 1.15).min(1.0), edge) * (out_w.min(1.0)) * out_col[3];
                if ink > 1e-4 {
                    rgb = [
                        rgb[0] * (1.0 - ink) + out_col[0] * ink,
                        rgb[1] * (1.0 - ink) + out_col[1] * ink,
                        rgb[2] * (1.0 - ink) + out_col[2] * ink,
                    ];
                }
            }
            let painted = premul(rgb, sp[3]);
            *dst = if blend_orig > 0.0 { lerp4(painted, orig_px, blend_orig) } else { painted };
        }
    });
    Buf { img: out, offset: b.offset, scale: b.scale }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EffectEnv, run_fx};

    fn ramp(w: u32, h: u32) -> Image {
        let mut img = Image::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.set(x, y, [x as f32 / w as f32, y as f32 / h as f32, 0.3, 1.0]);
            }
        }
        img
    }

    fn bumpy(w: u32, h: u32) -> Image {
        let mut img = ramp(w, h);
        for y in 0..h {
            for x in 0..w {
                let a = if (x as i32 - 20).pow(2) + (y as i32 - 16).pow(2) < 64 { 1.0 } else { 0.5 };
                let p = img.data[(y * w + x) as usize];
                img.set(x, y, [p[0] * a, p[1] * a, p[2] * a, a]);
            }
        }
        img
    }

    fn run(id: &str, vals: &[(&str, Value)], img: Image) -> Buf {
        run_fx(id, vals, img, 0.0, EffectEnv::default())
    }

    fn max_diff(a: &Image, b: &Image) -> f32 {
        a.data.iter().zip(&b.data).flat_map(|(p, q)| (0..4).map(move |c| (p[c] - q[c]).abs())).fold(0.0, f32::max)
    }

    const ALL: [&str; 7] = [
        "ec.stylize.ccblockload",
        "ec.stylize.ccburnfilm",
        "ec.stylize.ccglass",
        "ec.stylize.cchextile",
        "ec.stylize.ccmrsmoothie",
        "ec.stylize.ccplastic",
        "ec.stylize.outlinepaint",
    ];

    #[test]
    fn all_deterministic() {
        for id in ALL {
            let vals: &[(&str, Value)] = match id {
                "ec.stylize.ccblockload" => &[("completion", num(40.0))],
                "ec.stylize.ccburnfilm" => &[("burn", num(50.0))],
                _ => &[],
            };
            let a = run(id, vals, bumpy(40, 32));
            let b = run(id, vals, bumpy(40, 32));
            assert_eq!(a.img.data, b.img.data, "{id}");
        }
    }

    #[test]
    fn block_load_complete_is_identity_and_start_cleared() {
        let img = ramp(40, 30);
        assert_eq!(run("ec.stylize.ccblockload", &[("completion", num(100.0))], img.clone()).img.data, img.data);
        let o = run("ec.stylize.ccblockload", &[("completion", num(0.0))], img.clone());
        assert!(o.img.data.iter().all(|p| p[3] == 0.0));
        // Midway: blocky (fewer distinct colours than the source).
        let o = run("ec.stylize.ccblockload", &[("completion", num(50.0)), ("smoothing", Value::Bool(false))], img.clone());
        let mut cols: Vec<u32> = o.img.data.iter().map(|p| (p[0] * 1000.0) as u32).collect();
        cols.sort();
        cols.dedup();
        assert!(cols.len() < 40, "{}", cols.len());
    }

    #[test]
    fn burn_film_zero_identity_full_burns() {
        let img = ramp(40, 30);
        assert_eq!(run("ec.stylize.ccburnfilm", &[], img.clone()).img.data, img.data);
        let o = run("ec.stylize.ccburnfilm", &[("burn", num(100.0))], img);
        assert!(o.img.data.iter().all(|p| p[3] == 0.0));
    }

    #[test]
    fn glass_flat_without_highlight_is_identity() {
        let img = bumpy(40, 32);
        let o = run("ec.stylize.ccglass", &[("height", num(0.0)), ("specular", num(0.0))], img.clone());
        assert!(max_diff(&o.img, &img) < 1e-4, "{}", max_diff(&o.img, &img));
        let o = run("ec.stylize.ccglass", &[], img.clone());
        assert!(max_diff(&o.img, &img) > 0.01);
    }

    #[test]
    fn plastic_shades_bumps() {
        let img = bumpy(40, 32);
        let o = run("ec.stylize.ccplastic", &[("height", num(0.0)), ("specular", num(0.0))], img.clone());
        assert!(max_diff(&o.img, &img) < 1e-4);
        let o = run("ec.stylize.ccplastic", &[("height", num(80.0))], img.clone());
        assert!(max_diff(&o.img, &img) > 0.01);
        // Alpha is untouched.
        assert!(o.img.data.iter().zip(&img.data).all(|(a, b)| (a[3] - b[3]).abs() < 1e-6));
    }

    #[test]
    fn hextile_repeats_centre() {
        let img = ramp(64, 64);
        let o = run("ec.stylize.cchextile", &[("radius", num(8.0)), ("center", pt(32.0, 32.0))], img.clone());
        assert!(max_diff(&o.img, &img) > 0.1);
        // Pixels at the centre are unchanged (inside the central tile).
        let c = (32 * 64 + 32) as usize;
        assert!((0..4).all(|i| (o.img.data[c][i] - img.data[c][i]).abs() < 1e-4));
        // Full smearing restores the source.
        let o = run("ec.stylize.cchextile", &[("radius", num(8.0)), ("smearing", num(100.0))], img.clone());
        assert!(max_diff(&o.img, &img) < 1e-4);
    }

    #[test]
    fn smoothie_maps_to_sampled_palette() {
        let img = ramp(40, 30);
        let o = run("ec.stylize.ccmrsmoothie", &[("smoothness", num(0.0)), ("sampleA", pt(10.0, 15.0)), ("sampleB", pt(30.0, 15.0))], img.clone());
        // Every output colour lies on the palette line between A and B (blue stays 0.3, green ≈ 0.5).
        assert!(o.img.data.iter().all(|p| (p[2] - 0.3).abs() < 1e-3 && (p[1] - 0.5).abs() < 0.03));
        assert!(max_diff(&o.img, &img) > 0.1);
    }

    #[test]
    fn outline_paint_posterizes_smears_and_outlines() {
        let img = ramp(40, 30);
        // 100% blend with original is exact identity.
        let id = run("ec.stylize.outlinepaint", &[("blendWithOriginal", num(100.0))], img.clone());
        assert!(max_diff(&id.img, &img) < 1e-5);

        // Custom 3-color crisp posterization maps every opaque pixel to one of the 3 swatches.
        let c1 = col(0.9, 0.1, 0.1);
        let c2 = col(0.1, 0.8, 0.2);
        let c3 = col(0.1, 0.2, 0.9);
        let o = run(
            "ec.stylize.outlinepaint",
            &[
                ("sampleFromImage", Value::Bool(false)),
                ("paletteColors", num(3.0)),
                ("softness", num(0.0)),
                ("outlineWidth", num(0.0)),
                ("color1", c1.clone()),
                ("color2", c2.clone()),
                ("color3", c3.clone()),
            ],
            img.clone(),
        );
        let swatches = [[0.9f32, 0.1, 0.1], [0.1, 0.8, 0.2], [0.1, 0.2, 0.9]];
        assert!(o.img.data.iter().all(|px| {
            swatches.iter().any(|s| (px[0] - s[0]).abs() < 1e-3 && (px[1] - s[1]).abs() < 1e-3 && (px[2] - s[2]).abs() < 1e-3)
        }));

        // Sampled 4-color crisp posterization produces at most 4 distinct colors without outlines.
        let sampled = run(
            "ec.stylize.outlinepaint",
            &[("sampleFromImage", Value::Bool(true)), ("paletteColors", num(4.0)), ("softness", num(0.0)), ("outlineWidth", num(0.0))],
            img,
        );
        let mut distinct: Vec<[u32; 3]> = sampled
            .img
            .data
            .iter()
            .map(|p| [(p[0] * 1000.0).round() as u32, (p[1] * 1000.0).round() as u32, (p[2] * 1000.0).round() as u32])
            .collect();
        distinct.sort();
        distinct.dedup();
        assert!(distinct.len() <= 4 && distinct.len() >= 2, "distinct={}", distinct.len());
    }
}
