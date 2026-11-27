//! Adobe Scan-style perspective rectification (`dewarp_quad_rgba`) and local background
//! illumination cleanup (`enhance_whiteboard_or_card`) for photographed documents, whiteboards,
//! and business cards.
//!
//! Pure Rust, deterministic, zero `unsafe`, and bounded on malformed or degenerate inputs.

/// Maximum output pixels produced by [`dewarp_quad_rgba`] (`~40 MP`).
const MAX_DEWARP_PIXELS: f64 = 40_000_000.0;

/// Target cleanup profile for [`enhance_whiteboard_or_card`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScanCleanupKind {
    /// Whiteboard photo: pushes uneven grey/tinted board illumination cleanly to white while
    /// preserving saturated dry-erase marker colours (red, blue, green, black).
    #[default]
    Whiteboard,
    /// Business card or paper note: evens out directional lighting and vignetting while keeping
    /// subtle graphic tones and crisp dark typography.
    Card,
}

/// Perspective-rectified RGBA8 image produced by [`dewarp_quad_rgba`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DewarpedRgba {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Rectify a 4-corner quadrilateral (`corners`, in source pixel coordinates, any corner order)
/// from an unpremultiplied RGBA8 image (`src_width` × `src_height`) into an upright rectangle.
///
/// When `dst_width == 0` or `dst_height == 0`, the output dimensions are inferred from the longer
/// opposite edges of the ordered quadrilateral (capped at 40 megapixels).
pub fn dewarp_quad_rgba(
    rgba: &[u8],
    src_width: u32,
    src_height: u32,
    corners: [[f64; 2]; 4],
    dst_width: u32,
    dst_height: u32,
) -> Result<DewarpedRgba, String> {
    let (sw, sh) = (src_width as usize, src_height as usize);
    if sw == 0 || sh == 0 || sw.checked_mul(sh).and_then(|n| n.checked_mul(4)) != Some(rgba.len()) {
        return Err("source RGBA buffer size does not match width × height × 4".to_string());
    }
    let [tl, tr, br, bl] = order_convex_quad(corners).ok_or_else(|| "corners must form a non-degenerate convex quadrilateral".to_string())?;

    let (ow, oh) = resolve_output_size([tl, tr, br, bl], dst_width, dst_height)?;
    let dst_rect = [[0.0, 0.0], [ow as f64, 0.0], [ow as f64, oh as f64], [0.0, oh as f64]];
    let homography =
        solve_homography(dst_rect, [tl, tr, br, bl]).ok_or_else(|| "singular homography: quadrilateral corners are collinear".to_string())?;

    let mut out = Vec::with_capacity(ow as usize * oh as usize * 4);
    for y in 0..oh {
        let dy = y as f64 + 0.5;
        for x in 0..ow {
            let dx = x as f64 + 0.5;
            let sample = match apply_homography(&homography, [dx, dy]) {
                Some([sx, sy]) => sample_bilinear_rgba(rgba, sw, sh, sx, sy),
                None => [255, 255, 255, 255],
            };
            out.extend_from_slice(&sample);
        }
    }

    Ok(DewarpedRgba { width: ow, height: oh, rgba: out })
}

/// Normalize uneven local illumination, vignetting, and colour casts on a scanned whiteboard or
/// business card RGBA8 image (`width` × `height`), returning a new RGBA8 buffer of identical size.
pub fn enhance_whiteboard_or_card(rgba: &[u8], width: u32, height: u32, kind: ScanCleanupKind) -> Result<Vec<u8>, String> {
    let (w, h) = (width as usize, height as usize);
    if w == 0 || h == 0 || w.checked_mul(h).and_then(|n| n.checked_mul(4)) != Some(rgba.len()) {
        return Err("RGBA buffer size does not match width × height × 4".to_string());
    }

    // Choose a window radius large enough to span strokes/glyphs without blurring illumination gradients.
    let radius = (w.max(h) / 14).clamp(6, 96);
    let stride = w + 1;
    let sat_len = (w + 1).saturating_mul(h + 1);
    let mut sat_r = vec![0u64; sat_len];
    let mut sat_g = vec![0u64; sat_len];
    let mut sat_b = vec![0u64; sat_len];

    // Pass 1: build summed-area tables over raw RGB channels.
    for y in 0..h {
        let mut row_r = 0u64;
        let mut row_g = 0u64;
        let mut row_b = 0u64;
        for x in 0..w {
            let p = (y * w + x) * 4;
            row_r += u64::from(rgba[p]);
            row_g += u64::from(rgba[p + 1]);
            row_b += u64::from(rgba[p + 2]);
            let idx = (y + 1) * stride + (x + 1);
            let above = y * stride + (x + 1);
            sat_r[idx] = sat_r[above] + row_r;
            sat_g[idx] = sat_g[above] + row_g;
            sat_b[idx] = sat_b[above] + row_b;
        }
    }

    // Pass 2: build a bright-envelope image where dark foreground ink/marker pixels are replaced by
    // the initial local mean, then smooth again so thick marker strokes never create halos!
    let mut env_r = vec![0u64; sat_len];
    let mut env_g = vec![0u64; sat_len];
    let mut env_b = vec![0u64; sat_len];
    for y in 0..h {
        let mut row_r = 0u64;
        let mut row_g = 0u64;
        let mut row_b = 0u64;
        for x in 0..w {
            let p = (y * w + x) * 4;
            let mr = box_mean(&sat_r, stride, w, h, x, y, radius);
            let mg = box_mean(&sat_g, stride, w, h, x, y, radius);
            let mb = box_mean(&sat_b, stride, w, h, x, y, radius);
            let br = f32::from(rgba[p]).max(mr).round().clamp(1.0, 255.0) as u64;
            let bg = f32::from(rgba[p + 1]).max(mg).round().clamp(1.0, 255.0) as u64;
            let bb = f32::from(rgba[p + 2]).max(mb).round().clamp(1.0, 255.0) as u64;
            row_r += br;
            row_g += bg;
            row_b += bb;
            let idx = (y + 1) * stride + (x + 1);
            let above = y * stride + (x + 1);
            env_r[idx] = env_r[above] + row_r;
            env_g[idx] = env_g[above] + row_g;
            env_b[idx] = env_b[above] + row_b;
        }
    }

    let (white_clip, black_floor, sat_boost) = match kind {
        ScanCleanupKind::Whiteboard => (0.90f32, 0.14f32, 1.30f32),
        ScanCleanupKind::Card => (0.95f32, 0.08f32, 1.12f32),
    };

    let mut out = Vec::with_capacity(rgba.len());
    for y in 0..h {
        for x in 0..w {
            let p = (y * w + x) * 4;
            let bg_r = box_mean(&env_r, stride, w, h, x, y, radius).max(16.0);
            let bg_g = box_mean(&env_g, stride, w, h, x, y, radius).max(16.0);
            let bg_b = box_mean(&env_b, stride, w, h, x, y, radius).max(16.0);

            let nr = (f32::from(rgba[p]) / bg_r).clamp(0.0, 1.15);
            let ng = (f32::from(rgba[p + 1]) / bg_g).clamp(0.0, 1.15);
            let nb = (f32::from(rgba[p + 2]) / bg_b).clamp(0.0, 1.15);

            let stretch = |v: f32| -> f32 {
                if v >= white_clip {
                    1.0
                } else if v <= black_floor {
                    0.0
                } else {
                    (v - black_floor) / (white_clip - black_floor)
                }
            };
            let mut sr = stretch(nr);
            let mut sg = stretch(ng);
            let mut sb = stretch(nb);

            // Boost chromatic marker/logo saturation on non-white foreground strokes.
            let lum = 0.299 * sr + 0.587 * sg + 0.114 * sb;
            if lum < 0.96 {
                sr = (lum + (sr - lum) * sat_boost).clamp(0.0, 1.0);
                sg = (lum + (sg - lum) * sat_boost).clamp(0.0, 1.0);
                sb = (lum + (sb - lum) * sat_boost).clamp(0.0, 1.0);
            }

            out.push((sr * 255.0).round() as u8);
            out.push((sg * 255.0).round() as u8);
            out.push((sb * 255.0).round() as u8);
            out.push(rgba[p + 3]);
        }
    }

    Ok(out)
}

fn box_mean(sat: &[u64], stride: usize, w: usize, h: usize, x: usize, y: usize, r: usize) -> f32 {
    let x0 = x.saturating_sub(r);
    let y0 = y.saturating_sub(r);
    let x1 = (x + r + 1).min(w);
    let y1 = (y + r + 1).min(h);
    let count = ((x1 - x0) * (y1 - y0)).max(1) as f32;
    let sum = sat[y1 * stride + x1] + sat[y0 * stride + x0] - sat[y0 * stride + x1] - sat[y1 * stride + x0];
    sum as f32 / count
}

fn resolve_output_size(ordered: [[f64; 2]; 4], dst_width: u32, dst_height: u32) -> Result<(u32, u32), String> {
    let [tl, tr, br, bl] = ordered;
    let dist = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]);
    let mut ow = if dst_width > 0 { f64::from(dst_width) } else { dist(tl, tr).max(dist(bl, br)).round() };
    let mut oh = if dst_height > 0 { f64::from(dst_height) } else { dist(tl, bl).max(dist(tr, br)).round() };
    if !(ow.is_finite() && oh.is_finite()) || ow < 1.0 || oh < 1.0 {
        return Err("output dimensions must be positive finite numbers".to_string());
    }
    let scale = (MAX_DEWARP_PIXELS / (ow * oh).max(1.0)).sqrt().min(1.0);
    ow = (ow * scale).floor().max(1.0);
    oh = (oh * scale).floor().max(1.0);
    Ok((ow as u32, oh as u32))
}

fn order_convex_quad(pts: [[f64; 2]; 4]) -> Option<[[f64; 2]; 4]> {
    if pts.iter().flatten().any(|v| !v.is_finite()) {
        return None;
    }
    let cx = pts.iter().map(|p| p[0]).sum::<f64>() * 0.25;
    let cy = pts.iter().map(|p| p[1]).sum::<f64>() * 0.25;
    let mut ring = pts;
    ring.sort_by(|a, b| {
        let aa = (a[1] - cy).atan2(a[0] - cx);
        let ba = (b[1] - cy).atan2(b[0] - cx);
        aa.total_cmp(&ba)
    });

    // Rotate so top-left (smallest x + y) comes first.
    let mut start = 0usize;
    let mut best = f64::INFINITY;
    for (i, p) in ring.iter().enumerate() {
        let sum = p[0] + p[1];
        if sum < best {
            best = sum;
            start = i;
        }
    }
    ring.rotate_left(start);

    // Ensure clockwise screen winding (x right, y down): cross(TR - TL, BR - TR) > 0.
    let cross = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    if cross(ring[0], ring[1], ring[2]) < 0.0 {
        ring.swap(1, 3);
    }

    // Verify all 4 interior turns have positive cross product and enclose non-trivial area.
    let mut area2 = 0.0;
    for i in 0..4 {
        let a = ring[i];
        let b = ring[(i + 1) % 4];
        let c = ring[(i + 2) % 4];
        if cross(a, b, c) <= 1e-3 {
            return None;
        }
        area2 += a[0] * b[1] - b[0] * a[1];
    }
    if area2.abs() < 1.0 {
        return None;
    }
    Some(ring)
}

/// Solve the 3×3 projective homography (`h[8] == 1.0`) mapping `src[i] -> dst[i]`.
fn solve_homography(src: [[f64; 2]; 4], dst: [[f64; 2]; 4]) -> Option<[f64; 9]> {
    let mut m = [[0.0f64; 9]; 8];
    for i in 0..4 {
        let (x, y) = (src[i][0], src[i][1]);
        let (u, v) = (dst[i][0], dst[i][1]);
        m[2 * i] = [x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, u];
        m[2 * i + 1] = [0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y, v];
    }

    // Gaussian elimination with partial pivoting on the 8×9 augmented matrix.
    for col in 0..8 {
        let mut pivot = col;
        let mut best = m[col][col].abs();
        for (row_idx, row) in m.iter().enumerate().skip(col + 1) {
            let v = row[col].abs();
            if v > best {
                best = v;
                pivot = row_idx;
            }
        }
        if best < 1e-11 || !best.is_finite() {
            return None;
        }
        if pivot != col {
            m.swap(col, pivot);
        }
        let diag = m[col][col];
        for v in &mut m[col][col..9] {
            *v /= diag;
        }
        let pivot_row = m[col];
        for (r, row) in m.iter_mut().enumerate() {
            if r == col {
                continue;
            }
            let factor = row[col];
            if factor != 0.0 {
                for c in col..9 {
                    row[c] -= factor * pivot_row[c];
                }
            }
        }
    }

    let out = [m[0][8], m[1][8], m[2][8], m[3][8], m[4][8], m[5][8], m[6][8], m[7][8], 1.0];
    out.iter().all(|v| v.is_finite()).then_some(out)
}

fn apply_homography(h: &[f64; 9], p: [f64; 2]) -> Option<[f64; 2]> {
    let denom = h[6] * p[0] + h[7] * p[1] + h[8];
    if !denom.is_finite() || denom.abs() < 1e-9 {
        return None;
    }
    let x = (h[0] * p[0] + h[1] * p[1] + h[2]) / denom;
    let y = (h[3] * p[0] + h[4] * p[1] + h[5]) / denom;
    (x.is_finite() && y.is_finite()).then_some([x, y])
}

fn sample_bilinear_rgba(rgba: &[u8], w: usize, h: usize, sx: f64, sy: f64) -> [u8; 4] {
    let fx = sx - 0.5;
    let fy = sy - 0.5;
    if fx < -0.5 || fy < -0.5 || fx > w as f64 - 0.5 || fy > h as f64 - 0.5 {
        return [255, 255, 255, 255];
    }
    let x0 = fx.floor();
    let y0 = fy.floor();
    let ax = fx - x0;
    let ay = fy - y0;
    let clamp = |v: f64, lim: usize| v.clamp(0.0, (lim - 1) as f64) as usize;
    let (xa, xb) = (clamp(x0, w), clamp(x0 + 1.0, w));
    let (ya, yb) = (clamp(y0, h), clamp(y0 + 1.0, h));

    let px = |x: usize, y: usize| -> [f64; 4] {
        let idx = (y * w + x) * 4;
        match rgba.get(idx..idx + 4) {
            Some(s) => [f64::from(s[0]), f64::from(s[1]), f64::from(s[2]), f64::from(s[3])],
            None => [255.0; 4],
        }
    };
    let (p00, p10, p01, p11) = (px(xa, ya), px(xb, ya), px(xa, yb), px(xb, yb));
    let mut res = [0u8; 4];
    for c in 0..4 {
        let top = p00[c] * (1.0 - ax) + p10[c] * ax;
        let bot = p01[c] * (1.0 - ax) + p11[c] * ax;
        res[c] = (top * (1.0 - ay) + bot * ay).round().clamp(0.0, 255.0) as u8;
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dewarp_quad_rectifies_skewed_quadrilateral_and_rejects_degenerate() {
        // Paint a 60x60 dark canvas containing a tilted bright red-bordered card.
        let (sw, sh) = (60u32, 60u32);
        let quad = [[14.0, 10.0], [48.0, 14.0], [44.0, 50.0], [10.0, 46.0]];
        let inv = solve_homography(quad, [[0.0, 0.0], [32.0, 0.0], [32.0, 32.0], [0.0, 32.0]]).unwrap();

        let mut src = vec![20u8; (sw * sh * 4) as usize];
        for y in 0..sh {
            for x in 0..sw {
                let idx = ((y * sw + x) * 4) as usize;
                src[idx + 3] = 255;
                if let Some([u, v]) = apply_homography(&inv, [f64::from(x) + 0.5, f64::from(y) + 0.5])
                    && (0.0..=32.0).contains(&u)
                    && (0.0..=32.0).contains(&v)
                {
                    // Top-left quadrant blue, bottom-right quadrant red, rest white.
                    let color = if u < 16.0 && v < 16.0 {
                        [30, 80, 230, 255]
                    } else if u >= 16.0 && v >= 16.0 {
                        [230, 40, 40, 255]
                    } else {
                        [250, 250, 250, 255]
                    };
                    src[idx..idx + 4].copy_from_slice(&color);
                }
            }
        }

        // Scramble corner order deliberately to verify automatic ordering.
        let scrambled = [quad[2], quad[0], quad[3], quad[1]];
        let out = dewarp_quad_rgba(&src, sw, sh, scrambled, 32, 32).unwrap();
        assert_eq!((out.width, out.height), (32, 32));

        let px = |x: u32, y: u32| -> [u8; 4] {
            let i = ((y * 32 + x) * 4) as usize;
            [out.rgba[i], out.rgba[i + 1], out.rgba[i + 2], out.rgba[i + 3]]
        };
        let tl = px(6, 6);
        assert!(tl[2] > 180 && tl[0] < 80, "top-left quadrant rectified to blue: {tl:?}");
        let br = px(25, 25);
        assert!(br[0] > 180 && br[2] < 80, "bottom-right quadrant rectified to red: {br:?}");
        let tr = px(25, 6);
        assert!(tr[0] > 220 && tr[1] > 220 && tr[2] > 220, "top-right quadrant white: {tr:?}");

        // Collinear corners must return Err without panicking.
        assert!(dewarp_quad_rgba(&src, sw, sh, [[0.0, 0.0], [10.0, 10.0], [20.0, 20.0], [30.0, 30.0]], 16, 16).is_err());
    }

    #[test]
    fn enhance_whiteboard_removes_uneven_shadow_and_keeps_colored_markers() {
        let (w, h) = (48u32, 32u32);
        let mut img = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                // Strong shadow across x (from 135 on left to 235 on right) with warm yellow cast.
                let shade = 135.0 + 100.0 * (x as f32 / w as f32);
                let mut r = shade as u8;
                let mut g = (shade * 0.96) as u8;
                let mut b = (shade * 0.88) as u8;
                // Draw a horizontal red marker stroke at y == 16, x in 8..40.
                if y == 16 && (8..40).contains(&x) {
                    r = (shade * 0.78) as u8;
                    g = (shade * 0.12) as u8;
                    b = (shade * 0.12) as u8;
                }
                img.extend_from_slice(&[r, g, b, 255]);
            }
        }

        let cleaned = enhance_whiteboard_or_card(&img, w, h, ScanCleanupKind::Whiteboard).unwrap();
        let px = |x: u32, y: u32| -> [u8; 4] {
            let i = ((y * w + x) * 4) as usize;
            [cleaned[i], cleaned[i + 1], cleaned[i + 2], cleaned[i + 3]]
        };
        // Shadowed left board background becomes bright pure white!
        assert_eq!(px(4, 6), [255, 255, 255, 255]);
        assert_eq!(px(44, 6), [255, 255, 255, 255]);
        // Red marker stroke stays distinctly red!
        let stroke = px(16, 16);
        assert!(stroke[0] > stroke[1].saturating_add(100) && stroke[1] < 60, "red marker preserved: {stroke:?}");
    }
}
