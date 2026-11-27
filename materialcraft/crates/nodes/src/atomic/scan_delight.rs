use crate::ops;
use crate::{CookContext, CookResult, NodeDefinition, node};
use materialcraft_image::{Image, Kind};

pub(super) const SCAN_FILTERS: &str = "Material filters/Scan processing";

pub(super) fn definition() -> NodeDefinition {
    node("scan_delight", "Scan delight", SCAN_FILTERS)
        .desc("Removes uneven low-frequency lighting and cast shading from a captured surface bitmap.")
        .atomic()
        .color_in("input", "Input")
        .float("strength", "Strength", 0.85, 0.0, 1.0)
        .float("large_scale", "Large light radius", 0.35, 0.02, 1.0)
        .float("medium_scale", "Medium shadow radius", 0.10, 0.01, 0.5)
        .float("highlight_recovery", "Highlight recovery", 0.35, 0.0, 1.0)
        .float("shadow_lift", "Shadow lift", 0.25, 0.0, 1.0)
        .color_out()
        .cook(scan_delight)
}

fn scan_delight(context: &CookContext) -> CookResult {
    let src = context.any_or(0, Kind::Color, [0.5, 0.5, 0.5, 1.0]).to_color();
    let strength = context.f("strength").clamp(0.0, 1.0);
    let large_scale = context.f("large_scale").clamp(0.02, 1.0);
    let medium_scale = context.f("medium_scale").clamp(0.01, 0.5);
    let highlight_recovery = context.f("highlight_recovery").clamp(0.0, 1.0);
    let shadow_lift = context.f("shadow_lift").clamp(0.0, 1.0);
    Ok(vec![delight_color(&src, strength, large_scale, medium_scale, highlight_recovery, shadow_lift, context.tile)])
}

/// Multi-scale Gaussian low-frequency illumination removal in log-luminance space while preserving
/// intrinsic surface chromaticity, micro-contrast, and alpha.
pub(super) fn delight_color(src: &Image, strength: f32, large_scale: f32, medium_scale: f32, highlight_recovery: f32, shadow_lift: f32, tile: bool) -> Image {
    let w = src.w.max(1);
    let h = src.h.max(1);
    if strength <= 1e-5 {
        return src.clone();
    }

    let eps = 1e-3_f32;
    let log_lum = Image::gray(w, h, |x, y| {
        let p = src.get(x as i64, y as i64, tile);
        let lum = (0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2]).clamp(0.0, 4.0);
        (lum + eps).ln()
    });

    let span = w.max(h) as f32;
    let sigma_large = (large_scale * span * 0.25).clamp(0.5, span);
    let sigma_med = (medium_scale * span * 0.25).clamp(0.25, span);

    let low_large = ops::gaussian(&log_lum, sigma_large, tile);
    let low_med = ops::gaussian(&log_lum, sigma_med, tile);
    let mean_log = log_lum.mean()[0];
    let mean_lum = (mean_log.exp() - eps).clamp(0.02, 0.98);

    Image::generate(w, h, Kind::Color, |x, y| {
        let px = src.get(x as i64, y as i64, tile);
        let r = px[0].clamp(0.0, 4.0);
        let g = px[1].clamp(0.0, 4.0);
        let b = px[2].clamp(0.0, 4.0);
        let lum = (0.2126 * r + 0.7152 * g + 0.0722 * b).max(0.0);

        let ll = log_lum.get_gray(x as i64, y as i64, tile);
        let illum = 0.65 * low_large.get_gray(x as i64, y as i64, tile) + 0.35 * low_med.get_gray(x as i64, y as i64, tile);
        let intrinsic_log = mean_log + (ll - illum);
        let mut target_lum = (intrinsic_log.exp() - eps).max(0.0);

        // Lift deep cast shadows without flattening local texture contrast
        if shadow_lift > 0.0 && lum < mean_lum {
            let shadow_t = ((mean_lum - lum) / mean_lum.max(1e-3)).clamp(0.0, 1.0);
            target_lum += shadow_lift * 0.18 * shadow_t * shadow_t * (mean_lum - target_lum).max(0.0);
        }

        // Compress specular hot-spots toward intrinsic diffuse luminance
        let mut chroma_keep = 1.0_f32;
        if highlight_recovery > 0.0 && target_lum > mean_lum {
            let excess = target_lum - mean_lum;
            let compressed = excess / (1.0 + highlight_recovery * 2.5 * excess);
            target_lum = mean_lum + compressed;
            chroma_keep = 1.0 - 0.15 * highlight_recovery * (excess / (1.0 + excess));
        }

        let ratio = if lum > 1e-4 { (target_lum / lum).clamp(0.1, 6.0) } else { 1.0 };
        let dr = (r * ratio).clamp(0.0, 1.0);
        let dg = (g * ratio).clamp(0.0, 1.0);
        let db = (b * ratio).clamp(0.0, 1.0);

        let dlum = 0.2126 * dr + 0.7152 * dg + 0.0722 * db;
        let fr = dlum + (dr - dlum) * chroma_keep;
        let fg = dlum + (dg - dlum) * chroma_keep;
        let fb = dlum + (db - dlum) * chroma_keep;

        [
            (px[0] + (fr - px[0]) * strength).clamp(0.0, 1.0),
            (px[1] + (fg - px[1]) * strength).clamp(0.0, 1.0),
            (px[2] + (fb - px[2]) * strength).clamp(0.0, 1.0),
            px[3].clamp(0.0, 1.0),
        ]
    })
}
