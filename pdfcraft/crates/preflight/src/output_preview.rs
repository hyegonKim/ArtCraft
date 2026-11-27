//! Acrobat Pro Output Preview: CMYK channel separations (`C`, `M`, `Y`, `K` plate visibility mask)
//! and Total Area Coverage (`TAC % = C + M + Y + K %`) press ink-limit inspection & heatmap.

use pdfcraft_cos::{DecodeBudget, Dict, Document, Object};

/// Standard web-offset / coated sheetfed ink limits offered in Output Preview presets.
pub const DEFAULT_INK_LIMIT_PERCENT: f32 = 300.0;
pub const NEWSPRINT_INK_LIMIT_PERCENT: f32 = 240.0;

/// Visibility mask for the four process plates in Output Preview.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CmykPlates {
    pub cyan: bool,
    pub magenta: bool,
    pub yellow: bool,
    pub black: bool,
}

impl Default for CmykPlates {
    fn default() -> Self {
        Self::ALL
    }
}

impl CmykPlates {
    pub const ALL: Self = Self { cyan: true, magenta: true, yellow: true, black: true };
    pub const CYAN_ONLY: Self = Self { cyan: true, magenta: false, yellow: false, black: false };
    pub const MAGENTA_ONLY: Self = Self { cyan: false, magenta: true, yellow: false, black: false };
    pub const YELLOW_ONLY: Self = Self { cyan: false, magenta: false, yellow: true, black: false };
    pub const BLACK_ONLY: Self = Self { cyan: false, magenta: false, yellow: false, black: true };

    pub fn active_count(self) -> usize {
        usize::from(self.cyan) + usize::from(self.magenta) + usize::from(self.yellow) + usize::from(self.black)
    }

    pub fn all_on(self) -> bool {
        self == Self::ALL
    }
}

/// Summary statistics for CMYK separations and Total Area Coverage (`TAC %`) on a page.
#[derive(Clone, Debug, PartialEq)]
pub struct TacSummary {
    pub width: u32,
    pub height: u32,
    pub pixel_count: usize,
    /// Configured press ink limit (e.g. `300.0` or `240.0`).
    pub ink_limit_percent: f32,
    /// Peak Total Area Coverage (`0.0..=400.0`) observed across all pixels and explicit CMYK operators.
    pub max_tac_percent: f32,
    /// Mean Total Area Coverage across non-paper (inked) pixels, or `0.0` on a blank page.
    pub mean_tac_percent: f32,
    /// Coordinates `[x, y]` of the highest-TAC raster pixel, when `pixel_count > 0`.
    pub max_tac_at: Option<[u32; 2]>,
    /// Number of raster pixels whose `C + M + Y + K %` exceeds `ink_limit_percent`.
    pub over_limit_pixels: usize,
    /// Percentage (`0.0..=100.0`) of page area exceeding `ink_limit_percent`.
    pub over_limit_percent: f32,
    /// Mean ink coverage (`0.0..=100.0`) per plate `[C, M, Y, K]`.
    pub plate_mean_percent: [f32; 4],
    /// Maximum ink coverage (`0.0..=100.0`) per plate `[C, M, Y, K]`.
    pub plate_max_percent: [f32; 4],
    /// Peak explicit `/DeviceCMYK` operator coverage (`k` / `K` / CMYK image) in the PDF content stream.
    pub content_stream_max_tac: f32,
    /// Number of explicit `/DeviceCMYK` vector operators in the page content stream exceeding `ink_limit_percent`.
    pub content_stream_over_limit_ops: usize,
}

/// Simulated plate separation preview RGBA8 image, per-pixel TAC map, and [`TacSummary`].
#[derive(Clone, Debug, PartialEq)]
pub struct OutputPreviewResult {
    pub summary: TacSummary,
    /// Simulated composite RGBA8 (`width * height * 4`) showing enabled CMYK plates and optional TAC heatmap.
    pub preview_rgba: Vec<u8>,
    /// Per-pixel TAC in tenths of a percent (`0..=4000` for `0.0%..=400.0%`), for cursor inspection.
    pub tac_tenths: Vec<u16>,
}

/// Content-stream CMYK inspection summary from [`inspect_page_cmyk_ink`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PageCmykInspection {
    pub max_tac_percent: f32,
    pub over_limit_ops: usize,
    pub has_device_cmyk: bool,
}

/// Compute CMYK channel separations and Total Area Coverage (`TAC %`) directly from 8-bit CMYK
/// pixels (`width * height * 4`, where `255 == 100%` ink on each plate).
pub fn analyze_cmyk_output_preview(
    cmyk: &[u8],
    width: u32,
    height: u32,
    plates: CmykPlates,
    ink_limit_percent: f32,
    show_tac_heatmap: bool,
) -> Result<OutputPreviewResult, String> {
    let (w, h) = (width as usize, height as usize);
    let px_count = w.checked_mul(h).ok_or_else(|| "image dimensions overflow".to_string())?;
    if px_count == 0 || px_count.checked_mul(4) != Some(cmyk.len()) {
        return Err("CMYK buffer size does not match width × height × 4".to_string());
    }
    let limit = clamp_ink_limit(ink_limit_percent);
    Ok(evaluate_separations(
        EvalSpec { px_count, width, height, plates, ink_limit_percent: limit, show_tac_heatmap, stream_info: PageCmykInspection::default() },
        |i| {
            let p = i * 4;
            (
                [
                    f32::from(cmyk[p]) * (100.0 / 255.0),
                    f32::from(cmyk[p + 1]) * (100.0 / 255.0),
                    f32::from(cmyk[p + 2]) * (100.0 / 255.0),
                    f32::from(cmyk[p + 3]) * (100.0 / 255.0),
                ],
                255u8,
            )
        },
    ))
}

/// Compute CMYK channel separations and Total Area Coverage (`TAC %`) from rendered RGBA8 page
/// pixels (`width * height * 4`), optionally enriched with explicit `/DeviceCMYK` operator peaks.
pub fn analyze_rgba_output_preview(
    rgba: &[u8],
    width: u32,
    height: u32,
    plates: CmykPlates,
    ink_limit_percent: f32,
    show_tac_heatmap: bool,
) -> Result<OutputPreviewResult, String> {
    analyze_rgba_with_stream_info(rgba, width, height, plates, ink_limit_percent, show_tac_heatmap, PageCmykInspection::default())
}

/// [`analyze_rgba_output_preview`] combined with [`inspect_page_cmyk_ink`] findings from the PDF
/// page's content stream so rich-black CMYK fills above 300% are never hidden by sRGB clipping.
pub fn analyze_rgba_with_stream_info(
    rgba: &[u8],
    width: u32,
    height: u32,
    plates: CmykPlates,
    ink_limit_percent: f32,
    show_tac_heatmap: bool,
    stream_info: PageCmykInspection,
) -> Result<OutputPreviewResult, String> {
    let (w, h) = (width as usize, height as usize);
    let px_count = w.checked_mul(h).ok_or_else(|| "image dimensions overflow".to_string())?;
    if px_count == 0 || px_count.checked_mul(4) != Some(rgba.len()) {
        return Err("RGBA buffer size does not match width × height × 4".to_string());
    }
    let limit = clamp_ink_limit(ink_limit_percent);
    Ok(evaluate_separations(EvalSpec { px_count, width, height, plates, ink_limit_percent: limit, show_tac_heatmap, stream_info }, |i| {
        let p = i * 4;
        (rgb_to_press_cmyk_percent(rgba[p], rgba[p + 1], rgba[p + 2], rgba[p + 3]), rgba[p + 3])
    }))
}

/// Convert an RGBA8 sample (composited over paper white) into press CMYK percentages (`0..=100`
/// per plate) using medium-GCR separation (`GCR = 0.50` start at neutral `0.15`), which models
/// standard coated sheetfed/web-offset separations where dark neutral blacks carry rich CMY undercolour
/// (~310–330% TAC on unseparated RGB `(0, 0, 0)`).
pub fn rgb_to_press_cmyk_percent(r: u8, g: u8, b: u8, a: u8) -> [f32; 4] {
    let alpha = f32::from(a) / 255.0;
    let rf = (f32::from(r) / 255.0) * alpha + (1.0 - alpha);
    let gf = (f32::from(g) / 255.0) * alpha + (1.0 - alpha);
    let bf = (f32::from(b) / 255.0) * alpha + (1.0 - alpha);

    let c0 = (1.0 - rf).clamp(0.0, 1.0);
    let m0 = (1.0 - gf).clamp(0.0, 1.0);
    let y0 = (1.0 - bf).clamp(0.0, 1.0);
    let k_raw = c0.min(m0).min(y0);
    if k_raw <= 1e-5 {
        return [c0 * 100.0, m0 * 100.0, y0 * 100.0, 0.0];
    }
    // Medium GCR curve: starts generating K above 15% neutral density, removing 35% of undercolour
    // so RGB rich darks still flag TAC > 300% when unprofiled (matching Acrobat's uncalibrated RGB->CMYK warning).
    let k_gen = ((k_raw - 0.15).max(0.0) / 0.85).powf(1.15) * 0.95;
    let ucr = k_gen * 0.30;
    let c = (c0 - ucr).clamp(0.0, 1.0) * 100.0;
    let m = (m0 - ucr).clamp(0.0, 1.0) * 100.0;
    let y = (y0 - ucr).clamp(0.0, 1.0) * 100.0;
    let k = k_gen.clamp(0.0, 1.0) * 100.0;
    [c, m, y, k]
}

/// Inspect page `page` (0-based) of `doc` for explicit `/DeviceCMYK` vector colour operators
/// (`k`, `K`, and 4-operand `sc`/`scn` in `/DeviceCMYK`) and `/DeviceCMYK` image XObjects.
pub fn inspect_page_cmyk_ink(doc: &Document, page: usize, ink_limit_percent: f32) -> PageCmykInspection {
    let limit = clamp_ink_limit(ink_limit_percent);
    let mut out = PageCmykInspection::default();
    let Some(p) = pdfcraft_model::pages(doc).into_iter().nth(page) else {
        return out;
    };
    let mut budget = DecodeBudget::page();
    inspect_dict_streams(doc, &p.dict, limit, 0, &mut budget, &mut out);
    out
}

fn inspect_dict_streams(doc: &Document, dict: &Dict, limit: f32, depth: usize, budget: &mut DecodeBudget, out: &mut PageCmykInspection) {
    if depth > 8 {
        return;
    }
    let streams: Vec<Object> = match dict.get(b"Contents").map(|c| doc.resolve(c)) {
        Some(c) => match &*c {
            Object::Array(arr) => arr.clone(),
            Object::Stream(_) => vec![dict.get(b"Contents").cloned().unwrap_or(Object::Null)],
            _ => Vec::new(),
        },
        None => Vec::new(),
    };
    let mut pieces = Vec::new();
    for obj in streams {
        if let Object::Stream(s) = &*doc.resolve(&obj) {
            pieces.push(budget.decode(s).unwrap_or_default());
        }
    }
    if !pieces.is_empty() {
        let joined = pdfcraft_content::Pieces::join(&pieces);
        scan_ops_for_cmyk(joined.data(), limit, out);
    }

    // Also inspect Form and Image XObjects in `/Resources /XObject`.
    let Some(res) = dict.get(b"Resources").map(|r| doc.resolve(r)).and_then(|r| r.as_dict().cloned()) else {
        return;
    };
    let Some(xobjs) = res.get(b"XObject").map(|x| doc.resolve(x)).and_then(|x| x.as_dict().cloned()) else {
        return;
    };
    for (_, xobj_ref) in xobjs.iter() {
        let resolved = doc.resolve(xobj_ref);
        let Object::Stream(st) = &*resolved else { continue };
        match st.dict.name(b"Subtype") {
            Some(b"Form") => {
                if let Ok(bytes) = budget.decode(st) {
                    scan_ops_for_cmyk(&bytes, limit, out);
                }
                inspect_dict_streams(doc, &st.dict, limit, depth + 1, budget, out);
            }
            Some(b"Image") if st.dict.name(b"ColorSpace") == Some(b"DeviceCMYK") => {
                out.has_device_cmyk = true;
                let is_8bit = st.dict.get(b"BitsPerComponent").map(|b| matches!(&*doc.resolve(b), Object::Int(8))).unwrap_or(true);
                if is_8bit && let Ok(raw) = budget.decode(st) {
                    for px in raw.as_chunks::<4>().0.iter().take(250_000) {
                        let tac = (f32::from(px[0]) + f32::from(px[1]) + f32::from(px[2]) + f32::from(px[3])) * (100.0 / 255.0);
                        out.max_tac_percent = out.max_tac_percent.max(tac);
                        if tac > limit {
                            out.over_limit_ops = out.over_limit_ops.saturating_add(1);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn scan_ops_for_cmyk(data: &[u8], limit: f32, out: &mut PageCmykInspection) {
    let mut fill_cmyk = false;
    let mut stroke_cmyk = false;
    for op in pdfcraft_content::ops(data) {
        match op.op.as_slice() {
            b"cs" => {
                fill_cmyk = op.operands.first().and_then(|o| o.as_name()) == Some(b"DeviceCMYK");
                if fill_cmyk {
                    out.has_device_cmyk = true;
                }
            }
            b"CS" => {
                stroke_cmyk = op.operands.first().and_then(|o| o.as_name()) == Some(b"DeviceCMYK");
                if stroke_cmyk {
                    out.has_device_cmyk = true;
                }
            }
            b"k" | b"K" => {
                if let Some([c, m, y, k]) = op.nums::<4>() {
                    record_cmyk_quad([c, m, y, k], limit, out);
                }
            }
            b"sc" | b"scn" if fill_cmyk => {
                if let Some([c, m, y, k]) = op.nums::<4>() {
                    record_cmyk_quad([c, m, y, k], limit, out);
                }
            }
            b"SC" | b"SCN" if stroke_cmyk => {
                if let Some([c, m, y, k]) = op.nums::<4>() {
                    record_cmyk_quad([c, m, y, k], limit, out);
                }
            }
            _ => {}
        }
    }
}

fn record_cmyk_quad(vals: [f64; 4], limit: f32, out: &mut PageCmykInspection) {
    if vals.iter().any(|v| !v.is_finite()) {
        return;
    }
    out.has_device_cmyk = true;
    let tac = vals.iter().map(|v| v.clamp(0.0, 1.0) as f32 * 100.0).sum::<f32>();
    out.max_tac_percent = out.max_tac_percent.max(tac);
    if tac > limit {
        out.over_limit_ops = out.over_limit_ops.saturating_add(1);
    }
}

fn clamp_ink_limit(limit: f32) -> f32 {
    if limit.is_finite() { limit.clamp(100.0, 400.0) } else { DEFAULT_INK_LIMIT_PERCENT }
}

struct EvalSpec {
    px_count: usize,
    width: u32,
    height: u32,
    plates: CmykPlates,
    ink_limit_percent: f32,
    show_tac_heatmap: bool,
    stream_info: PageCmykInspection,
}

fn evaluate_separations(spec: EvalSpec, mut sample_at: impl FnMut(usize) -> ([f32; 4], u8)) -> OutputPreviewResult {
    let EvalSpec { px_count, width, height, plates, ink_limit_percent, show_tac_heatmap, stream_info } = spec;
    let w = width as usize;
    let mut preview_rgba = Vec::with_capacity(px_count * 4);
    let mut tac_tenths = Vec::with_capacity(px_count);

    let mut max_tac = 0.0f32;
    let mut max_at: Option<[u32; 2]> = None;
    let mut inked_tac_sum = 0.0f64;
    let mut inked_count = 0usize;
    let mut over_limit_pixels = 0usize;
    let mut plate_sum = [0.0f64; 4];
    let mut plate_max = [0.0f32; 4];

    let active_single_plate = (plates.active_count() == 1).then_some(if plates.cyan {
        0usize
    } else if plates.magenta {
        1
    } else if plates.yellow {
        2
    } else {
        3
    });

    for idx in 0..px_count {
        let ([c, m, y, k], alpha) = sample_at(idx);
        let c = c.clamp(0.0, 100.0);
        let m = m.clamp(0.0, 100.0);
        let y = y.clamp(0.0, 100.0);
        let k = k.clamp(0.0, 100.0);
        let tac = c + m + y + k;

        plate_sum[0] += f64::from(c);
        plate_sum[1] += f64::from(m);
        plate_sum[2] += f64::from(y);
        plate_sum[3] += f64::from(k);
        plate_max[0] = plate_max[0].max(c);
        plate_max[1] = plate_max[1].max(m);
        plate_max[2] = plate_max[2].max(y);
        plate_max[3] = plate_max[3].max(k);

        if tac > 0.5 {
            inked_tac_sum += f64::from(tac);
            inked_count += 1;
        }
        if tac > max_tac {
            max_tac = tac;
            max_at = Some([(idx % w) as u32, (idx / w) as u32]);
        }
        let is_over = tac > ink_limit_percent;
        if is_over {
            over_limit_pixels += 1;
        }
        tac_tenths.push((tac * 10.0).round().clamp(0.0, 4000.0) as u16);

        let rgb = render_pixel_preview([c, m, y, k], tac, ink_limit_percent, show_tac_heatmap, plates, active_single_plate);
        preview_rgba.extend_from_slice(&[rgb[0], rgb[1], rgb[2], alpha]);
    }

    let denom = px_count.max(1) as f64;
    let summary = TacSummary {
        width,
        height,
        pixel_count: px_count,
        ink_limit_percent,
        max_tac_percent: max_tac.max(stream_info.max_tac_percent),
        mean_tac_percent: if inked_count > 0 { (inked_tac_sum / inked_count as f64) as f32 } else { 0.0 },
        max_tac_at: max_at,
        over_limit_pixels,
        over_limit_percent: (over_limit_pixels as f64 * 100.0 / denom) as f32,
        plate_mean_percent: [
            (plate_sum[0] / denom) as f32,
            (plate_sum[1] / denom) as f32,
            (plate_sum[2] / denom) as f32,
            (plate_sum[3] / denom) as f32,
        ],
        plate_max_percent: plate_max,
        content_stream_max_tac: stream_info.max_tac_percent,
        content_stream_over_limit_ops: stream_info.over_limit_ops,
    };

    OutputPreviewResult { summary, preview_rgba, tac_tenths }
}

fn render_pixel_preview(
    vals: [f32; 4],
    tac: f32,
    ink_limit_percent: f32,
    show_tac_heatmap: bool,
    plates: CmykPlates,
    active_single_plate: Option<usize>,
) -> [u8; 3] {
    let [c, m, y, k] = vals;
    if show_tac_heatmap && tac > ink_limit_percent {
        let excess = ((tac - ink_limit_percent) / 80.0).clamp(0.0, 1.0);
        if excess < 0.5 {
            let t = excess * 2.0;
            [(30.0 + 225.0 * t).round() as u8, 225, 45]
        } else {
            let t = (excess - 0.5) * 2.0;
            [255, (225.0 * (1.0 - 0.85 * t)).round() as u8, 35]
        }
    } else if show_tac_heatmap {
        let gray = composite_plates_gray(c, m, y, k, plates);
        [gray, gray, gray]
    } else if let Some(single) = active_single_plate {
        single_plate_rgb(single, vals[single] / 100.0)
    } else {
        composite_plates_rgb(c, m, y, k, plates)
    }
}

fn composite_plates_rgb(c: f32, m: f32, y: f32, k: f32, plates: CmykPlates) -> [u8; 3] {
    let cf = if plates.cyan { c / 100.0 } else { 0.0 };
    let mf = if plates.magenta { m / 100.0 } else { 0.0 };
    let yf = if plates.yellow { y / 100.0 } else { 0.0 };
    let kf = if plates.black { k / 100.0 } else { 0.0 };
    [
        ((1.0 - cf) * (1.0 - kf) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((1.0 - mf) * (1.0 - kf) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((1.0 - yf) * (1.0 - kf) * 255.0).round().clamp(0.0, 255.0) as u8,
    ]
}

fn composite_plates_gray(c: f32, m: f32, y: f32, k: f32, plates: CmykPlates) -> u8 {
    let [r, g, b] = composite_plates_rgb(c, m, y, k, plates);
    let lum = 0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b);
    // Lighten slightly toward paper white so TAC heatmap colours pop.
    (lum * 0.65 + 255.0 * 0.35).round().clamp(0.0, 255.0) as u8
}

fn single_plate_rgb(plate_idx: usize, ink: f32) -> [u8; 3] {
    let t = ink.clamp(0.0, 1.0);
    // Process ink colours on coated stock: Process Cyan, Process Magenta, Process Yellow, Process Black.
    let solid = match plate_idx {
        0 => [0.0, 166.0, 226.0],
        1 => [226.0, 0.0, 122.0],
        2 => [255.0, 237.0, 0.0],
        _ => [24.0, 24.0, 24.0],
    };
    [
        (255.0 * (1.0 - t) + solid[0] * t).round() as u8,
        (255.0 * (1.0 - t) + solid[1] * t).round() as u8,
        (255.0 * (1.0 - t) + solid[2] * t).round() as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmyk_output_preview_flags_pixels_exceeding_300_and_240_tac_limits() {
        // 4 pixels (2x2):
        // (0,0) paper white: 0%
        // (1,0) pure cyan: 100%
        // (0,1) coated rich black: C=60, M=50, Y=50, K=95 -> 255% TAC (exceeds 240%, passes 300%)
        // (1,1) registration 4-plate black: 100+100+100+100 -> 400% TAC (exceeds both)
        let pct = |p: f32| (p * 2.55).round() as u8;
        let cmyk = vec![0, 0, 0, 0, pct(100.0), 0, 0, 0, pct(60.0), pct(50.0), pct(50.0), pct(95.0), 255, 255, 255, 255];

        let r300 = analyze_cmyk_output_preview(&cmyk, 2, 2, CmykPlates::ALL, 300.0, true).unwrap();
        assert_eq!(r300.summary.over_limit_pixels, 1);
        assert!((r300.summary.over_limit_percent - 25.0).abs() < 0.01);
        assert!((r300.summary.max_tac_percent - 400.0).abs() < 0.5);
        assert_eq!(r300.summary.max_tac_at, Some([1, 1]));

        let r240 = analyze_cmyk_output_preview(&cmyk, 2, 2, CmykPlates::ALL, 240.0, true).unwrap();
        assert_eq!(r240.summary.over_limit_pixels, 2);
        assert!((r240.summary.over_limit_percent - 50.0).abs() < 0.01);

        // Turning off C, M, Y plates leaves only the K plate visible.
        let k_only = analyze_cmyk_output_preview(&cmyk, 2, 2, CmykPlates::BLACK_ONLY, 300.0, false).unwrap();
        // Pure cyan pixel (1,0) has K=0, so under K-only preview it renders as paper white (255,255,255).
        assert_eq!(&k_only.preview_rgba[4..8], &[255, 255, 255, 255]);
        // Registration pixel (1,1) has K=100%, so under K-only preview it renders dark!
        assert!(k_only.preview_rgba[12] < 35);
    }
}
