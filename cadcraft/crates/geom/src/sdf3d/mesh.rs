//! Watertight Dual Surface Nets isosurface triangle mesh extraction and `.stl` / `.obj` writers.

use serde::{Deserialize, Serialize};

use super::Solid3dNode;
use crate::Vec3;

/// Minimum and maximum grid resolution along the longest bounding-box axis.
pub const MIN_MESH_RESOLUTION: usize = 6;
pub const MAX_MESH_RESOLUTION: usize = 96;

const CELL_EDGES: [(usize, usize); 12] = [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)];

/// Indexed 3D triangle mesh with per-vertex surface normals.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TriMesh3d {
    pub vertices: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub triangles: Vec<[u32; 3]>,
}

struct GridSpec {
    origin: Vec3,
    step: f64,
    nx: usize,
    ny: usize,
    nz: usize,
}

impl GridSpec {
    fn from_solid(solid: &Solid3dNode, resolution: usize) -> Option<Self> {
        let bounds = solid.bounds();
        if !bounds.is_finite() {
            return None;
        }
        let res = resolution.clamp(MIN_MESH_RESOLUTION, MAX_MESH_RESOLUTION);
        let max_ext = bounds.max_extent().max(1e-3);
        let step = max_ext / (res.saturating_sub(2).max(4) as f64);
        if !step.is_finite() || step <= 0.0 {
            return None;
        }
        let padded = bounds.padded(step * 2.0);
        let size = padded.size();
        let axis_samples = |len: f64| ((len / step).ceil() as usize + 1).clamp(MIN_MESH_RESOLUTION, MAX_MESH_RESOLUTION + 5);
        Some(Self { origin: padded.min, step, nx: axis_samples(size.x), ny: axis_samples(size.y), nz: axis_samples(size.z) })
    }

    fn point(&self, ix: usize, iy: usize, iz: usize) -> Vec3 {
        Vec3::new(self.origin.x + ix as f64 * self.step, self.origin.y + iy as f64 * self.step, self.origin.z + iz as f64 * self.step)
    }

    fn sample_idx(&self, ix: usize, iy: usize, iz: usize) -> usize {
        (iz * self.ny + iy) * self.nx + ix
    }

    fn cell_idx(&self, cx: usize, cy: usize, cz: usize) -> usize {
        let cnx = self.nx - 1;
        let cny = self.ny - 1;
        (cz * cny + cy) * cnx + cx
    }
}

/// Extract a closed, watertight 2-manifold triangle mesh of `solid`'s `f(p) = 0` isosurface.
pub fn extract_isosurface(solid: &Solid3dNode, resolution: usize) -> TriMesh3d {
    let Some(grid) = GridSpec::from_solid(solid, resolution) else {
        return TriMesh3d::default();
    };
    let samples = sample_sdf_grid(solid, &grid);
    let mut mesh = TriMesh3d::default();
    let cell_verts = build_cell_vertices(solid, &grid, &samples, &mut mesh);
    emit_x_edge_quads(&grid, &samples, &cell_verts, &mut mesh.triangles);
    emit_y_edge_quads(&grid, &samples, &cell_verts, &mut mesh.triangles);
    emit_z_edge_quads(&grid, &samples, &cell_verts, &mut mesh.triangles);
    mesh
}

fn sample_sdf_grid(solid: &Solid3dNode, grid: &GridSpec) -> Vec<f64> {
    let mut samples = vec![1.0; grid.nx * grid.ny * grid.nz];
    for iz in 0..grid.nz {
        for iy in 0..grid.ny {
            for ix in 0..grid.nx {
                let idx = grid.sample_idx(ix, iy, iz);
                if let Some(slot) = samples.get_mut(idx) {
                    *slot = solid.eval(grid.point(ix, iy, iz));
                }
            }
        }
    }
    samples
}

fn corner_offset(corner: usize) -> (usize, usize, usize) {
    (corner & 1, (corner >> 1) & 1, (corner >> 2) & 1)
}

fn build_cell_vertices(solid: &Solid3dNode, grid: &GridSpec, samples: &[f64], mesh: &mut TriMesh3d) -> Vec<Option<u32>> {
    let (cnx, cny, cnz) = (grid.nx - 1, grid.ny - 1, grid.nz - 1);
    let mut cell_verts = vec![None; cnx * cny * cnz];
    let eps = grid.step * 0.02;
    for cz in 0..cnz {
        for cy in 0..cny {
            for cx in 0..cnx {
                if let Some(pos) = cell_surface_point(solid, grid, samples, (cx, cy, cz))
                    && let Ok(v_idx) = u32::try_from(mesh.vertices.len())
                    && let Some(slot) = cell_verts.get_mut(grid.cell_idx(cx, cy, cz))
                {
                    mesh.vertices.push(pos);
                    mesh.normals.push(solid.normal(pos, eps));
                    *slot = Some(v_idx);
                }
            }
        }
    }
    cell_verts
}

fn cell_surface_point(solid: &Solid3dNode, grid: &GridSpec, samples: &[f64], (cx, cy, cz): (usize, usize, usize)) -> Option<Vec3> {
    let mut vals = [0.0; 8];
    let mut pts = [Vec3::ZERO; 8];
    let (mut any_in, mut any_out) = (false, false);
    for corner in 0..8 {
        let (dx, dy, dz) = corner_offset(corner);
        let s = samples.get(grid.sample_idx(cx + dx, cy + dy, cz + dz)).copied().unwrap_or(1.0);
        if s < 0.0 {
            any_in = true;
        } else {
            any_out = true;
        }
        vals[corner] = s;
        pts[corner] = grid.point(cx + dx, cy + dy, cz + dz);
    }
    if !(any_in && any_out) {
        return None;
    }
    let mut sum = Vec3::ZERO;
    let mut crossings = 0u32;
    for &(a, b) in &CELL_EDGES {
        let (sa, sb) = (vals[a], vals[b]);
        if (sa < 0.0) != (sb < 0.0) {
            let denom = (sa - sb).abs().max(1e-15);
            let t = (sa.abs() / denom).clamp(0.0, 1.0);
            sum = sum + pts[a] + (pts[b] - pts[a]) * t;
            crossings += 1;
        }
    }
    if crossings == 0 {
        return None;
    }
    let init = sum * (1.0 / f64::from(crossings));
    Some(project_onto_isosurface(solid, init, pts[0], pts[7], grid.step))
}

fn project_onto_isosurface(solid: &Solid3dNode, start: Vec3, cell_min: Vec3, cell_max: Vec3, step: f64) -> Vec3 {
    let eps = step * 0.02;
    let margin = step * 0.25;
    let lo = cell_min - Vec3::new(margin, margin, margin);
    let hi = cell_max + Vec3::new(margin, margin, margin);
    let mut p = start;
    for _ in 0..3 {
        let d = solid.eval(p);
        if d.abs() <= step * 1e-4 {
            break;
        }
        let g = solid.gradient(p, eps);
        let glen2 = g.dot(g);
        if glen2 > 1e-12 {
            let delta = (d / glen2).clamp(-step * 0.6, step * 0.6);
            let next = p - g * delta;
            p = Vec3::new(next.x.clamp(lo.x, hi.x), next.y.clamp(lo.y, hi.y), next.z.clamp(lo.z, hi.z));
        }
    }
    p
}

fn emit_quad(quad: [Option<u32>; 4], flip: bool, triangles: &mut Vec<[u32; 3]>) {
    let [Some(c00), Some(c10), Some(c11), Some(c01)] = quad else {
        return;
    };
    if flip {
        triangles.push([c00, c11, c10]);
        triangles.push([c00, c01, c11]);
    } else {
        triangles.push([c00, c10, c11]);
        triangles.push([c00, c11, c01]);
    }
}

fn emit_x_edge_quads(grid: &GridSpec, samples: &[f64], cell_verts: &[Option<u32>], triangles: &mut Vec<[u32; 3]>) {
    for iz in 1..grid.nz - 1 {
        for iy in 1..grid.ny - 1 {
            for ix in 0..grid.nx - 1 {
                let sa = samples.get(grid.sample_idx(ix, iy, iz)).copied().unwrap_or(1.0);
                let sb = samples.get(grid.sample_idx(ix + 1, iy, iz)).copied().unwrap_or(1.0);
                if (sa < 0.0) != (sb < 0.0) {
                    let quad = [
                        cell_verts.get(grid.cell_idx(ix, iy - 1, iz - 1)).copied().flatten(),
                        cell_verts.get(grid.cell_idx(ix, iy, iz - 1)).copied().flatten(),
                        cell_verts.get(grid.cell_idx(ix, iy, iz)).copied().flatten(),
                        cell_verts.get(grid.cell_idx(ix, iy - 1, iz)).copied().flatten(),
                    ];
                    emit_quad(quad, sa >= 0.0, triangles);
                }
            }
        }
    }
}

fn emit_y_edge_quads(grid: &GridSpec, samples: &[f64], cell_verts: &[Option<u32>], triangles: &mut Vec<[u32; 3]>) {
    for iz in 1..grid.nz - 1 {
        for iy in 0..grid.ny - 1 {
            for ix in 1..grid.nx - 1 {
                let sa = samples.get(grid.sample_idx(ix, iy, iz)).copied().unwrap_or(1.0);
                let sb = samples.get(grid.sample_idx(ix, iy + 1, iz)).copied().unwrap_or(1.0);
                if (sa < 0.0) != (sb < 0.0) {
                    let quad = [
                        cell_verts.get(grid.cell_idx(ix - 1, iy, iz - 1)).copied().flatten(),
                        cell_verts.get(grid.cell_idx(ix - 1, iy, iz)).copied().flatten(),
                        cell_verts.get(grid.cell_idx(ix, iy, iz)).copied().flatten(),
                        cell_verts.get(grid.cell_idx(ix, iy, iz - 1)).copied().flatten(),
                    ];
                    emit_quad(quad, sa >= 0.0, triangles);
                }
            }
        }
    }
}

fn emit_z_edge_quads(grid: &GridSpec, samples: &[f64], cell_verts: &[Option<u32>], triangles: &mut Vec<[u32; 3]>) {
    for iz in 0..grid.nz - 1 {
        for iy in 1..grid.ny - 1 {
            for ix in 1..grid.nx - 1 {
                let sa = samples.get(grid.sample_idx(ix, iy, iz)).copied().unwrap_or(1.0);
                let sb = samples.get(grid.sample_idx(ix, iy, iz + 1)).copied().unwrap_or(1.0);
                if (sa < 0.0) != (sb < 0.0) {
                    let quad = [
                        cell_verts.get(grid.cell_idx(ix - 1, iy - 1, iz)).copied().flatten(),
                        cell_verts.get(grid.cell_idx(ix, iy - 1, iz)).copied().flatten(),
                        cell_verts.get(grid.cell_idx(ix, iy, iz)).copied().flatten(),
                        cell_verts.get(grid.cell_idx(ix - 1, iy, iz)).copied().flatten(),
                    ];
                    emit_quad(quad, sa >= 0.0, triangles);
                }
            }
        }
    }
}

fn sanitize_mesh_name(name: &str) -> String {
    let cleaned: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-')).take(64).collect();
    if cleaned.is_empty() { "solid".to_string() } else { cleaned }
}

fn tri_positions(mesh: &TriMesh3d, tri: [u32; 3]) -> Option<[Vec3; 3]> {
    let v0 = *mesh.vertices.get(tri[0] as usize)?;
    let v1 = *mesh.vertices.get(tri[1] as usize)?;
    let v2 = *mesh.vertices.get(tri[2] as usize)?;
    Some([v0, v1, v2])
}

/// Format `mesh` as an ASCII `.stl` solid.
pub fn write_stl_ascii(name: &str, mesh: &TriMesh3d) -> String {
    use std::fmt::Write as _;
    let safe = sanitize_mesh_name(name);
    let mut out = String::with_capacity(64 + mesh.triangles.len() * 180);
    let _ = writeln!(out, "solid {safe}");
    for &tri in &mesh.triangles {
        let Some([v0, v1, v2]) = tri_positions(mesh, tri) else { continue };
        let n = (v1 - v0).cross(v2 - v0).normalized();
        let _ = writeln!(out, "  facet normal {:.6} {:.6} {:.6}", n.x, n.y, n.z);
        let _ = writeln!(out, "    outer loop");
        for v in [v0, v1, v2] {
            let _ = writeln!(out, "      vertex {:.6} {:.6} {:.6}", v.x, v.y, v.z);
        }
        let _ = writeln!(out, "    endloop");
        let _ = writeln!(out, "  endfacet");
    }
    let _ = writeln!(out, "endsolid {safe}");
    out
}

/// Encode `mesh` as a binary `.stl` byte stream (80-byte header + u32 count + 50-byte facets).
pub fn write_stl_binary(header: &str, mesh: &TriMesh3d) -> Vec<u8> {
    let mut valid_tris = Vec::with_capacity(mesh.triangles.len());
    for &tri in &mesh.triangles {
        if let Some(verts) = tri_positions(mesh, tri) {
            valid_tris.push(verts);
        }
    }
    let count = u32::try_from(valid_tris.len()).unwrap_or(u32::MAX);
    let kept = count as usize;
    let mut bytes = Vec::with_capacity(84 + kept * 50);
    let mut hdr = [0u8; 80];
    // Never start a binary STL header with "solid" so strict parsers recognize binary STL unambiguously.
    let label = format!("CADCraft Binary STL: {}", sanitize_mesh_name(header));
    let src = label.as_bytes();
    let copy_len = src.len().min(80);
    hdr[..copy_len].copy_from_slice(&src[..copy_len]);
    bytes.extend_from_slice(&hdr);
    bytes.extend_from_slice(&count.to_le_bytes());
    for [v0, v1, v2] in valid_tris.into_iter().take(kept) {
        let n = (v1 - v0).cross(v2 - v0).normalized();
        for vec in [n, v0, v1, v2] {
            bytes.extend_from_slice(&(vec.x as f32).to_le_bytes());
            bytes.extend_from_slice(&(vec.y as f32).to_le_bytes());
            bytes.extend_from_slice(&(vec.z as f32).to_le_bytes());
        }
        bytes.extend_from_slice(&0u16.to_le_bytes());
    }
    bytes
}

/// Format `mesh` as a Wavefront `.obj` mesh with vertex normals.
pub fn write_obj(name: &str, mesh: &TriMesh3d) -> String {
    use std::fmt::Write as _;
    let safe = sanitize_mesh_name(name);
    let mut out = String::with_capacity(128 + mesh.vertices.len() * 64 + mesh.triangles.len() * 36);
    let _ = writeln!(out, "# CADCraft 3D Mesh");
    let _ = writeln!(out, "o {safe}");
    for v in &mesh.vertices {
        let _ = writeln!(out, "v {:.6} {:.6} {:.6}", v.x, v.y, v.z);
    }
    for idx in 0..mesh.vertices.len() {
        let n = mesh.normals.get(idx).copied().unwrap_or(Vec3::Z);
        let _ = writeln!(out, "vn {:.6} {:.6} {:.6}", n.x, n.y, n.z);
    }
    let n_verts = mesh.vertices.len();
    for &[a, b, c] in &mesh.triangles {
        let (ia, ib, ic) = (a as usize + 1, b as usize + 1, c as usize + 1);
        if ia <= n_verts && ib <= n_verts && ic <= n_verts {
            let _ = writeln!(out, "f {ia}//{ia} {ib}//{ib} {ic}//{ic}");
        }
    }
    out
}
