use super::perlin_noise::sample_perlin_fbm;
use crate::{CookContext, CookResult, NodeDefinition, node};

const NOISES: &str = "Texture generators/Noises";

pub(super) fn definition() -> NodeDefinition {
    node("grunge", "Grunge", NOISES)
        .desc("Multi-scale domain-warped grunge map with directional streaks and spots.")
        .generator()
        .int("scale", "Scale", 4, 1, 32)
        .float("balance", "Balance", 0.5, 0.0, 1.0)
        .float("contrast", "Contrast", 1.2, 0.0, 4.0)
        .float("streaks", "Streaks", 0.25, 0.0, 1.0)
        .float("spots", "Spots", 0.3, 0.0, 1.0)
        .float("warp_intensity", "Warp intensity", 0.35, 0.0, 2.0)
        .gray_out()
        .cook(grunge)
}

fn grunge(context: &CookContext) -> CookResult {
    let scale = context.i("scale").clamp(1, 32);
    let balance = context.f("balance").clamp(0.0, 1.0);
    let contrast = context.f("contrast").clamp(0.0, 4.0);
    let streaks = context.f("streaks").clamp(0.0, 1.0);
    let spots = context.f("spots").clamp(0.0, 1.0);
    let warp_k = context.f("warp_intensity").clamp(0.0, 2.0) * 0.15;
    let seed = context.seed;
    let tile = context.tile;

    Ok(vec![context.gen_gray(|u, v| {
        let wx = sample_perlin_fbm(u, v, scale, 3, 0.55, 0.2, tile, seed.wrapping_add(11)) - 0.5;
        let wy = sample_perlin_fbm(u, v, scale, 3, 0.55, 0.2, tile, seed.wrapping_add(29)) - 0.5;
        let wu = if tile { (u + wx * warp_k).rem_euclid(1.0) } else { (u + wx * warp_k).clamp(0.0, 1.0) };
        let wv = if tile { (v + wy * warp_k).rem_euclid(1.0) } else { (v + wy * warp_k).clamp(0.0, 1.0) };

        let base = sample_perlin_fbm(wu, wv, scale, 5, 0.55, 0.35, tile, seed);
        let fine = sample_perlin_fbm(wu, wv, (scale * 3).min(64), 4, 0.6, 0.5, tile, seed.wrapping_add(101));

        let streak_u = if tile { (u * 4.0).rem_euclid(1.0) } else { (u * 4.0).clamp(0.0, 1.0) };
        let streak_map = sample_perlin_fbm(streak_u, wv, (scale * 2).min(64), 3, 0.5, 0.1, tile, seed.wrapping_add(211));
        let spot_raw = sample_perlin_fbm(wu, wv, (scale * 4).min(64), 2, 0.5, 0.7, tile, seed.wrapping_add(307));
        let spot_mask = ((spot_raw - 0.62) * 3.0).clamp(0.0, 1.0);

        let mut val = base * 0.65 + fine * 0.35;
        val = val * (1.0 - streaks * 0.5) + streak_map * (streaks * 0.5);
        val = (val - spot_mask * spots * 0.55 + (balance - 0.5) * 0.9).clamp(0.0, 1.0);
        ((val - 0.5) * contrast + 0.5).clamp(0.0, 1.0)
    })])
}
