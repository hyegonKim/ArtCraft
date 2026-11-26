//! 3D Material Override (After Effects 27.0): overrides the surface material of 3D Text, 3D
//! Shapes, Parametric Meshes, and 3D Models from any timeline layer (footage, precomp, solid,
//! shape, text) or built-in procedural PBR preset, with Uniform Tiling and Randomize Offset.

use std::collections::HashSet;
use std::sync::Arc;

use effectcraft_effects::Buf;
use effectcraft_geom::{Mat4, vec3};
use effectcraft_model::Primitive;
use effectcraft_project::{Layer, LayerId, LayerSource, MaterialOverride, ProceduralMaterial};
use effectcraft_raster::Image;

use super::adv::shade::srgb_to_linear;
use crate::{EvalCtx, Renderer};

/// Resolved material override texture and PBR parameters ready for 3D shading.
pub(crate) struct ResolvedOverride {
    pub width: u32,
    pub height: u32,
    /// Straight-alpha RGBA `[0..=1]`. In linear light when `already_linear` is true, else sRGB.
    pub texels: Vec<[f32; 4]>,
    pub already_linear: bool,
    pub opaque: bool,
    pub metallic: f32,
    pub roughness: f32,
}

/// Whether `layer` supports 3D Material Override and currently has an active override.
pub(crate) fn active_override(layer: &Layer) -> Option<&MaterialOverride> {
    if !matches!(
        layer.source,
        LayerSource::Text | LayerSource::Shape | LayerSource::Primitive { .. } | LayerSource::Model { .. }
    ) {
        return None;
    }
    layer.material_override.as_ref().filter(|ov| ov.is_active())
}

/// Guard against direct (`A -> A`) or transitive (`A -> B -> A`) material-override cycles in `ctx.comp`.
fn has_override_cycle(ctx: &EvalCtx, start: LayerId, target: LayerId) -> bool {
    if start == target {
        return true;
    }
    let mut visited = HashSet::new();
    visited.insert(start);
    let mut cur = Some(target);
    while let Some(id) = cur {
        if !visited.insert(id) {
            return true;
        }
        cur = ctx
            .comp
            .layer(id)
            .and_then(|l| l.material_override.as_ref())
            .filter(|ov| ov.enabled)
            .and_then(|ov| ov.source_layer);
    }
    false
}

/// Resolves `layer`'s active [`MaterialOverride`] into straight-alpha texels and PBR weights.
pub(crate) fn resolve(r: &Renderer, ctx: &EvalCtx, layer: &Layer) -> Option<(MaterialOverride, ResolvedOverride)> {
    let ov = active_override(layer)?.clone();
    let metallic = (ov.metallic as f32).clamp(0.0, 1.0);
    let roughness = (ov.roughness as f32).clamp(0.04, 1.0);

    if let Some(src_id) = ov.source_layer {
        if has_override_cycle(ctx, layer.id, src_id) {
            return None;
        }
        let src_layer = ctx.comp.layer(src_id)?;
        let buf = r.blend_layer_buf(ctx, src_layer)?;
        if buf.img.width == 0 || buf.img.height == 0 {
            return None;
        }
        let mut opaque = true;
        let texels = buf
            .img
            .data
            .iter()
            .map(|p| {
                let a = p[3].clamp(0.0, 1.0);
                if a < 0.999 {
                    opaque = false;
                }
                if a <= 1e-6 {
                    [0.0, 0.0, 0.0, 0.0]
                } else {
                    [p[0] / a, p[1] / a, p[2] / a, a]
                }
            })
            .collect();
        let already_linear = r.pipe.linear || r.pipe.linear_blend;
        return Some((
            ov,
            ResolvedOverride {
                width: buf.img.width,
                height: buf.img.height,
                texels,
                already_linear,
                opaque,
                metallic,
                roughness,
            },
        ));
    }

    let proc_kind = ov.procedural?;
    let (texels, opaque) = synthesize_procedural(proc_kind, ov.random_seed);
    Some((
        ov,
        ResolvedOverride {
            width: PROC_SIZE,
            height: PROC_SIZE,
            texels,
            already_linear: false,
            opaque,
            metallic,
            roughness,
        },
    ))
}

const PROC_SIZE: u32 = 128;

fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

fn hash_unit(seed: u32, a: u32, b: u32) -> f32 {
    let h = hash_u32(seed ^ a.wrapping_mul(0x9e37_79b9) ^ b.wrapping_mul(0x85eb_ca6b));
    (h as f32) * (1.0 / 4_294_967_296.0)
}

/// Deterministic UV phase shift in `[0, 1)` when `randomize_offset` is enabled.
pub(crate) fn uv_offset(ov: &MaterialOverride, sub_index: u32) -> [f32; 2] {
    if !ov.randomize_offset {
        return [0.0, 0.0];
    }
    [
        hash_unit(ov.random_seed, sub_index, 0x1357_9bdf),
        hash_unit(ov.random_seed, sub_index, 0x2468_ace0),
    ]
}

fn synthesize_procedural(kind: ProceduralMaterial, seed: u32) -> (Vec<[f32; 4]>, bool) {
    let n = PROC_SIZE;
    let mut out = Vec::with_capacity((n * n) as usize);
    let mut opaque = true;
    let tau = std::f32::consts::TAU;
    for y in 0..n {
        for x in 0..n {
            let u = x as f32 / n as f32;
            let v = y as f32 / n as f32;
            let px = match kind {
                ProceduralMaterial::BrushedMetal => {
                    let row_n = hash_unit(seed, y, 17) * 2.0 - 1.0;
                    let streak = 0.5 + 0.5 * (u * tau * 3.0 + row_n * 4.5).sin();
                    let fine = hash_unit(seed, x / 8, y) * 0.06;
                    let g = (0.70 + 0.16 * streak + fine).clamp(0.0, 1.0);
                    [g * 0.97, g * 0.99, (g * 1.03).clamp(0.0, 1.0), 1.0]
                }
                ProceduralMaterial::CarbonFiber => {
                    let cx = (x / 4) % 4;
                    let cy = (y / 4) % 4;
                    let warp = ((cx + cy) % 4) < 2;
                    let local = if warp { (x % 4) as f32 / 3.0 } else { (y % 4) as f32 / 3.0 };
                    let ridge = 1.0 - (local * 2.0 - 1.0).abs();
                    let base = if warp { 0.11 + 0.13 * ridge } else { 0.07 + 0.10 * ridge };
                    [base, base * 1.03, base * 1.08, 1.0]
                }
                ProceduralMaterial::FrostedGlass => {
                    opaque = false;
                    let n1 = hash_unit(seed, x / 2, y / 2);
                    let n2 = hash_unit(seed ^ 0x55aa, (x + 1) / 4, (y + 1) / 4);
                    let frost = 0.84 + 0.14 * (0.6 * n1 + 0.4 * n2);
                    [frost * 0.92, frost * 0.97, frost.clamp(0.0, 1.0), 0.68]
                }
                ProceduralMaterial::MattePlaster => {
                    let g1 = hash_unit(seed, x / 2, y / 2) - 0.5;
                    let g2 = (u * tau * 2.0).cos() * (v * tau * 2.0).sin() * 0.025;
                    let l = (0.89 + 0.07 * g1 + g2).clamp(0.0, 1.0);
                    [l * 1.01, l * 0.99, l * 0.95, 1.0]
                }
                ProceduralMaterial::GoldFoil => {
                    let crinkle = ((u * tau * 5.0 + (v * tau * 3.0).sin()).cos()
                        * (v * tau * 4.0 - (u * tau * 2.0).cos()).sin())
                        * 0.12;
                    let sparkle = (hash_unit(seed, x / 3, y / 3) - 0.5) * 0.08;
                    let k = (0.88 + crinkle + sparkle).clamp(0.55, 1.15);
                    [(1.0 * k).clamp(0.0, 1.0), (0.78 * k).clamp(0.0, 1.0), (0.26 * k).clamp(0.0, 1.0), 1.0]
                }
                ProceduralMaterial::Checkerboard => {
                    let cell = ((x / 16) + (y / 16)) % 2 == 0;
                    if cell { [0.92, 0.92, 0.94, 1.0] } else { [0.18, 0.20, 0.24, 1.0] }
                }
            };
            out.push(px);
        }
    }
    (out, opaque)
}

/// Converts [`ResolvedOverride`] texels to linear light when `scene_linear_io` is false.
pub(crate) fn linear_texels(res: &ResolvedOverride, scene_linear_io: bool) -> Vec<[f32; 4]> {
    if res.already_linear || scene_linear_io {
        res.texels.clone()
    } else {
        res.texels
            .iter()
            .map(|p| [srgb_to_linear(p[0]), srgb_to_linear(p[1]), srgb_to_linear(p[2]), p[3]])
            .collect()
    }
}

/// Remaps or generates UVs on a 3D [`Primitive`] for `ov`, accounting for `uniform_tiling`,
/// `tile_scale`, and per-submesh `randomize_offset`.
pub(crate) fn remap_primitive(
    p: &Primitive,
    world: &Mat4,
    ov: &MaterialOverride,
    res: &ResolvedOverride,
    sub_index: u32,
) -> Primitive {
    let mut out = p.clone();
    if out.positions.is_empty() {
        return out;
    }
    let inv_scale = (1.0 / ov.tile_scale.abs().clamp(0.01, 1000.0)) as f32;
    let tex_aspect = (res.width.max(1) as f32) / (res.height.max(1) as f32);
    let off = uv_offset(ov, sub_index);

    // Local & world bounding boxes for planar generation and uniform-tiling aspect correction.
    let mut lmin = [f32::INFINITY; 3];
    let mut lmax = [f32::NEG_INFINITY; 3];
    let mut wmin = [f32::INFINITY; 3];
    let mut wmax = [f32::NEG_INFINITY; 3];
    for q in &out.positions {
        let w = world.apply(vec3(q[0] as f64, q[1] as f64, q[2] as f64));
        let wp = [w.x as f32, w.y as f32, w.z as f32];
        for c in 0..3 {
            lmin[c] = lmin[c].min(q[c]);
            lmax[c] = lmax[c].max(q[c]);
            wmin[c] = wmin[c].min(wp[c]);
            wmax[c] = wmax[c].max(wp[c]);
        }
    }
    let lspan = [
        (lmax[0] - lmin[0]).max(1e-3),
        (lmax[1] - lmin[1]).max(1e-3),
        (lmax[2] - lmin[2]).max(1e-3),
    ];
    let wspan = [
        (wmax[0] - wmin[0]).max(1e-3),
        (wmax[1] - wmin[1]).max(1e-3),
        (wmax[2] - wmin[2]).max(1e-3),
    ];
    let wx = world.apply_vec(vec3(1.0, 0.0, 0.0)).length().max(1e-4) as f32;
    let wy = world.apply_vec(vec3(0.0, 1.0, 0.0)).length().max(1e-4) as f32;
    let wz = world.apply_vec(vec3(0.0, 0.0, 1.0)).length().max(1e-4) as f32;

    let has_uvs = out.uvs.len() == out.positions.len();
    let mut uvs = Vec::with_capacity(out.positions.len());
    let ref_unit = wspan[0].max(wspan[1]).max(256.0);

    for (k, q) in out.positions.iter().enumerate() {
        let (u, v) = if has_uvs {
            let base = out.uvs[k];
            if ov.uniform_tiling {
                let su = (wspan[0] / ref_unit) / tex_aspect;
                let sv = wspan[1].max(wspan[2]) / ref_unit;
                (base[0] * su * inv_scale, base[1] * sv * inv_scale)
            } else {
                (base[0] * inv_scale, base[1] * inv_scale)
            }
        } else {
            let n = out.normals.get(k).copied().unwrap_or([0.0, 0.0, -1.0]);
            let (ax, ay, az) = (n[0].abs(), n[1].abs(), n[2].abs());
            if ov.uniform_tiling {
                let (pu, pv) = if az >= ax && az >= ay {
                    ((q[0] - lmin[0]) * wx, (q[1] - lmin[1]) * wy)
                } else if ax >= ay {
                    ((q[2] - lmin[2]) * wz, (q[1] - lmin[1]) * wy)
                } else {
                    ((q[0] - lmin[0]) * wx, (q[2] - lmin[2]) * wz)
                };
                ((pu / (ref_unit * tex_aspect)) * inv_scale, (pv / ref_unit) * inv_scale)
            } else {
                let (nu, nv) = if az >= ax && az >= ay {
                    ((q[0] - lmin[0]) / lspan[0], (q[1] - lmin[1]) / lspan[1])
                } else if ax >= ay {
                    ((q[2] - lmin[2]) / lspan[2], (q[1] - lmin[1]) / lspan[1])
                } else {
                    ((q[0] - lmin[0]) / lspan[0], (q[2] - lmin[2]) / lspan[2])
                };
                (nu * inv_scale, nv * inv_scale)
            }
        };
        uvs.push([u + off[0], v + off[1]]);
    }
    out.uvs = uvs;
    out
}

fn sample_repeat(res: &ResolvedOverride, u: f32, v: f32, want_linear: bool) -> [f32; 4] {
    let w = res.width.max(1);
    let h = res.height.max(1);
    let fu = u.rem_euclid(1.0) * (w as f32) - 0.5;
    let fv = v.rem_euclid(1.0) * (h as f32) - 0.5;
    let x0 = (fu.floor() as i32).rem_euclid(w as i32) as u32;
    let y0 = (fv.floor() as i32).rem_euclid(h as i32) as u32;
    let x1 = (x0 + 1) % w;
    let y1 = (y0 + 1) % h;
    let tx = fu - fu.floor();
    let ty = fv - fv.floor();

    let fetch = |x: u32, y: u32| -> [f32; 4] {
        let p = res.texels[(y * w + x) as usize];
        if want_linear && !res.already_linear {
            [srgb_to_linear(p[0]), srgb_to_linear(p[1]), srgb_to_linear(p[2]), p[3]]
        } else {
            p
        }
    };
    let c00 = fetch(x0, y0);
    let c10 = fetch(x1, y0);
    let c01 = fetch(x0, y1);
    let c11 = fetch(x1, y1);
    let mut out = [0.0f32; 4];
    for i in 0..4 {
        let top = c00[i] * (1.0 - tx) + c10[i] * tx;
        let bot = c01[i] * (1.0 - tx) + c11[i] * tx;
        out[i] = top * (1.0 - ty) + bot * ty;
    }
    out
}

/// Applies [`ResolvedOverride`] onto a flat 3D Text or 3D Shape card buffer while preserving its
/// vector/glyph alpha silhouette.
pub(crate) fn modulate_card_buf(
    buf: &Buf,
    world: &Mat4,
    ov: &MaterialOverride,
    res: &ResolvedOverride,
    blend_linear: bool,
    sub_index: u32,
) -> Arc<Buf> {
    let w = buf.img.width;
    let h = buf.img.height;
    if w == 0 || h == 0 {
        return Arc::new(buf.clone());
    }
    let inv_scale = (1.0 / ov.tile_scale.abs().clamp(0.01, 1000.0)) as f32;
    let off = uv_offset(ov, sub_index);
    let s = buf.scale.max(1e-9) as f32;
    let wx = world.apply_vec(vec3(1.0, 0.0, 0.0)).length().max(1e-4) as f32;
    let wy = world.apply_vec(vec3(0.0, 1.0, 0.0)).length().max(1e-4) as f32;
    let tex_w = (res.width.max(1) as f32).max(1.0);
    let tex_h = (res.height.max(1) as f32).max(1.0);
    let ref_px = ((w as f32 / s) * wx).max((h as f32 / s) * wy).max(256.0);
    let tex_aspect = tex_w / tex_h;

    let mut data = Vec::with_capacity(buf.img.data.len());
    for y in 0..h {
        for x in 0..w {
            let p = buf.img.data[(y * w + x) as usize];
            let a = p[3];
            if a <= 1e-6 {
                data.push([0.0; 4]);
                continue;
            }
            let (u, v) = if ov.uniform_tiling {
                let lx = (x as f32 - buf.offset[0] as f32) / s * wx;
                let ly = (y as f32 - buf.offset[1] as f32) / s * wy;
                ((lx / (ref_px * tex_aspect)) * inv_scale, (ly / ref_px) * inv_scale)
            } else {
                ((x as f32 / w.max(1) as f32) * inv_scale, (y as f32 / h.max(1) as f32) * inv_scale)
            };
            let m = sample_repeat(res, u + off[0], v + off[1], blend_linear);
            let out_a = (a * m[3]).clamp(0.0, 1.0);
            data.push([m[0] * out_a, m[1] * out_a, m[2] * out_a, out_a]);
        }
    }
    Arc::new(Buf {
        img: Image { width: w, height: h, data },
        offset: buf.offset,
        scale: buf.scale,
    })
}
