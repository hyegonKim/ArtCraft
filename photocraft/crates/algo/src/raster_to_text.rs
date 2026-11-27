//! Photoshop 2027 `Type > Convert to Text Layers` (`텍스트 레이어로 변환`):
//! - Segments high-contrast text lines & glyph connected-components from a raster region.
//! - Estimates foreground ink color, background fill color, baseline/bounding box, point size,
//!   and bold stroke density.
//! - Recognizes standard ASCII characters via normalized 5×7 topological template correlation
//!   (with sensible fallback text when custom hand-lettering has no exact ASCII match).
//! - Cleanly inpaints the extracted text pixels on the raster surface so the newly created
//!   editable `TextLayer`s sit over an unblemished background!

use photocraft_geom::Rect;
use photocraft_raster::{Surface, from_rgba_into, to_rgba};
use serde::{Deserialize, Serialize};

/// One detected text line recovered from a raster layer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DetectedTextLine {
    /// Bounding box in document coordinates.
    pub bounds: Rect,
    /// Recognized UTF-8 string.
    pub text: String,
    /// Estimated font size in points (at 72 DPI: 1 pt = 1 px).
    pub size_pt: f32,
    /// Estimated foreground ink RGBA (`0.0..=1.0`).
    pub color: [f32; 4],
    /// True when stroke density indicates Bold weight.
    pub bold: bool,
}

/// Result of `detect_and_clean_raster_text`.
#[derive(Clone, Debug)]
pub struct RasterToTextResult {
    pub lines: Vec<DetectedTextLine>,
    /// Cleaned background surface with extracted glyph pixels inpainted.
    pub cleaned_surface: Surface,
}

/// Compact 5×7 glyph bitmaps (`u64` low 35 bits, MSB row-major) for fast clean-room OCR matching.
const GLYPH_TEMPLATES: &[(char, [u8; 7])] = &[
    ('A', [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]),
    ('B', [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110]),
    ('C', [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110]),
    ('D', [0b11100, 0b10010, 0b10001, 0b10001, 0b10001, 0b10010, 0b11100]),
    ('E', [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111]),
    ('F', [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000]),
    ('G', [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110]),
    ('H', [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]),
    ('I', [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
    ('J', [0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100]),
    ('K', [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001]),
    ('L', [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111]),
    ('M', [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001]),
    ('N', [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001]),
    ('O', [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
    ('P', [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000]),
    ('Q', [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101]),
    ('R', [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001]),
    ('S', [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110]),
    ('T', [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100]),
    ('U', [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110]),
    ('V', [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100]),
    ('W', [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001]),
    ('X', [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001]),
    ('Y', [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100]),
    ('Z', [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111]),
    ('0', [0b01110, 0b10011, 0b10101, 0b10101, 0b11001, 0b10001, 0b01110]),
    ('1', [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
    ('2', [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111]),
    ('3', [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110]),
    ('4', [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010]),
    ('5', [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110]),
    ('6', [0b01110, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b01110]),
    ('7', [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000]),
    ('8', [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110]),
    ('9', [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110]),
];

fn match_glyph(mask: &[bool], w: usize, x0: usize, y0: usize, x1: usize, y1: usize) -> char {
    let gw = x1.saturating_sub(x0).max(1);
    let gh = y1.saturating_sub(y0).max(1);
    // Narrow vertical stem -> 'I' or 'l'
    if gw * 3 <= gh {
        return 'I';
    }
    let mut grid = [0.0f32; 35];
    for gy in 0..7 {
        let sy0 = y0 + (gy * gh) / 7;
        let sy1 = (y0 + ((gy + 1) * gh) / 7).max(sy0 + 1).min(y1);
        for gx in 0..5 {
            let sx0 = x0 + (gx * gw) / 5;
            let sx1 = (x0 + ((gx + 1) * gw) / 5).max(sx0 + 1).min(x1);
            let mut on = 0usize;
            let mut tot = 0usize;
            for yy in sy0..sy1 {
                for xx in sx0..sx1 {
                    tot += 1;
                    if mask[yy * w + xx] {
                        on += 1;
                    }
                }
            }
            grid[gy * 5 + gx] = if tot > 0 { on as f32 / tot as f32 } else { 0.0 };
        }
    }

    let mut best = ('T', f32::MIN);
    for &(ch, rows) in GLYPH_TEMPLATES {
        let mut score = 0.0f32;
        for (gy, &row_bits) in rows.iter().enumerate() {
            for gx in 0..5 {
                let want = if (row_bits >> (4 - gx)) & 1 == 1 { 1.0 } else { 0.0 };
                let got = grid[gy * 5 + gx];
                score += 1.0 - (want - got).abs();
            }
        }
        if score > best.1 {
            best = (ch, score);
        }
    }
    best.0
}

/// Detects rasterized text lines inside `rect` of `src`, recognizes glyph strings + styling,
/// and returns both the detected lines and a cleaned background surface.
pub fn detect_and_clean_raster_text(src: &Surface, rect: Rect, resolution_dpi: f32) -> RasterToTextResult {
    let fmt = src.format();
    if rect.is_empty() {
        return RasterToTextResult { lines: Vec::new(), cleaned_surface: src.clone() };
    }
    let ch = fmt.channels();
    let w = rect.width() as usize;
    let h = rect.height() as usize;
    let raw = src.read_region(rect);
    let mut px: Vec<[f32; 4]> = raw.chunks_exact(ch).map(|p| to_rgba(&fmt, p)).collect();

    // 1. Estimate dominant background color from border pixels of `rect`.
    let mut border_samples: Vec<[f32; 4]> = Vec::with_capacity(2 * (w + h));
    for x in 0..w {
        border_samples.push(px[x]);
        border_samples.push(px[(h - 1) * w + x]);
    }
    for y in 0..h {
        border_samples.push(px[y * w]);
        border_samples.push(px[y * w + (w - 1)]);
    }
    let mut bg = [0.0f32; 4];
    if !border_samples.is_empty() {
        for s in &border_samples {
            for c in 0..4 {
                bg[c] += s[c];
            }
        }
        let inv = 1.0 / border_samples.len() as f32;
        for b in &mut bg {
            *b *= inv;
        }
    }

    // 2. Foreground ink mask: either opaque over transparent background OR strong color/luma contrast against bg!
    let bg_transparent = bg[3] < 0.15;
    let mut fg_mask = vec![false; w * h];
    let mut max_dist = 0.0f32;
    let mut dist_map = vec![0.0f32; w * h];
    for (i, q) in px.iter().enumerate() {
        let d = if bg_transparent {
            q[3]
        } else {
            let dr = q[0] - bg[0];
            let dg = q[1] - bg[1];
            let db = q[2] - bg[2];
            (dr * dr + dg * dg + db * db).sqrt() * q[3]
        };
        dist_map[i] = d;
        if d > max_dist {
            max_dist = d;
        }
    }

    let thresh = (max_dist * 0.35).max(0.12);
    let mut total_fg = 0usize;
    for i in 0..w * h {
        if dist_map[i] >= thresh {
            fg_mask[i] = true;
            total_fg += 1;
        }
    }

    let dpi_scale = if resolution_dpi.is_finite() && resolution_dpi > 0.0 { 72.0 / resolution_dpi } else { 1.0 };
    if total_fg == 0 {
        return RasterToTextResult { lines: Vec::new(), cleaned_surface: src.clone() };
    }

    // 3. Horizontal projection profile -> text line bands `[ly0..ly1)`.
    let mut row_counts = vec![0usize; h];
    for y in 0..h {
        row_counts[y] = (0..w).filter(|&x| fg_mask[y * w + x]).count();
    }
    let mut bands: Vec<(usize, usize)> = Vec::new();
    let mut y = 0usize;
    while y < h {
        if row_counts[y] > 0 {
            let start = y;
            while y < h && row_counts[y] > 0 {
                y += 1;
            }
            if y - start >= 3 {
                bands.push((start, y));
            }
        } else {
            y += 1;
        }
    }
    if bands.is_empty() {
        bands.push((0, h));
    }

    let lines = extract_band_lines(&bands, &fg_mask, &px, w, rect, dpi_scale);

    // 4. Cleanly inpaint foreground text pixels + 1 px anti-aliased halo on `px` using local non-fg neighbors!
    inpaint_text_mask(&mut px, &fg_mask, w, h, bg_transparent, bg);

    let mut out_buf = vec![0.0f32; w * h * ch];
    for (p, dst) in px.iter().zip(out_buf.chunks_exact_mut(ch)) {
        from_rgba_into(&fmt, *p, dst);
    }
    let mut cleaned_surface = src.clone();
    cleaned_surface.write_region(rect, &out_buf);

    RasterToTextResult { lines, cleaned_surface }
}

fn find_char_spans(fg_mask: &[bool], w: usize, by0: usize, by1: usize) -> Vec<(usize, usize)> {
    let mut chars_x: Vec<(usize, usize)> = Vec::new();
    let mut x = 0usize;
    while x < w {
        if (by0..by1).any(|yy| fg_mask[yy * w + x]) {
            let s = x;
            while x < w && (by0..by1).any(|yy| fg_mask[yy * w + x]) {
                x += 1;
            }
            chars_x.push((s, x));
        } else {
            x += 1;
        }
    }
    chars_x
}

fn extract_band_lines(bands: &[(usize, usize)], fg_mask: &[bool], px: &[[f32; 4]], w: usize, rect: Rect, dpi_scale: f32) -> Vec<DetectedTextLine> {
    let mut lines = Vec::new();
    for &(by0, by1) in bands {
        let bh = by1 - by0;
        let chars_x = find_char_spans(fg_mask, w, by0, by1);
        if chars_x.is_empty() {
            continue;
        }
        let lx0 = chars_x.first().map_or(0, |c| c.0);
        let lx1 = chars_x.last().map_or(w, |c| c.1);
        let space_gap = (bh * 3 / 5).max(4);

        let mut text = String::new();
        let mut fg_sum = [0.0f64; 4];
        let mut fg_cnt = 0usize;
        for (i, &(cx0, cx1)) in chars_x.iter().enumerate() {
            if i > 0 && cx0.saturating_sub(chars_x[i - 1].1) >= space_gap {
                text.push(' ');
            }
            let mut cy0 = by1;
            let mut cy1 = by0;
            for yy in by0..by1 {
                for xx in cx0..cx1 {
                    if fg_mask[yy * w + xx] {
                        cy0 = cy0.min(yy);
                        cy1 = cy1.max(yy + 1);
                        let q = px[yy * w + xx];
                        for (acc, &qv) in fg_sum.iter_mut().zip(&q) {
                            *acc += f64::from(qv);
                        }
                        fg_cnt += 1;
                    }
                }
            }
            if cy1 > cy0 {
                text.push(match_glyph(fg_mask, w, cx0, cy0, cx1, cy1));
            }
        }
        if text.trim().is_empty() {
            text = "Text".into();
        }
        let ink = if fg_cnt > 0 {
            [
                (fg_sum[0] / fg_cnt as f64) as f32,
                (fg_sum[1] / fg_cnt as f64) as f32,
                (fg_sum[2] / fg_cnt as f64) as f32,
                (fg_sum[3] / fg_cnt as f64).clamp(0.5, 1.0) as f32,
            ]
        } else {
            [0.0, 0.0, 0.0, 1.0]
        };
        let box_area = ((lx1 - lx0) * bh).max(1);
        let fill_density = fg_cnt as f32 / box_area as f32;
        let size_pt = ((bh as f32 * 1.28) * dpi_scale).clamp(6.0, 512.0);
        lines.push(DetectedTextLine {
            bounds: Rect::new(rect.x0 + lx0 as i32, rect.y0 + by0 as i32, rect.x0 + lx1 as i32, rect.y0 + by1 as i32),
            text,
            size_pt,
            color: ink,
            bold: fill_density > 0.42,
        });
    }
    lines
}

fn inpaint_text_mask(px: &mut Vec<[f32; 4]>, fg_mask: &[bool], w: usize, h: usize, bg_transparent: bool, bg: [f32; 4]) {
    let mut dilate = fg_mask.to_vec();
    for y in 0..h {
        for x in 0..w {
            if fg_mask[y * w + x] {
                for dy in -1isize..=1 {
                    for dx in -1isize..=1 {
                        let ny = (y as isize + dy).clamp(0, h as isize - 1) as usize;
                        let nx = (x as isize + dx).clamp(0, w as isize - 1) as usize;
                        dilate[ny * w + nx] = true;
                    }
                }
            }
        }
    }
    if bg_transparent {
        for (i, &m) in dilate.iter().enumerate() {
            if m {
                px[i] = [0.0, 0.0, 0.0, 0.0];
            }
        }
        return;
    }
    let mut clean = px.clone();
    let mut known: Vec<bool> = dilate.iter().map(|&m| !m).collect();
    for _ in 0..12 {
        let prev = clean.clone();
        let prev_k = known.clone();
        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                if prev_k[idx] {
                    continue;
                }
                let mut sum = [0.0f32; 4];
                let mut cnt = 0.0f32;
                for (dx, dy) in [(-1isize, 0isize), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
                    let nx = x as isize + dx;
                    let ny = y as isize + dy;
                    if nx >= 0 && nx < w as isize && ny >= 0 && ny < h as isize {
                        let ni = ny as usize * w + nx as usize;
                        if prev_k[ni] {
                            for (s, &p_val) in sum.iter_mut().zip(&prev[ni]) {
                                *s += p_val;
                            }
                            cnt += 1.0;
                        }
                    }
                }
                if cnt > 0.0 {
                    for (dst, &s) in clean[idx].iter_mut().zip(&sum) {
                        *dst = s / cnt;
                    }
                    known[idx] = true;
                }
            }
        }
    }
    for (c_px, &is_known) in clean.iter_mut().zip(&known) {
        if !is_known {
            *c_px = bg;
        }
    }
    *px = clean;
}

#[cfg(test)]
mod tests {
    use super::*;
    use photocraft_color::PixelFormat;

    #[test]
    fn detects_raster_letters_and_inpaints_background_cleanly() {
        let fmt = PixelFormat::RGBA8;
        let mut surf = Surface::new(fmt);
        let rect = Rect::new(0, 0, 64, 32);
        surf.fill_rect(rect, &[240.0 / 255.0, 240.0 / 255.0, 245.0 / 255.0, 1.0]);
        // Stamp 'H' and 'I' using 5x7 bitmaps scaled 2x at (8, 8) and (22, 8) in dark navy ink!
        let stamp = |surf: &mut Surface, ox: i32, oy: i32, bits: [u8; 7]| {
            for (ry, row) in bits.into_iter().enumerate() {
                for rx in 0..5 {
                    if (row >> (4 - rx)) & 1 == 1 {
                        surf.fill_rect(Rect::new(ox + rx * 2, oy + ry as i32 * 2, ox + rx * 2 + 2, oy + ry as i32 * 2 + 2), &[0.08, 0.12, 0.55, 1.0]);
                    }
                }
            }
        };
        stamp(&mut surf, 8, 8, [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001]);
        stamp(&mut surf, 22, 8, [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]);

        let res = detect_and_clean_raster_text(&surf, rect, 72.0);
        assert_eq!(res.lines.len(), 1);
        assert_eq!(res.lines[0].text, "HI");
        assert!(res.lines[0].color[2] > 0.45 && res.lines[0].color[0] < 0.20);
        // Check cleaned surface at (8, 8) was restored to light background!
        let cleaned_px = res.cleaned_surface.pixel(9, 9);
        assert!(cleaned_px[0] > 0.85, "expected cleaned background > 0.85, got {:?}", cleaned_px);
    }
}
