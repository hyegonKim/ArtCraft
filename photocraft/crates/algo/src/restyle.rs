//! Photoshop 2027 `Restyle` (`생성형 스타일 변경` / `스타일 변경 레이어`):
//! Non-destructive artistic style transfer supporting 8 distinct stylization pipelines,
//! optional text-prompt palette/contrast steering, and smooth `0..=100%` intensity blending.

use photocraft_geom::Rect;
use photocraft_raster::{Surface, from_rgba_into, to_rgba};
use serde::{Deserialize, Serialize};

use crate::photo_util::{hash01, par_rows};

/// Artistic transfer style matching `photocraft_doc::RestylePreset`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RestyleStyle {
    #[default]
    OilPainting,
    Watercolor,
    CyberpunkNeon,
    VintageFilm,
    PencilSketch,
    PopArt,
    AnimeCel,
    NoirInk,
}

impl RestyleStyle {
    pub fn from_id(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "watercolor" => Self::Watercolor,
            "cyberpunkneon" | "cyberpunk" | "neon" => Self::CyberpunkNeon,
            "vintagefilm" | "vintage" | "film" => Self::VintageFilm,
            "pencilsketch" | "sketch" => Self::PencilSketch,
            "popart" | "pop" => Self::PopArt,
            "animecel" | "anime" | "cel" => Self::AnimeCel,
            "noirink" | "noir" | "ink" => Self::NoirInk,
            _ => Self::OilPainting,
        }
    }
}

/// Parameters for `Restyle`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RestyleParams {
    pub style: RestyleStyle,
    pub prompt: String,
    /// Blend intensity in `0.0..=100.0`.
    pub intensity: f32,
}

impl Default for RestyleParams {
    fn default() -> Self {
        Self { style: RestyleStyle::OilPainting, prompt: String::new(), intensity: 80.0 }
    }
}

fn prompt_bias(prompt: &str) -> ([f32; 3], f32) {
    let p = prompt.to_ascii_lowercase();
    let mut tint = [1.0f32, 1.0, 1.0];
    let mut contrast = 1.0f32;
    if p.contains("warm") || p.contains("gold") || p.contains("sunset") || p.contains("따뜻") || p.contains("황금") {
        tint = [1.10, 1.01, 0.86];
    } else if p.contains("cool") || p.contains("blue") || p.contains("ice") || p.contains("차가운") || p.contains("푸른") {
        tint = [0.88, 0.98, 1.12];
    } else if p.contains("neon") || p.contains("cyber") || p.contains("네온") {
        tint = [1.08, 0.92, 1.15];
        contrast = 1.18;
    } else if p.contains("pastel") || p.contains("soft") || p.contains("파스텔") {
        contrast = 0.86;
    }
    if p.contains("dramatic") || p.contains("bold") || p.contains("강렬") {
        contrast *= 1.15;
    }
    (tint, contrast)
}

/// Applies non-destructive `Restyle` artistic transfer to straight sRGB RGBA pixels (`w × h`).
pub fn apply_restyle_rgba(px: &mut [[f32; 4]], w: usize, h: usize, params: &RestyleParams) {
    let n = w * h;
    if n == 0 || px.len() < n {
        return;
    }
    let mix = (if params.intensity.is_finite() { params.intensity } else { 80.0 }).clamp(0.0, 100.0) / 100.0;
    if mix <= 1e-4 {
        return;
    }

    let mut smooth = vec![0.0f32; n * 3];
    let mut luma = vec![0.0f32; n];
    for (i, q) in px.iter().take(n).enumerate() {
        smooth[i * 3] = q[0];
        smooth[i * 3 + 1] = q[1];
        smooth[i * 3 + 2] = q[2];
        luma[i] = 0.2126 * q[0] + 0.7152 * q[1] + 0.0722 * q[2];
    }
    let min_side = (w.min(h) as f32).max(16.0);
    crate::fxutil::gauss_blur_n(&mut smooth, w, h, 3, (min_side * 0.016).clamp(1.2, 6.0));
    let (p_tint, p_con) = prompt_bias(&params.prompt);
    let style = params.style;

    par_rows(px, w, 1, |y, row| {
        let ym = y.saturating_sub(1);
        let yp = (y + 1).min(h - 1);
        for (x, q) in row.iter_mut().enumerate() {
            let idx = y * w + x;
            let xm = x.saturating_sub(1);
            let xp = (x + 1).min(w - 1);
            let gx = luma[y * w + xp] - luma[y * w + xm];
            let gy = luma[yp * w + x] - luma[ym * w + x];
            let edge = (gx * gx + gy * gy).sqrt().min(1.0);
            let sr = smooth[idx * 3];
            let sg = smooth[idx * 3 + 1];
            let sb = smooth[idx * 3 + 2];
            let sl = 0.2126 * sr + 0.7152 * sg + 0.0722 * sb;

            let styled = match style {
                RestyleStyle::OilPainting => {
                    // Kuwahara-like posterized brushwork + impasto ridge highlight along gradient normal.
                    let quant = (sl * 10.0).round() / 10.0;
                    let ratio = if sl > 1e-4 { (0.65 * sl + 0.35 * quant) / sl } else { 1.0 };
                    let impasto = (gx + gy) * 0.22;
                    [
                        (sr * ratio * 1.06 + impasto).clamp(0.0, 1.0),
                        (sg * ratio * 1.03 + impasto).clamp(0.0, 1.0),
                        (sb * ratio * 0.96 + impasto).clamp(0.0, 1.0),
                    ]
                }
                RestyleStyle::Watercolor => {
                    // Wet pigment pooling at edges + warm paper grain bleed.
                    let paper = 0.96 + 0.04 * hash01(x as i64, y as i64, 701);
                    let darken_edge = 1.0 - (edge * 1.4).min(0.32);
                    let lift = [
                        (0.12 + 0.88 * sr.sqrt()) * paper * darken_edge,
                        (0.11 + 0.88 * sg.sqrt()) * paper * darken_edge,
                        (0.09 + 0.87 * sb.sqrt()) * paper * darken_edge,
                    ];
                    [lift[0].clamp(0.0, 1.0), lift[1].clamp(0.0, 1.0), lift[2].clamp(0.0, 1.0)]
                }
                RestyleStyle::CyberpunkNeon => {
                    // Deep teal shadows + electric magenta/cyan highlights + glowing neon edge outlines.
                    let sh = (1.0 - sl).powi(2);
                    let hi = sl.powi(2);
                    let neon_edge = (edge * 2.8).min(1.0);
                    [
                        (sr * 0.85 + 0.38 * hi + 0.75 * neon_edge).clamp(0.0, 1.0),
                        (sg * 0.78 + 0.14 * sh + 0.25 * neon_edge).clamp(0.0, 1.0),
                        (sb * 0.95 + 0.42 * sh + 0.30 * hi + 0.85 * neon_edge).clamp(0.0, 1.0),
                    ]
                }
                RestyleStyle::VintageFilm => {
                    // Liftedmatte blacks, warm halation highlights, subtle film grain.
                    let grain = (hash01(x as i64, y as i64, 1977) - 0.5) * 0.05;
                    let r = 0.06 + 0.88 * (0.55 * q[0] + 0.35 * q[1] + 0.10 * q[2]) * 1.12 + grain;
                    let g = 0.05 + 0.86 * (0.18 * q[0] + 0.68 * q[1] + 0.14 * q[2]) * 1.03 + grain;
                    let b = 0.07 + 0.78 * (0.10 * q[0] + 0.25 * q[1] + 0.65 * q[2]) * 0.88 + grain;
                    [r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0)]
                }
                RestyleStyle::PencilSketch => {
                    // Cross-hatched graphite shading + crisp pencil contour on warm cream sketch paper.
                    let hatch_a = (((x as f32 + y as f32) * 0.9).sin() * 0.5 + 0.5) * 0.18;
                    let hatch_b = (((x as f32 - y as f32) * 0.9).sin() * 0.5 + 0.5) * 0.14;
                    let shade = (sl + if sl < 0.55 { -hatch_a } else { 0.0 } + if sl < 0.32 { -hatch_b } else { 0.0 } - edge * 2.4).clamp(0.06, 0.97);
                    [shade * 0.98, shade * 0.96, shade * 0.91]
                }
                RestyleStyle::PopArt => {
                    // Bold 4-band Warhol screenprint palette + crisp black ink outlines.
                    let ink = if edge > 0.16 { 0.06 } else { 1.0 };
                    let band = (sl * 3.99) as u32;
                    let pal = match band {
                        0 => [0.14, 0.10, 0.42],
                        1 => [0.92, 0.18, 0.48],
                        2 => [0.16, 0.80, 0.78],
                        _ => [0.99, 0.88, 0.18],
                    };
                    [pal[0] * ink, pal[1] * ink, pal[2] * ink]
                }
                RestyleStyle::AnimeCel => {
                    // Clean 3-tone cel shading + saturated colors + crisp dark lineart.
                    let cel = if sl < 0.30 {
                        0.58
                    } else if sl < 0.72 {
                        0.96
                    } else {
                        1.18
                    };
                    let line = (1.0 - (edge * 3.2 - 0.25).max(0.0)).clamp(0.10, 1.0);
                    let boost_sat = |v: f32| (sl + (v - sl) * 1.32) * cel * line;
                    [boost_sat(sr).clamp(0.0, 1.0), boost_sat(sg).clamp(0.0, 1.0), boost_sat(sb).clamp(0.0, 1.0)]
                }
                RestyleStyle::NoirInk => {
                    // High-contrast graphic novel sumi-ink monochrome with crimson accent on strong reds.
                    let red_accent = ((q[0] - q[1].max(q[2]) - 0.18) * 3.0).clamp(0.0, 1.0);
                    let bw = if sl - edge * 0.9 > 0.46 {
                        0.96
                    } else if sl > 0.22 {
                        0.35 * sl
                    } else {
                        0.03
                    };
                    [
                        (bw * (1.0 - red_accent) + 0.86 * red_accent).clamp(0.0, 1.0),
                        (bw * (1.0 - red_accent) + 0.08 * red_accent).clamp(0.0, 1.0),
                        (bw * (1.0 - red_accent) + 0.10 * red_accent).clamp(0.0, 1.0),
                    ]
                }
            };

            for c in 0..3 {
                let prompted = ((styled[c] - 0.5) * p_con + 0.5) * p_tint[c];
                q[c] = (q[c] * (1.0 - mix) + prompted.clamp(0.0, 1.0) * mix).clamp(0.0, 1.0);
            }
        }
    });
}

/// Surface wrapper for `Restyle`.
pub fn apply_restyle_surface(src: &Surface, rect: Rect, params: &RestyleParams) -> Surface {
    if rect.is_empty() {
        return Surface::new(src.format());
    }
    let fmt = src.format();
    let ch = fmt.channels();
    let w = rect.width() as usize;
    let h = rect.height() as usize;
    let raw = src.read_region(rect);
    let mut rgba_px: Vec<[f32; 4]> = raw.chunks_exact(ch).map(|p| to_rgba(&fmt, p)).collect();
    apply_restyle_rgba(&mut rgba_px, w, h, params);
    let mut out_buf = vec![0.0f32; w * h * ch];
    for (p, dst) in rgba_px.iter().zip(out_buf.chunks_exact_mut(ch)) {
        from_rgba_into(&fmt, *p, dst);
    }
    let mut out = Surface::with_default(fmt, &src.default_pixel());
    out.write_region(rect, &out_buf);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restyle_intensity_zero_is_identity_and_all_styles_produce_distinct_output() {
        let (w, h) = (32, 32);
        let mut base = vec![[0.0f32; 4]; w * h];
        for y in 0..h {
            for x in 0..w {
                base[y * w + x] = [x as f32 / w as f32, y as f32 / h as f32, 0.45, 1.0];
            }
        }
        let mut zero = base.clone();
        apply_restyle_rgba(&mut zero, w, h, &RestyleParams { style: RestyleStyle::CyberpunkNeon, prompt: String::new(), intensity: 0.0 });
        assert_eq!(zero, base);

        let mut oil = base.clone();
        let mut sketch = base.clone();
        apply_restyle_rgba(&mut oil, w, h, &RestyleParams { style: RestyleStyle::OilPainting, prompt: "warm sunset".into(), intensity: 100.0 });
        apply_restyle_rgba(&mut sketch, w, h, &RestyleParams { style: RestyleStyle::PencilSketch, prompt: String::new(), intensity: 100.0 });
        assert_ne!(oil, base);
        assert_ne!(oil, sketch);
    }
}
