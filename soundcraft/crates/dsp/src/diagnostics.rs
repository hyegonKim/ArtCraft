//! Audio Diagnostics & Restoration (`DeClicker`, `DeClipper`, and `Scan/Delete Silence`).
//!
//! - **DeClicker**: Detects impulsive clicks/crackles via second-order difference + local MAD
//!   (Median Absolute Deviation) thresholding (`detect_clicks`) and repairs each damaged span via
//!   bidirectional Burg/Levinson-Durbin Autoregressive `AR(p)` linear prediction blended with a
//!   local median envelope guard (`repair_clicks`), plus the zero-allocation [`DeClicker`] plugin.
//! - **DeClipper**: Detects flat-top clipped peak runs (`detect_clipped_regions`) and reconstructs
//!   rounded waveform peaks above the clipping ceiling via boundary-tangent Cubic Hermite spline
//!   interpolation (`repair_clipping`), plus the zero-allocation [`DeClipper`] plugin.
//! - **Scan / Delete Silence**: Scans planar buffers for sub-threshold silence intervals
//!   (`scan_silence_regions`) and excises them with click-free cosine micro-fades
//!   (`delete_silence_regions`).

use crate::params::{Params, lin, param_plumbing, toggle};
use crate::util::{frames_in, sane_channels, sane_sr};
use crate::{Category, FadeShape, ParamInfo, Plugin, PluginInfo, Unit, db_to_gain};

/// Maximum AR linear prediction order used by [`repair_clicks`].
const AR_ORDER: usize = 8;
/// Context samples on each side of a click used to fit the AR predictor.
const AR_CONTEXT: usize = 64;
/// Maximum block size for realtime zero-allocation [`DeClicker`] / [`DeClipper`] scratch buffers.
const REALTIME_SCRATCH_MIN: usize = 4096;

// ─── DeClicker (Median-MAD Detection + Bidirectional AR Interpolation) ──────────────────────────

/// One impulsive click span detected in a planar channel.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ClickRegion {
    pub channel: usize,
    pub start: usize,
    pub end: usize,
    pub peak_derivative: f32,
}

/// Detect impulsive clicks across planar `channels`.
///
/// `sensitivity` is in `0.0..=100.0` (default `60.0`); `max_click_samples` caps repaired click width
/// (default `32` samples).
pub fn detect_clicks(channels: &[Vec<f32>], sample_rate: f32, sensitivity: f32, max_click_samples: usize) -> Vec<ClickRegion> {
    let _sr = sane_sr(sample_rate);
    let sens = if sensitivity.is_finite() { sensitivity.clamp(0.0, 100.0) } else { 60.0 };
    let max_w = max_click_samples.clamp(1, 256);
    let mut regions = Vec::new();
    let mut d2_buf = Vec::new();
    let mut scratch = Vec::new();

    for (ch_idx, samples) in channels.iter().enumerate() {
        detect_channel_clicks(ch_idx, samples, sens, max_w, &mut d2_buf, &mut scratch, &mut regions);
    }
    regions
}

fn detect_channel_clicks(
    ch_idx: usize,
    samples: &[f32],
    sens: f32,
    max_w: usize,
    d2_buf: &mut Vec<f32>,
    scratch: &mut Vec<f32>,
    out: &mut Vec<ClickRegion>,
) {
    let n = samples.len();
    if n < 8 {
        return;
    }
    d2_buf.clear();
    d2_buf.resize(n, 0.0);
    for i in 1..(n - 1) {
        let a = samples[i - 1];
        let b = samples[i];
        let c = samples[i + 1];
        let d2 = (b - 0.5 * (a + c)).abs();
        d2_buf[i] = if d2.is_finite() { d2 } else { 0.0 };
    }

    // Sensitivity 0..100 maps MAD multiplier from 11.5 sigma down to 4.0 sigma.
    let sigma_mult = 11.5 - 0.075 * sens;
    let win = 512_usize;
    let merge_gap = 3_usize;

    let mut win_start = 0_usize;
    let mut pending: Option<ClickRegion> = None;

    while win_start < n {
        let win_end = (win_start + win).min(n);
        let sigma = mad_scale(&d2_buf[win_start..win_end], scratch);
        let thr = (sigma * sigma_mult).max(1.0e-4);

        for (offset, &d2) in d2_buf[win_start..win_end].iter().enumerate() {
            let idx = win_start + offset;
            if d2 > thr {
                if let Some(cur) = &mut pending {
                    if idx <= cur.end + merge_gap && (idx + 1 - cur.start) <= max_w {
                        cur.end = (idx + 1).min(n);
                        cur.peak_derivative = cur.peak_derivative.max(d2);
                        continue;
                    }
                    out.push(*cur);
                }
                pending = Some(ClickRegion { channel: ch_idx, start: idx.saturating_sub(1), end: (idx + 2).min(n), peak_derivative: d2 });
            }
        }
        win_start = win_end;
    }

    if let Some(cur) = pending
        && cur.end - cur.start <= max_w
    {
        out.push(cur);
    }
}

/// Robust noise scale `1.4826 * median(slice)` without heap allocation when `scratch` is sized.
fn mad_scale(slice: &[f32], scratch: &mut Vec<f32>) -> f32 {
    if slice.is_empty() {
        return 0.0;
    }
    scratch.clear();
    scratch.extend_from_slice(slice);
    scratch.sort_by(f32::total_cmp);
    let mid = scratch.len() / 2;
    let med = scratch[mid];
    for v in scratch.iter_mut() {
        *v = (*v - med).abs();
    }
    scratch.sort_by(f32::total_cmp);
    1.4826 * scratch[mid] + 0.15 * med
}

/// Detect and repair impulsive clicks in-place using bidirectional `AR(p)` linear prediction
/// regularized by a local median envelope. Returns all repaired [`ClickRegion`]s.
pub fn repair_clicks(channels: &mut [Vec<f32>], sample_rate: f32, sensitivity: f32, max_click_samples: usize) -> Vec<ClickRegion> {
    let regions = detect_clicks(channels, sample_rate, sensitivity, max_click_samples);
    let mut med_scratch = Vec::with_capacity(AR_CONTEXT * 2);
    for reg in &regions {
        if let Some(ch) = channels.get_mut(reg.channel) {
            interpolate_click_ar_median(ch, reg.start, reg.end, &mut med_scratch);
        }
    }
    regions
}

fn interpolate_click_ar_median(samples: &mut [f32], start: usize, end: usize, scratch: &mut Vec<f32>) {
    let n = samples.len();
    let end = end.min(n);
    if start >= end {
        return;
    }
    let gap = end - start;
    let left_lo = start.saturating_sub(AR_CONTEXT);
    let right_hi = (end + AR_CONTEXT).min(n);
    let left_ctx = &samples[left_lo..start];
    let right_ctx = &samples[end..right_hi];

    // Local median & peak bound from clean neighborhood so repaired samples stay well-behaved.
    scratch.clear();
    scratch.extend_from_slice(left_ctx);
    scratch.extend_from_slice(right_ctx);
    if scratch.is_empty() {
        return;
    }
    scratch.sort_by(f32::total_cmp);
    let med = scratch[scratch.len() / 2];
    let bound = scratch.iter().fold(0.0_f32, |m, &v| m.max((v - med).abs())).max(1.0e-4) * 1.35;

    let fwd_coeffs = fit_ar_levinson(left_ctx);
    let mut rev_right = [0.0_f32; AR_CONTEXT];
    let r_len = right_ctx.len().min(AR_CONTEXT);
    for i in 0..r_len {
        rev_right[i] = right_ctx[r_len - 1 - i];
    }
    let bwd_coeffs = fit_ar_levinson(&rev_right[..r_len]);

    let mut fwd_pred = vec![med; gap];
    let mut bwd_pred = vec![med; gap];

    if let Some(a) = fwd_coeffs {
        let mut hist = [0.0_f32; AR_ORDER];
        for p in 0..AR_ORDER {
            hist[p] = left_ctx[left_ctx.len() - 1 - p];
        }
        for slot in &mut fwd_pred {
            let mut y = 0.0_f32;
            for p in 0..AR_ORDER {
                y += a[p] * hist[p];
            }
            let clamped = y.clamp(med - bound, med + bound);
            *slot = clamped;
            hist.copy_within(0..(AR_ORDER - 1), 1);
            hist[0] = clamped;
        }
    } else if let Some(&left_edge) = left_ctx.last() {
        fwd_pred.fill(left_edge);
    }

    if let Some(b) = bwd_coeffs {
        let mut hist = [0.0_f32; AR_ORDER];
        hist.copy_from_slice(&right_ctx[..AR_ORDER]);
        for k in (0..gap).rev() {
            let mut y = 0.0_f32;
            for p in 0..AR_ORDER {
                y += b[p] * hist[p];
            }
            let clamped = y.clamp(med - bound, med + bound);
            bwd_pred[k] = clamped;
            hist.copy_within(0..(AR_ORDER - 1), 1);
            hist[0] = clamped;
        }
    } else if let Some(&right_edge) = right_ctx.first() {
        bwd_pred.fill(right_edge);
    }

    for k in 0..gap {
        let t = (k as f32 + 1.0) / (gap as f32 + 1.0);
        let w = FadeShape::SCurve.gain(t);
        samples[start + k] = fwd_pred[k] * (1.0 - w) + bwd_pred[k] * w;
    }
}

/// Levinson-Durbin recursion on Yule-Walker autocorrelation equations of order [`AR_ORDER`].
fn fit_ar_levinson(ctx: &[f32]) -> Option<[f32; AR_ORDER]> {
    if ctx.len() < AR_ORDER * 2 {
        return None;
    }
    let mut r = [0.0_f64; AR_ORDER + 1];
    for lag in 0..=AR_ORDER {
        let mut sum = 0.0_f64;
        for i in lag..ctx.len() {
            sum += f64::from(ctx[i]) * f64::from(ctx[i - lag]);
        }
        r[lag] = sum;
    }
    // Slight diagonal ridge prevents instability on pure sinusoids or near-silence.
    r[0] *= 1.0005;
    if r[0] <= 1.0e-12 {
        return None;
    }

    let mut a = [0.0_f64; AR_ORDER];
    let mut prev = [0.0_f64; AR_ORDER];
    let mut err = r[0];

    for m in 0..AR_ORDER {
        let mut num = r[m + 1];
        for j in 0..m {
            num -= a[j] * r[m - j];
        }
        let k = (num / err).clamp(-0.995, 0.995);
        prev[..m].copy_from_slice(&a[..m]);
        a[m] = k;
        for j in 0..m {
            a[j] = prev[j] - k * prev[m - 1 - j];
        }
        err *= 1.0 - k * k;
        if err <= 1.0e-14 {
            break;
        }
    }

    let mut out = [0.0_f32; AR_ORDER];
    for (dst, &val) in out.iter_mut().zip(a.iter()) {
        *dst = if val.is_finite() { val as f32 } else { 0.0 };
    }
    Some(out)
}

// ─── DeClipper (Flat-Top Peak Detection + Cubic Hermite Peak Reconstruction) ───────────────────

/// One clipped flat-top peak region detected in a planar channel.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ClippedRegion {
    pub channel: usize,
    pub start: usize,
    pub end: usize,
    pub ceiling: f32,
}

/// Detect flat-top / threshold-saturated peak regions across planar `channels`.
pub fn detect_clipped_regions(channels: &[Vec<f32>], threshold_db: f32, min_run: usize) -> Vec<ClippedRegion> {
    let thr_db = if threshold_db.is_finite() { threshold_db.clamp(-24.0, 0.0) } else { -0.5 };
    let base_thr = db_to_gain(thr_db);
    let min_len = min_run.max(2);
    let mut out = Vec::new();

    for (ch_idx, samples) in channels.iter().enumerate() {
        detect_channel_clipping(ch_idx, samples, base_thr, true, min_len, &mut out);
    }
    out
}

fn detect_channel_clipping(ch_idx: usize, samples: &[f32], base_thr: f32, auto_ceiling: bool, min_len: usize, out: &mut Vec<ClippedRegion>) {
    let n = samples.len();
    if n < min_len + 2 {
        return;
    }
    let ch_peak = samples.iter().fold(0.0_f32, |m, &v| if v.is_finite() { m.max(v.abs()) } else { m });
    if ch_peak < 0.05 {
        return;
    }
    let eff_thr = if auto_ceiling { base_thr.min(ch_peak * 0.985).max(0.05) } else { base_thr.max(0.05) };

    let mut i = 1_usize;
    while i < n - 1 {
        let s = samples[i];
        if s.is_finite() && s.abs() >= eff_thr {
            let sign = s.signum();
            let start = i;
            let mut peak_abs = s.abs();
            while i < n - 1 && samples[i].is_finite() && samples[i].signum() == sign && samples[i].abs() >= eff_thr {
                peak_abs = peak_abs.max(samples[i].abs());
                i += 1;
            }
            let end = i;
            if end - start >= min_len {
                out.push(ClippedRegion { channel: ch_idx, start, end, ceiling: peak_abs });
            }
        } else {
            i += 1;
        }
    }
}

/// Detect flat-top clipped peaks and reconstruct smooth rounded peaks above the clipping ceiling
/// using boundary-tangent Cubic Hermite spline interpolation.
///
/// When `ceiling_db < 0.0`, scales each channel only if its reconstructed peak would exceed
/// `db_to_gain(ceiling_db)` (pass `ceiling_db > 0.0`, e.g. `6.0`, to preserve full unattenuated
/// reconstructed peak curvature).
pub fn repair_clipping(channels: &mut [Vec<f32>], threshold_db: f32, ceiling_db: f32) -> Vec<ClippedRegion> {
    let regions = detect_clipped_regions(channels, threshold_db, 2);
    for reg in &regions {
        if let Some(ch) = channels.get_mut(reg.channel) {
            hermite_reconstruct_peak(ch, reg.start, reg.end);
        }
    }
    let ceil_db = if ceiling_db.is_finite() { ceiling_db } else { 6.0 };
    if ceil_db <= 0.0 && !regions.is_empty() {
        let max_gain = db_to_gain(ceil_db.clamp(-24.0, 0.0));
        let peak = channels.iter().flat_map(|c| c.iter()).fold(0.0_f32, |m, &v| if v.is_finite() { m.max(v.abs()) } else { m });
        if peak > max_gain && peak > 0.0 {
            let scale = max_gain / peak;
            for ch in channels.iter_mut() {
                for s in ch.iter_mut() {
                    *s *= scale;
                }
            }
        }
    }
    regions
}

/// Reconstruct `[start..end]` from unclipped boundary values and slopes via Cubic Hermite spline.
fn hermite_reconstruct_peak(samples: &mut [f32], start: usize, end: usize) {
    let n = samples.len();
    if start == 0 || end >= n || start >= end {
        return;
    }
    let run = end - start;
    let p0 = samples[start - 1];
    let p1 = samples[end];
    let sign = samples[start].signum();
    let span = (run + 1) as f32;

    // Estimate outward entry derivative `dy/dn` from up to 3 unclipped samples before `start - 1`.
    let raw_left_slope = if start >= 3 {
        0.65 * (samples[start - 1] - samples[start - 2]) + 0.35 * 0.5 * (samples[start - 1] - samples[start - 3])
    } else if start >= 2 {
        samples[start - 1] - samples[start - 2]
    } else {
        0.1 * sign * p0.abs()
    };

    // Estimate inward exit derivative `dy/dn` from up to 3 unclipped samples after `end`.
    let raw_right_slope = if end + 2 < n {
        0.65 * (samples[end + 1] - samples[end]) + 0.35 * 0.5 * (samples[end + 2] - samples[end])
    } else if end + 1 < n {
        samples[end + 1] - samples[end]
    } else {
        -0.1 * sign * p1.abs()
    };

    // Enforce outward curvature at `p0` (`sign * m0 > 0`) and inward at `p1` (`sign * m1 < 0`),
    // capped so multi-millisecond runs never blow up unrealistically.
    let min_slope = 0.015 * p0.abs().max(p1.abs());
    let max_tangent = 3.2 * p0.abs().max(p1.abs()).max(0.1);
    let m0 = (sign * (sign * raw_left_slope).max(min_slope) * span).clamp(-max_tangent, max_tangent);
    let m1 = (-sign * (-sign * raw_right_slope).max(min_slope) * span).clamp(-max_tangent, max_tangent);

    for k in 0..run {
        let t = (k + 1) as f32 / span;
        let t2 = t * t;
        let t3 = t2 * t;
        let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
        let h10 = t3 - 2.0 * t2 + t;
        let h01 = -2.0 * t3 + 3.0 * t2;
        let h11 = t3 - t2;
        samples[start + k] = h00 * p0 + h10 * m0 + h01 * p1 + h11 * m1;
    }
}

// ─── Scan / Delete Silence ──────────────────────────────────────────────────────────────────────

/// Options for [`scan_silence_regions`] and [`delete_silence_regions`].
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SilenceScanOptions {
    pub threshold_db: f32,
    pub min_silence_ms: f32,
    pub pad_before_ms: f32,
    pub pad_after_ms: f32,
}

impl Default for SilenceScanOptions {
    fn default() -> Self {
        Self { threshold_db: -48.0, min_silence_ms: 100.0, pad_before_ms: 5.0, pad_after_ms: 20.0 }
    }
}

/// Contiguous sample interval classified as silence across all channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SilenceRegion {
    pub start: usize,
    pub end: usize,
}

/// Scan planar `channels` for silence intervals below `opts.threshold_db` lasting at least
/// `opts.min_silence_ms` (after trimming `pad_after_ms` and `pad_before_ms` around active audio).
pub fn scan_silence_regions(channels: &[Vec<f32>], sample_rate: f32, opts: &SilenceScanOptions) -> Vec<SilenceRegion> {
    let sr = sane_sr(sample_rate);
    let frames = channels.iter().map(Vec::len).min().unwrap_or(0);
    if channels.is_empty() || frames == 0 {
        return Vec::new();
    }
    let thr = db_to_gain(if opts.threshold_db.is_finite() { opts.threshold_db.clamp(-144.0, 0.0) } else { -48.0 });
    let min_frames = (((opts.min_silence_ms.max(0.0)) * 0.001 * sr).round() as usize).max(1);
    let pad_pre = ((opts.pad_before_ms.max(0.0) * 0.001 * sr).round() as usize).min(frames);
    let pad_post = ((opts.pad_after_ms.max(0.0) * 0.001 * sr).round() as usize).min(frames);

    let mut regions = Vec::new();
    let mut i = 0_usize;
    while i < frames {
        let silent = channels.iter().all(|c| c[i].abs() <= thr);
        if silent {
            let run_start = i;
            while i < frames && channels.iter().all(|c| c[i].abs() <= thr) {
                i += 1;
            }
            let run_end = i;
            if run_end - run_start >= min_frames {
                let eff_start = if run_start == 0 { 0 } else { (run_start + pad_post).min(run_end) };
                let eff_end = if run_end == frames { frames } else { run_end.saturating_sub(pad_pre).max(eff_start) };
                if eff_end > eff_start {
                    regions.push(SilenceRegion { start: eff_start, end: eff_end });
                }
            }
        } else {
            i += 1;
        }
    }
    regions
}

/// Scan and delete silence intervals, applying 2 ms equal-power micro-fades at cut boundaries.
pub fn delete_silence_regions(channels: &[Vec<f32>], sample_rate: f32, opts: &SilenceScanOptions) -> (Vec<Vec<f32>>, Vec<SilenceRegion>) {
    let sr = sane_sr(sample_rate);
    let frames = channels.iter().map(Vec::len).min().unwrap_or(0);
    let regions = scan_silence_regions(channels, sr, opts);
    if regions.is_empty() || channels.is_empty() {
        return (channels.to_vec(), regions);
    }

    let fade = ((0.002 * sr).round() as usize).clamp(4, 128);
    let mut out: Vec<Vec<f32>> = channels.iter().map(|_| Vec::with_capacity(frames)).collect();
    let mut cursor = 0_usize;

    for reg in &regions {
        if reg.start > cursor {
            append_kept_segment(&mut out, channels, cursor, reg.start, fade, cursor > 0, true);
        }
        cursor = reg.end;
    }
    if cursor < frames {
        append_kept_segment(&mut out, channels, cursor, frames, fade, cursor > 0, false);
    }

    (out, regions)
}

fn append_kept_segment(out: &mut [Vec<f32>], channels: &[Vec<f32>], start: usize, end: usize, fade: usize, fade_in_edge: bool, fade_out_edge: bool) {
    let seg_len = end.saturating_sub(start);
    if seg_len == 0 {
        return;
    }
    let f_len = fade.min(seg_len / 2);
    for (c, dst) in out.iter_mut().enumerate() {
        let base = dst.len();
        dst.extend_from_slice(&channels[c][start..end]);
        if fade_in_edge && f_len > 0 {
            for k in 0..f_len {
                let t = (k as f32 + 0.5) / f_len as f32;
                dst[base + k] *= FadeShape::EqualPower.gain(t);
            }
        }
        if fade_out_edge && f_len > 0 {
            let tail = dst.len() - f_len;
            for k in 0..f_len {
                let t = (k as f32 + 0.5) / f_len as f32;
                dst[tail + k] *= FadeShape::EqualPower.gain(1.0 - t);
            }
        }
    }
}

// ─── Realtime / AudioSuite Built-in Plugins (`declicker` & `declipper`) ────────────────────────

static DECLICKER_PARAMS: [ParamInfo; 2] =
    [lin("sensitivity", "Sensitivity", 0.0, 100.0, 60.0, Unit::Percent), lin("max_width", "Max Click Width", 0.1, 4.0, 1.0, Unit::Ms)];

pub static DECLICKER_INFO: PluginInfo = PluginInfo {
    id: "declicker",
    name: "DeClicker",
    short_name: "DeClick",
    category: Category::Other,
    params: &DECLICKER_PARAMS,
    is_instrument: false,
    audiosuite: true,
};

/// Zero-allocation realtime & AudioSuite impulsive click remover.
pub struct DeClicker {
    p: Params,
    sr: f32,
    channels: usize,
    d2_buf: Vec<f32>,
    med_scratch: Vec<f32>,
}

impl Default for DeClicker {
    fn default() -> Self {
        Self::new()
    }
}

impl DeClicker {
    pub fn new() -> Self {
        Self {
            p: Params::new(&DECLICKER_PARAMS),
            sr: 48_000.0,
            channels: 2,
            d2_buf: vec![0.0; REALTIME_SCRATCH_MIN],
            med_scratch: vec![0.0; REALTIME_SCRATCH_MIN],
        }
    }

    fn update(&mut self) {}
}

impl Plugin for DeClicker {
    param_plumbing!(DECLICKER_INFO);

    fn prepare(&mut self, sample_rate: f32, max_block: usize, channels: usize) {
        self.sr = sane_sr(sample_rate);
        self.channels = sane_channels(channels);
        let cap = max_block.max(REALTIME_SCRATCH_MIN);
        self.d2_buf.resize(cap, 0.0);
        self.med_scratch.resize(cap, 0.0);
    }

    fn reset(&mut self) {
        self.d2_buf.fill(0.0);
    }

    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        let ch = self.channels.min(io.len());
        let n = frames_in(io, frames, ch).min(self.d2_buf.len());
        if n < 8 {
            return;
        }
        let sens = self.p.v(0).clamp(0.0, 100.0);
        let max_w = (((self.p.v(1).clamp(0.1, 4.0) * 0.001) * self.sr).round() as usize).clamp(2, 192);
        let sigma_mult = 11.5 - 0.075 * sens;

        for chan in io.iter_mut().take(ch) {
            process_block_declick(&mut chan[..n], sigma_mult, max_w, &mut self.d2_buf[..n], &mut self.med_scratch[..n]);
        }
    }
}

/// Zero-allocation block declicker used inside [`DeClicker::process`].
fn process_block_declick(samples: &mut [f32], sigma_mult: f32, max_w: usize, d2: &mut [f32], scratch: &mut [f32]) {
    let n = samples.len();
    d2[0] = 0.0;
    d2[n - 1] = 0.0;
    for i in 1..(n - 1) {
        let v = (samples[i] - 0.5 * (samples[i - 1] + samples[i + 1])).abs();
        d2[i] = if v.is_finite() { v } else { 0.0 };
    }
    scratch.copy_from_slice(d2);
    scratch.sort_by(f32::total_cmp);
    let mid = n / 2;
    let med = scratch[mid];
    for v in scratch.iter_mut() {
        *v = (*v - med).abs();
    }
    scratch.sort_by(f32::total_cmp);
    let sigma = 1.4826 * scratch[mid] + 0.15 * med;
    let thr = (sigma * sigma_mult).max(1.0e-4);

    let mut i = 1_usize;
    while i < n - 1 {
        if d2[i] > thr {
            let start = i.saturating_sub(1);
            let mut end = (i + 2).min(n - 1);
            while end < n - 1 && (end - start) < max_w && d2[end] > thr * 0.75 {
                end += 1;
            }
            repair_span_zero_alloc(samples, start, end);
            i = end + 1;
        } else {
            i += 1;
        }
    }
}

fn repair_span_zero_alloc(samples: &mut [f32], start: usize, end: usize) {
    let n = samples.len();
    if start == 0 || end >= n || start >= end {
        return;
    }
    let gap = end - start;
    let ctx_lo = start.saturating_sub(AR_CONTEXT);
    let ctx_hi = (end + AR_CONTEXT).min(n);
    let left = &samples[ctx_lo..start];
    let right = &samples[end..ctx_hi];

    // Fixed-stack median & AR(8) bidirectional blend (zero heap allocations).
    let mut neighborhood = [0.0_f32; AR_CONTEXT * 2];
    let mut count = 0_usize;
    for &v in left.iter().chain(right.iter()).take(neighborhood.len()) {
        neighborhood[count] = v;
        count += 1;
    }
    neighborhood[..count].sort_by(f32::total_cmp);
    let med = if count > 0 { neighborhood[count / 2] } else { 0.0 };
    let bound = neighborhood[..count].iter().fold(0.0_f32, |m, &v| m.max((v - med).abs())).max(1.0e-4) * 1.35;

    let fwd_a = fit_ar_levinson(left);
    let mut rev_right = [0.0_f32; AR_CONTEXT];
    let r_len = right.len().min(AR_CONTEXT);
    for idx in 0..r_len {
        rev_right[idx] = right[r_len - 1 - idx];
    }
    let bwd_a = fit_ar_levinson(&rev_right[..r_len]);

    let mut fwd_hist = [med; AR_ORDER];
    if left.len() >= AR_ORDER {
        for p in 0..AR_ORDER {
            fwd_hist[p] = left[left.len() - 1 - p];
        }
    }
    let mut bwd_out = [med; 256];
    let capped_gap = gap.min(bwd_out.len());
    if let Some(b) = bwd_a {
        let mut hist = [0.0_f32; AR_ORDER];
        hist.copy_from_slice(&right[..AR_ORDER]);
        for k in (0..capped_gap).rev() {
            let mut y = 0.0_f32;
            for p in 0..AR_ORDER {
                y += b[p] * hist[p];
            }
            let c = y.clamp(med - bound, med + bound);
            bwd_out[k] = c;
            hist.copy_within(0..(AR_ORDER - 1), 1);
            hist[0] = c;
        }
    } else {
        bwd_out[..capped_gap].fill(samples[end]);
    }

    let left_anchor = samples[start - 1];
    for k in 0..capped_gap {
        let yf = if let Some(a) = fwd_a {
            let mut y = 0.0_f32;
            for p in 0..AR_ORDER {
                y += a[p] * fwd_hist[p];
            }
            let c = y.clamp(med - bound, med + bound);
            fwd_hist.copy_within(0..(AR_ORDER - 1), 1);
            fwd_hist[0] = c;
            c
        } else {
            left_anchor
        };
        let t = (k as f32 + 1.0) / (capped_gap as f32 + 1.0);
        let w = FadeShape::SCurve.gain(t);
        samples[start + k] = yf * (1.0 - w) + bwd_out[k] * w;
    }
}

static DECLIPPER_PARAMS: [ParamInfo; 3] = [
    lin("threshold", "Clip Threshold", -18.0, 0.0, -0.5, Unit::Db),
    lin("ceiling", "Output Ceiling", -12.0, 6.0, -0.3, Unit::Db),
    toggle("auto_threshold", "Auto Detect Ceiling", true),
];

pub static DECLIPPER_INFO: PluginInfo = PluginInfo {
    id: "declipper",
    name: "DeClipper",
    short_name: "DeClip",
    category: Category::Other,
    params: &DECLIPPER_PARAMS,
    is_instrument: false,
    audiosuite: true,
};

/// Zero-allocation realtime & AudioSuite flat-top peak reconstructor.
pub struct DeClipper {
    p: Params,
    sr: f32,
    channels: usize,
}

impl Default for DeClipper {
    fn default() -> Self {
        Self::new()
    }
}

impl DeClipper {
    pub fn new() -> Self {
        Self { p: Params::new(&DECLIPPER_PARAMS), sr: 48_000.0, channels: 2 }
    }

    fn update(&mut self) {}
}

impl Plugin for DeClipper {
    param_plumbing!(DECLIPPER_INFO);

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, channels: usize) {
        self.sr = sane_sr(sample_rate);
        self.channels = sane_channels(channels);
    }

    fn reset(&mut self) {}

    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        let ch = self.channels.min(io.len());
        let n = frames_in(io, frames, ch);
        if n < 6 {
            return;
        }
        let base_thr = db_to_gain(self.p.v(0));
        let ceiling_db = self.p.v(1);
        let auto_ceil = self.p.v(2) >= 0.5;
        let mut any_repaired = false;

        for chan in io.iter_mut().take(ch) {
            let slice = &mut chan[..n];
            let peak = slice.iter().fold(0.0_f32, |m, &v| if v.is_finite() { m.max(v.abs()) } else { m });
            if peak < 0.05 {
                continue;
            }
            let eff_thr = if auto_ceil { base_thr.min(peak * 0.985).max(0.05) } else { base_thr.max(0.05) };
            let mut i = 1_usize;
            while i < n - 1 {
                if slice[i].is_finite() && slice[i].abs() >= eff_thr {
                    let sign = slice[i].signum();
                    let start = i;
                    while i < n - 1 && slice[i].is_finite() && slice[i].signum() == sign && slice[i].abs() >= eff_thr {
                        i += 1;
                    }
                    if i - start >= 2 {
                        hermite_reconstruct_peak(slice, start, i);
                        any_repaired = true;
                    }
                } else {
                    i += 1;
                }
            }
        }

        if any_repaired && ceiling_db <= 0.0 {
            let max_gain = db_to_gain(ceiling_db);
            let mut block_peak = 0.0_f32;
            for chan in io.iter().take(ch) {
                for &v in &chan[..n] {
                    if v.is_finite() && v.abs() > block_peak {
                        block_peak = v.abs();
                    }
                }
            }
            if block_peak > max_gain && block_peak > 0.0 {
                let scale = max_gain / block_peak;
                for chan in io.iter_mut().take(ch) {
                    for v in &mut chan[..n] {
                        *v *= scale;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn declicker_detects_and_repairs_impulsive_spikes_on_sine_wave() {
        let sr = 48_000.0_f32;
        let frames = 4096;
        let clean: Vec<f32> = (0..frames).map(|n| 0.4 * (std::f32::consts::TAU * 440.0 * n as f32 / sr).sin()).collect();
        let mut noisy = clean.clone();
        // Inject 3 sharp impulsive clicks!
        noisy[800] = 0.98;
        noisy[801] = -0.92;
        noisy[2000] = -0.99;
        noisy[3100] = 0.95;

        let mut buf = vec![noisy.clone()];
        let detected = repair_clicks(&mut buf, sr, 65.0, 32);
        assert_eq!(detected.len(), 3, "expected 3 click regions, got {detected:?}");

        let err_before: f32 = noisy.iter().zip(&clean).map(|(a, b)| (a - b).powi(2)).sum();
        let err_after: f32 = buf[0].iter().zip(&clean).map(|(a, b)| (a - b).powi(2)).sum();
        assert!(err_after < err_before * 0.02, "AR-median click repair should slash MSE by >50x: before={err_before}, after={err_after}");
    }

    #[test]
    fn declipper_reconstructs_clipped_flat_top_peaks_via_cubic_hermite() {
        let sr = 48_000.0_f32;
        let frames = 2048;
        let ceiling = 0.70_f32;
        let clean: Vec<f32> = (0..frames).map(|n| 0.95 * (std::f32::consts::TAU * 500.0 * n as f32 / sr).sin()).collect();
        let clipped: Vec<f32> = clean.iter().map(|&x| x.clamp(-ceiling, ceiling)).collect();

        let mut buf = vec![clipped.clone()];
        // Pass `ceiling_db = 6.0` so reconstructed peaks above `ceiling` are preserved unattenuated.
        let regions = repair_clipping(&mut buf, -3.5, 6.0);
        assert!(!regions.is_empty(), "expected flat-top sine plateaus to be detected");

        let restored_peak = buf[0].iter().fold(0.0_f32, |m, &x| m.max(x.abs()));
        assert!(
            restored_peak > ceiling + 0.10,
            "Cubic Hermite should reconstruct peak curvature well above clipped ceiling {ceiling}, got {restored_peak}"
        );
        let mse_before: f32 = clipped.iter().zip(&clean).map(|(a, b)| (a - b).powi(2)).sum();
        let mse_after: f32 = buf[0].iter().zip(&clean).map(|(a, b)| (a - b).powi(2)).sum();
        assert!(mse_after < mse_before * 0.25, "expected major error reduction: before={mse_before}, after={mse_after}");
    }

    #[test]
    fn scan_and_delete_silence_removes_quiet_gaps_cleanly() {
        let sr = 10_000.0_f32;
        let mut sig = vec![0.0_f32; 10_000]; // 1.0s
        // Active tone 0.0..0.2s and 0.6..0.9s; silence 0.2..0.6s (400ms) and 0.9..1.0s (100ms).
        for (i, s) in sig[0..2000].iter_mut().enumerate() {
            *s = 0.5 * (std::f32::consts::TAU * 300.0 * i as f32 / sr).sin();
        }
        for (i, s) in sig[6000..9000].iter_mut().enumerate() {
            *s = 0.5 * (std::f32::consts::TAU * 300.0 * i as f32 / sr).sin();
        }
        let opts = SilenceScanOptions { threshold_db: -40.0, min_silence_ms: 80.0, pad_before_ms: 5.0, pad_after_ms: 5.0 };
        let (compacted, regions) = delete_silence_regions(&[sig], sr, &opts);
        assert_eq!(regions.len(), 2);
        assert!(compacted[0].len() < 5500 && compacted[0].len() > 4900);
    }
}
