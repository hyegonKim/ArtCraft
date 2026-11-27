//! Clean-room Substance 3D PBR Material, SDF Clay Modeling, and 3D Stager algorithms:
//! - **Suite A (`sampler`)**: `Photo → PBR Material` (`BaseColor` Retinex delighting, `Height`,
//!   `Normal`, `Roughness`, `Metallic`, `AmbientOcclusion`), `Make It Tile` (`make_seamless_rgba`),
//!   and `Photo → IBL` (`extract_ibl_environment_rgba`).
//! - **Suite B (`stage3d`)**: 3D Stager + SDF Clay Modeler (`Sphere`, `RoundedBox`, `Bottle`,
//!   `Can`, `Cylinder`, `Torus`, `DisplayCard`, bounded `CustomObj` Wavefront `.obj` parser),
//!   smooth SDF boolean operators, 64-step sphere-tracing raymarcher with 2×2 silhouette SSAA,
//!   Cook-Torrance GGX BRDF, backdrop-matched Equirectangular IBL, Active Layer Decal UV wrapping,
//!   curvature edge wear (`weathering`), and a soft ground-plane shadow catcher.
//! - **Suite C (`designer`)**: Procedural PBR Material Generator (`BrushedSteel`, `CarbonHexWeave`,
//!   `TerrazzoMarble`, `CeramicTiles`, `WeatheredCopper`, `WovenFabric`) shaded with Cook-Torrance GGX.

pub mod brdf;
pub mod designer;
pub mod sampler;
pub mod stage3d;

pub use brdf::{d_ggx, f_schlick, g_smith};
pub use designer::{ProceduralPbrParams, ProceduralPbrPreset, render_procedural_pbr_rgba, render_procedural_pbr_surface};
pub use sampler::{
    MaterialSamplerParams, PbrChannel, PbrMaterialMaps, extract_ibl_environment_rgba, make_seamless_rgba, make_seamless_surface, sample_pbr_channel_rgba,
    sample_pbr_channel_surface, sample_pbr_maps_rgba,
};
pub use stage3d::{
    MAX_OBJ_ELEMENTS, ObjMesh, SdfBooleanOp, Stage3dInputs, Stage3dParams, StageMaterialPreset, StageShape, parse_obj_mesh, render_stage_3d_rgba,
    render_stage_3d_surface,
};

#[cfg(test)]
mod tests {
    use photocraft_color::{ColorMode, PixelFormat, SampleType};
    use photocraft_geom::Rect;
    use photocraft_raster::Surface;

    use super::{
        MaterialSamplerParams, ProceduralPbrParams, ProceduralPbrPreset, SdfBooleanOp, Stage3dInputs, Stage3dParams, StageMaterialPreset, StageShape,
        extract_ibl_environment_rgba, make_seamless_rgba, parse_obj_mesh, render_procedural_pbr_surface, render_stage_3d_rgba, sample_pbr_maps_rgba,
    };

    #[test]
    fn sampler_normals_are_unit_length_and_delighting_flattens_illumination_ramp() {
        let (w, h) = (64usize, 64usize);
        let mut src = vec![[0.0f32; 4]; w * h];
        for y in 0..h {
            for x in 0..w {
                // Strong left-to-right lighting gradient + local stone grain
                let ramp = 0.22 + 0.68 * (x as f32 / (w - 1) as f32);
                let tex = 0.85 + 0.15 * ((x as f32 * 0.9).sin() * (y as f32 * 0.8).cos());
                let v = (ramp * tex).clamp(0.0, 1.0);
                src[y * w + x] = [v, v * 0.95, v * 0.88, 1.0];
            }
        }

        let in_left: f32 = (0..h).map(|y| src[y * w + 6][0]).sum::<f32>() / h as f32;
        let in_right: f32 = (0..h).map(|y| src[y * w + (w - 7)][0]).sum::<f32>() / h as f32;
        let in_grad = (in_right - in_left).abs();

        let maps = sample_pbr_maps_rgba(
            &src,
            w,
            h,
            &MaterialSamplerParams { delight_strength: 1.0, normal_strength: 3.0, metallic_level: 0.4, ..MaterialSamplerParams::default() },
        );

        let out_left: f32 = (0..h).map(|y| maps.base_color[y * w + 6][0]).sum::<f32>() / h as f32;
        let out_right: f32 = (0..h).map(|y| maps.base_color[y * w + (w - 7)][0]).sum::<f32>() / h as f32;
        let out_grad = (out_right - out_left).abs();
        assert!(out_grad < in_grad * 0.45, "delighting should reduce global illumination ramp ({out_grad} vs {in_grad})");

        // Check every tangent-space normal decodes to unit length and +Z hemisphere
        for n_px in &maps.normal {
            let nx = n_px[0] * 2.0 - 1.0;
            let ny = n_px[1] * 2.0 - 1.0;
            let nz = n_px[2] * 2.0 - 1.0;
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            assert!((len - 1.0).abs() < 1e-3, "normal length must be ~1.0, got {len}");
            assert!(nz > 0.0, "tangent normal Z must remain positive, got {nz}");
        }

        // Check Roughness, Height, AO, and Metallic stay strictly in valid physical bounds
        for i in 0..w * h {
            assert!((0.04..=1.0).contains(&maps.roughness[i][0]));
            assert!((0.0..=1.0).contains(&maps.height_map[i][0]));
            assert!((0.0..=1.0).contains(&maps.ambient_occlusion[i][0]));
            assert!((0.0..=1.0).contains(&maps.metallic[i][0]));
        }
    }

    #[test]
    fn make_seamless_rgba_eliminates_wraparound_seam_jump() {
        let (w, h) = (64usize, 64usize);
        let mut src = vec![[0.0f32; 4]; w * h];
        for y in 0..h {
            for x in 0..w {
                // Strong wraparound jump between x=0 (0.15) and x=w-1 (0.85), y=0 and y=h-1
                let rx = 0.15 + 0.70 * (x as f32 / (w - 1) as f32);
                let ry = 0.15 + 0.70 * (y as f32 / (h - 1) as f32);
                let grain = 0.03 * ((x as f32 * 0.5).sin() + (y as f32 * 0.5).cos());
                let v = (0.5 * (rx + ry) + grain).clamp(0.0, 1.0);
                src[y * w + x] = [v, v, v, 1.0];
            }
        }

        let seam_jump_x = |img: &[[f32; 4]]| -> f32 { (0..h).map(|y| (img[y * w][0] - img[y * w + (w - 1)][0]).abs()).sum::<f32>() / h as f32 };
        let seam_jump_y = |img: &[[f32; 4]]| -> f32 { (0..w).map(|x| (img[x][0] - img[(h - 1) * w + x][0]).abs()).sum::<f32>() / w as f32 };

        let before_x = seam_jump_x(&src);
        let before_y = seam_jump_y(&src);
        let tiled = make_seamless_rgba(&src, w, h, 0.25);
        let after_x = seam_jump_x(&tiled);
        let after_y = seam_jump_y(&tiled);

        assert!(after_x < before_x * 0.15, "horizontal wraparound jump {after_x} not reduced from {before_x}");
        assert!(after_y < before_y * 0.15, "vertical wraparound jump {after_y} not reduced from {before_y}");

        // Also verify equirectangular IBL extraction produces smooth 2:1 map with periodic azimuth
        let env = extract_ibl_environment_rgba(&src, w, h, 64, 32);
        assert_eq!(env.len(), 64 * 32);
        let seam_env: f32 = (0..32).map(|y| (env[y * 64][0] - env[y * 64 + 63][0]).abs()).sum::<f32>() / 32.0;
        assert!(seam_env < 0.05, "IBL azimuth seam jump too large: {seam_env}");
    }

    #[test]
    fn stage_3d_renders_shaded_object_shadow_catcher_boolean_ops_and_obj_mesh() {
        let (w, h) = (64usize, 64usize);
        let mut backdrop = vec![[0.85f32, 0.75, 0.62, 1.0]; w * h];
        for (i, px) in backdrop.iter_mut().enumerate() {
            if i / w > h / 2 {
                *px = [0.32, 0.36, 0.44, 1.0];
            }
        }
        let decal = vec![[0.90f32, 0.15, 0.15, 1.0]; 32 * 32];

        let base_params = Stage3dParams {
            shape: StageShape::Sphere,
            sdf_op: SdfBooleanOp::None,
            material: StageMaterialPreset::ActiveLayerDecal,
            shadow_catcher: 0.85,
            ground_y: -0.58,
            weathering: 0.35,
            ..Stage3dParams::default()
        };
        let inputs = Stage3dInputs { backdrop_rgba: Some((&backdrop, w, h)), decal_rgba: Some((&decal, 32, 32)), custom_mesh: None };
        let single = render_stage_3d_rgba(w, h, &base_params, &inputs);
        let center_px = single[(h / 2) * w + (w / 2)];
        assert!(center_px[3] > 0.95, "center of sphere must be opaque: {center_px:?}");
        assert!(center_px[0] > center_px[1] + 0.15, "red decal should wrap onto sphere front: {center_px:?}");

        // Verify ground-plane shadow catcher produces semi-transparent black shadow pixels below object
        let has_shadow_catcher = single.iter().any(|q| q[0] == 0.0 && q[1] == 0.0 && q[2] == 0.0 && (0.05..0.90).contains(&q[3]));
        assert!(has_shadow_catcher, "expected semi-transparent ground-plane shadow catcher alpha");

        // Verify SDF SmoothUnion expands silhouette while SmoothSubtract carves into primary shape!
        let union_img = render_stage_3d_rgba(
            w,
            h,
            &Stage3dParams {
                sdf_op: SdfBooleanOp::SmoothUnion,
                secondary_shape: StageShape::Sphere,
                secondary_offset: [0.42, 0.0, 0.0],
                smooth_k: 0.20,
                shadow_catcher: 0.0,
                ..base_params
            },
            &inputs,
        );
        let subtract_img = render_stage_3d_rgba(
            w,
            h,
            &Stage3dParams {
                sdf_op: SdfBooleanOp::SmoothSubtract,
                secondary_shape: StageShape::Sphere,
                secondary_offset: [0.22, 0.0, 0.35],
                smooth_k: 0.12,
                shadow_catcher: 0.0,
                ..base_params
            },
            &inputs,
        );
        let single_no_shadow = render_stage_3d_rgba(w, h, &Stage3dParams { shadow_catcher: 0.0, ..base_params }, &inputs);
        let alpha_sum = |img: &[[f32; 4]]| img.iter().map(|q| q[3]).sum::<f32>();
        assert!(alpha_sum(&union_img) > alpha_sum(&single_no_shadow) + 25.0, "SmoothUnion should expand coverage");
        assert_ne!(subtract_img, single_no_shadow, "SmoothSubtract should alter geometry/normals");

        // Test Wavefront .obj parser on valid tetrahedron + hostile inputs
        let valid_obj = "\
            # Tetrahedron
            v  0.0  1.0  0.0
            v -1.0 -1.0  1.0
            v  1.0 -1.0  1.0
            v  0.0 -1.0 -1.0
            f 1/1/1 2/2/1 3/3/1
            f 1 3 4
            f 1 4 2
            f -3 -1 -2
        ";
        let mesh = parse_obj_mesh(valid_obj).unwrap();
        assert_eq!(mesh.vertices.len(), 4);
        assert_eq!(mesh.faces.len(), 4);

        let obj_img = render_stage_3d_rgba(
            32,
            32,
            &Stage3dParams { shape: StageShape::CustomObj, material: StageMaterialPreset::PolishedGold, ..Stage3dParams::default() },
            &Stage3dInputs { backdrop_rgba: None, decal_rgba: None, custom_mesh: Some(&mesh) },
        );
        assert!(obj_img.iter().any(|q| q[3] > 0.8), "custom OBJ mesh should render visible pixels");

        for hostile in ["", "v NaN Inf -Inf\nf 1 2 3", "v 0 0 0\nv 1 0 0\nf 1 999999 -999999 0", "garbage /// \\\0\n f / / /"] {
            assert!(parse_obj_mesh(hostile).is_err());
        }
    }

    #[test]
    fn procedural_pbr_renders_cleanly_across_u8_u16_and_f32_surfaces() {
        let rect = Rect::from_xywh(0, 0, 32, 32);
        for depth in [SampleType::U8, SampleType::U16, SampleType::F32] {
            let fmt = PixelFormat::new(ColorMode::Rgb, depth, true);
            let mut src = Surface::new(fmt);
            src.fill_rect(rect, &[0.5, 0.5, 0.5, 1.0]);

            for preset in ProceduralPbrPreset::ALL {
                let out = render_procedural_pbr_surface(&src, rect, &ProceduralPbrParams { preset, scale: 3.0, ..ProceduralPbrParams::default() });
                assert_eq!(out.format(), fmt);
                let raw = out.read_region(rect);
                assert_eq!(raw.len(), 32 * 32 * 4);
                assert!(raw.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
                // Verify procedural pattern has non-trivial contrast
                let min_r = raw.as_chunks::<4>().0.iter().map(|p| p[0]).fold(1.0f32, f32::min);
                let max_r = raw.as_chunks::<4>().0.iter().map(|p| p[0]).fold(0.0f32, f32::max);
                assert!(max_r > min_r + 0.02, "{preset:?} at {depth:?} should have shaded surface relief");
            }
        }
    }
}
