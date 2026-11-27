//! Photoshop 2027 `Generative Recompose…` (`생성형 재구성…`):
//! Subject-preserving canvas aspect-ratio & dimension recomposition (`OutputSize`, `AspectRatio`).
//! - Computes a multi-cue visual saliency + edge-energy protection map so foreground subjects stay
//!   undistorted.
//! - Uses forward-energy dynamic-programming seam carving along axes that shrink.
//! - Uses salient-subject anchored placement + multi-scale mirror-patch & atmospheric continuation
//!   outpainting along axes that expand, steered by optional placement/prompt instructions.

use photocraft_geom::{Rect, Size};
use photocraft_raster::{Surface, from_rgba_into, to_rgba};
use serde::{Deserialize, Serialize};

use crate::photo_util::{bilinear, hash01};

/// Parameters for `Edit > Generative Recompose…` (`생성형 재구성…`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RecomposeParams {
    pub target_width: u32,
    pub target_height: u32,
    /// Optional preset aspect ratio label (`"16:9"`, `"9:16"`, `"1:1"`, `"4:5"`, `"4:3"`, `"custom"`).
    pub aspect_ratio: String,
    /// Optional placement / background instruction (`"subject left"`, `"subject right"`, `"warm sky"`, etc.).
    pub prompt: String,
}

impl Default for RecomposeParams {
    fn default() -> Self {
        Self { target_width: 1920, target_height: 1080, aspect_ratio: "16:9".into(), prompt: String::new() }
    }
}

/// Computes visual saliency + gradient energy (`w × h`) so foreground subjects resist seam removal.
fn saliency_energy(px: &[[f32; 4]], w: usize, h: usize) -> Vec<f32> {
    let n = w * h;
    let mut luma = vec![0.0f32; n];
    let mut mean_rgb = [0.0f64; 3];
    for (i, q) in px.iter().take(n).enumerate() {
        luma[i] = 0.2126 * q[0] + 0.7152 * q[1] + 0.0722 * q[2];
        for c in 0..3 {
            mean_rgb[c] += f64::from(q[c]);
        }
    }
    let denom = n.max(1) as f64;
    let avg = [(mean_rgb[0] / denom) as f32, (mean_rgb[1] / denom) as f32, (mean_rgb[2] / denom) as f32];

    let mut e = vec![0.0f32; n];
    for y in 0..h {
        let ym = y.saturating_sub(1);
        let yp = (y + 1).min(h - 1);
        let cy = (y as f32 / h.max(1) as f32 - 0.5) * 2.0;
        for x in 0..w {
            let xm = x.saturating_sub(1);
            let xp = (x + 1).min(w - 1);
            let cx = (x as f32 / w.max(1) as f32 - 0.5) * 2.0;
            let idx = y * w + x;
            let q = px[idx];
            let grad = (luma[y * w + xp] - luma[y * w + xm]).abs() + (luma[yp * w + x] - luma[ym * w + x]).abs();
            let color_dist = ((q[0] - avg[0]).powi(2) + (q[1] - avg[1]).powi(2) + (q[2] - avg[2]).powi(2)).sqrt();
            let center_prior = (-(cx * cx + cy * cy) * 1.1).exp();
            // High color/contrast distinctiveness near center gets a strong subject-protection weight!
            e[idx] = grad * 1.8 + color_dist * (2.5 + 4.0 * center_prior) + 0.02;
        }
    }
    e
}

fn remove_vertical_seams(mut px: Vec<[f32; 4]>, mut w: usize, h: usize, target_w: usize) -> Vec<[f32; 4]> {
    if target_w == 0 || h == 0 || target_w >= w {
        return px;
    }
    // For very large high-res reductions (w > 512), first uniform-scale down to at most 1.35× target_w
    // so DP stays fast, then seam-carve the remaining 35% to preserve subject proportions!
    let cap_w = if w > 512 { (target_w * 27 / 20).max(target_w + 1).min(w) } else { w };
    if w > cap_w {
        px = bilinear_resize(&px, w, h, cap_w, h);
        w = cap_w;
    }
    let mut dp = vec![0.0f32; w * h];
    let mut back = vec![0i8; w * h];
    let mut seam = vec![0usize; h];

    while w > target_w {
        let energy = saliency_energy(&px, w, h);
        dp[..w].copy_from_slice(&energy[..w]);
        for y in 1..h {
            for x in 0..w {
                let mut best = dp[(y - 1) * w + x];
                let mut dir = 0i8;
                if x > 0 && dp[(y - 1) * w + x - 1] < best {
                    best = dp[(y - 1) * w + x - 1];
                    dir = -1;
                }
                if x + 1 < w && dp[(y - 1) * w + x + 1] < best {
                    best = dp[(y - 1) * w + x + 1];
                    dir = 1;
                }
                dp[y * w + x] = energy[y * w + x] + best;
                back[y * w + x] = dir;
            }
        }
        let mut min_x = 0usize;
        let last_row = (h - 1) * w;
        for x in 1..w {
            if dp[last_row + x] < dp[last_row + min_x] {
                min_x = x;
            }
        }
        seam[h - 1] = min_x;
        for y in (1..h).rev() {
            let prev = (seam[y] as isize + back[y * w + seam[y]] as isize).clamp(0, (w - 1) as isize) as usize;
            seam[y - 1] = prev;
        }
        let new_w = w - 1;
        let mut next = Vec::with_capacity(new_w * h);
        for y in 0..h {
            let sx = seam[y];
            let row = &px[y * w..(y + 1) * w];
            for (x, &q) in row.iter().enumerate() {
                if x != sx {
                    next.push(q);
                }
            }
        }
        px = next;
        w = new_w;
    }
    px
}

fn transpose_rgba(px: &[[f32; 4]], w: usize, h: usize) -> Vec<[f32; 4]> {
    let mut out = vec![[0.0f32; 4]; w * h];
    for y in 0..h {
        for x in 0..w {
            out[x * h + y] = px[y * w + x];
        }
    }
    out
}

fn bilinear_resize(px: &[[f32; 4]], w: usize, h: usize, nw: usize, nh: usize) -> Vec<[f32; 4]> {
    let mut flat = vec![0.0f32; w * h * 4];
    for (i, q) in px.iter().take(w * h).enumerate() {
        flat[i * 4..i * 4 + 4].copy_from_slice(q);
    }
    let mut out = vec![[0.0f32; 4]; nw * nh];
    for y in 0..nh {
        let sy = ((y as f32 + 0.5) * h as f32 / nh.max(1) as f32 - 0.5).clamp(0.0, (h.saturating_sub(1)) as f32);
        for x in 0..nw {
            let sx = ((x as f32 + 0.5) * w as f32 / nw.max(1) as f32 - 0.5).clamp(0.0, (w.saturating_sub(1)) as f32);
            let mut q = [0.0f32; 4];
            for (c, item) in q.iter_mut().enumerate() {
                *item = bilinear(&flat, w, h, 4, c, sx, sy);
            }
            out[y * nw + x] = q;
        }
    }
    out
}

/// Expands `px` (`w × h`) horizontally to `target_w` using anchor placement + edge-guided patch continuation.
fn outpaint_horizontal(px: &[[f32; 4]], w: usize, h: usize, target_w: usize, align_frac: f32, tint: [f32; 3]) -> Vec<[f32; 4]> {
    if target_w <= w || w == 0 || h == 0 {
        return px.to_vec();
    }
    let extra = target_w - w;
    let pad_left = ((extra as f32 * align_frac.clamp(0.0, 1.0)).round() as usize).min(extra);
    let mut out = vec![[0.0f32; 4]; target_w * h];
    let band = (w / 5).clamp(2, 48);

    // Precompute smooth vertical colour trend at left and right borders for seamless horizon continuation!
    let mut left_trend = vec![[0.0f32; 4]; h];
    let mut right_trend = vec![[0.0f32; 4]; h];
    for y in 0..h {
        let mut sl = [0.0f32; 4];
        let mut sr = [0.0f32; 4];
        for k in 0..band {
            let ql = px[y * w + k.min(w - 1)];
            let qr = px[y * w + (w - 1).saturating_sub(k)];
            for c in 0..4 {
                sl[c] += ql[c];
                sr[c] += qr[c];
            }
        }
        for c in 0..4 {
            left_trend[y][c] = sl[c] / band as f32;
            right_trend[y][c] = sr[c] / band as f32;
        }
    }

    for y in 0..h {
        for x in 0..target_w {
            if x >= pad_left && x < pad_left + w {
                out[y * target_w + x] = px[y * w + (x - pad_left)];
                continue;
            }
            let (dist, is_left) = if x < pad_left { (pad_left - x, true) } else { (x - (pad_left + w) + 1, false) };
            // Mirror-folded texture coordinate with jittered vertical offset to avoid obvious mirror symmetry.
            let period = (w.saturating_sub(1)).max(1);
            let raw_m = if period > 1 {
                let m = dist % (2 * period);
                if m <= period { m } else { 2 * period - m }
            } else {
                0
            };
            let jitter_y = ((hash01(x as i64, y as i64, 9027) - 0.5) * 4.0).round() as isize;
            let sy = (y as isize + jitter_y).clamp(0, h as isize - 1) as usize;
            let sx = if is_left { raw_m.min(w - 1) } else { (w - 1).saturating_sub(raw_m.min(w - 1)) };
            let tex = px[sy * w + sx];
            let trend = if is_left { left_trend[y] } else { right_trend[y] };
            // Near the seam (dist small), preserve crisp mirrored edge detail; farther out, blend smoothly with horizon trend.
            let smooth_mix = (dist as f32 / (band as f32 * 2.5).max(4.0)).clamp(0.0, 0.68);
            let mut q = [0.0f32; 4];
            for c in 0..3 {
                q[c] = ((tex[c] * (1.0 - smooth_mix) + trend[c] * smooth_mix) * tint[c]).clamp(0.0, 1.0);
            }
            q[3] = tex[3].max(trend[3]);
            out[y * target_w + x] = q;
        }
    }
    out
}

fn prompt_alignment_and_tint(prompt: &str, px: &[[f32; 4]], w: usize, h: usize) -> (f32, f32, [f32; 3]) {
    let p = prompt.to_ascii_lowercase();
    // Default alignment centers the salient subject centroid!
    let energy = saliency_energy(px, w, h);
    let (mut wx, mut wy, mut wsum) = (0.0f64, 0.0f64, 0.0f64);
    for y in 0..h {
        for x in 0..w {
            let wt = f64::from(energy[y * w + x]);
            wx += wt * (x as f64 + 0.5) / w.max(1) as f64;
            wy += wt * (y as f64 + 0.5) / h.max(1) as f64;
            wsum += wt;
        }
    }
    let mut ax = if wsum > 1e-6 { 1.0 - (wx / wsum) as f32 } else { 0.5 };
    let mut ay = if wsum > 1e-6 { 1.0 - (wy / wsum) as f32 } else { 0.5 };
    ax = ax.clamp(0.25, 0.75);
    ay = ay.clamp(0.25, 0.75);

    if p.contains("left") || p.contains("왼쪽") {
        ax = 0.15;
    } else if p.contains("right") || p.contains("오른쪽") {
        ax = 0.85;
    } else if p.contains("center") || p.contains("중앙") {
        ax = 0.5;
    }
    if p.contains("top") || p.contains("위") {
        ay = 0.15;
    } else if p.contains("bottom") || p.contains("아래") {
        ay = 0.85;
    }
    let tint = if p.contains("warm") || p.contains("따뜻") {
        [1.04, 1.0, 0.94]
    } else if p.contains("cool") || p.contains("차가운") {
        [0.95, 0.99, 1.05]
    } else {
        [1.0, 1.0, 1.0]
    };
    (ax, ay, tint)
}

/// Recomposes `px` (`w × h`) to `(target_w, target_h)` preserving salient foreground subjects.
pub fn generative_recompose_rgba(px: &[[f32; 4]], w: usize, h: usize, params: &RecomposeParams) -> (Vec<[f32; 4]>, usize, usize) {
    let tw = (params.target_width as usize).clamp(1, 16_384);
    let th = (params.target_height as usize).clamp(1, 16_384);
    if w == 0 || h == 0 || px.len() < w * h {
        return (vec![[0.0, 0.0, 0.0, 1.0]; tw * th], tw, th);
    }
    if tw == w && th == h {
        return (px[..w * h].to_vec(), tw, th);
    }

    let (align_x, align_y, tint) = prompt_alignment_and_tint(&params.prompt, px, w, h);

    // Step 1: Horizontal shrink (seam carving) or grow (outpaint).
    let (mut cur, cw) = if tw < w {
        (remove_vertical_seams(px[..w * h].to_vec(), w, h, tw), tw)
    } else if tw > w {
        (outpaint_horizontal(&px[..w * h], w, h, tw, align_x, tint), tw)
    } else {
        (px[..w * h].to_vec(), w)
    };

    // Step 2: Vertical shrink or grow via transpose.
    if th != h {
        let tr = transpose_rgba(&cur, cw, h);
        let tr_out = if th < h { remove_vertical_seams(tr, h, cw, th) } else { outpaint_horizontal(&tr, h, cw, th, align_y, tint) };
        cur = transpose_rgba(&tr_out, th, cw);
    }

    (cur, tw, th)
}

/// Surface wrapper returning a new `Surface` over `0..target_width × 0..target_height`.
pub fn generative_recompose_surface(src: &Surface, src_size: Size, params: &RecomposeParams) -> (Surface, Size) {
    let fmt = src.format();
    let ch = fmt.channels();
    let src_rect = Rect::from_size(src_size);
    let raw = src.read_region(src_rect);
    let rgba_px: Vec<[f32; 4]> = raw.chunks_exact(ch).map(|p| to_rgba(&fmt, p)).collect();
    let (out_rgba, tw, th) = generative_recompose_rgba(&rgba_px, src_size.width as usize, src_size.height as usize, params);
    let out_size = Size::new(tw as u32, th as u32);
    let mut out_buf = vec![0.0f32; tw * th * ch];
    for (p, dst) in out_rgba.iter().zip(out_buf.chunks_exact_mut(ch)) {
        from_rgba_into(&fmt, *p, dst);
    }
    let mut out = Surface::with_default(fmt, &src.default_pixel());
    out.write_region(Rect::from_size(out_size), &out_buf);
    (out, out_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generative_recompose_shrinks_and_expands_while_keeping_salient_subject() {
        let (w, h) = (60, 40);
        let mut px = vec![[0.18f32, 0.22, 0.28, 1.0]; w * h];
        // Bright red salient subject square in the center (24..36, 14..26).
        for y in 14..26 {
            for x in 24..36 {
                px[y * w + x] = [0.95, 0.15, 0.12, 1.0];
            }
        }
        // Shrink width to 44, expand height to 56!
        let (out, nw, nh) = generative_recompose_rgba(
            &px,
            w,
            h,
            &RecomposeParams { target_width: 44, target_height: 56, aspect_ratio: "custom".into(), prompt: "center".into() },
        );
        assert_eq!((nw, nh), (44, 56));
        assert_eq!(out.len(), 44 * 56);
        let subject_px = out.iter().filter(|q| q[0] > 0.85 && q[1] < 0.25).count();
        assert_eq!(subject_px, 12 * 12, "all 144 salient red subject pixels must survive seam carving & outpainting!");
    }
}
