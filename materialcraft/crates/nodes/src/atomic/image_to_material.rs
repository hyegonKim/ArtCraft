use super::scan_delight::{SCAN_FILTERS, delight_color};
use crate::ops;
use crate::{CookContext, CookResult, NodeDefinition, PortKind, node};
use materialcraft_image::{Image, Kind};
use std::f32::consts::{FRAC_PI_2, TAU};

pub(super) fn definition() -> NodeDefinition {
    node("image_to_material", "Image to material", SCAN_FILTERS)
        .desc("Synthesizes Base Color, Normal, Height, Roughness, Ambient Occlusion, and Metallic PBR channels from a single surface photograph.")
        .atomic()
        .color_in("input", "Input")
        .float("delight", "Delight strength", 0.80, 0.0, 1.0)
        .float("height_detail", "Height detail", 1.0, 0.0, 2.0)
        .float("height_contrast", "Height contrast", 1.0, 0.0, 2.0)
        .float("normal_intensity", "Normal intensity", 2.0, 0.0, 10.0)
        .boolean("opengl", "OpenGL normals", true)
        .float("roughness_base", "Roughness base", 0.50, 0.0, 1.0)
        .float("roughness_variance", "Roughness contrast", 1.0, 0.0, 2.0)
        .float("ao_depth", "AO depth", 1.0, 0.0, 5.0)
        .float("ao_radius", "AO radius", 0.08, 0.01, 0.5)
        .float("metallic_sensitivity", "Metallic sensitivity", 1.0, 0.0, 2.0)
        .float("metallic_threshold", "Metallic threshold", 0.45, 0.0, 1.0)
        .output("basecolor", "Base color", PortKind::Color)
        .output("normal", "Normal", PortKind::Color)
        .output("height", "Height", PortKind::Gray)
        .output("roughness", "Roughness", PortKind::Gray)
        .output("ao", "Ambient occlusion", PortKind::Gray)
        .output("metallic", "Metallic", PortKind::Gray)
        .cook(image_to_material)
}

fn image_to_material(context: &CookContext) -> CookResult {
    let src = context.any_or(0, Kind::Color, [0.5, 0.5, 0.5, 1.0]).to_color();
    let tile = context.tile;
    let w = context.w.max(1);
    let h = context.h.max(1);

    let delight = context.f("delight").clamp(0.0, 1.0);
    let height_detail = context.f("height_detail").clamp(0.0, 2.0);
    let height_contrast = context.f("height_contrast").clamp(0.0, 2.0);
    let normal_intensity = context.f("normal_intensity").clamp(0.0, 10.0);
    let opengl = context.b("opengl");
    let roughness_base = context.f("roughness_base").clamp(0.0, 1.0);
    let roughness_variance = context.f("roughness_variance").clamp(0.0, 2.0);
    let ao_depth = context.f("ao_depth").clamp(0.0, 5.0);
    let ao_radius = context.f("ao_radius").clamp(0.01, 0.5);
    let metallic_sensitivity = context.f("metallic_sensitivity").clamp(0.0, 2.0);
    let metallic_threshold = context.f("metallic_threshold").clamp(0.0, 1.0);

    // 1. Multi-scale Gaussian delighting -> BaseColor
    let basecolor = delight_color(&src, delight, 0.35, 0.10, 0.35, 0.25, tile);

    // 2. Multi-octave Difference-of-Gaussians luminance synthesis -> Height
    let lum_raw = Image::gray(w, h, |x, y| {
        let p_src = src.get(x as i64, y as i64, tile);
        let p_del = basecolor.get(x as i64, y as i64, tile);
        let l_src = 0.2126 * p_src[0] + 0.7152 * p_src[1] + 0.0722 * p_src[2];
        let l_del = 0.2126 * p_del[0] + 0.7152 * p_del[1] + 0.0722 * p_del[2];
        (0.35 * l_src + 0.65 * l_del).clamp(0.0, 1.0)
    });

    let span = w.max(h) as f32;
    let g1 = ops::gaussian(&lum_raw, (span * 0.010).max(0.6), tile);
    let g2 = ops::gaussian(&lum_raw, (span * 0.030).max(1.4), tile);
    let g3 = ops::gaussian(&lum_raw, (span * 0.085).max(2.8), tile);
    let g4 = ops::gaussian(&lum_raw, (span * 0.220).max(5.5), tile);

    let height = Image::gray(w, h, |x, y| {
        let ix = x as i64;
        let iy = y as i64;
        let l0 = lum_raw.get_gray(ix, iy, tile);
        let l1 = g1.get_gray(ix, iy, tile);
        let l2 = g2.get_gray(ix, iy, tile);
        let l3 = g3.get_gray(ix, iy, tile);
        let l4 = g4.get_gray(ix, iy, tile);

        let b0 = l0 - l1;
        let b1 = l1 - l2;
        let b2 = l2 - l3;
        let b3 = l3 - l4;
        let octave_sum = 0.36 * height_detail * b0 + 0.30 * height_detail * b1 + 0.21 * b2 + 0.13 * b3;
        (0.5 + height_contrast * octave_sum * 2.4).clamp(0.0, 1.0)
    });

    // 3. Scharr tangent-space normal generation (OpenGL Y+ / DirectX Y-)
    let normal_scale = normal_intensity * (w as f32 / 32.0);
    let normal = scharr_height_to_normal(&height, normal_scale, !opengl, tile);

    // 4. Local micro-variance roughness estimation
    let lum_delit = Image::gray(w, h, |x, y| {
        let p = basecolor.get(x as i64, y as i64, tile);
        (0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2]).clamp(0.0, 1.0)
    });
    let lum_sq = Image::gray(w, h, |x, y| {
        let l = lum_delit.get_gray(x as i64, y as i64, tile);
        l * l
    });
    let sigma_var = (span * 0.018).max(1.0);
    let mean_l = ops::gaussian(&lum_delit, sigma_var, tile);
    let mean_l2 = ops::gaussian(&lum_sq, sigma_var, tile);

    let roughness = Image::gray(w, h, |x, y| {
        let ix = x as i64;
        let iy = y as i64;
        let l = lum_delit.get_gray(ix, iy, tile);
        let mu = mean_l.get_gray(ix, iy, tile);
        let mu2 = mean_l2.get_gray(ix, iy, tile);
        let micro_std = (mu2 - mu * mu).max(0.0).sqrt();
        let cavity_term = (0.5 - l) * 0.30;
        (roughness_base + roughness_variance * (2.4 * micro_std + cavity_term - 0.08)).clamp(0.0, 1.0)
    });

    // 5. Cavity + horizon ambient occlusion from synthesized height
    let smooth_height = ops::gaussian(&height, (span * ao_radius).max(1.2), tile);
    let ao = synthesize_cavity_ao(&height, &smooth_height, ao_depth, ao_radius, tile);

    // 6. Color-saturation & reflectance conductor metallic estimation
    let metallic = Image::gray(w, h, |x, y| {
        if metallic_sensitivity <= 1e-5 {
            return 0.0;
        }
        let ix = x as i64;
        let iy = y as i64;
        let px = basecolor.get(ix, iy, tile);
        let px_src = src.get(ix, iy, tile);
        let [_hue, sat, light_delit] = ops::rgb_to_hsl(px);
        let [_hue_s, sat_src, light_src] = ops::rgb_to_hsl(px_src);
        let sat = 0.5 * (sat + sat_src);
        let light = 0.55 * light_src + 0.45 * light_delit;
        let mu = mean_l.get_gray(ix, iy, tile);
        let mu2 = mean_l2.get_gray(ix, iy, tile);
        let micro_std = (mu2 - mu * mu).max(0.0).sqrt();

        // Neutral specular conductors (steel, chrome, aluminum) have low saturation & high reflectance;
        // colored conductors (gold, copper) combine high luminance with warm R >= G > B reflectance.
        let neutral_metal = (1.0 - sat).powi(2) * light.powf(1.1);
        let warm_conductor = if px[0] >= px[1] && px[1] > px[2] {
            let warmth = (px[0] - px[2]).clamp(0.0, 1.0);
            warmth * (1.0 - (sat - 0.55).abs()).clamp(0.0, 1.0) * light.powf(1.3) * 0.85
        } else {
            0.0
        };
        let raw_score = neutral_metal.max(warm_conductor) * (1.0 - 0.6 * micro_std).clamp(0.2, 1.0) * metallic_sensitivity;
        let span_t = (1.0 - metallic_threshold).max(0.12);
        let t = ((raw_score - metallic_threshold) / span_t).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    });

    Ok(vec![basecolor, normal, height, roughness, ao, metallic])
}

/// 3x3 rotationally-symmetric Scharr `[3, 10, 3]` gradient normal synthesis.
fn scharr_height_to_normal(height: &Image, k: f32, directx: bool, tile: bool) -> Image {
    Image::generate(height.w, height.h, Kind::Color, |x, y| {
        let ix = x as i64;
        let iy = y as i64;
        let h00 = height.get_gray(ix - 1, iy - 1, tile);
        let h10 = height.get_gray(ix, iy - 1, tile);
        let h20 = height.get_gray(ix + 1, iy - 1, tile);
        let h01 = height.get_gray(ix - 1, iy, tile);
        let h21 = height.get_gray(ix + 1, iy, tile);
        let h02 = height.get_gray(ix - 1, iy + 1, tile);
        let h12 = height.get_gray(ix, iy + 1, tile);
        let h22 = height.get_gray(ix + 1, iy + 1, tile);

        let gx = ((3.0 * h20 + 10.0 * h21 + 3.0 * h22) - (3.0 * h00 + 10.0 * h01 + 3.0 * h02)) / 32.0;
        let gy = ((3.0 * h02 + 10.0 * h12 + 3.0 * h22) - (3.0 * h00 + 10.0 * h10 + 3.0 * h20)) / 32.0;

        let nx = -gx * k;
        let ny_raw = gy * k;
        let ny = if directx { -ny_raw } else { ny_raw };
        let nz = 1.0_f32;
        let len = (nx * nx + ny * ny + nz * nz).sqrt().max(1e-6);
        [nx / len * 0.5 + 0.5, ny / len * 0.5 + 0.5, nz / len * 0.5 + 0.5, 1.0]
    })
}

fn synthesize_cavity_ao(height: &Image, smooth_height: &Image, depth: f32, radius: f32, tile: bool) -> Image {
    let w = height.w.max(1);
    let h = height.h.max(1);
    if depth <= 1e-5 {
        return Image::filled(w, h, Kind::Gray, [1.0; 4]);
    }
    let directions: [(f32, f32); 8] = std::array::from_fn(|d| {
        let a = (d as f32 / 8.0) * TAU;
        (a.cos(), a.sin())
    });
    let samples = 6_i32;
    let iw = 1.0 / w as f32;
    let ih = 1.0 / h as f32;

    Image::gray(w, h, |x, y| {
        let ix = x as i64;
        let iy = y as i64;
        let u0 = (x as f32 + 0.5) * iw;
        let v0 = (y as f32 + 0.5) * ih;
        let h0 = height.get_gray(ix, iy, tile);

        let mut occ_sum = 0.0_f32;
        for &(dx, dy) in &directions {
            let mut max_angle = 0.0_f32;
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

        let horizon_ao = (1.0 - occ_sum / 8.0).clamp(0.0, 1.0);
        let local_cavity = (smooth_height.get_gray(ix, iy, tile) - h0).max(0.0) * depth * 0.45;
        (horizon_ao * (1.0 - local_cavity).clamp(0.0, 1.0)).clamp(0.0, 1.0)
    })
}
