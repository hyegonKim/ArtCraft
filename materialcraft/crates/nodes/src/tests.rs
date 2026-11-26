use std::collections::BTreeMap;

use materialcraft_core::Value;
use materialcraft_image::{Image, Kind};

use super::*;

fn cook(def: &NodeDefinition, inputs: &[Option<Image>], size: u32) -> CookResult {
    let params = resolve_params(def, &BTreeMap::new());
    let no_res = |_: &str| None;
    let vars = BTreeMap::new();
    let ctx = CookContext { w: size, h: size, seed: 7, tile: true, inputs, params: &params, resource: &no_res, vars: &vars };
    (def.cook)(&ctx)
}

/// Image nodes cook finite outputs at the requested size; an empty processor reports its missing result
#[test]
fn every_node_cooks() {
    let ids: Vec<&str> = catalog().iter().map(|definition| definition.id).collect();
    let mut seen = std::collections::BTreeSet::new();
    for id in &ids {
        assert!(seen.insert(*id), "duplicate node id {id}");
    }

    let odd_size_image = Image::generate(5, 3, Kind::Color, |x, y| [x as f32 / 4.0, y as f32 / 2.0, 0.5, 1.0]);
    for definition in catalog() {
        if matches!(definition.id, "graph_instance" | "value_processor" | "input_value") {
            continue;
        }

        for inputs in [vec![None; definition.inputs.len()], vec![Some(odd_size_image.clone()); definition.inputs.len()]] {
            let result = cook(definition, &inputs, 32);
            if definition.id == "pixel_processor" {
                assert_eq!(result.unwrap_err(), "function has no result node");
                continue;
            }

            let outputs = result.unwrap_or_else(|error| panic!("{}: {error}", definition.id));
            assert_eq!(outputs.len(), definition.outputs.len().max(1), "{}", definition.id);
            for image in &outputs {
                assert_eq!((image.w, image.h), (32, 32), "{}", definition.id);
                assert!(image.data.iter().all(|value| value.is_finite()), "{} produced a non-finite value", definition.id);
            }
        }
    }

    assert_eq!(catalog().len(), 37);
}

/// Every node cooks with each numeric parameter at a hostile value (huge, negative huge, NaN): the hard limits keep the cook bounded, finite and the right size
#[test]
fn every_node_survives_hostile_params() {
    use std::time::{Duration, Instant};

    let mut slow: Vec<(String, Duration)> = Vec::new();
    let total = Instant::now();
    for def in catalog() {
        if matches!(def.id, "graph_instance" | "value_processor" | "input_value") {
            continue;
        }
        for p in &def.params {
            for h in [1e30_f32, -1e30, f32::NAN] {
                let v = match p.default {
                    Value::Float(_) => Value::Float(h),
                    Value::Int(_) => Value::Int(h as i32),
                    Value::Float2(_) => Value::Float2([h, h]),
                    Value::Enum(_) => Value::Enum(h as u32),
                    _ => continue,
                };

                let mut params = resolve_params(def, &BTreeMap::from([(p.id.to_string(), v)]));
                clamp_params(def, &mut params);
                let inputs = vec![None; def.inputs.len()];
                let no_res = |_: &str| None;
                let vars = BTreeMap::new();
                let ctx = CookContext { w: 16, h: 16, seed: 7, tile: true, inputs: &inputs, params: &params, resource: &no_res, vars: &vars };
                let t = Instant::now();
                let out = (def.cook)(&ctx).unwrap_or_else(|e| panic!("{}.{}={h}: {e}", def.id, p.id));
                let dt = t.elapsed();
                assert!(dt < Duration::from_secs(2), "{}.{}={h} took {dt:?}", def.id, p.id);
                if dt > Duration::from_millis(250) {
                    slow.push((format!("{}.{}={h}", def.id, p.id), dt));
                }
                for img in &out {
                    assert_eq!((img.w, img.h), (16, 16), "{}.{}={h}", def.id, p.id);
                    assert!(img.data.iter().all(|x| x.is_finite()), "{}.{}={h} produced a non-finite value", def.id, p.id);
                }
            }
        }
    }
    eprintln!("hostile params: total {:?}; cooks over 250 ms: {slow:?}", total.elapsed());
}

#[test]
fn blend_and_levels_math() {
    let fg = Image::filled(4, 4, Kind::Gray, [0.25; 4]);
    let bg = Image::filled(4, 4, Kind::Gray, [0.5; 4]);
    let def = find("blend").unwrap();
    let mut params = resolve_params(def, &BTreeMap::new());
    params.insert("mode".into(), materialcraft_core::Value::Enum(3));
    let no_res = |_: &str| None;
    let vars = BTreeMap::new();
    let inputs = [Some(fg), Some(bg), None];
    let ctx = CookContext { w: 4, h: 4, seed: 0, tile: true, inputs: &inputs, params: &params, resource: &no_res, vars: &vars };
    let out = (def.cook)(&ctx).unwrap();
    assert!((out[0].data[0] - 0.125).abs() < 1e-6);
    assert!((atomic::levels_fn(0.5, 0.0, 0.5, 1.0, 0.0, 1.0) - 0.5).abs() < 1e-5);
    assert_eq!(atomic::levels_fn(0.2, 0.4, 0.5, 0.8, 0.0, 1.0), 0.0);
}

#[test]
fn dynamic_gradient_preserves_the_gradient_image_type() {
    let definition = find("gradient_dynamic").unwrap();
    for kind in [Kind::Gray, Kind::Color] {
        let source = Image::filled(4, 4, Kind::Gray, [0.5; 4]);
        let gradient = Image::filled(4, 4, kind, [0.25, 0.5, 0.75, 1.0]);
        let result = cook(definition, &[Some(source), Some(gradient.clone())], 16).unwrap();
        assert_eq!(result[0].kind, kind);
        assert_eq!(result[0].get(0, 0, false), gradient.get(0, 0, false));
    }
}

#[test]
fn procedural_generators_and_normal_filters_behave_as_expected() {
    for id in ["perlin_noise", "voronoi", "grunge", "brick", "tile_sampler"] {
        let def = find(id).unwrap();
        let out = cook(def, &vec![None; def.inputs.len()], 32).unwrap();
        let min = out[0].data.iter().copied().fold(f32::INFINITY, f32::min);
        let max = out[0].data.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(max - min > 0.1, "{id} should have non-flat contrast (min={min}, max={max})");
    }

    // Brick mortar along y=0 edge is 0.0
    let brick_def = find("brick").unwrap();
    let mut brick_params = resolve_params(brick_def, &BTreeMap::new());
    brick_params.insert("mortar_height".into(), Value::Float(0.25));
    let no_res = |_: &str| None;
    let vars = BTreeMap::new();
    let brick_ctx = CookContext { w: 64, h: 64, seed: 3, tile: true, inputs: &[], params: &brick_params, resource: &no_res, vars: &vars };
    let brick_img = (brick_def.cook)(&brick_ctx).unwrap();
    assert_eq!(brick_img[0].get_gray(32, 0, true), 0.0);

    // Mound heightfield -> normal -> curvature & ambient occlusion & slope blur
    let mound = Image::generate_uv(32, 32, Kind::Gray, |u, v| {
        let dx = u - 0.5;
        let dy = v - 0.5;
        [(1.0 - (dx * dx + dy * dy) * 4.0).clamp(0.0, 1.0), 0.0, 0.0, 0.0]
    });
    let normal_img = cook(find("normal").unwrap(), &[Some(mound.clone())], 32).unwrap();
    let curv_img = cook(find("curvature").unwrap(), &[Some(normal_img[0].clone())], 32).unwrap();
    assert!(curv_img[0].get_gray(16, 16, true) > 0.5, "center of convex mound should exceed neutral 0.5 curvature");

    // Pit center has darker ambient occlusion than flat rim
    let pit = Image::generate_uv(32, 32, Kind::Gray, |u, v| {
        let dx = u - 0.5;
        let dy = v - 0.5;
        [((dx * dx + dy * dy) * 8.0).clamp(0.0, 1.0), 0.0, 0.0, 0.0]
    });
    let ao_img = cook(find("ambient_occlusion").unwrap(), &[Some(pit.clone())], 32).unwrap();
    assert!(ao_img[0].get_gray(16, 16, true) < ao_img[0].get_gray(2, 2, true));

    // Slope blur preserves color kind when given a Color input
    let color_src = Image::generate_uv(16, 16, Kind::Color, |u, v| [u, v, 0.5, 1.0]);
    let sb_img = cook(find("slope_blur").unwrap(), &[Some(color_src), Some(mound)], 16).unwrap();
    assert_eq!(sb_img[0].kind, Kind::Color);
}
