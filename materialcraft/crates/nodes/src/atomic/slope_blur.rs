use crate::ops;
use crate::{CookContext, CookResult, NodeDefinition, node};
use materialcraft_image::{Image, Kind};

const BLUR_FILTERS: &str = "Filters/Blurs";

pub(super) fn definition() -> NodeDefinition {
    node("slope_blur", "Slope blur", BLUR_FILTERS)
        .desc("Advects and blurs an image iteratively along the gradient of a grayscale slope map.")
        .atomic()
        .any_in("input", "Input")
        .primary_input("input")
        .gray_in("slope", "Slope")
        .float("intensity", "Intensity", 1.0, -4.0, 4.0)
        .int("samples", "Samples", 8, 1, 32)
        .choice("mode", "Mode", &["Blur", "Min", "Max", "Warp"], 0)
        .any_out()
        .cook(slope_blur)
}

fn slope_blur(context: &CookContext) -> CookResult {
    let fallback_kind = context.main_kind(Kind::Gray);
    let img = context.any_or(0, fallback_kind, [0.0, 0.0, 0.0, 1.0]);
    let slope = context.gray_or(1, 0.5);
    let intensity = context.f("intensity").clamp(-4.0, 4.0);
    let samples = context.i("samples").clamp(1, 32);
    let mode = context.i("mode").clamp(0, 3);
    let tile = context.tile;

    let step_scale = (intensity * 0.08) / samples as f32;

    Ok(vec![Image::generate_uv(context.w, context.h, img.kind, |u0, v0| {
        let mut u = u0;
        let mut v = v0;
        let base = img.sample(u, v, tile);
        let mut sum = base;
        let mut min_px = base;
        let mut max_px = base;

        for _ in 0..samples {
            let sx = ((u * slope.w as f32).floor().clamp(-1.0e6, 1.0e6)) as i64;
            let sy = ((v * slope.h as f32).floor().clamp(-1.0e6, 1.0e6)) as i64;
            let (dx, dy) = ops::slope(&slope, sx, sy, tile);
            u += dx * step_scale;
            v += dy * step_scale;
            let px = img.sample(u, v, tile);
            for ch in 0..4 {
                sum[ch] += px[ch];
                min_px[ch] = min_px[ch].min(px[ch]);
                max_px[ch] = max_px[ch].max(px[ch]);
            }
        }

        match mode {
            1 => min_px,
            2 => max_px,
            3 => img.sample(u, v, tile),
            _ => {
                let inv = 1.0 / (samples + 1) as f32;
                [sum[0] * inv, sum[1] * inv, sum[2] * inv, sum[3] * inv]
            }
        }
    })])
}
