use crate::random::hash;
use crate::{CookContext, CookResult, NodeDefinition, node};
use std::f32::consts::TAU;

const PATTERNS: &str = "Texture generators/Patterns";

pub(super) fn definition() -> NodeDefinition {
    node("tile_sampler", "Tile sampler", PATTERNS)
        .desc("Scatters procedural or custom grayscale stamps on a jittered grid with mask gating.")
        .atomic()
        .gray_in("pattern", "Pattern")
        .gray_in("mask", "Mask")
        .choice("shape", "Shape", &["Square", "Disc", "Paraboloid", "Brick", "Bell"], 0)
        .int("amount_x", "Amount X", 4, 1, 32)
        .int("amount_y", "Amount Y", 4, 1, 32)
        .float("size_x", "Size X", 0.85, 0.05, 2.0)
        .float("size_y", "Size Y", 0.85, 0.05, 2.0)
        .float("offset", "Row offset", 0.0, 0.0, 1.0)
        .float("position_random", "Position random", 0.0, 0.0, 1.0)
        .float("scale_random", "Scale random", 0.0, 0.0, 1.0)
        .float("rotation_random", "Rotation random", 0.0, 0.0, 1.0)
        .float("luminance_random", "Luminance random", 0.15, 0.0, 1.0)
        .float("mask_threshold", "Mask threshold", 0.0, 0.0, 1.0)
        .gray_out()
        .cook(tile_sampler)
}

#[inline]
fn cell_rand(cx: i32, cy: i32, channel: u32, seed: u32) -> f32 {
    let h = hash((cx as u32).wrapping_mul(0x9e37_79b9) ^ (cy as u32).wrapping_mul(0x85eb_ca6b) ^ channel.wrapping_mul(0xc2b2_ae35) ^ seed);
    (h as f32) * (1.0 / u32::MAX as f32)
}

#[inline]
fn eval_shape(shape: i32, lx: f32, ly: f32) -> f32 {
    if lx.abs() > 1.0 || ly.abs() > 1.0 {
        return 0.0;
    }
    match shape {
        1 => {
            let r = (lx * lx + ly * ly).sqrt();
            if r <= 1.0 { ((1.0 - r) * 6.0).clamp(0.0, 1.0) } else { 0.0 }
        }
        2 => (1.0 - (lx * lx + ly * ly)).clamp(0.0, 1.0),
        3 => {
            let ex = (1.0 - lx.abs()) * 5.0;
            let ey = (1.0 - ly.abs()) * 5.0;
            ex.min(ey).clamp(0.0, 1.0)
        }
        4 => {
            let r2 = lx * lx + ly * ly;
            if r2 >= 1.0 {
                0.0
            } else {
                let t = 1.0 - r2;
                t * t
            }
        }
        _ => 1.0,
    }
}

fn tile_sampler(context: &CookContext) -> CookResult {
    let custom_pattern = context.input(0).map(|img| img.to_gray());
    let mask_img = context.input(1).map(|img| img.to_gray());
    let shape = context.i("shape").clamp(0, 4);
    let ax = context.i("amount_x").clamp(1, 32);
    let ay = context.i("amount_y").clamp(1, 32);
    let sx = context.f("size_x").clamp(0.05, 2.0);
    let sy = context.f("size_y").clamp(0.05, 2.0);
    let offset = context.f("offset").clamp(0.0, 1.0);
    let pos_rnd = context.f("position_random").clamp(0.0, 1.0);
    let scl_rnd = context.f("scale_random").clamp(0.0, 1.0);
    let rot_rnd = context.f("rotation_random").clamp(0.0, 1.0);
    let lum_rnd = context.f("luminance_random").clamp(0.0, 1.0);
    let mask_thresh = context.f("mask_threshold").clamp(0.0, 1.0);
    let seed = context.seed;
    let tile = context.tile;

    Ok(vec![context.gen_gray(|u, v| {
        let gx = u * ax as f32;
        let gy = v * ay as f32;
        let base_cy = gy.floor() as i32;
        let mut best = 0.0f32;

        for oy in -2..=2 {
            let cy_raw = base_cy + oy;
            if !tile && (cy_raw < 0 || cy_raw >= ay) {
                continue;
            }
            let cy_id = if tile { cy_raw.rem_euclid(ay) } else { cy_raw };
            let row_off = if cy_raw.rem_euclid(2) != 0 { offset } else { 0.0 };

            for ox in -2..=2 {
                let cx_raw = (gx - row_off).floor() as i32 + ox;
                if !tile && (cx_raw < 0 || cx_raw >= ax) {
                    continue;
                }
                let cx_id = if tile { cx_raw.rem_euclid(ax) } else { cx_raw };

                let jx = (cell_rand(cx_id, cy_id, 1, seed) - 0.5) * pos_rnd;
                let jy = (cell_rand(cx_id, cy_id, 2, seed) - 0.5) * pos_rnd;
                let center_x = cx_raw as f32 + 0.5 + row_off + jx;
                let center_y = cy_raw as f32 + 0.5 + jy;

                if let Some(mask) = &mask_img {
                    let mu = center_x / ax as f32;
                    let mv = center_y / ay as f32;
                    if mask.sample_gray(mu, mv, tile) < mask_thresh {
                        continue;
                    }
                }

                let scale_mod = (1.0 - scl_rnd * cell_rand(cx_id, cy_id, 3, seed) * 0.75).max(0.05);
                let half_w = (sx * scale_mod * 0.5).max(0.01);
                let half_h = (sy * scale_mod * 0.5).max(0.01);

                let dx = gx - center_x;
                let dy = gy - center_y;
                let angle = (cell_rand(cx_id, cy_id, 4, seed) - 0.5) * TAU * rot_rnd;
                let (sin_a, cos_a) = angle.sin_cos();
                let rx = (dx * cos_a + dy * sin_a) / half_w;
                let ry = (-dx * sin_a + dy * cos_a) / half_h;

                if rx.abs() > 1.0 || ry.abs() > 1.0 {
                    continue;
                }

                let stamp =
                    if let Some(pat) = &custom_pattern { pat.sample_gray((rx + 1.0) * 0.5, (ry + 1.0) * 0.5, false) } else { eval_shape(shape, rx, ry) };
                let lum = 1.0 - lum_rnd * cell_rand(cx_id, cy_id, 5, seed) * 0.8;
                best = best.max(stamp * lum);
            }
        }
        best.clamp(0.0, 1.0)
    })])
}
