use crate::random::hash;
use crate::{CookContext, CookResult, NodeDefinition, node};
use std::f32::consts::TAU;

const NOISES: &str = "Texture generators/Noises";

pub(super) fn definition() -> NodeDefinition {
    node("perlin_noise", "Perlin noise", NOISES)
        .desc("Tileable multi-octave gradient Perlin noise.")
        .generator()
        .int("scale", "Scale", 4, 1, 64)
        .int("octaves", "Octaves", 4, 1, 8)
        .float("persistence", "Persistence", 0.5, 0.0, 1.0)
        .float("disorder", "Disorder", 0.0, 0.0, 1.0)
        .float("contrast", "Contrast", 1.0, 0.0, 4.0)
        .gray_out()
        .cook(perlin_noise)
}

#[inline]
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn hash_unit(x: i32, y: i32, seed: u32) -> f32 {
    let h = hash((x as u32).wrapping_mul(0x9e37_79b9) ^ (y as u32).wrapping_mul(0x85eb_ca6b) ^ seed);
    (h as f32) * (1.0 / u32::MAX as f32)
}

#[inline]
fn grad_dot(ix: i32, iy: i32, period: i32, tile: bool, dx: f32, dy: f32, disorder: f32, seed: u32) -> f32 {
    let (gx_i, gy_i) = if tile {
        let p = period.max(1);
        (ix.rem_euclid(p), iy.rem_euclid(p))
    } else {
        (ix, iy)
    };
    let base_angle = hash_unit(gx_i, gy_i, seed) * TAU;
    let extra = if disorder > 0.0 { hash_unit(gx_i ^ 0x5bd1_e995, gy_i ^ 0x27d4_eb2d, seed.wrapping_add(0x1357_9bdf)) * TAU * disorder } else { 0.0 };
    let angle = base_angle + extra;
    angle.cos() * dx + angle.sin() * dy
}

pub(crate) fn sample_perlin_fbm(u: f32, v: f32, base_scale: i32, octaves: i32, persistence: f32, disorder: f32, tile: bool, seed: u32) -> f32 {
    let base_scale = base_scale.clamp(1, 64);
    let octaves = octaves.clamp(1, 8);
    let persistence = persistence.clamp(0.0, 1.0);
    let disorder = disorder.clamp(0.0, 1.0);

    let mut amp = 1.0f32;
    let mut weight_sum = 0.0f32;
    let mut acc = 0.0f32;
    let mut freq = base_scale;

    for oct in 0..octaves {
        let oct_seed = seed.wrapping_add((oct as u32).wrapping_mul(0x9e37_79b9));
        let px = u * freq as f32;
        let py = v * freq as f32;
        let x0 = px.floor();
        let y0 = py.floor();
        let fx = px - x0;
        let fy = py - y0;
        let ix = x0 as i32;
        let iy = y0 as i32;

        let n00 = grad_dot(ix, iy, freq, tile, fx, fy, disorder, oct_seed);
        let n10 = grad_dot(ix + 1, iy, freq, tile, fx - 1.0, fy, disorder, oct_seed);
        let n01 = grad_dot(ix, iy + 1, freq, tile, fx, fy - 1.0, disorder, oct_seed);
        let n11 = grad_dot(ix + 1, iy + 1, freq, tile, fx - 1.0, fy - 1.0, disorder, oct_seed);

        let sx = fade(fx);
        let sy = fade(fy);
        let nx0 = n00 + (n10 - n00) * sx;
        let nx1 = n01 + (n11 - n01) * sx;
        let val = nx0 + (nx1 - nx0) * sy;

        acc += val * amp;
        weight_sum += amp;
        amp *= persistence;
        freq = freq.saturating_mul(2).min(512);
    }

    let norm = if weight_sum > 1e-6 { acc / weight_sum } else { 0.0 };
    (norm * 0.7071_0677 + 0.5).clamp(0.0, 1.0)
}

fn perlin_noise(context: &CookContext) -> CookResult {
    let scale = context.i("scale").clamp(1, 64);
    let octaves = context.i("octaves").clamp(1, 8);
    let persistence = context.f("persistence").clamp(0.0, 1.0);
    let disorder = context.f("disorder").clamp(0.0, 1.0);
    let contrast = context.f("contrast").clamp(0.0, 4.0);
    let seed = context.seed;
    let tile = context.tile;

    Ok(vec![context.gen_gray(|u, v| {
        let n = sample_perlin_fbm(u, v, scale, octaves, persistence, disorder, tile, seed);
        ((n - 0.5) * contrast + 0.5).clamp(0.0, 1.0)
    })])
}
