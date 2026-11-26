use crate::{CookContext, CookResult, NodeDefinition, node};
use materialcraft_image::{Image, Kind};

const NORMAL_FILTERS: &str = "Filters/Normal map";

pub(super) fn definition() -> NodeDefinition {
    node("curvature", "Curvature", NORMAL_FILTERS)
        .desc("Computes convex and concave surface curvature from a tangent-space normal or height map.")
        .atomic()
        .color_in("input", "Normal")
        .float("intensity", "Intensity", 1.0, 0.0, 10.0)
        .choice("mode", "Mode", &["Both", "Convex", "Concave"], 0)
        .boolean("opengl", "OpenGL (+Y)", true)
        .gray_out()
        .cook(curvature)
}

fn curvature(context: &CookContext) -> CookResult {
    let intensity = context.f("intensity").clamp(0.0, 10.0);
    let mode = context.i("mode").clamp(0, 2);
    let opengl = context.b("opengl");
    let tile = context.tile;

    let normal = match context.input(0) {
        Some(src) if src.kind == Kind::Gray => {
            let resized = src.resize(context.w, context.h, tile);
            super::height_to_normal(&resized, 2.0, !opengl, tile, false)
        }
        Some(src) => src.resize(context.w, context.h, tile),
        None => Image::filled(context.w, context.h, Kind::Color, [0.5, 0.5, 1.0, 1.0]),
    };

    let y_sign = if opengl { -1.0f32 } else { 1.0f32 };

    Ok(vec![Image::gray(context.w, context.h, |x, y| {
        let xi = x as i64;
        let yi = y as i64;
        let left = normal.get(xi - 1, yi, tile)[0] * 2.0 - 1.0;
        let right = normal.get(xi + 1, yi, tile)[0] * 2.0 - 1.0;
        let up = (normal.get(xi, yi - 1, tile)[1] * 2.0 - 1.0) * y_sign;
        let down = (normal.get(xi, yi + 1, tile)[1] * 2.0 - 1.0) * y_sign;

        // Convex ridge has outward-diverging normals (right > left, down > up).
        let div: f32 = ((right - left) + (down - up)) * 0.5 * intensity;
        match mode {
            1 => div.clamp(0.0, 1.0),
            2 => (-div).clamp(0.0, 1.0),
            _ => (0.5_f32 + div * 0.5).clamp(0.0, 1.0),
        }
    })])
}
