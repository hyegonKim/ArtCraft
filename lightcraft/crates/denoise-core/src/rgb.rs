//! Non-Raw Denoise (Camera Raw 19.0 / Lightroom 2027): collaborative 3-D transform-domain
//! filtering (`CBM3D` + Anscombe variance stabilisation) directly on 3-channel linear RGB images
//! (JPEG, PNG, TIFF, WebP, HEIC/AVIF and linear DNG merges).
//!
//! Measures the Poisson-Gaussian noise model `variance = a · signal + b` across the image's RGB
//! planes, processes overlapping `256 × 256` tiles in parallel on variance-stabilised `[R, G, G, B]`
//! planes (whose Hadamard rotation separates luminance `0.5·(R + 2G + B)` from orthogonal chroma
//! differences), protects clipped/HDR values, and blends tiles seamlessly.

use std::sync::atomic::{AtomicUsize, Ordering};

use lightcraft_raster::Rgb32f;
use rayon::prelude::*;

use crate::bayer::reflect;
use crate::bm3d::{self, Params as Bm3dParams};
use crate::nlm::Stabiliser;
use crate::run::{Control, Error, MAX_PIXELS};
use crate::tiles::{starts, weight};

const RGB_TILE: usize = 256;
const RGB_OVERLAP: usize = 16;
const CLIP_PROTECT: f32 = 0.99;

/// Estimate Poisson-Gaussian noise `(a, b)` across a 3-channel linear RGB image using a 2-D
/// Laplacian (`[1 -2 1; -2 4 -2; 1 -2 1]`, variance `36 σ²` on white noise) over `16 × 16` blocks,
/// taking lower-quartile block variances across brightness bins so texture and edges do not bias it.
pub fn estimate_rgb_noise(img: &Rgb32f) -> (f32, f32) {
    let (w, h) = (img.width, img.height);
    if w < 18 || h < 18 {
        return (1e-4, 1e-5);
    }
    const BLOCK: usize = 16;
    let mut samples: Vec<(f32, f32)> = Vec::new();
    for ch in 0..3 {
        let at = |x: usize, y: usize| -> f32 {
            let v = img.data[y * w + x][ch];
            if v.is_finite() { v.max(0.0) } else { 0.0 }
        };
        let mut by = 1;
        while by + BLOCK < h {
            let mut bx = 1;
            while bx + BLOCK < w {
                let mut sum = 0.0f64;
                let mut sq = 0.0f64;
                for y in by..by + BLOCK {
                    for x in bx..bx + BLOCK {
                        let c = at(x, y);
                        let lap = (at(x - 1, y - 1) + at(x + 1, y - 1) + at(x - 1, y + 1) + at(x + 1, y + 1))
                            - 2.0 * (at(x, y - 1) + at(x, y + 1) + at(x - 1, y) + at(x + 1, y))
                            + 4.0 * c;
                        sum += f64::from(c);
                        sq += f64::from(lap * lap);
                    }
                }
                let n = (BLOCK * BLOCK) as f64;
                let mean = (sum / n) as f32;
                let var = (sq / n / 36.0) as f32;
                if mean.is_finite() && var.is_finite() && (0.002..0.90).contains(&mean) {
                    samples.push((mean, var.max(1e-10)));
                }
                bx += BLOCK;
            }
            by += BLOCK;
        }
    }
    if samples.len() < 4 {
        return (1e-4, 1e-5);
    }
    samples.sort_by(|a, b| a.0.total_cmp(&b.0));
    let bins = 8.min(samples.len());
    let mut pts: Vec<(f64, f64)> = Vec::with_capacity(bins);
    for b_idx in 0..bins {
        let lo = b_idx * samples.len() / bins;
        let hi = ((b_idx + 1) * samples.len() / bins).max(lo + 1);
        let mut slice: Vec<(f32, f32)> = samples[lo..hi].to_vec();
        slice.sort_by(|a, b| a.1.total_cmp(&b.1));
        // 20th percentile block variance tracks noise floor rather than scene texture/JPEG edges
        let pick = slice[(slice.len() / 5).min(slice.len() - 1)];
        pts.push((f64::from(pick.0), f64::from(pick.1)));
    }
    let n = pts.len() as f64;
    let mx = pts.iter().map(|p| p.0).sum::<f64>() / n;
    let my = pts.iter().map(|p| p.1).sum::<f64>() / n;
    let mut num = 0.0f64;
    let mut den = 0.0f64;
    for (x, y) in &pts {
        num += (x - mx) * (y - my);
        den += (x - mx) * (x - mx);
    }
    let slope = if den > 1e-12 { (num / den).max(0.0) } else { 0.0 };
    let intercept = (my - slope * mx).max(1e-10);
    // Modest floor so quantized 8-bit JPEGs with flat blocks still clean compression mosquito/chroma noise
    let a = (slope as f32).clamp(0.0, 0.25);
    let b = (intercept as f32).clamp(1e-8, 0.05);
    (a, b)
}

/// Denoise a 3-channel linear RGB image (`Rgb32f`) using collaborative 3-D block-matching (`CBM3D`)
/// over overlapping `256 × 256` tiles.
pub fn denoise_rgb(img: &Rgb32f, parallel: usize, ctl: &Control<'_>) -> Result<Rgb32f, Error> {
    let (w, h) = (img.width, img.height);
    let pixels = w
        .checked_mul(h)
        .filter(|&n| n > 0 && n <= MAX_PIXELS)
        .ok_or_else(|| Error::Input("the picture has no size, or is too large".into()))?;
    if img.data.len() != pixels {
        return Err(Error::Input("pixel buffer size does not match width × height".into()));
    }
    if ctl.cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
        return Err(Error::Cancelled);
    }

    let (a, b) = estimate_rgb_noise(img);
    let st = Stabiliser::new(a, b);
    // Boost chroma/mosquito cleaning slightly on rendered images while keeping sharp edges via Wiener step:
    let noise_mid = ((a * 0.18 + b).max(1e-7).sqrt() * 1.15).clamp(0.002, 0.20);
    let bm3d_params = Bm3dParams::default();

    let sx = starts(w, RGB_TILE, RGB_OVERLAP);
    let sy = starts(h, RGB_TILE, RGB_OVERLAP);
    let total = sx.len().saturating_mul(sy.len());
    if let Some(report) = ctl.progress {
        report(0, total);
    }

    let mut coords = Vec::with_capacity(total);
    for (iy, &y0) in sy.iter().enumerate() {
        for (ix, &x0) in sx.iter().enumerate() {
            coords.push((ix, iy, x0, y0));
        }
    }

    let mut acc = vec![[0.0f32; 3]; pixels];
    let mut wsum = vec![0.0f32; pixels];
    let done = AtomicUsize::new(0);
    let chunk_size = parallel.clamp(1, 16);

    for batch in coords.chunks(chunk_size) {
        if ctl.cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            return Err(Error::Cancelled);
        }
        let tile_results: Result<Vec<(usize, usize, usize, usize, Vec<[f32; 3]>)>, Error> = batch
            .par_iter()
            .map(|&(ix, iy, x0, y0)| {
                if ctl.cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
                    return Err(Error::Cancelled);
                }
                let n_tile = RGB_TILE * RGB_TILE;
                let mut planes = vec![vec![0.0f32; n_tile]; 4];
                for ty in 0..RGB_TILE {
                    let sy_p = reflect(y0 as isize + ty as isize, h);
                    for tx in 0..RGB_TILE {
                        let sx_p = reflect(x0 as isize + tx as isize, w);
                        let c = img.data[sy_p * w + sx_p];
                        let clean = c.map(|v| if v.is_finite() { v.max(0.0) } else { 0.0 });
                        let k = ty * RGB_TILE + tx;
                        let r_s = st.forward(clean[0]);
                        let g_s = st.forward(clean[1]);
                        let b_s = st.forward(clean[2]);
                        planes[0][k] = r_s;
                        planes[1][k] = g_s;
                        planes[2][k] = g_s;
                        planes[3][k] = b_s;
                    }
                }
                let filtered = bm3d::filter(&planes, RGB_TILE, &bm3d_params, noise_mid);
                let mut out_tile = vec![[0.0f32; 3]; n_tile];
                for ty in 0..RGB_TILE {
                    let sy_p = reflect(y0 as isize + ty as isize, h);
                    for tx in 0..RGB_TILE {
                        let sx_p = reflect(x0 as isize + tx as isize, w);
                        let k = ty * RGB_TILE + tx;
                        let orig = img.data[sy_p * w + sx_p];
                        let r = st.inverse(filtered[0][k]).max(0.0);
                        let g = (0.5 * (st.inverse(filtered[1][k]) + st.inverse(filtered[2][k]))).max(0.0);
                        let b_ch = st.inverse(filtered[3][k]).max(0.0);
                        // Keep clipped / extended HDR highlights untouched:
                        out_tile[k] = [
                            if orig[0] >= CLIP_PROTECT { orig[0] } else { r },
                            if orig[1] >= CLIP_PROTECT { orig[1] } else { g },
                            if orig[2] >= CLIP_PROTECT { orig[2] } else { b_ch },
                        ];
                    }
                }
                let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                if let Some(report) = ctl.progress {
                    report(d, total);
                }
                Ok((ix, iy, x0, y0, out_tile))
            })
            .collect();

        for (ix, iy, x0, y0, out_tile) in tile_results? {
            let has_left = ix > 0;
            let has_right = ix + 1 < sx.len();
            let has_top = iy > 0;
            let has_bottom = iy + 1 < sy.len();
            let valid_w = RGB_TILE.min(w.saturating_sub(x0));
            let valid_h = RGB_TILE.min(h.saturating_sub(y0));
            for ty in 0..valid_h {
                let wy = weight(ty, valid_h, RGB_OVERLAP, has_top, has_bottom);
                let dst_row = (y0 + ty) * w;
                for tx in 0..valid_w {
                    let wt = wy * weight(tx, valid_w, RGB_OVERLAP, has_left, has_right);
                    let dst = dst_row + x0 + tx;
                    let c = out_tile[ty * RGB_TILE + tx];
                    acc[dst][0] += c[0] * wt;
                    acc[dst][1] += c[1] * wt;
                    acc[dst][2] += c[2] * wt;
                    wsum[dst] += wt;
                }
            }
        }
    }

    for (px, &wt) in acc.iter_mut().zip(&wsum) {
        if wt > 0.0 {
            let inv = 1.0 / wt;
            px[0] *= inv;
            px[1] *= inv;
            px[2] *= inv;
        }
    }
    Ok(Rgb32f { width: w, height: h, data: acc })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denoise_rgb_reduces_synthetic_gaussian_noise_and_preserves_edges() {
        let clean = Rgb32f::from_fn(96, 80, |x, _y| if x < 48 { [0.15, 0.18, 0.22] } else { [0.55, 0.50, 0.44] });
        let noisy = Rgb32f::from_fn(96, 80, |x, y| {
            let base = clean.data[y * 96 + x];
            let h = ((x as u32).wrapping_mul(747796405) ^ (y as u32).wrapping_mul(2891336453)).wrapping_mul(277803737);
            let n0 = ((h & 0xffff) as f32 / 32768.0 - 1.0) * 0.045;
            let n1 = (((h >> 8) & 0xffff) as f32 / 32768.0 - 1.0) * 0.045;
            let n2 = (((h >> 16) & 0xffff) as f32 / 32768.0 - 1.0) * 0.045;
            [(base[0] + n0).clamp(0.0, 1.0), (base[1] + n1).clamp(0.0, 1.0), (base[2] + n2).clamp(0.0, 1.0)]
        });
        let mse = |a: &Rgb32f, b: &Rgb32f| -> f64 {
            a.data
                .iter()
                .zip(&b.data)
                .flat_map(|(p, q)| (0..3).map(move |c| f64::from(p[c] - q[c]).powi(2)))
                .sum::<f64>()
                / (a.data.len() * 3) as f64
        };
        let before = mse(&noisy, &clean);
        let denoised = denoise_rgb(&noisy, 2, &Control::default()).unwrap();
        let after = mse(&denoised, &clean);
        assert!(after < before * 0.45, "RGB CBM3D reduced MSE substantially: {before:.6} -> {after:.6}");
        // Step edge contrast across x=48 preserved:
        assert!(denoised.data[40 * 96 + 60][0] - denoised.data[40 * 96 + 36][0] > 0.32);
    }
}
