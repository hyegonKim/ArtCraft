use std::collections::HashMap;

use super::extrude::{BevelStyle, ExtrudeParams, Outline, extrude, group_contours};
use super::prim::{PrimParams, PrimitiveKind, mesh};
use super::triangulate::{signed_area, triangulate};
use super::*;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn resolver(name: &str) -> Option<Vec<u8>> {
    std::fs::read(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).ok()
}

fn check_quad(m: &Model) {
    assert_eq!(m.nodes.len(), 2);
    assert_eq!(m.roots, vec![0]);
    assert_eq!(m.nodes[0].children, vec![1]);
    assert_eq!(m.meshes.len(), 1);
    let p = &m.meshes[0].primitives[0];
    assert_eq!(p.positions.len(), 4);
    assert_eq!(p.indices, vec![0, 1, 2, 0, 2, 3]);
    assert_eq!(p.uvs[0], [0.0, 1.0]);
    assert_eq!(p.normals[2], [0.0, 0.0, 1.0]);
    let mat = &m.materials[0];
    assert_eq!(mat.name, "Painted");
    assert_eq!(mat.base_color, [1.0, 0.5, 0.25, 1.0]);
    assert!((mat.metallic - 0.25).abs() < 1e-6 && (mat.roughness - 0.75).abs() < 1e-6);
    assert_eq!(mat.emissive, [0.1, 0.2, 0.3]);
    assert!(mat.double_sided);
    let t = &m.textures[mat.base_color_tex.unwrap().texture];
    assert_eq!((t.width, t.height), (2, 2));
    assert_eq!(t.data[0], [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(t.data[3], [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(t.wrap, [Wrap::Clamp, Wrap::Repeat]);
    // Hierarchy: scale 2 under a +1 Y translation.
    let (lo, hi) = m.bounds().unwrap();
    assert!((lo[0] + 1.0).abs() < 1e-9 && (hi[0] - 1.0).abs() < 1e-9);
    assert!((lo[1] - 0.0).abs() < 1e-9 && (hi[1] - 2.0).abs() < 1e-9);
    // Animation: root translation (0,1,0) → (2,1,0) over 1 s.
    assert_eq!(m.animations.len(), 1);
    assert_eq!(m.animations[0].name, "Move");
    assert!((m.animations[0].duration() - 1.0).abs() < 1e-9);
    let inst = m.instances(Some(0), 0.5);
    assert_eq!(inst.len(), 1);
    let c = inst[0].world.apply(vec3(0.0, 0.0, 0.0));
    assert!((c.x - 1.0).abs() < 1e-6 && (c.y - 1.0).abs() < 1e-6, "{c:?}");
}

#[test]
fn gltf_with_data_uris() {
    let m = gltf::parse(&fixture("quad.gltf"), &|_| None).unwrap();
    check_quad(&m);
}

#[test]
fn glb_matches_gltf() {
    let m = load("quad.glb", &fixture("quad.glb"), &resolver).unwrap();
    check_quad(&m);
    let g = gltf::parse(&fixture("quad.gltf"), &|_| None).unwrap();
    assert_eq!(m.meshes, g.meshes);
    assert_eq!(m.materials, g.materials);
}

#[test]
fn gltf_errors_are_reported() {
    assert!(gltf::parse(b"{\"asset\":{\"version\":\"1.0\"}}", &|_| None).is_err());
    assert!(gltf::parse(b"not json", &|_| None).is_err());
    let missing = br#"{"asset":{"version":"2.0"},"buffers":[{"uri":"nowhere.bin","byteLength":4}]}"#;
    assert!(matches!(gltf::parse(missing, &|_| None), Err(ModelError::Missing(_))));
    assert!(gltf::split_glb(b"glTF\x01\0\0\0").is_err());
}

#[test]
fn base64_and_uri_decoding() {
    assert_eq!(gltf::base64_decode("aGVsbG8=").unwrap(), b"hello");
    assert_eq!(gltf::base64_decode("aGVsbG8").unwrap(), b"hello");
    assert_eq!(gltf::uri_decode("my%20file.bin"), "my file.bin");
}

#[test]
fn obj_with_mtl() {
    let m = load("cube.obj", &fixture("cube.obj"), &resolver).unwrap();
    assert_eq!(m.materials.len(), 2);
    let red = &m.materials[0];
    assert_eq!(red.name, "Red");
    assert_eq!(red.base_color, [1.0, 0.0, 0.0, 1.0]);
    assert!((red.roughness - (2.0f32 / 100.0).sqrt()).abs() < 1e-6);
    assert_eq!(red.metallic, 0.0);
    let gold = &m.materials[1];
    assert_eq!((gold.metallic, gold.roughness), (1.0, 0.3));
    let prims = &m.meshes[0].primitives;
    assert_eq!(prims.len(), 2);
    assert_eq!(prims[0].material, Some(0));
    assert_eq!(prims.iter().map(|p| p.triangle_count()).sum::<usize>(), 12);
    // Four unique (v, vt, vn) corners per face.
    assert_eq!(m.vertex_count(), 24);
    // Every face is wound counter-clockwise from outside (face normal agrees with vn).
    for p in prims {
        for t in p.indices.chunks(3) {
            let (a, b, c) = (p.positions[t[0] as usize], p.positions[t[1] as usize], p.positions[t[2] as usize]);
            let f = cross(sub(b, a), sub(c, a));
            assert!(dot(f, p.normals[t[0] as usize]) > 0.0);
        }
    }
    let (lo, hi) = m.bounds().unwrap();
    assert_eq!((lo, hi), ([-0.5; 3], [0.5; 3]));
    // V is flipped to the image's top-down rows.
    assert_eq!(prims[0].uvs[0], [0.0, 1.0]);
}

#[test]
fn obj_without_normals_gets_them() {
    let src = b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";
    let m = obj::parse(src, &|_| None).unwrap();
    let p = &m.meshes[0].primitives[0];
    assert_eq!(p.normals, vec![[0.0, 0.0, 1.0]; 3]);
    assert!(obj::parse(b"v 0 0 0\n", &|_| None).is_err());
}

#[test]
fn obj_mixed_authored_normals_keep_flat_missing_corners() {
    let src = b"v 0 0 0\nv 1 0 0\nv 0 1 0\nv 0 0 1\nvn 0 1 0\ns off\nf 1//1 2 3\nf 1 3 4\n";
    let m = obj::parse(src, &|_| None).unwrap();
    let corners: Vec<_> = m.meshes[0]
        .primitives
        .iter()
        .flat_map(|p| p.positions.iter().zip(&p.normals))
        .filter(|(position, _)| **position == [0.0, 1.0, 0.0])
        .map(|(_, normal)| *normal)
        .collect();
    assert!(corners.contains(&[0.0, 0.0, 1.0]), "{corners:?}");
    assert!(corners.contains(&[1.0, 0.0, 0.0]), "{corners:?}");
    assert!(m.meshes[0].primitives.iter().any(|p| p.normals.contains(&[0.0, 1.0, 0.0])));
}

#[test]
fn obj_textures_from_another_machine_find_the_file_next_to_the_model() {
    // No committed media: the material file and its textures are written by the test itself.
    let d = std::env::temp_dir().join(format!("ec-obj-sibling-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let png = {
        let img = image::RgbaImage::from_fn(2, 2, |x, y| if (x + y) % 2 == 0 { image::Rgba([255, 0, 0, 255]) } else { image::Rgba([0, 0, 255, 255]) });
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    };
    std::fs::write(d.join("checker.png"), &png).unwrap();
    std::fs::write(d.join("Diffuse Map.png"), &png).unwrap();
    let mtl = "newmtl Mat\nKd 0.640000 0.640000 0.640000\nmap_Kd C:\\Users\\somebody\\Desktop\\textures\\checker.png\n\
               newmtl Spaced\nKd 1.000000 1.000000 1.000000\nmap_Kd Diffuse Map.png\n\
               newmtl Missing\nKd 0.500000 0.500000 0.500000\nmap_Kd nowhere.png\n";
    let obj = "mtllib quad.mtl\nv -1 -1 0\nv 1 -1 0\nv 1 1 0\nv -1 1 0\nvt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\n\
               usemtl Mat\nf 1/1 2/2 3/3\nf 1/1 3/3 4/4\n";
    // Like the media layer's resolver: a relative name joins the model's folder; an absolute
    // path is only looked up where it points (here: nowhere else).
    let resolve = |uri: &str| -> Option<Vec<u8>> {
        let p = std::path::Path::new(uri);
        if p.is_absolute() {
            return None;
        }
        std::fs::read(d.join(p)).ok()
    };
    let resolver = |uri: &str| -> Option<Vec<u8>> { if uri == "quad.mtl" { Some(mtl.as_bytes().to_vec()) } else { resolve(uri) } };
    let m = obj::parse(obj.as_bytes(), &resolver).unwrap();
    assert_eq!(m.materials.len(), 3);
    let mat = &m.materials[0];
    let t = mat.base_color_tex.expect("the absolute map_Kd path finds checker.png next to the .obj");
    let tex = &m.textures[t.texture];
    assert_eq!((tex.width, tex.height), (2, 2));
    assert_eq!(tex.data[0], [1.0, 0.0, 0.0, 1.0]);
    assert!((mat.base_color[0] - obj::srgb_to_linear(0.64)).abs() < 1e-6, "Kd still tints the texture");
    assert!(m.materials[1].base_color_tex.is_some(), "a texture name containing spaces is one name");
    assert!(m.materials[2].base_color_tex.is_none(), "a genuinely missing texture keeps the Kd colour");
    assert!((m.materials[2].base_color[0] - obj::srgb_to_linear(0.5)).abs() < 1e-6);
    // The same fallback applies to the material library itself.
    let here = obj.replace("mtllib quad.mtl", "mtllib C:\\build\\quad.mtl");
    assert_eq!(obj::parse(here.as_bytes(), &resolver).unwrap().materials.len(), 3);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn obj_texture_prefers_the_complete_name_over_its_final_token() {
    // Two textures sit beside the model: `Map.png` (the final whitespace token of the reference)
    // and `Diffuse Map.png` (the complete name). The complete name must win, its basename next,
    // the final token last (the `-bm 1 file.png` options convention).
    let d = std::env::temp_dir().join(format!("ec-obj-full-name-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let png = |w: u32| {
        let img = image::RgbaImage::from_fn(w, 2, |_, _| image::Rgba([255, 0, 0, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    };
    std::fs::write(d.join("Map.png"), png(2)).unwrap();
    std::fs::write(d.join("Diffuse Map.png"), png(4)).unwrap();
    let mtl = "newmtl Spaced\nmap_Kd Diffuse Map.png\n\
               newmtl Absolute\nmap_Kd C:\\tex\\Diffuse Map.png\n\
               newmtl Options\nmap_Kd -bm 1 Map.png\n";
    let obj = "mtllib quad.mtl\nv -1 -1 0\nv 1 -1 0\nv 1 1 0\nv -1 1 0\nvt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\n\
               usemtl Spaced\nf 1/1 2/2 3/3\nf 1/1 3/3 4/4\n";
    // Like the media layer's resolver: a relative name joins the model's folder; an absolute
    // path is only looked up where it points (here: nowhere else).
    let resolve = |uri: &str| -> Option<Vec<u8>> {
        let p = std::path::Path::new(uri);
        if p.is_absolute() {
            return None;
        }
        std::fs::read(d.join(p)).ok()
    };
    let resolver = |uri: &str| -> Option<Vec<u8>> { if uri == "quad.mtl" { Some(mtl.as_bytes().to_vec()) } else { resolve(uri) } };
    let m = obj::parse(obj.as_bytes(), &resolver).unwrap();
    assert_eq!(m.materials.len(), 3);
    let size = |mat: &Material| mat.base_color_tex.map(|t| (m.textures[t.texture].width, m.textures[t.texture].height));
    assert_eq!(size(&m.materials[0]), Some((4, 2)), "the complete `Diffuse Map.png` wins over its final token `Map.png`");
    assert_eq!(size(&m.materials[1]), Some((4, 2)), "an absolute path's spaced basename beats its final token");
    assert_eq!(size(&m.materials[2]), Some((2, 2)), "the `-bm 1 file.png` options convention still finds the final token");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn channel_interpolation_modes() {
    let mut c = Channel { node: 0, path: ChannelPath::Translation, interp: Interp::Step, times: vec![0.0, 1.0], values: vec![0.0, 0.0, 0.0, 10.0, 0.0, 0.0] };
    assert_eq!(c.sample(0.7).unwrap()[0], 0.0);
    c.interp = Interp::Linear;
    assert!((c.sample(0.7).unwrap()[0] - 7.0).abs() < 1e-9);
    assert_eq!(c.sample(5.0).unwrap()[0], 10.0);
    // Cubic spline with zero tangents: smoothstep.
    c.interp = Interp::CubicSpline;
    c.values = vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 10.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    assert!((c.sample(0.5).unwrap()[0] - 5.0).abs() < 1e-9);
    assert!((c.sample(0.25).unwrap()[0] - 10.0 * (3.0 * 0.0625 - 2.0 * 0.015625)).abs() < 1e-9);
    // Rotation: slerp keeps unit length, half way around Z.
    let r =
        Channel { node: 0, path: ChannelPath::Rotation, interp: Interp::Linear, times: vec![0.0, 1.0], values: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0] };
    let q = r.sample(0.5).unwrap();
    assert!((q[2] - (0.5f64).sqrt()).abs() < 1e-9 && (q[3] - (0.5f64).sqrt()).abs() < 1e-9);
}

#[test]
fn skinning_moves_vertices_with_joints() {
    let mut p = mesh(PrimitiveKind::Plane, &PrimParams { size: [2.0, 2.0, 0.0], ..Default::default() });
    p.joints = vec![[0, 0, 0, 0], [1, 0, 0, 0], [1, 0, 0, 0], [0, 0, 0, 0]];
    p.weights = vec![[1.0, 0.0, 0.0, 0.0]; 4];
    let mut m = Model::single(Mesh { name: "s".into(), primitives: vec![p] }, vec![Material::default()]);
    m.nodes[0].skin = Some(0);
    m.nodes.push(Node::default());
    m.nodes.push(Node { translation: [0.0, 0.0, 5.0], ..Node::default() });
    m.roots = vec![0, 1, 2];
    m.skins.push(Skin { joints: vec![1, 2], inverse_bind: vec![Mat4::IDENTITY; 2] });
    let inst = m.instances(None, 0.0);
    let pr = &inst[0].mesh.primitives[0];
    assert_eq!(skin_point(&inst[0], pr, 0, pr.positions[0])[2], 0.0);
    assert_eq!(skin_point(&inst[0], pr, 1, pr.positions[1])[2], 5.0);
}

/// Edges of a mesh welded by position: (undirected edge → uses, directed edge → uses).
fn edge_stats(p: &Primitive) -> (HashMap<(u32, u32), usize>, HashMap<(u32, u32), usize>) {
    let mut weld: HashMap<[u32; 3], u32> = HashMap::new();
    let ids: Vec<u32> = p
        .positions
        .iter()
        .map(|q| {
            let k = q.map(|v| (v + 0.0).to_bits());
            let n = weld.len() as u32;
            *weld.entry(k).or_insert(n)
        })
        .collect();
    let mut und = HashMap::new();
    let mut dir = HashMap::new();
    for t in p.indices.chunks(3) {
        let v = [ids[t[0] as usize], ids[t[1] as usize], ids[t[2] as usize]];
        if v[0] == v[1] || v[1] == v[2] || v[0] == v[2] {
            continue;
        }
        for k in 0..3 {
            let (a, b) = (v[k], v[(k + 1) % 3]);
            *und.entry((a.min(b), a.max(b))).or_insert(0) += 1;
            *dir.entry((a, b)).or_insert(0) += 1;
        }
    }
    (und, dir)
}

fn assert_watertight(p: &Primitive) {
    let (und, dir) = edge_stats(p);
    for (e, n) in &und {
        assert_eq!(*n, 2, "edge {e:?} used {n} times");
    }
    for (e, n) in &dir {
        assert_eq!(*n, 1, "directed edge {e:?} used {n} times (inconsistent winding)");
    }
}

/// Signed volume (positive when faces wind outward).
fn volume(p: &Primitive) -> f64 {
    p.indices
        .chunks(3)
        .map(|t| {
            let [a, b, c] = [0, 1, 2].map(|k| p.positions[t[k] as usize].map(|v| v as f64));
            (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])) / 6.0
        })
        .sum()
}

/// Vertex normals agree with the faces they belong to.
fn assert_normals_outward(p: &Primitive) {
    for t in p.indices.chunks(3) {
        let (a, b, c) = (p.positions[t[0] as usize], p.positions[t[1] as usize], p.positions[t[2] as usize]);
        let f = cross(sub(b, a), sub(c, a));
        if dot(f, f) < 1e-10 {
            continue;
        }
        for &i in t {
            let n = p.normals[i as usize];
            assert!((dot(n, n) - 1.0).abs() < 1e-3, "normal not unit");
            assert!(dot(f, n) > 0.0, "normal {n:?} faces away from triangle {f:?}");
        }
    }
}

#[test]
fn primitive_vertex_counts_and_normals() {
    let pr = PrimParams { segments: 16, rings: 8, ..Default::default() };
    let cases = [
        (PrimitiveKind::Cube, 24, 36),
        (PrimitiveKind::Plane, 4, 6),
        (PrimitiveKind::Sphere, 17 * 9, 6 * 16 * 7),
        (PrimitiveKind::Torus, 17 * 9, 6 * 16 * 8),
        (PrimitiveKind::Cylinder, 2 * 17 + 2 * 18, 6 * 16 + 2 * 3 * 16),
        (PrimitiveKind::Cone, 2 * 17 + 18, 3 * 16 + 3 * 16),
    ];
    for (k, v, i) in cases {
        let m = mesh(k, &pr);
        assert_eq!((m.positions.len(), m.indices.len()), (v, i), "{k:?}");
        assert_eq!(m.normals.len(), v);
        assert_eq!(m.uvs.len(), v);
        assert_eq!(m.tangents.len(), v);
        assert_normals_outward(&m);
        if k != PrimitiveKind::Plane {
            assert!(volume(&m) > 0.0, "{k:?} winds inward");
            assert_watertight(&m);
        }
    }
    // Volumes approach the analytic ones.
    let cube = mesh(PrimitiveKind::Cube, &PrimParams { size: [2.0, 3.0, 4.0], ..Default::default() });
    assert!((volume(&cube) - 24.0).abs() < 1e-4);
    let s = mesh(PrimitiveKind::Sphere, &PrimParams { radius: 1.0, segments: 128, rings: 64, ..Default::default() });
    assert!((volume(&s) - 4.0 / 3.0 * std::f64::consts::PI).abs() < 0.01);
}

#[test]
fn triangulation_covers_polygon_with_hole() {
    let outer = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [5.0, 12.0], [0.0, 10.0]];
    let hole = vec![[3.0, 3.0], [3.0, 7.0], [7.0, 7.0], [7.0, 3.0]];
    let pts: Vec<[f64; 2]> = outer.iter().chain(hole.iter()).copied().collect();
    let tris = triangulate(&outer, std::slice::from_ref(&hole));
    let area: f64 = tris.iter().map(|t| signed_area(&[pts[t[0]], pts[t[1]], pts[t[2]]]) * 0.5).sum();
    let expect = signed_area(&outer).abs() * 0.5 - 16.0;
    assert!((area - expect).abs() < 1e-9, "{area} vs {expect}");
    assert!(tris.iter().all(|t| signed_area(&[pts[t[0]], pts[t[1]], pts[t[2]]]) >= 0.0));
    // A concave "C".
    let c = vec![[0.0, 0.0], [6.0, 0.0], [6.0, 2.0], [2.0, 2.0], [2.0, 4.0], [6.0, 4.0], [6.0, 6.0], [0.0, 6.0]];
    let tris = triangulate(&c, &[]);
    let area: f64 = tris.iter().map(|t| signed_area(&[c[t[0]], c[t[1]], c[t[2]]]) * 0.5).sum();
    assert!((area - 28.0).abs() < 1e-9);
}

fn ring_outline() -> Outline {
    let circle =
        |r: f64, n: usize| -> Vec<[f64; 2]> { (0..n).map(|i| (i as f64 / n as f64) * std::f64::consts::TAU).map(|a| [r * a.cos(), r * a.sin()]).collect() };
    Outline { outer: circle(100.0, 48), holes: vec![circle(50.0, 32)] }
}

#[test]
fn extrusion_is_watertight_with_outward_normals() {
    let square = Outline { outer: vec![[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]], holes: vec![] };
    for bevel in BevelStyle::ALL {
        for o in [square.clone(), ring_outline()] {
            let p = ExtrudeParams { bevel, bevel_depth: 8.0, hole_bevel: 0.5, depth: 40.0 };
            let m = extrude(std::slice::from_ref(&o), &p);
            assert_watertight(&m);
            assert_normals_outward(&m);
            assert!(volume(&m) > 0.0, "{bevel:?}: inward winding");
            let (lo, hi) = m.bounds().unwrap();
            assert!(lo[2].abs() < 1e-6 && (hi[2] - 40.0).abs() < 1e-4);
        }
    }
    // Without a bevel the square block's volume is exact.
    let m = extrude(&[square], &ExtrudeParams { bevel: BevelStyle::None, bevel_depth: 0.0, hole_bevel: 1.0, depth: 10.0 });
    assert!((volume(&m) - 100_000.0).abs() < 1e-2);
    // Front cap faces the camera (−z), back cap away.
    assert!(m.normals.iter().any(|n| n[2] < -0.99) && m.normals.iter().any(|n| n[2] > 0.99));
}

#[test]
fn bevels_shrink_the_caps() {
    let square = Outline { outer: vec![[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]], holes: vec![] };
    let m = extrude(std::slice::from_ref(&square), &ExtrudeParams { bevel: BevelStyle::Angular, bevel_depth: 10.0, hole_bevel: 1.0, depth: 50.0 });
    // Front cap vertices (z = 0) are inset by the bevel.
    let front: Vec<_> = m.positions.iter().filter(|p| p[2] == 0.0).collect();
    assert!(front.iter().all(|p| p[0] >= 9.999 && p[0] <= 90.001));
    // The widest ring (the walls) still reaches the outline.
    assert!(m.positions.iter().any(|p| p[0] == 0.0 && p[2] == 10.0));
    // Convex bevels have more profile steps than angular ones.
    let c = extrude(std::slice::from_ref(&square), &ExtrudeParams { bevel: BevelStyle::Convex, bevel_depth: 10.0, hole_bevel: 1.0, depth: 50.0 });
    assert!(c.positions.len() > m.positions.len());
}

#[test]
fn extrusions_split_into_front_bevel_side_and_back() {
    use super::extrude::{Surface, extrude_surfaces, extrude_surfaces_stacked};
    let square = Outline { outer: vec![[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]], holes: vec![] };
    let p = ExtrudeParams { bevel: BevelStyle::Angular, bevel_depth: 10.0, hole_bevel: 1.0, depth: 50.0 };
    let whole = extrude(std::slice::from_ref(&square), &p);
    let parts = extrude_surfaces(std::slice::from_ref(&square), &p);
    // The parts hold exactly the triangles of the whole mesh.
    assert_eq!(parts.iter().map(|m| m.triangle_count()).sum::<usize>(), whole.triangle_count());
    let [front, bevel, side, back] = &parts;
    // Caps: flat, at the layer plane (front, facing −z) and at the extrusion depth (back, +z).
    assert!(front.positions.iter().all(|q| q[2] == 0.0) && front.normals.iter().all(|n| n[2] < -0.99));
    assert!(back.positions.iter().all(|q| q[2] == 50.0) && back.normals.iter().all(|n| n[2] > 0.99));
    // Walls: the full outline, between the bevels (z 10..40), with sideways normals.
    assert!(side.positions.iter().all(|q| (10.0..=40.0).contains(&q[2])) && side.normals.iter().all(|n| n[2].abs() < 1e-6));
    assert!(side.positions.iter().any(|q| q[0] == 0.0));
    // Bevels: slanted between the caps and the walls, front and back.
    assert!(bevel.triangle_count() > 0 && bevel.normals.iter().all(|n| n[2].abs() > 0.1 && n[2].abs() < 0.99));
    assert!(bevel.positions.iter().any(|q| q[2] == 0.0) && bevel.positions.iter().any(|q| q[2] == 50.0));
    assert_eq!(Surface::ALL.map(Surface::index), [0, 1, 2, 3]);
    // Without a bevel there is nothing on the Bevel surface.
    let flat = extrude_surfaces(std::slice::from_ref(&square), &ExtrudeParams { bevel: BevelStyle::None, ..p });
    assert_eq!(flat[Surface::Bevel.index()].triangle_count(), 0);
    assert!(flat[Surface::Side.index()].triangle_count() > 0);
    // Stacked like `extrude_stacked`.
    let up = extrude_surfaces_stacked(std::slice::from_ref(&square), &p, 2);
    assert!(up[0].positions.iter().all(|q| (q[2] + 2.0 * super::extrude::STACK_GAP as f32).abs() < 1e-6));
}

#[test]
fn contours_group_into_outlines_by_nesting() {
    let sq = |x: f64, y: f64, s: f64| vec![[x, y], [x + s, y], [x + s, y + s], [x, y + s]];
    // "O" (outer + hole) next to an island inside another hole.
    let g = group_contours(vec![sq(0.0, 0.0, 10.0), sq(2.0, 2.0, 6.0), sq(20.0, 0.0, 10.0), sq(22.0, 2.0, 6.0), sq(24.0, 4.0, 2.0)]);
    assert_eq!(g.len(), 3);
    assert_eq!(g.iter().map(|o| o.holes.len()).sum::<usize>(), 2);
}

#[test]
fn overlapping_contours_keep_every_container_in_their_depth() {
    let rect = |x0: f64, y0: f64, x1: f64, y1: f64| vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
    // C's first vertex lies inside both A and B, which overlap without nesting: C is two deep,
    // an outer contour, not a hole cut out of B.
    let g = group_contours(vec![rect(0.0, 0.0, 10.0, 10.0), rect(5.0, -2.0, 13.0, 8.0), rect(9.0, 1.0, 12.0, 2.0)]);
    assert_eq!(g.len(), 3);
    assert_eq!(g.iter().map(|o| o.holes.len()).sum::<usize>(), 0);
}

#[test]
fn a_contour_inside_a_hole_takes_the_innermost_outline_not_the_hole() {
    let rect = |x0: f64, y0: f64, x1: f64, y1: f64| vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
    // A and B overlap; C lies in A only (a hole of A); D lies in A, B and C (three deep, a hole).
    // The smallest container of D is the hole C, which has no outline of its own: D goes to B.
    let g = group_contours(vec![rect(0.0, 0.0, 10.0, 10.0), rect(5.0, -2.0, 13.0, 8.0), rect(1.0, 1.0, 9.0, 9.0), rect(6.0, 2.0, 7.0, 3.0)]);
    assert_eq!(g.len(), 2);
    assert_eq!(g.iter().map(|o| o.holes.len()).collect::<Vec<_>>(), [1, 1], "C is a hole of A, D a hole of B (not dropped)");
}

#[test]
fn a_hole_whose_outline_is_deeper_than_it_is_still_attached() {
    let rect = |x0: f64, y0: f64, x1: f64, y1: f64| vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
    // D is three deep and its innermost outline A four deep: A's slot must exist before D is attached.
    let a = rect(0.0, 0.0, 10.0, 10.0);
    let g = group_contours(vec![
        a.clone(),
        rect(-1.0, -1.0, 20.0, 20.0),
        rect(-2.0, -2.0, 25.0, 25.0),
        rect(5.0, 5.0, 6.0, 6.0),
        rect(-100.0, -100.0, 3.0, 3.0),
        rect(-0.5, -1000.0, 0.5, 1000.0),
    ]);
    let o = g.iter().find(|o| o.outer == a).expect("A is an outline");
    assert_eq!(o.holes.len(), 1, "D is a hole of A");
}

#[test]
fn gltf_images_naming_one_file_share_their_wrap_variants() {
    let png = {
        let img = image::RgbaImage::from_fn(2, 2, |x, y| if (x + y) % 2 == 0 { image::Rgba([255, 0, 0, 255]) } else { image::Rgba([0, 0, 255, 255]) });
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    };
    // Two image entries with one URI share a decode; two textures clamp it, so they share one copy.
    let doc = br#"{"asset":{"version":"2.0"},"images":[{"uri":"t.png"},{"uri":"t.png"}],
        "samplers":[{"wrapS":33071,"wrapT":33071}],"textures":[{"source":0,"sampler":0},{"source":1,"sampler":0}]}"#;
    let m = gltf::parse(doc, &|u| (u == "t.png").then(|| png.clone())).unwrap();
    assert_eq!(m.textures.len(), 1, "one clamped copy for both textures");
}

#[test]
fn gltf_cameras_and_punctual_lights() {
    let doc = r#"{
        "asset": {"version": "2.0"},
        "scene": 0,
        "scenes": [{"nodes": [0, 2]}],
        "nodes": [
            {"name": "Rig", "translation": [0, 0, 10], "children": [1]},
            {"name": "Cam", "camera": 0, "translation": [1, 0, 0]},
            {"name": "Sun", "rotation": [-0.7071068, 0, 0, 0.7071068], "extensions": {"KHR_lights_punctual": {"light": 1}}},
            {"name": "Orphan", "camera": 1}
        ],
        "cameras": [
            {"type": "perspective", "name": "Shot", "perspective": {"yfov": 0.5, "znear": 0.1, "aspectRatio": 1.5}},
            {"type": "orthographic", "orthographic": {"xmag": 2, "ymag": 1, "znear": 0, "zfar": 10}}
        ],
        "extensions": {"KHR_lights_punctual": {"lights": [
            {"type": "spot", "name": "Key", "color": [1, 0.5, 0.25], "intensity": 3, "spot": {"innerConeAngle": 0.2, "outerConeAngle": 0.4}},
            {"type": "directional", "intensity": 2}
        ]}}
    }"#;
    let m = crate::gltf::parse(doc.as_bytes(), &|_| None).unwrap();
    assert_eq!(m.cameras.len(), 2);
    assert_eq!(m.cameras[0].name, "Shot");
    assert_eq!(m.cameras[0].projection, CameraProjection::Perspective { yfov: 0.5, aspect: Some(1.5), znear: 0.1 });
    assert_eq!(m.cameras[1].projection, CameraProjection::Orthographic { xmag: 2.0, ymag: 1.0 });
    assert_eq!(m.lights.len(), 2);
    assert_eq!(m.lights[0].kind, ModelLightKind::Spot { inner: 0.2, outer: 0.4 });
    assert_eq!(m.lights[0].color, [1.0, 0.5, 0.25]);
    assert_eq!(m.lights[1].kind, ModelLightKind::Directional);
    assert_eq!(m.lights[1].name, "Light 2");
    // Only nodes of the scene are placed; the camera node inherits its parent's translation.
    let cams = m.placed(false, None, 0.0);
    assert_eq!(cams.len(), 1);
    assert_eq!((cams[0].index, cams[0].node), (0, 1));
    let eye = cams[0].world.apply(vec3(0.0, 0.0, 0.0));
    assert!((eye.x - 1.0).abs() < 1e-9 && (eye.z - 10.0).abs() < 1e-9, "{eye:?}");
    let lights = m.placed(true, None, 0.0);
    assert_eq!(lights.len(), 1);
    // −90° about X: the light's −Z axis points down (−Y in glTF).
    let d = lights[0].world.apply_vec(vec3(0.0, 0.0, -1.0));
    assert!((d.y + 1.0).abs() < 1e-6, "{d:?}");
}

#[test]
fn stacked_extrusions_move_towards_the_camera() {
    let square = Outline { outer: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]], holes: vec![] };
    let p = ExtrudeParams { bevel: BevelStyle::None, bevel_depth: 0.0, hole_bevel: 1.0, depth: 10.0 };
    let base = extrude(std::slice::from_ref(&square), &p);
    let up = super::extrude::extrude_stacked(std::slice::from_ref(&square), &p, 3);
    assert_eq!(base.indices, up.indices);
    for (a, b) in base.positions.iter().zip(&up.positions) {
        assert!((a[2] - b[2] - 3.0 * super::extrude::STACK_GAP as f32).abs() < 1e-6);
        assert_eq!((a[0], a[1]), (b[0], b[1]));
    }
}

/// A binary STL: `header` (padded to 80 bytes) and facets of (normal, vertices, attribute).
fn binary_stl(header: &[u8], facets: &[([f32; 3], [[f32; 3]; 3], u16)]) -> Vec<u8> {
    let mut b = header.to_vec();
    b.resize(80, 0);
    b.extend((facets.len() as u32).to_le_bytes());
    for (n, v, attr) in facets {
        for c in n.iter().chain(v.iter().flatten()) {
            b.extend(c.to_le_bytes());
        }
        b.extend(attr.to_le_bytes());
    }
    b
}

/// The unit square in the XY plane (Z-up file), facing +Z, as two facets.
const SQUARE: [[[f32; 3]; 3]; 2] = [[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]], [[0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]]];

#[test]
fn stl_binary_is_faceted_and_turned_y_up() {
    // A header that begins with `solid`, as many binary exporters write it.
    let bytes = binary_stl(b"solid part", &[([0.0, 0.0, 1.0], SQUARE[0], 0), ([0.0, 0.0, 1.0], SQUARE[1], 0)]);
    let m = load("part.STL", &bytes, &|_| None).unwrap();
    assert_eq!(format_of("part.STL"), Some("stl"));
    assert_eq!(m.meshes[0].name, "part");
    assert!(m.materials.is_empty());
    let p = &m.meshes[0].primitives[0];
    assert_eq!((p.triangle_count(), p.positions.len()), (2, 6));
    assert!(p.uvs.is_empty());
    // Z-up +Z becomes Y-up +Y; file Y becomes −Z.
    assert_eq!(p.normals, vec![[0.0, 1.0, 0.0]; 6]);
    assert_eq!(p.positions[2], [1.0, 0.0, -1.0]);
    // Counter-clockwise from outside after the turn.
    let (a, b, c) = (p.positions[0], p.positions[1], p.positions[2]);
    assert!(dot(cross(sub(b, a), sub(c, a)), p.normals[0]) > 0.0);
    // Trailing bytes after the last facet are ignored.
    let mut longer = bytes.clone();
    longer.extend([0u8; 7]);
    assert_eq!(stl::parse(&longer).unwrap().meshes, m.meshes);
}

#[test]
fn stl_ascii_with_polygon_loops_and_several_solids() {
    let src = "solid Bracket\n  facet normal 0 0 1\n    outer loop\n      vertex 0 0 0\n      vertex 1 0 0\n      vertex 1 1 0\n      vertex 0 1 0\n    endloop\n  endfacet\nendsolid Bracket\n\
               solid second\nfacet normal 0 0 0\nouter loop\nvertex 0 0 1\nvertex 1 0 1\nvertex 1E0 1 1\nendloop\nendfacet\nendsolid\n";
    let m = load("bracket.stl", src.as_bytes(), &|_| None).unwrap();
    assert_eq!(m.meshes[0].name, "Bracket");
    // The quad fans into two triangles; the second solid adds one.
    assert_eq!(m.triangle_count(), 3);
    let (lo, hi) = m.bounds().unwrap();
    assert_eq!((lo, hi), ([0.0, 0.0, -1.0], [1.0, 1.0, 0.0]));
    assert!(stl::parse(b"solid x\nfacet normal 0 0 1\nouter loop\nvertex 0 zero 0\n").is_err());
}

#[test]
fn stl_binary_colours() {
    // VisCAM / SolidView: bit 15 set, red in bits 10–14.
    let red = 0x8000 | (31 << 10);
    let m = stl::parse(&binary_stl(b"", &[([0.0; 3], SQUARE[0], red), ([0.0; 3], SQUARE[1], 0)])).unwrap();
    assert_eq!(m.meshes[0].primitives.len(), 2);
    assert_eq!(m.materials.len(), 2);
    assert!(m.materials[0].base_color.iter().zip([1.0, 0.0, 0.0, 1.0]).all(|(x, y)| (x - y).abs() < 1e-5));
    assert_eq!(m.materials[0].metallic, 0.0);
    assert_eq!(m.meshes[0].primitives[1].material, Some(1));
    // Materialise Magics: a part colour in the header; bit 15 clear = the facet's own colour
    // with red in bits 0–4.
    let mut header = b"COLOR=".to_vec();
    header.extend([0, 255, 0, 255]);
    let m = stl::parse(&binary_stl(&header, &[([0.0; 3], SQUARE[0], 31), ([0.0; 3], SQUARE[1], 0x8000)])).unwrap();
    let close = |a: [f32; 4], b: [f32; 4]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5);
    assert!(close(m.materials[0].base_color, [1.0, 0.0, 0.0, 1.0]), "{:?}", m.materials[0]);
    assert!(close(m.materials[1].base_color, [0.0, 1.0, 0.0, 1.0]), "{:?}", m.materials[1]);
}

#[test]
fn stl_hostile_input_is_an_error() {
    // Claims four billion facets but has one: truncated.
    let mut bytes = binary_stl(b"", &[([0.0; 3], SQUARE[0], 0)]);
    bytes.splice(80..84, u32::MAX.to_le_bytes());
    assert!(stl::parse(&bytes).is_err());
    assert!(stl::parse(b"").is_err());
    assert!(stl::parse(&[0u8; 83]).is_err());
    // No facets, or only non-finite ones.
    assert!(stl::parse(&binary_stl(b"", &[])).is_err());
    assert!(stl::parse(&binary_stl(b"", &[([0.0; 3], [[f32::NAN, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], 0)])).is_err());
    assert!(stl::parse(b"solid empty\nendsolid empty\n").is_err());
    // A degenerate facet keeps its stored normal.
    let m = stl::parse(&binary_stl(b"", &[([1.0, 0.0, 0.0], [[0.0; 3]; 3], 0)])).unwrap();
    assert_eq!(m.meshes[0].primitives[0].normals[0], [1.0, 0.0, 0.0]);
}

/// A one-triangle glTF whose POSITION is accessor 0, over a buffer of `bytes` zero bytes.
fn gltf_doc(accessors: &str, views: &str, bytes: usize) -> String {
    let b64 = "A".repeat(bytes / 3 * 4) + ["", "AA==", "AAA="][bytes % 3];
    format!(
        r#"{{"asset": {{"version": "2.0"}}, "scenes": [{{"nodes": [0]}}], "nodes": [{{"mesh": 0}}],
        "meshes": [{{"primitives": [{{"attributes": {{"POSITION": 0}}}}]}}],
        "buffers": [{{"byteLength": {bytes}, "uri": "data:application/octet-stream;base64,{b64}"}}],
        "bufferViews": [{views}], "accessors": [{accessors}]}}"#
    )
}

#[test]
fn gltf_accessor_counts_are_checked_against_the_buffer_before_allocating() {
    // Untrusted counts, offsets and strides must fail the import, not allocate from the count or
    // overflow (#1235).
    let view = r#"{"buffer": 0, "byteLength": 36}"#;
    let cases = [
        // A count far beyond the 36-byte buffer.
        (r#"{"bufferView": 0, "componentType": 5126, "type": "VEC3", "count": 4000000000}"#, view, "limit allows"),
        // count * components overflows.
        (r#"{"bufferView": 0, "componentType": 5126, "type": "MAT4", "count": 9223372036854775807}"#, view, "limit allows"),
        // Offsets that overflow when added.
        (
            r#"{"bufferView": 0, "componentType": 5126, "type": "VEC3", "count": 3, "byteOffset": 16}"#,
            r#"{"buffer": 0, "byteOffset": 18446744073709551610}"#,
            "past its buffer",
        ),
        // A stride that overflows with the count.
        (r#"{"bufferView": 0, "componentType": 5126, "type": "VEC3", "count": 3000000000}"#, r#"{"buffer": 0, "byteStride": 252}"#, "limit allows"),
        // A stride smaller than an element would re-read the same bytes for a huge count.
        (r#"{"bufferView": 0, "componentType": 5126, "type": "MAT4", "count": 30}"#, r#"{"buffer": 0, "byteStride": 1}"#, "smaller than"),
        // No buffer view: nothing backs the count, so it is capped.
        (r#"{"componentType": 5126, "type": "VEC3", "count": 1000000000}"#, view, "limit allows"),
    ];
    let t0 = std::time::Instant::now();
    for (acc, v, want) in cases {
        let doc = gltf_doc(acc, v, 36);
        let e = crate::gltf::parse(doc.as_bytes(), &|_| None).err().unwrap_or_else(|| panic!("{acc} imported"));
        assert!(e.to_string().contains(want), "{acc}: {e}");
    }
    assert!(t0.elapsed() < std::time::Duration::from_secs(10), "{:?}", t0.elapsed());
}

#[test]
fn gltf_integer_accessors_are_checked_too() {
    let doc = gltf_doc(r#"{"bufferView": 0, "componentType": 5126, "type": "VEC3", "count": 3}"#, r#"{"buffer": 0, "byteLength": 36}"#, 36)
        .replace(r#""attributes": {"POSITION": 0}"#, r#""attributes": {"POSITION": 0}, "indices": 1"#);
    for idx in [
        r#"{"bufferView": 0, "componentType": 5125, "type": "SCALAR", "count": 4000000000}"#,
        r#"{"componentType": 5125, "type": "SCALAR", "count": 4000000000}"#,
        r#"{"bufferView": 0, "componentType": 5125, "type": "VEC4", "count": 9223372036854775807}"#,
    ] {
        let doc = doc.replace(r#""count": 3}]"#, &format!(r#""count": 3}}, {idx}]"#));
        assert!(crate::gltf::parse(doc.as_bytes(), &|_| None).is_err(), "{idx}");
    }
}

#[test]
fn gltf_strided_and_buffer_less_accessors_still_import() {
    // Interleaved positions (stride 24) ending exactly at the buffer's end, and a small all-zero
    // accessor without a view.
    let doc = gltf_doc(r#"{"bufferView": 0, "componentType": 5126, "type": "VEC3", "count": 3}"#, r#"{"buffer": 0, "byteStride": 24}"#, 60);
    let m = crate::gltf::parse(doc.as_bytes(), &|_| None).unwrap();
    assert_eq!(m.meshes[0].primitives[0].positions.len(), 3);
    let doc = gltf_doc(r#"{"componentType": 5126, "type": "VEC3", "count": 6}"#, r#"{"buffer": 0}"#, 3);
    let m = crate::gltf::parse(doc.as_bytes(), &|_| None).unwrap();
    assert_eq!(m.meshes[0].primitives[0].positions.len(), 6);
    // One byte short of the third element is an error.
    let doc = gltf_doc(r#"{"bufferView": 0, "componentType": 5126, "type": "VEC3", "count": 3}"#, r#"{"buffer": 0, "byteStride": 24}"#, 59);
    assert!(crate::gltf::parse(doc.as_bytes(), &|_| None).is_err());
}

#[test]
fn gltf_issue_2139_huge_mat4_count_is_an_error() {
    // The audit's trigger (#2139): a MAT4 accessor with a count of 10^11, with and without a
    // buffer view, must fail cleanly instead of sizing an allocation from the count.
    let view = r#"{"buffer": 0, "byteLength": 64}"#;
    for acc in [
        r#"{"bufferView": 0, "componentType": 5126, "type": "MAT4", "count": 100000000000}"#,
        r#"{"componentType": 5126, "type": "MAT4", "count": 100000000000}"#,
    ] {
        let doc = gltf_doc(r#"{"bufferView": 0, "componentType": 5126, "type": "VEC3", "count": 1}"#, view, 64)
            .replace(r#""count": 1}]"#, &format!(r#""count": 1}}, {acc}]"#))
            .replace(r#""nodes": [{"mesh": 0}]"#, r#""nodes": [{"mesh": 0, "skin": 0}], "skins": [{"joints": [0], "inverseBindMatrices": 1}]"#);
        assert!(doc.contains("inverseBindMatrices"), "{doc}");
        assert!(crate::gltf::parse(doc.as_bytes(), &|_| None).is_err(), "{acc}");
    }
}

#[test]
fn gltf_reused_buffer_less_accessor_is_read_once() {
    // The shared reader parses a repeated accessor only once, so referencing it from several
    // primitives does not multiply its one bounded allocation.
    let acc = r#"{"componentType": 5126, "type": "VEC3", "count": 5000000}"#;
    let prims = [r#"{"attributes": {"POSITION": 0}}"#; 4].join(", ");
    let doc = gltf_doc(acc, r#"{"buffer": 0}"#, 3).replace(r#"[{"attributes": {"POSITION": 0}}]"#, &format!("[{prims}]"));
    assert_eq!(doc.matches(r#""POSITION": 0"#).count(), 4);
    let result = crate::gltf::parse(doc.as_bytes(), &|_| None);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn texture_decoding_is_charged_to_the_import_budget() {
    let png = {
        let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([0, 255, 0, 255]));
        let mut b = std::io::Cursor::new(vec![]);
        img.write_to(&mut b, image::ImageFormat::Png).unwrap();
        b.into_inner()
    };
    let mut left = 4;
    assert_eq!(Texture::decode_within(&png, &mut left).unwrap().data.len(), 4);
    assert_eq!(left, 0);
    assert!(matches!(Texture::decode_within(&png, &mut left), Err(ModelError::Unsupported(_))));
}

#[test]
fn obj_repeated_material_libraries_and_textures_load_once() {
    let png = {
        let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 255, 0, 255]));
        let mut b = std::io::Cursor::new(vec![]);
        img.write_to(&mut b, image::ImageFormat::Png).unwrap();
        b.into_inner()
    };
    let mtl = b"newmtl A\nmap_Kd t.png\nnewmtl B\nmap_Kd t.png\n".to_vec();
    let src = b"mtllib m.mtl\nmtllib m.mtl\nv 0 0 0\nv 1 0 0\nv 0 1 0\nusemtl A\nf 1 2 3\nusemtl B\nf 1 2 3\nusemtl A\nf 1 2 3\n";
    let m = obj::parse(src, &|name| match name {
        "m.mtl" => Some(mtl.clone()),
        "t.png" => Some(png.clone()),
        _ => None,
    })
    .unwrap();
    assert_eq!(m.materials.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["A", "B"]);
    assert_eq!(m.textures.len(), 1);
    assert_eq!(m.meshes[0].primitives.iter().map(|p| (p.material, p.triangle_count())).collect::<Vec<_>>(), [(Some(0), 2), (Some(1), 1)]);
}

#[test]
fn gltf_textures_naming_one_image_share_it() {
    let doc = String::from_utf8(fixture("quad.gltf")).unwrap().replace("\r\n", "\n");
    // Three textures of one image: the fixture's sampler, the same sampler again, no sampler.
    let doc = doc.replace(
        "\"textures\": [\n  {\n   \"source\": 0,\n   \"sampler\": 0\n  }\n ]",
        "\"textures\": [{\"source\": 0, \"sampler\": 0}, {\"source\": 0, \"sampler\": 0}, {\"source\": 0}]",
    );
    let m = gltf::parse(doc.as_bytes(), &|_| None).unwrap();
    assert_eq!(m.textures.len(), 2);
    assert_eq!(m.textures[0].wrap, [Wrap::Clamp, Wrap::Repeat]);
    assert_eq!(m.textures[1].wrap, [Wrap::Repeat, Wrap::Repeat]);
}

#[test]
fn instances_of_one_skin_share_its_joint_matrices() {
    let p = mesh(PrimitiveKind::Plane, &PrimParams { size: [2.0, 2.0, 0.0], ..Default::default() });
    let mut m = Model::single(Mesh { name: "s".into(), primitives: vec![p] }, vec![Material::default()]);
    m.nodes[0].skin = Some(0);
    m.nodes.push(Node { mesh: Some(0), skin: Some(0), ..Node::default() });
    m.roots = vec![0, 1];
    m.skins.push(Skin { joints: vec![0], inverse_bind: vec![Mat4::IDENTITY] });
    let inst = m.instances(None, 0.0);
    assert_eq!(inst.len(), 2);
    assert!(Arc::ptr_eq(inst[0].joints.as_ref().unwrap(), inst[1].joints.as_ref().unwrap()));
}

#[test]
fn contour_grouping_is_bounded() {
    let sq = |x: f64, y: f64, s: f64| vec![[x, y], [x + s, y], [x + s, y + s], [x, y + s]];
    let n = extrude::MAX_CONTOURS + 1;
    let cs: Vec<Vec<[f64; 2]>> = (0..n).map(|i| sq(i as f64 * 2.0, 0.0, 1.0)).collect();
    let g = group_contours(cs);
    assert_eq!(g.len(), extrude::MAX_CONTOURS);
    assert!(g.iter().all(|o| o.holes.is_empty()));
}

/// A texture the import leaves out (too large, or not an image) is named in `Model::warnings`,
/// which the import shows the user; the model still loads.
#[test]
fn left_out_textures_are_reported_as_import_warnings() {
    // A 24-bit BMP header claiming 9000×9000 (81 MP, over one texture's limit) over no pixel data.
    let mut big = b"BM".to_vec();
    big.extend_from_slice(&[0; 8]);
    big.extend_from_slice(&54u32.to_le_bytes());
    big.extend_from_slice(&40u32.to_le_bytes());
    big.extend_from_slice(&9000u32.to_le_bytes());
    big.extend_from_slice(&9000u32.to_le_bytes());
    big.extend_from_slice(&[1, 0, 24, 0]);
    big.extend_from_slice(&[0; 24]);
    let mtl = b"newmtl A\nmap_Kd big.bmp\nnewmtl B\nmap_Kd junk.png\nnewmtl C\nmap_Kd gone.png\n".to_vec();
    let src = b"mtllib m.mtl\nv 0 0 0\nv 1 0 0\nv 0 1 0\nusemtl A\nf 1 2 3\n";
    let m = obj::parse(src, &|name| match name {
        "m.mtl" => Some(mtl.clone()),
        "big.bmp" => Some(big.clone()),
        "junk.png" => Some(b"not an image".to_vec()),
        _ => None,
    })
    .unwrap();
    assert!(m.textures.is_empty());
    let all = m.warnings.join("|");
    assert!(all.contains("big.bmp") && all.contains("junk.png") && all.contains("gone.png"), "{all}");
}

// ---------------------------------------------------------------------------
// OpenUSD (.usda / .usdz) & Substance Procedural PBR Tests (Gap 12 / Gap 13)
// ---------------------------------------------------------------------------

fn solid_png(r: u8, g: u8, b: u8, a: u8) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([r, g, b, a]));
    let mut cur = std::io::Cursor::new(Vec::new());
    img.write_to(&mut cur, image::ImageFormat::Png).unwrap();
    cur.into_inner()
}

/// Build an in-memory `.usdz` ZIP container (`deflate_first`: wraps the first entry in a valid
/// RFC 1951 Stored-type deflate block with method 8 to exercise the pure-Rust inflate path).
fn make_test_usdz(entries: &[(&str, &[u8])], deflate_first: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let mut cd = Vec::new();
    for (idx, &(name, raw)) in entries.iter().enumerate() {
        let lfh_off = out.len() as u32;
        let (method, payload): (u16, Vec<u8>) = if deflate_first && idx == 0 && raw.len() < 65_000 {
            let len = raw.len() as u16;
            let nlen = !len;
            let mut blk = Vec::with_capacity(5 + raw.len());
            blk.push(0x01); // BFINAL=1, BTYPE=00 (uncompressed RFC 1951 block inside method-8 stream)
            blk.extend_from_slice(&len.to_le_bytes());
            blk.extend_from_slice(&nlen.to_le_bytes());
            blk.extend_from_slice(raw);
            (8, blk)
        } else {
            (0, raw.to_vec())
        };
        let nb = name.as_bytes();
        // Local file header
        out.extend_from_slice(b"PK\x03\x04");
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&method.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        out.extend_from_slice(&(nb.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(nb);
        out.extend_from_slice(&payload);

        // Central directory entry
        cd.extend_from_slice(b"PK\x01\x02");
        cd.extend_from_slice(&20u16.to_le_bytes());
        cd.extend_from_slice(&20u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&method.to_le_bytes());
        cd.extend_from_slice(&0u32.to_le_bytes());
        cd.extend_from_slice(&0u32.to_le_bytes());
        cd.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        cd.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        cd.extend_from_slice(&(nb.len() as u16).to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u32.to_le_bytes());
        cd.extend_from_slice(&lfh_off.to_le_bytes());
        cd.extend_from_slice(nb);
    }
    let cd_off = out.len() as u32;
    let cd_size = cd.len() as u32;
    out.extend_from_slice(&cd);
    out.extend_from_slice(b"PK\x05\x06");
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_off.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

#[test]
fn usda_hierarchical_xforms_ngon_mesh_and_preview_surface_import() {
    let albedo_png = solid_png(220, 40, 60, 255);
    let normal_png = solid_png(128, 128, 255, 255);
    let usda = r#"#usda 1.0
(
    upAxis = "Z"
    metersPerUnit = 1.0
)

def Scope "Looks"
{
    def Material "PaintedMetal"
    {
        token outputs:surface.connect = </Looks/PaintedMetal/PBRShader.outputs:surface>

        def Shader "PBRShader"
        {
            uniform token info:id = "UsdPreviewSurface"
            color3f inputs:diffuseColor.connect = </Looks/PaintedMetal/AlbedoTex.outputs:rgb>
            float inputs:metallic = 0.85
            float inputs:roughness = 0.22
            color3f inputs:emissiveColor = (0.1, 0.2, 0.3)
            float inputs:opacity = 0.9
            normal3f inputs:normal.connect = </Looks/PaintedMetal/NormTex.outputs:rgb>
            token outputs:surface
        }

        def Shader "AlbedoTex"
        {
            uniform token info:id = "UsdUVTexture"
            asset inputs:file = @textures/albedo.png@
            token inputs:wrapS = "clamp"
            token inputs:wrapT = "mirror"
            float3 outputs:rgb
        }

        def Shader "NormTex"
        {
            uniform token info:id = "UsdUVTexture"
            asset inputs:file = @textures/normal.png@
            float3 outputs:rgb
        }
    }
}

def Xform "RootRig"
{
    double3 xformOp:translate = (10.0, 20.0, 30.0)
    float3 xformOp:scale = (2.0, 2.0, 2.0)
    uniform token[] xformOpOrder = ["xformOp:translate", "xformOp:scale"]

    def Mesh "PentagonPanel" (
        prepend apiSchemas = ["MaterialBindingAPI"]
    )
    {
        rel material:binding = </Looks/PaintedMetal>
        uniform bool doubleSided = 1
        point3f[] points = [(0, 0, 0), (2, 0, 0), (3, 1, 0), (1, 2, 0), (-1, 1, 0)]
        int[] faceVertexCounts = [5]
        int[] faceVertexIndices = [0, 1, 2, 3, 4]
        texCoord2f[] primvars:st = [(0, 0), (1, 0), (1, 0.5), (0.5, 1), (0, 0.5)]
    }
}
"#;
    let m = crate::load("panel.usda", usda.as_bytes(), &|uri| match uri {
        "textures/albedo.png" => Some(albedo_png.clone()),
        "textures/normal.png" => Some(normal_png.clone()),
        _ => None,
    })
    .unwrap();

    // Pentagon fan-triangulates to 3 triangles; tangents and normals generated.
    assert_eq!(m.meshes.len(), 1);
    assert_eq!(m.triangle_count(), 3);
    let prim = &m.meshes[0].primitives[0];
    assert_eq!(prim.normals.len(), prim.positions.len());
    assert_eq!(prim.tangents.len(), prim.positions.len());

    // Material + double-sided variant carries metallic, roughness, emissive, blend opacity, and wrap modes.
    let mat = &m.materials[prim.material.unwrap()];
    assert!((mat.metallic - 0.85).abs() < 1e-5);
    assert!((mat.roughness - 0.22).abs() < 1e-5);
    assert!((mat.emissive[2] - 0.3).abs() < 1e-5);
    assert_eq!(mat.alpha_mode, AlphaMode::Blend);
    assert!(mat.double_sided);
    assert!(mat.base_color_tex.is_some());
    assert!(mat.normal_tex.is_some());
    assert_eq!(m.textures[mat.base_color_tex.unwrap().texture].wrap, [Wrap::Clamp, Wrap::Mirror]);

    // Z-up stage rotated -90° around X: local Z=30 maps to model +Y=30, local Y=20 maps to model -Z=-20.
    let (lo, hi) = m.bounds().unwrap();
    assert!((lo[0] - 8.0).abs() < 1e-5 && (hi[0] - 16.0).abs() < 1e-5, "{lo:?}..{hi:?}");
    assert!((lo[1] - 30.0).abs() < 1e-5 && (hi[1] - 30.0).abs() < 1e-5, "{lo:?}..{hi:?}");
    assert!((lo[2] - -24.0).abs() < 1e-5 && (hi[2] - -20.0).abs() < 1e-5, "{lo:?}..{hi:?}");
}

#[test]
fn usdz_stored_and_deflated_archives_decode_stage_and_textures() {
    let albedo = solid_png(40, 180, 240, 255);
    let rough = solid_png(90, 90, 90, 255);
    let stage = r#"#usda 1.0
def Material "Mat"
{
    token outputs:surface.connect = </Mat/Surf.outputs:surface>
    def Shader "Surf"
    {
        uniform token info:id = "UsdPreviewSurface"
        color3f inputs:diffuseColor.connect = </Mat/Base.outputs:rgb>
        float inputs:roughness.connect = </Mat/Rough.outputs:r>
        float inputs:metallic = 0.4
    }
    def Shader "Base"
    {
        uniform token info:id = "UsdUVTexture"
        asset inputs:file = @./tex/base.png@
    }
    def Shader "Rough"
    {
        uniform token info:id = "UsdUVTexture"
        asset inputs:file = @tex/rough.png@
    }
}
def Mesh "Quad"
{
    rel material:binding = </Mat>
    point3f[] points = [(-1, -1, 0), (1, -1, 0), (1, 1, 0), (-1, 1, 0)]
    int[] faceVertexCounts = [4]
    int[] faceVertexIndices = [0, 1, 2, 3]
    texCoord2f[] primvars:st = [(0, 0), (1, 0), (1, 1), (0, 1)]
}
"#;
    for deflated in [false, true] {
        let usdz = make_test_usdz(
            &[
                ("root/stage.usda", stage.as_bytes()),
                ("root/tex/base.png", &albedo),
                ("root/tex/rough.png", &rough),
            ],
            deflated,
        );
        let m = crate::load("asset.usdz", &usdz, &|_| None).unwrap();
        assert_eq!(m.triangle_count(), 2);
        assert_eq!(m.materials.len(), 1);
        assert!(m.materials[0].base_color_tex.is_some());
        assert!(m.materials[0].metallic_roughness_tex.is_some());
        assert!(m.warnings.is_empty(), "{:?}", m.warnings);
    }
}

#[test]
fn usd_and_usdz_hostile_inputs_never_panic() {
    assert!(matches!(crate::usd::parse(b"PXR-USDC\x00\x01", &|_| None), Err(ModelError::Unsupported(_))));
    assert!(crate::usd::parse(b"", &|_| None).is_err());
    assert!(crate::usd::parse(b"not usd", &|_| None).is_err());
    assert!(crate::usd::parse_usdz(b"PK\x03\x04truncated").is_err());

    // Deeply nested Xforms past MAX_USD_DEPTH must return Err without stack overflow.
    let mut deep = String::from("#usda 1.0\n");
    for i in 0..200 {
        deep.push_str(&format!("def Xform \"N{i}\" {{\n"));
    }
    for _ in 0..200 {
        deep.push_str("}\n");
    }
    assert!(crate::usd::parse_usda(deep.as_bytes(), &|_| None).is_err());

    // Non-finite numbers, negative/out-of-range face indices, unterminated tokens.
    let bad_cases = [
        "#usda 1.0\ndef Mesh \"M\" { point3f[] points = [(nan, 0, 0)] }",
        "#usda 1.0\ndef Mesh \"M\" { asset inputs:file = @unterminated }",
        "#usda 1.0\ndef Mesh \"M\" { rel material:binding = </unterminated\n }",
    ];
    for c in bad_cases {
        assert!(crate::usd::parse_usda(c.as_bytes(), &|_| None).is_err(), "{c}");
    }

    // Out-of-bounds faceVertexIndices are safely skipped rather than panicking.
    let oob_mesh = r#"#usda 1.0
def Mesh "Broken" {
    point3f[] points = [(0, 0, 0), (1, 0, 0), (0, 1, 0)]
    int[] faceVertexCounts = [3, 3]
    int[] faceVertexIndices = [0, 1, 2, -5, 999999, 0]
}"#;
    let m = crate::usd::parse_usda(oob_mesh.as_bytes(), &|_| None).unwrap();
    assert_eq!(m.triangle_count(), 1);
}

#[test]
fn substance_presets_synthesize_unit_normals_bounded_pbr_maps_and_determinism() {
    use crate::substance::{SubstanceParams, SubstancePreset, generate};

    let params = SubstanceParams { resolution: 32, seed: 42, wear: 0.3, normal_scale: 1.0 };
    for preset in SubstancePreset::ALL {
        let parsed = SubstancePreset::parse(preset.id()).unwrap();
        assert_eq!(parsed, preset);
        assert_eq!(SubstancePreset::parse(preset.label()), Some(preset));

        let maps = generate(preset, &params);
        assert_eq!(maps.base_color.width, 32);
        assert_eq!(maps.metallic_roughness.data.len(), 32 * 32);
        assert_eq!(maps.normal.data.len(), 32 * 32);
        assert_eq!(maps.occlusion.data.len(), 32 * 32);

        for px in &maps.base_color.data {
            assert!(px.iter().all(|c| c.is_finite() && (0.0..=1.0).contains(c)), "{preset:?}: {px:?}");
        }
        for mr in &maps.metallic_roughness.data {
            assert!((0.04..=1.0).contains(&mr[1]), "{preset:?} roughness: {mr:?}");
            assert!((0.0..=1.0).contains(&mr[2]), "{preset:?} metallic: {mr:?}");
        }
        for ao in &maps.occlusion.data {
            assert!((0.0..=1.0).contains(&ao[0]), "{preset:?} ao: {ao:?}");
        }
        for n in &maps.normal.data {
            let [nx, ny, nz] = [n[0] * 2.0 - 1.0, n[1] * 2.0 - 1.0, n[2] * 2.0 - 1.0];
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            assert!((len - 1.0).abs() < 1e-3 && nz > 0.0, "{preset:?} non-unit normal: {nx},{ny},{nz} (len={len})");
        }

        // Exact determinism for identical parameters.
        let maps2 = generate(preset, &params);
        assert_eq!(maps, maps2);
    }

    // WeatheredCopper oxidation shifts metallic -> dielectric patina as wear increases.
    let fresh = generate(SubstancePreset::WeatheredCopper, &SubstanceParams { resolution: 32, wear: 0.0, ..params });
    let oxidized = generate(SubstancePreset::WeatheredCopper, &SubstanceParams { resolution: 32, wear: 0.95, ..params });
    let mean_metal = |m: &crate::substance::SubstancePbrMaps| m.metallic_roughness.data.iter().map(|p| p[2]).sum::<f32>() / m.metallic_roughness.data.len() as f32;
    assert!(mean_metal(&oxidized) < mean_metal(&fresh) - 0.25);
}
