use crate::random::hash;
use crate::{CookContext, CookResult, NodeDefinition, node};

const NOISES: &str = "Texture generators/Noises";

pub(super) fn definition() -> NodeDefinition {
    node("voronoi", "Voronoi", NOISES)
        .desc("Cellular Worley/Voronoi distance and cell-ID generator.")
        .generator()
        .int("scale", "Scale", 4, 1, 64)
        .float("jitter", "Jitter", 1.0, 0.0, 1.0)
        .choice("metric", "Distance metric", &["Euclidean", "Manhattan", "Chebyshev"], 0)
        .choice("mode", "Output mode", &["F1", "F2", "F2 - F1", "Cell ID"], 0)
        .boolean("invert", "Invert", false)
        .gray_out()
        .cook(voronoi)
}

#[inline]
fn cell_point(cx: i32, cy: i32, scale: i32, tile: bool, jitter: f32, seed: u32) -> (f32, f32, f32) {
    let (wx, wy) = if tile {
        let s = scale.max(1);
        (cx.rem_euclid(s), cy.rem_euclid(s))
    } else {
        (cx, cy)
    };
    let h0 = hash((wx as u32).wrapping_mul(0x9e37_79b9) ^ (wy as u32).wrapping_mul(0x85eb_ca6b) ^ seed);
    let h1 = hash(h0 ^ 0xc2b2_ae35);
    let h2 = hash(h1 ^ 0x27d4_eb2f);
    let inv = 1.0 / u32::MAX as f32;
    let jx = 0.5 + ((h0 as f32) * inv - 0.5) * jitter;
    let jy = 0.5 + ((h1 as f32) * inv - 0.5) * jitter;
    let id = (h2 as f32) * inv;
    (cx as f32 + jx, cy as f32 + jy, id)
}

fn voronoi(context: &CookContext) -> CookResult {
    let scale = context.i("scale").clamp(1, 64);
    let jitter = context.f("jitter").clamp(0.0, 1.0);
    let metric = context.i("metric").clamp(0, 2);
    let mode = context.i("mode").clamp(0, 3);
    let invert = context.b("invert");
    let seed = context.seed;
    let tile = context.tile;

    Ok(vec![context.gen_gray(|u, v| {
        let px = u * scale as f32;
        let py = v * scale as f32;
        let bx = px.floor() as i32;
        let by = py.floor() as i32;

        let mut f1 = f32::MAX;
        let mut f2 = f32::MAX;
        let mut best_id = 0.0f32;

        for oy in -1..=1 {
            for ox in -1..=1 {
                let (sx, sy, id) = cell_point(bx + ox, by + oy, scale, tile, jitter, seed);
                let dx = (px - sx).abs();
                let dy = (py - sy).abs();
                let d = match metric {
                    1 => (dx + dy) * 0.7071_0677,
                    2 => dx.max(dy),
                    _ => (dx * dx + dy * dy).sqrt(),
                };
                if d < f1 {
                    f2 = f1;
                    f1 = d;
                    best_id = id;
                } else if d < f2 {
                    f2 = d;
                }
            }
        }

        let mut out = match mode {
            1 => (f2 * 0.75).clamp(0.0, 1.0),
            2 => (f2 - f1).clamp(0.0, 1.0),
            3 => best_id.clamp(0.0, 1.0),
            _ => f1.clamp(0.0, 1.0),
        };
        if invert {
            out = 1.0 - out;
        }
        out.clamp(0.0, 1.0)
    })])
}
