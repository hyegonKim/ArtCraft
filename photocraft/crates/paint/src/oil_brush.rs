//! Photoshop 2027 `Oil Brush Tool` (`유화 브러시 도구`):
//! Multi-bristle impasto & wet-pigment mixing stroke engine supporting the 3 official presets:
//! - `Blocky` (`블록`): flat chisel-ferrule rectangular bristle bundle with crisp palette-knife edges.
//! - `Glaze` (`윤기`): soft translucent glazing wash with smooth pigment leveling.
//! - `Round` (`원형`): classic round filbert/hog-bristle bundle with grooved filament tracks.
//!   Plus `Paint Mix` (`페인트 혼합`, `0.0..=1.0`) controlling wet canvas pigment pickup & smearing.

use photocraft_geom::Rect;
use photocraft_raster::{Surface, from_rgba_into, to_rgba};
use serde::{Deserialize, Serialize};

use crate::StrokePoint;

/// Official Photoshop 2027 Oil Brush presets (`블록` / `윤기` / `원형`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OilBrushPreset {
    Blocky,
    Glaze,
    #[default]
    Round,
}

impl OilBrushPreset {
    pub const ALL: [Self; 3] = [Self::Blocky, Self::Glaze, Self::Round];

    pub fn id(self) -> &'static str {
        match self {
            Self::Blocky => "blocky",
            Self::Glaze => "glaze",
            Self::Round => "round",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Blocky => "Blocky",
            Self::Glaze => "Glaze",
            Self::Round => "Round",
        }
    }

    pub fn from_id(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "blocky" | "블록" => Some(Self::Blocky),
            "glaze" | "윤기" => Some(Self::Glaze),
            "round" | "원형" => Some(Self::Round),
            _ => None,
        }
    }
}

/// Stroke parameters for the Oil Brush Tool.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OilBrushParams {
    pub preset: OilBrushPreset,
    /// Brush diameter in pixels (`1.0..=500.0`).
    pub size: f32,
    /// `Paint Mix` (`페인트 혼합`, `0.0..=1.0`): 0 = pure loaded oil pigment, 1 = wet canvas pickup & smear.
    pub paint_mix: f32,
    /// Overall stroke opacity (`0.0..=1.0`).
    pub opacity: f32,
    /// Loaded oil pigment straight RGBA (`0.0..=1.0`).
    pub color: [f32; 4],
}

impl Default for OilBrushParams {
    fn default() -> Self {
        Self {
            preset: OilBrushPreset::Round,
            size: 28.0,
            paint_mix: 0.45,
            opacity: 1.0,
            color: [0.85, 0.25, 0.18, 1.0],
        }
    }
}

fn build_oil_dabs(points: &[StrokePoint], preset: OilBrushPreset, radius: f32) -> Vec<(f32, f32, f32, f32)> {
    let spacing = match preset {
        OilBrushPreset::Blocky => (radius * 0.22).max(0.75),
        OilBrushPreset::Glaze => (radius * 0.16).max(0.65),
        OilBrushPreset::Round => (radius * 0.18).max(0.70),
    };
    let mut dabs: Vec<(f32, f32, f32, f32)> = Vec::new();
    if points.len() == 1 {
        let p = points[0];
        dabs.push((p.x as f32, p.y as f32, 0.0, p.pressure.clamp(0.05, 1.0)));
        return dabs;
    }
    let mut carry = 0.0f32;
    for seg in points.windows(2) {
        let (ax, ay) = (seg[0].x as f32, seg[0].y as f32);
        let (bx, by) = (seg[1].x as f32, seg[1].y as f32);
        let dx = bx - ax;
        let dy = by - ay;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < 1e-4 {
            continue;
        }
        let angle = dy.atan2(dx);
        if dabs.is_empty() {
            dabs.push((ax, ay, angle, seg[0].pressure.clamp(0.05, 1.0)));
        }
        let mut d = spacing - carry;
        while d <= dist {
            let t = d / dist;
            let pr = seg[0].pressure + (seg[1].pressure - seg[0].pressure) * t;
            dabs.push((ax + dx * t, ay + dy * t, angle, pr.clamp(0.05, 1.0)));
            d += spacing;
        }
        carry = (dist + carry) % spacing;
    }
    dabs
}

/// Renders an Oil Brush stroke onto `surface` within `clip_bounds`, optionally respecting `selection`
/// coverage and `lock_alpha`, and returns the modified pixel bounding [`Rect`].
pub fn paint_oil_stroke(
    surface: &mut Surface,
    points: &[StrokePoint],
    params: &OilBrushParams,
    clip_bounds: Rect,
    selection: Option<&Surface>,
    lock_alpha: bool,
) -> Rect {
    if points.is_empty() || clip_bounds.is_empty() {
        return Rect::EMPTY;
    }
    let radius = (if params.size.is_finite() { params.size * 0.5 } else { 14.0 }).clamp(0.5, 250.0);
    let mix = (if params.paint_mix.is_finite() { params.paint_mix } else { 0.45 }).clamp(0.0, 1.0);
    let opacity = (if params.opacity.is_finite() { params.opacity } else { 1.0 }).clamp(0.0, 1.0);
    if opacity <= 1e-5 {
        return Rect::EMPTY;
    }

    // Compute bounding box of all stroke segments + radius margin.
    let margin = (radius * 1.35).ceil() as i32 + 2;
    let mut x0 = i32::MAX;
    let mut y0 = i32::MAX;
    let mut x1 = i32::MIN;
    let mut y1 = i32::MIN;
    for p in points {
        if !p.x.is_finite() || !p.y.is_finite() {
            continue;
        }
        let px = p.x.round() as i32;
        let py = p.y.round() as i32;
        x0 = x0.min(px - margin);
        y0 = y0.min(py - margin);
        x1 = x1.max(px + margin);
        y1 = y1.max(py + margin);
    }
    if x0 >= x1 || y0 >= y1 {
        return Rect::EMPTY;
    }
    let roi = Rect::new(x0, y0, x1, y1).intersect(&clip_bounds);
    if roi.is_empty() {
        return Rect::EMPTY;
    }

    let fmt = surface.format();
    let ch = fmt.channels();
    let w = roi.width() as usize;
    let h = roi.height() as usize;
    let raw = surface.read_region(roi);
    let mut canvas: Vec<[f32; 4]> = raw.chunks_exact(ch).map(|p| to_rgba(&fmt, p)).collect();
    let orig = canvas.clone();

    let dabs = build_oil_dabs(points, params.preset, radius);

    // 9 virtual bristle filaments across the ferrule width `v in -1..=1` carrying live wet paint!
    let mut bristles = [params.color; 9];

    for (cx, cy, angle, pressure) in dabs {
        let r = (radius * (0.45 + 0.55 * pressure)).max(0.75);
        let (sin_a, cos_a) = angle.sin_cos();

        // Sample canvas pigment at the leading edge of the bristle tips (`Paint Mix`).
        if mix > 1e-4 {
            let lead = r * 0.85;
            for (bi, bcol) in bristles.iter_mut().enumerate() {
                let v_off = ((bi as f32 / 8.0) * 2.0 - 1.0) * r * 0.78;
                let sx = (cx + cos_a * lead - sin_a * v_off - roi.x0 as f32).round() as isize;
                let sy = (cy + sin_a * lead + cos_a * v_off - roi.y0 as f32).round() as isize;
                if sx >= 0 && sx < w as isize && sy >= 0 && sy < h as isize {
                    let under = canvas[sy as usize * w + sx as usize];
                    if under[3] > 0.05 {
                        let pickup_rate = 0.22 * mix;
                        for c in 0..3 {
                            // Replenish slightly toward loaded color `(1 - mix)` while picking up canvas color `mix`.
                            let target = params.color[c] * (1.0 - mix * 0.85) + under[c] * (mix * 0.85);
                            bcol[c] += (target - bcol[c]) * pickup_rate;
                        }
                    }
                }
            }
        }

        let reach = (r * 1.2).ceil() as i32;
        let px0 = ((cx.floor() as i32 - reach).max(roi.x0) - roi.x0) as usize;
        let py0 = ((cy.floor() as i32 - reach).max(roi.y0) - roi.y0) as usize;
        let px1 = ((cx.ceil() as i32 + reach + 1).min(roi.x1) - roi.x0) as usize;
        let py1 = ((cy.ceil() as i32 + reach + 1).min(roi.y1) - roi.y0) as usize;

        for ly in py0..py1 {
            let wy = roi.y0 as f32 + ly as f32 + 0.5;
            let dy = wy - cy;
            for lx in px0..px1 {
                let wx = roi.x0 as f32 + lx as f32 + 0.5;
                let dx = wx - cx;
                // Transform into brush-local coordinates: `u` along stroke tangent, `v` across ferrule.
                let u = (dx * cos_a + dy * sin_a) / r;
                let v = (-dx * sin_a + dy * cos_a) / r;

                let (mask, impasto_ridge) = match params.preset {
                    OilBrushPreset::Blocky => {
                        // Chisel-edge flat brush: rectangular footprint (`|u| <= 0.62, |v| <= 0.96`) + crisp bristle tracks.
                        let ax = (1.0 - (u.abs() - 0.52).max(0.0) / 0.16).clamp(0.0, 1.0);
                        let ay = (1.0 - (v.abs() - 0.88).max(0.0) / 0.14).clamp(0.0, 1.0);
                        let groove = 0.86 + 0.14 * (v * 18.0).cos();
                        let edge_ridge = ((v.abs() - 0.72).max(0.0) / 0.24).clamp(0.0, 1.0) * v.signum() * 0.09;
                        (ax * ay * groove * 0.88, edge_ridge)
                    }
                    OilBrushPreset::Glaze => {
                        // Translucent oil glaze: smooth elliptical dome, low build-up, subtle glossy sheen.
                        let d2 = (u * 1.15).powi(2) + v * v;
                        let fall = (1.0 - d2).max(0.0).powi(2);
                        (fall * 0.26, -u * fall * 0.025)
                    }
                    OilBrushPreset::Round => {
                        // Classic round hog-bristle brush: circular footprint + filament striations + impasto bevel.
                        let d = (u * u + v * v).sqrt();
                        let fall = (1.0 - (d - 0.72).max(0.0) / 0.28).clamp(0.0, 1.0);
                        let striation = 0.78 + 0.22 * (v * 22.0 + u * 2.5).sin();
                        let ridge = (v * 22.0).cos() * 0.045 * fall;
                        (fall * striation * 0.72, ridge)
                    }
                };

                let dab_a = (mask * opacity * params.color[3]).clamp(0.0, 1.0);
                if dab_a <= 1e-4 {
                    continue;
                }

                // Interpolate color across the 9 bristle filaments at `v in -1..1`.
                let bf = ((v * 0.5 + 0.5).clamp(0.0, 1.0)) * 8.0;
                let bi0 = (bf.floor() as usize).min(8);
                let bi1 = (bi0 + 1).min(8);
                let bt = bf - bi0 as f32;
                let mut paint_rgb = [0.0f32; 3];
                for c in 0..3 {
                    paint_rgb[c] = (bristles[bi0][c] * (1.0 - bt) + bristles[bi1][c] * bt + impasto_ridge).clamp(0.0, 1.0);
                }

                let dst = &mut canvas[ly * w + lx];
                let out_a = if lock_alpha {
                    dst[3]
                } else {
                    (dab_a + dst[3] * (1.0 - dab_a)).clamp(0.0, 1.0)
                };
                if out_a > 1e-6 {
                    let eff_a = if lock_alpha { dab_a.min(dst[3]) } else { dab_a };
                    for c in 0..3 {
                        dst[c] = ((paint_rgb[c] * eff_a + dst[c] * dst[3] * (1.0 - eff_a)) / out_a).clamp(0.0, 1.0);
                    }
                }
                dst[3] = out_a;
            }
        }
    }

    // Apply selection mask if active!
    if let Some(sel) = selection {
        for ly in 0..h {
            let wy = roi.y0 + ly as i32;
            for lx in 0..w {
                let wx = roi.x0 + lx as i32;
                let cov = sel.sample_channel(wx, wy, 0).clamp(0.0, 1.0);
                if cov < 1.0 {
                    let idx = ly * w + lx;
                    for c in 0..4 {
                        canvas[idx][c] = orig[idx][c] * (1.0 - cov) + canvas[idx][c] * cov;
                    }
                }
            }
        }
    }

    let mut out_buf = vec![0.0f32; w * h * ch];
    for (p, dst) in canvas.iter().zip(out_buf.chunks_exact_mut(ch)) {
        from_rgba_into(&fmt, *p, dst);
    }
    surface.write_region(roi, &out_buf);
    roi
}

#[cfg(test)]
mod tests {
    use super::*;
    use photocraft_color::PixelFormat;

    #[test]
    fn oil_brush_presets_and_paint_mix_pick_up_canvas_pigment() {
        let fmt = PixelFormat::RGBA8;
        let bounds = Rect::new(0, 0, 80, 40);
        let mut s_pure = Surface::new(fmt);
        let mut s_mixed = Surface::new(fmt);
        // Left half bright blue, right half white.
        for s in [&mut s_pure, &mut s_mixed] {
            s.fill_rect(Rect::new(0, 0, 40, 40), &[0.0, 0.2, 1.0, 1.0]);
            s.fill_rect(Rect::new(40, 0, 80, 40), &[1.0, 1.0, 1.0, 1.0]);
        }
        let pts = vec![StrokePoint::new(10.0, 20.0, 1.0), StrokePoint::new(70.0, 20.0, 1.0)];
        // Stroke with yellow paint across blue -> white boundary!
        paint_oil_stroke(
            &mut s_pure,
            &pts,
            &OilBrushParams { preset: OilBrushPreset::Blocky, size: 16.0, paint_mix: 0.0, opacity: 1.0, color: [1.0, 0.9, 0.0, 1.0] },
            bounds,
            None,
            false,
        );
        paint_oil_stroke(
            &mut s_mixed,
            &pts,
            &OilBrushParams { preset: OilBrushPreset::Blocky, size: 16.0, paint_mix: 0.9, opacity: 1.0, color: [1.0, 0.9, 0.0, 1.0] },
            bounds,
            None,
            false,
        );
        // At x=48 (just past blue region), high Paint Mix carries picked-up blue pigment onto the white half!
        let p_pure = s_pure.pixel(48, 20);
        let p_mix = s_mixed.pixel(48, 20);
        assert!(p_mix[2] > p_pure[2] + 0.12, "Paint Mix=0.9 should carry blue pigment ({:?}) vs Paint Mix=0.0 ({:?})", p_mix, p_pure);
    }
}
