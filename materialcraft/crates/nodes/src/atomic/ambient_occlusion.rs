use crate::{CookContext, CookResult, NodeDefinition, node};
use materialcraft_image::Image;
use std::f32::consts::{FRAC_PI_2, TAU};

const NORMAL_FILTERS: &str = "Filters/Normal map";

pub(super) fn definition() -> NodeDefinition {
    node("ambient_occlusion", "Ambient occlusion", NORMAL_FILTERS)
        .desc("Horizon-based ambient occlusion computed from a grayscale height map.")
        .atomic()
        .gray_in("input", "Height")
        .float("depth", "Depth", 1.0, 0.0, 5.0)
        .float("radius", "Radius", 0.1, 0.01, 0.5)
        .int("directions", "Directions", 8, 4, 16)
        .int("samples", "Samples", 8, 2, 24)
        .float("spread", "Spread", 1.0, 0.1, 4.0)
        .gray_out()
        .cook(ambient_occlusion)
}

fn ambient_occlusion(context: &CookContext) -> CookResult {
    let height = context.gray_or(0, 0.5);
    let depth = context.f("depth").clamp(0.0, 5.0);
    let radius = context.f("radius").clamp(0.01, 0.5);
    let directions = context.i("directions").clamp(4, 16);
    let samples = context.i("samples").clamp(2, 24);
    let spread = context.f("spread").clamp(0.1, 4.0);
    let tile = context.tile;

    let dirs: Vec<(f32, f32)> = (0..directions)
        .map(|d| {
            let a = (d as f32 / directions as f32) * TAU;
            (a.cos(), a.sin())
        })
        .collect();

    let iw = 1.0 / context.w.max(1) as f32;
    let ih = 1.0 / context.h.max(1) as f32;

    Ok(vec![Image::gray(context.w, context.h, |x, y| {
        if depth <= 1e-5 {
            return 1.0;
        }
        let u0 = (x as f32 + 0.5) * iw;
        let v0 = (y as f32 + 0.5) * ih;
        let h0 = height.get_gray(x as i64, y as i64, tile);

        let mut occ_sum = 0.0f32;
        for &(dx, dy) in &dirs {
            let mut max_angle = 0.0f32;
            for step in 1..=samples {
                let t = (step as f32 / samples as f32) * radius;
                let hs = height.sample_gray(u0 + dx * t, v0 + dy * t, tile);
                let dh = (hs - h0) * depth * 0.25;
                if dh > 0.0 {
                    let angle = dh.atan2(t.max(1e-5));
                    let atten = 1.0 - (step as f32 - 1.0) / (samples as f32 * 1.25);
                    max_angle = max_angle.max(angle * atten.clamp(0.1, 1.0));
                }
            }
            occ_sum += (max_angle / FRAC_PI_2).clamp(0.0, 1.0);
        }

        let ao = (1.0 - occ_sum / directions as f32).clamp(0.0, 1.0);
        ao.powf(spread).clamp(0.0, 1.0)
    })])
}
