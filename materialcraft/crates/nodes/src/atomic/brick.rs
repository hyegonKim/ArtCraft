use crate::random::hash;
use crate::{CookContext, CookResult, NodeDefinition, node};

const PATTERNS: &str = "Texture generators/Patterns";

pub(super) fn definition() -> NodeDefinition {
    node("brick", "Brick", PATTERNS)
        .desc("Running-bond brick height pattern with mortar, bevel, luminance and slope variation.")
        .generator()
        .int("bricks_x", "Bricks X", 4, 1, 32)
        .int("bricks_y", "Bricks Y", 8, 1, 64)
        .float("offset", "Row offset", 0.5, 0.0, 1.0)
        .float("mortar_width", "Mortar width", 0.08, 0.0, 0.49)
        .float("mortar_height", "Mortar height", 0.08, 0.0, 0.49)
        .float("bevel", "Bevel", 0.15, 0.0, 1.0)
        .float("luminance_random", "Luminance random", 0.15, 0.0, 1.0)
        .float("slope_random", "Slope random", 0.1, 0.0, 1.0)
        .gray_out()
        .cook(brick)
}

#[inline]
fn rand01(col: i32, row: i32, channel: u32, seed: u32) -> f32 {
    let h = hash((col as u32).wrapping_mul(0x9e37_79b9) ^ (row as u32).wrapping_mul(0x85eb_ca6b) ^ channel.wrapping_mul(0xc2b2_ae35) ^ seed);
    (h as f32) * (1.0 / u32::MAX as f32)
}

fn brick(context: &CookContext) -> CookResult {
    let bx = context.i("bricks_x").clamp(1, 32);
    let by = context.i("bricks_y").clamp(1, 64);
    let offset = context.f("offset").clamp(0.0, 1.0);
    let mw = context.f("mortar_width").clamp(0.0, 0.49) * 0.5;
    let mh = context.f("mortar_height").clamp(0.0, 0.49) * 0.5;
    let bevel = context.f("bevel").clamp(0.0, 1.0) * 0.45;
    let lum_rnd = context.f("luminance_random").clamp(0.0, 1.0);
    let slope_rnd = context.f("slope_random").clamp(0.0, 1.0);
    let seed = context.seed;
    let tile = context.tile;

    Ok(vec![context.gen_gray(|u, v| {
        let gy = v * by as f32;
        let row_raw = gy.floor() as i32;
        let fy = gy - gy.floor();
        let row = if tile { row_raw.rem_euclid(by) } else { row_raw };

        let row_shift = if row_raw.rem_euclid(2) != 0 { offset } else { 0.0 };
        let gx = u * bx as f32 + row_shift;
        let col_raw = gx.floor() as i32;
        let fx = gx - gx.floor();
        let col = if tile { col_raw.rem_euclid(bx) } else { col_raw };

        let dx_edge = fx.min(1.0 - fx);
        let dy_edge = fy.min(1.0 - fy);
        if dx_edge <= mw || dy_edge <= mh {
            return 0.0;
        }

        let rx = if bevel > 1e-5 { ((dx_edge - mw) / bevel).clamp(0.0, 1.0) } else { 1.0 };
        let ry = if bevel > 1e-5 { ((dy_edge - mh) / bevel).clamp(0.0, 1.0) } else { 1.0 };
        let edge_profile = rx.min(ry);
        let smooth = edge_profile * edge_profile * (3.0 - 2.0 * edge_profile);

        let lum = 1.0 - lum_rnd * rand01(col, row, 1, seed) * 0.6;
        let sx = (rand01(col, row, 2, seed) - 0.5) * 2.0 * slope_rnd * 0.35;
        let sy = (rand01(col, row, 3, seed) - 0.5) * 2.0 * slope_rnd * 0.35;
        let tilt = 1.0 + sx * (fx - 0.5) + sy * (fy - 0.5);

        (smooth * lum * tilt).clamp(0.0, 1.0)
    })])
}
