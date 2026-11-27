//! Beat-synchronous music duration retimer (`Remix`).
//!
//! Retimes musical recordings to hit an arbitrary target duration (`target_seconds`) cleanly by:
//! 1. Computing a multi-band spectral-flux / energy-jump onset novelty curve.
//! 2. Estimating tempo (BPM) via tempo-prior-weighted autocorrelation and tracking beat positions
//!    via dynamic programming (`detect_beats`).
//! 3. Extracting beat-synchronous timbral, dynamic, and metrical-phase descriptors to form a
//!    beat-to-beat transition cost / self-similarity matrix (`compute_seam_matrix`).
//! 4. Planning optimal forward cuts (to shorten) or backward loops (to lengthen) via dynamic
//!    programming while preserving the intro and outro, and synthesizing seamless equal-power
//!    crossfades (`remix_to_duration`).

use crate::FadeShape;
use crate::offline::MAX_OFFLINE_SAMPLES;
use crate::util::sane_sr;

/// Configuration for [`remix_to_duration`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RemixOptions {
    /// Equal-power crossfade duration at every beat seam, in milliseconds.
    pub crossfade_ms: f32,
    /// Minimum number of consecutive beats played between two edit seams.
    pub min_segment_beats: usize,
    /// Minimum intro beats kept intact before the first edit seam.
    pub preserve_intro_beats: usize,
    /// Minimum outro beats kept intact at the end of the remixed output.
    pub preserve_outro_beats: usize,
}

impl Default for RemixOptions {
    fn default() -> Self {
        Self { crossfade_ms: 60.0, min_segment_beats: 4, preserve_intro_beats: 2, preserve_outro_beats: 2 }
    }
}

/// Detected beat grid and tempo estimate for an audio clip.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BeatGrid {
    /// Estimated tempo in quarter-note beats per minute.
    pub bpm: f32,
    /// Nominal beat period in samples.
    pub beat_period_samples: usize,
    /// Monotonically increasing beat onset sample indices (`0 <= beats[k] < frames`).
    pub beats: Vec<usize>,
    /// Hop size in samples used for the onset novelty curve.
    pub novelty_hop: usize,
}

/// One beat-aligned cut or loop transition rendered by [`remix_to_duration`].
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SeamEdit {
    /// Source sample where the outgoing segment ended.
    pub from_sample: usize,
    /// Source sample where the incoming segment resumed.
    pub to_sample: usize,
    /// Output timeline sample where the crossfade is centered.
    pub output_sample: usize,
    /// Normalized seam similarity in `0.0..=1.0` (`1.0` = identical musical continuation).
    pub similarity: f32,
}

/// Output of [`remix_to_duration`].
#[derive(Debug, Clone, PartialEq)]
pub struct RemixResult {
    /// Planar `f32` audio channels, each exactly `target_frames` samples long.
    pub channels: Vec<Vec<f32>>,
    /// Detected source tempo in BPM.
    pub bpm: f32,
    /// Number of beats detected in the source material.
    pub beat_count: usize,
    /// Beat-aligned cut/loop edits applied to reach `target_frames`.
    pub edits: Vec<SeamEdit>,
    /// Exact frame count per channel produced.
    pub target_frames: usize,
}

#[derive(Clone, Copy, Default)]
struct BeatFeature {
    rms: f32,
    low_energy: f32,
    mid_energy: f32,
    high_energy: f32,
    zcr: f32,
    boundary_rms: f32,
    bar_phase: usize,
}

/// Detect tempo and beat grid from planar `channels` at `sample_rate`.
pub fn detect_beats(channels: &[Vec<f32>], sample_rate: f32) -> BeatGrid {
    let sr = sane_sr(sample_rate);
    let frames = channels.iter().map(Vec::len).min().unwrap_or(0);
    let hop = ((sr * 0.010).round() as usize).max(1);
    let fallback_period = ((sr * 0.5).round() as usize).max(hop * 2);

    if frames < fallback_period * 2 {
        let mut beats = Vec::new();
        let mut pos = 0;
        while pos < frames {
            beats.push(pos);
            pos += fallback_period;
        }
        return BeatGrid { bpm: 120.0, beat_period_samples: fallback_period, beats, novelty_hop: hop };
    }

    let mono = mix_to_mono(channels, frames);
    let novelty = compute_onset_novelty(&mono, sr, hop);
    let period_hops = estimate_period_hops(&novelty, sr, hop);
    let beat_period_samples = (period_hops * hop).max(hop * 2);
    let bpm = (60.0 * sr / beat_period_samples as f32).clamp(50.0, 220.0);

    let tracked_hops = dp_track_beats(&novelty, period_hops);
    let mut beats: Vec<usize> = tracked_hops.into_iter().map(|h| refine_beat_peak(&mono, h * hop, hop).min(frames.saturating_sub(1))).collect();
    beats.dedup();

    if beats.len() < 2 {
        beats.clear();
        let mut pos = 0;
        while pos < frames {
            beats.push(pos);
            pos += beat_period_samples;
        }
    }

    BeatGrid { bpm, beat_period_samples, beats, novelty_hop: hop }
}

fn mix_to_mono(channels: &[Vec<f32>], frames: usize) -> Vec<f32> {
    let ch_count = channels.len().max(1);
    let inv = 1.0 / ch_count as f32;
    let mut mono = vec![0.0_f32; frames];
    for ch in channels {
        for (dst, &s) in mono.iter_mut().zip(ch.iter().take(frames)) {
            if s.is_finite() {
                *dst += s * inv;
            }
        }
    }
    mono
}

/// Multi-band log-energy first-difference novelty envelope at `hop`-sample resolution.
fn compute_onset_novelty(mono: &[f32], sr: f32, hop: usize) -> Vec<f32> {
    let win = (hop * 2).max(4);
    let num_hops = mono.len() / hop;
    if num_hops == 0 {
        return Vec::new();
    }

    // One-pole lowpass coefficient around 220 Hz to separate kick/bass from mid/high transients.
    let lp_alpha = (1.0 - (-std::f32::consts::TAU * 220.0 / sr).exp()).clamp(0.001, 0.99);
    let mut lp_state = 0.0_f32;
    let mut prev_low = 0.0_f32;
    let mut prev_high = 0.0_f32;
    let mut prev_broad = 0.0_f32;
    let mut novelty = Vec::with_capacity(num_hops);

    for h in 0..num_hops {
        let start = h * hop;
        let end = (start + win).min(mono.len());
        let mut e_low = 0.0_f32;
        let mut e_high = 0.0_f32;
        let mut e_broad = 0.0_f32;
        let mut prev_x = mono.get(start).copied().unwrap_or(0.0);

        for &x in &mono[start..end] {
            lp_state += lp_alpha * (x - lp_state);
            let hi = x - prev_x;
            prev_x = x;
            e_low += lp_state * lp_state;
            e_high += hi * hi;
            e_broad += x * x;
        }
        let inv_n = 1.0 / (end - start).max(1) as f32;
        let l_log = (1.0 + 40.0 * e_low * inv_n).ln();
        let h_log = (1.0 + 80.0 * e_high * inv_n).ln();
        let b_log = (1.0 + 30.0 * e_broad * inv_n).ln();

        let flux = (l_log - prev_low).max(0.0) * 1.2 + (h_log - prev_high).max(0.0) + (b_log - prev_broad).max(0.0);
        prev_low = l_log;
        prev_high = h_log;
        prev_broad = b_log;
        novelty.push(flux);
    }

    // Subtract local moving mean (half-wave rectified) and normalize peak to 1.0.
    let radius = 8_usize;
    let mut cleaned = vec![0.0_f32; novelty.len()];
    let mut max_val = 0.0_f32;
    for i in 0..novelty.len() {
        let lo = i.saturating_sub(radius);
        let hi = (i + radius + 1).min(novelty.len());
        let mean = novelty[lo..hi].iter().sum::<f32>() / (hi - lo) as f32;
        let v = (novelty[i] - mean).max(0.0);
        cleaned[i] = v;
        if v > max_val {
            max_val = v;
        }
    }
    if max_val > 1e-7 {
        for v in &mut cleaned {
            *v /= max_val;
        }
    }
    cleaned
}

fn estimate_period_hops(novelty: &[f32], sr: f32, hop: usize) -> usize {
    let hop_rate = sr / hop as f32;
    let min_lag = ((hop_rate * 60.0 / 185.0).round() as usize).max(2);
    let max_lag = ((hop_rate * 60.0 / 60.0).round() as usize).max(min_lag + 1);
    let default_lag = ((hop_rate * 0.5).round() as usize).max(2);

    if novelty.len() <= min_lag * 2 {
        return default_lag;
    }
    let energy: f32 = novelty.iter().map(|v| v * v).sum();
    if energy <= 1e-8 {
        return default_lag;
    }

    let mut best_lag = default_lag;
    let mut best_score = -1.0_f32;
    let upper = max_lag.min(novelty.len() / 2);

    for lag in min_lag..=upper {
        let mut corr = 0.0_f32;
        for i in lag..novelty.len() {
            corr += novelty[i] * novelty[i - lag];
        }
        corr /= (novelty.len() - lag).max(1) as f32;
        // Include 2x period reinforcement and a gentle perceptual tempo prior centered at 120 BPM.
        let mut harmonic = corr;
        if lag * 2 < novelty.len() {
            let mut corr2 = 0.0_f32;
            for i in (lag * 2)..novelty.len() {
                corr2 += novelty[i] * novelty[i - lag * 2];
            }
            harmonic += 0.35 * corr2 / (novelty.len() - lag * 2).max(1) as f32;
        }
        let bpm = 60.0 * hop_rate / lag as f32;
        let log_ratio = (bpm / 120.0).ln();
        let prior = (-0.5 * (log_ratio / 0.55).powi(2)).exp();
        let score = harmonic * (0.65 + 0.35 * prior);
        if score > best_score {
            best_score = score;
            best_lag = lag;
        }
    }
    best_lag
}

/// Ellis-style dynamic-programming beat tracker over `novelty`.
fn dp_track_beats(novelty: &[f32], period: usize) -> Vec<usize> {
    let n = novelty.len();
    if n == 0 || period == 0 {
        return Vec::new();
    }
    let min_step = (period / 2).max(1);
    let max_step = (period * 2).max(min_step + 1);
    let tightness = 12.0_f32;

    let mut score = vec![0.0_f32; n];
    let mut back = vec![usize::MAX; n];

    for t in 0..n {
        let mut best_prev_score = 0.0_f32;
        let mut best_prev = usize::MAX;
        if t >= min_step {
            let lo = t.saturating_sub(max_step);
            let hi = t - min_step;
            for (prev, &prev_sc) in score.iter().enumerate().take(hi + 1).skip(lo) {
                let delta = (t - prev) as f32;
                let ratio = (delta / period as f32).ln();
                let cand = prev_sc - tightness * ratio * ratio;
                if best_prev == usize::MAX || cand > best_prev_score {
                    best_prev_score = cand;
                    best_prev = prev;
                }
            }
        }
        score[t] = novelty[t] + if best_prev == usize::MAX { 0.0 } else { best_prev_score };
        back[t] = best_prev;
    }

    // Backtrack from the highest-scoring beat in the final beat period.
    let tail_start = n.saturating_sub(period);
    let mut cur = tail_start;
    for (idx, &sc) in score[tail_start..].iter().enumerate() {
        if sc > score[cur] {
            cur = tail_start + idx;
        }
    }
    let mut beats = Vec::new();
    while cur < n {
        beats.push(cur);
        cur = back[cur];
    }
    beats.reverse();
    beats
}

fn refine_beat_peak(mono: &[f32], center: usize, hop: usize) -> usize {
    let radius = hop / 2;
    let lo = center.saturating_sub(radius);
    let hi = (center + radius + 1).min(mono.len());
    let mut best = center.min(mono.len().saturating_sub(1));
    let mut best_amp = -1.0_f32;
    for (idx, &s) in mono[lo..hi].iter().enumerate() {
        let a = s.abs();
        if a > best_amp {
            best_amp = a;
            best = lo + idx;
        }
    }
    best
}

/// Compute an `N x N` beat-transition cost matrix where entry `[i][j]` is the cost (`>= 0.0`) of
/// transitioning from the end of beat `i` directly into the start of beat `j`.
///
/// Lower cost corresponds to a cleaner, higher-similarity seam (`similarity = 1 / (1 + cost)`).
pub fn compute_seam_matrix(channels: &[Vec<f32>], sample_rate: f32, grid: &BeatGrid) -> Vec<Vec<f32>> {
    let n = grid.beats.len();
    if n == 0 {
        return Vec::new();
    }
    let sr = sane_sr(sample_rate);
    let frames = channels.iter().map(Vec::len).min().unwrap_or(0);
    let mono = mix_to_mono(channels, frames);
    let features = extract_beat_features(&mono, sr, grid, frames);

    let mut matrix = vec![vec![0.0_f32; n]; n];
    for i in 0..n {
        // Natural continuation after beat `i` is beat `i + 1` (or `i` at the final beat).
        let natural = features.get(i + 1).copied().unwrap_or(features[i]);
        let end_rms = features[i].boundary_rms;
        for (j, target) in features.iter().enumerate() {
            let d_rms = (natural.rms - target.rms).abs() * 3.0;
            let d_low = (natural.low_energy - target.low_energy).abs() * 2.5;
            let d_mid = (natural.mid_energy - target.mid_energy).abs() * 2.0;
            let d_high = (natural.high_energy - target.high_energy).abs() * 1.5;
            let d_zcr = (natural.zcr - target.zcr).abs() * 1.5;
            let d_bound = (end_rms - target.boundary_rms).abs() * 2.5;
            let phase_mismatch = if natural.bar_phase == target.bar_phase {
                0.0
            } else if natural.bar_phase % 2 == target.bar_phase % 2 {
                0.35
            } else {
                0.85
            };
            matrix[i][j] = d_rms + d_low + d_mid + d_high + d_zcr + d_bound + phase_mismatch;
        }
    }
    matrix
}

fn extract_beat_features(mono: &[f32], sr: f32, grid: &BeatGrid, frames: usize) -> Vec<BeatFeature> {
    let lp_low = (1.0 - (-std::f32::consts::TAU * 200.0 / sr).exp()).clamp(0.001, 0.99);
    let lp_mid = (1.0 - (-std::f32::consts::TAU * 2200.0 / sr).exp()).clamp(0.001, 0.99);
    let boundary_win = ((sr * 0.025).round() as usize).clamp(8, 2048);

    let mut out = Vec::with_capacity(grid.beats.len());
    for (idx, &b_start) in grid.beats.iter().enumerate() {
        let b_end = grid.beats.get(idx + 1).copied().unwrap_or_else(|| (b_start + grid.beat_period_samples).min(frames)).min(frames);
        if b_end <= b_start {
            out.push(BeatFeature { bar_phase: idx % 4, ..Default::default() });
            continue;
        }
        let slice = &mono[b_start..b_end];
        let mut s_low = 0.0_f32;
        let mut s_mid = 0.0_f32;
        let mut e_tot = 0.0_f32;
        let mut e_low = 0.0_f32;
        let mut e_mid = 0.0_f32;
        let mut e_high = 0.0_f32;
        let mut zc = 0_usize;
        let mut prev = slice[0];

        for &x in slice {
            s_low += lp_low * (x - s_low);
            s_mid += lp_mid * (x - s_mid);
            let band_mid = s_mid - s_low;
            let band_high = x - s_mid;
            e_tot += x * x;
            e_low += s_low * s_low;
            e_mid += band_mid * band_mid;
            e_high += band_high * band_high;
            if (x >= 0.0) != (prev >= 0.0) {
                zc += 1;
            }
            prev = x;
        }
        let inv = 1.0 / slice.len() as f32;
        let b_win = boundary_win.min(slice.len());
        let b_rms = (slice[..b_win].iter().map(|x| x * x).sum::<f32>() / b_win as f32).sqrt();
        out.push(BeatFeature {
            rms: (e_tot * inv).sqrt(),
            low_energy: (e_low * inv).sqrt(),
            mid_energy: (e_mid * inv).sqrt(),
            high_energy: (e_high * inv).sqrt(),
            zcr: zc as f32 * inv,
            boundary_rms: b_rms,
            bar_phase: idx % 4,
        });
    }
    out
}

/// Retime `channels` to `target_seconds` using beat-synchronous cuts/loops and equal-power crossfades.
pub fn remix_to_duration(channels: &[Vec<f32>], sample_rate: f32, target_seconds: f64, options: &RemixOptions) -> RemixResult {
    let sr = sane_sr(sample_rate);
    let safe_sec = if target_seconds.is_finite() { target_seconds.max(0.0) } else { 0.0 };
    let target_frames = ((safe_sec * f64::from(sr)).round() as usize).min(MAX_OFFLINE_SAMPLES);
    let ch_count = channels.len();
    let in_frames = channels.iter().map(Vec::len).min().unwrap_or(0);

    if ch_count == 0 || target_frames == 0 {
        return RemixResult { channels: vec![Vec::new(); ch_count], bpm: 120.0, beat_count: 0, edits: Vec::new(), target_frames: 0 };
    }
    if in_frames == 0 {
        return RemixResult { channels: vec![vec![0.0; target_frames]; ch_count], bpm: 120.0, beat_count: 0, edits: Vec::new(), target_frames };
    }
    if target_frames == in_frames {
        let grid = detect_beats(channels, sr);
        return RemixResult {
            channels: channels.iter().map(|c| c[..in_frames].to_vec()).collect(),
            bpm: grid.bpm,
            beat_count: grid.beats.len(),
            edits: Vec::new(),
            target_frames,
        };
    }

    let grid = detect_beats(channels, sr);
    if grid.beats.len() < 4 || in_frames < grid.beat_period_samples * 3 {
        return fallback_resample_duration(channels, in_frames, target_frames, &grid);
    }

    let seam_matrix = compute_seam_matrix(channels, sr, &grid);
    let beat_bounds = build_beat_bounds(&grid, in_frames);
    let xfade_ms = if options.crossfade_ms.is_finite() { options.crossfade_ms.clamp(5.0, 500.0) } else { 60.0 };
    let xfade_len = (((xfade_ms * 0.001) * sr).round() as usize).clamp(16, (grid.beat_period_samples / 2).max(16));

    let beat_sequence = plan_beat_path(&grid, &seam_matrix, target_frames, options);
    synthesize_beat_plan(channels, &beat_bounds, &seam_matrix, &beat_sequence, target_frames, xfade_len, &grid)
}

fn build_beat_bounds(grid: &BeatGrid, in_frames: usize) -> Vec<(usize, usize)> {
    let mut bounds = Vec::with_capacity(grid.beats.len());
    for (i, &start) in grid.beats.iter().enumerate() {
        let end = grid.beats.get(i + 1).copied().unwrap_or(in_frames).min(in_frames);
        bounds.push((start, end.max(start + 1).min(in_frames)));
    }
    bounds
}

/// Dynamic-programming planner over `(step, source_beat)` that anchors the intro and outro beats
/// while choosing natural continuations (`b -> b + 1` at cost `0`) or low-cost jumps (`b -> j`).
fn plan_beat_path(grid: &BeatGrid, seam_matrix: &[Vec<f32>], target_frames: usize, options: &RemixOptions) -> Vec<usize> {
    let n = grid.beats.len();
    let target_beats = ((target_frames as f64 / grid.beat_period_samples.max(1) as f64).round() as usize)
        .clamp(1, MAX_OFFLINE_SAMPLES / grid.beat_period_samples.max(1).max(1));

    if target_beats == 1 {
        return vec![0];
    }

    let intro = options.preserve_intro_beats.min(n / 3).min(target_beats / 2).max(1);
    let outro = options.preserve_outro_beats.min(n / 3).min(target_beats.saturating_sub(intro)).max(1);
    let min_seg = options.min_segment_beats.clamp(1, 16);
    let shortening = target_beats < n;

    // DP state: `dp[b]` = minimum cumulative seam cost to land on source beat `b` at step `t`,
    // along with `run[b]` = consecutive natural beats played up to `b`.
    let inf = 1.0e30_f32;
    let mut dp = vec![inf; n];
    let mut run = vec![0_usize; n];
    let mut back: Vec<Vec<usize>> = vec![vec![0; n]; target_beats];

    dp[0] = 0.0;
    run[0] = 1;

    for step in 1..target_beats {
        let mut next_dp = vec![inf; n];
        let mut next_run = vec![0_usize; n];

        // Force intro beats `0..intro` at the start and outro beats `(n - outro)..n` at the end.
        let forced_beat = if step < intro {
            Some(step.min(n - 1))
        } else if step >= target_beats - outro {
            let rem = target_beats - 1 - step;
            Some(n.saturating_sub(1 + rem))
        } else {
            None
        };

        for prev in 0..n {
            if dp[prev] >= inf * 0.5 {
                continue;
            }
            // 1. Natural continuation `prev -> prev + 1` (zero transition cost).
            if prev + 1 < n && forced_beat.is_none_or(|fb| fb == prev + 1) {
                let cand = dp[prev];
                if cand < next_dp[prev + 1] {
                    next_dp[prev + 1] = cand;
                    next_run[prev + 1] = run[prev] + 1;
                    back[step][prev + 1] = prev;
                }
            }

            // 2. Seam jump `prev -> next`, allowed after `min_seg` consecutive beats (or when forced into outro).
            let allow_jump = run[prev] >= min_seg || step == target_beats - outro;
            if !allow_jump {
                continue;
            }
            for next in 0..n {
                if next == prev + 1 || forced_beat.is_some_and(|fb| fb != next) {
                    continue;
                }
                if shortening && next <= prev + 1 && forced_beat.is_none() {
                    continue;
                }
                // Avoid jumping back into the very first beat or past the outro unless forced.
                if forced_beat.is_none() && (next == 0 || next >= n.saturating_sub(outro)) {
                    continue;
                }
                let jump_penalty = 0.45 + seam_matrix[prev][next];
                let cand = dp[prev] + jump_penalty;
                if cand < next_dp[next] {
                    next_dp[next] = cand;
                    next_run[next] = 1;
                    back[step][next] = prev;
                }
            }
        }

        // Fallback if constraints were over-tight on very short clips: allow best previous -> forced/any beat.
        if next_dp.iter().all(|&v| v >= inf * 0.5) {
            relax_step_fallback(&dp, seam_matrix, forced_beat, step, &mut next_dp, &mut next_run, &mut back);
        }

        dp = next_dp;
        run = next_run;
    }

    let mut end_beat = n - 1;
    if dp[end_beat] >= inf * 0.5 {
        for (b, &cost) in dp.iter().enumerate() {
            if cost < dp[end_beat] {
                end_beat = b;
            }
        }
    }

    let mut path = vec![0_usize; target_beats];
    let mut cur = end_beat;
    for step in (0..target_beats).rev() {
        path[step] = cur;
        cur = back[step][cur];
    }
    path
}

fn relax_step_fallback(
    dp: &[f32],
    seam_matrix: &[Vec<f32>],
    forced_beat: Option<usize>,
    step: usize,
    next_dp: &mut [f32],
    next_run: &mut [usize],
    back: &mut [Vec<usize>],
) {
    let n = dp.len();
    let targets = forced_beat.map_or(0..n, |fb| fb..(fb + 1));
    for next in targets {
        for (prev, &prev_cost) in dp.iter().enumerate() {
            if prev_cost >= 1.0e29 {
                continue;
            }
            let c = prev_cost + if next == prev + 1 { 0.0 } else { 0.5 + seam_matrix[prev][next] };
            if c < next_dp[next] {
                next_dp[next] = c;
                next_run[next] = 1;
                back[step][next] = prev;
            }
        }
    }
}

/// Group consecutive natural beats into contiguous source spans `[src_start..src_end]`, render
/// equal-power crossfades at every seam, and conform the output length to `target_frames`.
fn synthesize_beat_plan(
    channels: &[Vec<f32>],
    beat_bounds: &[(usize, usize)],
    seam_matrix: &[Vec<f32>],
    beat_sequence: &[usize],
    target_frames: usize,
    xfade_len: usize,
    grid: &BeatGrid,
) -> RemixResult {
    let in_frames = channels.iter().map(Vec::len).min().unwrap_or(0);
    // Merge contiguous beat indices `(b, b+1, b+2, ...)` into continuous spans `(first_beat, last_beat)`.
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for &b in beat_sequence {
        if let Some(last) = spans.last_mut()
            && b == last.1 + 1
        {
            last.1 = b;
        } else {
            spans.push((b, b));
        }
    }
    // Include pre-first-beat pickup on the opening span if it starts at beat 0.
    let mut out: Vec<Vec<f32>> = channels.iter().map(|_| Vec::with_capacity(target_frames + xfade_len)).collect();
    let mut edits: Vec<SeamEdit> = Vec::new();

    for (span_idx, &(first_b, last_b)) in spans.iter().enumerate() {
        let seg_start = if span_idx == 0 && first_b == 0 { 0 } else { beat_bounds[first_b].0 };
        let seg_end = if span_idx + 1 == spans.len() && last_b + 1 == beat_bounds.len() { in_frames } else { beat_bounds[last_b].1 };
        if seg_end <= seg_start {
            continue;
        }
        if span_idx == 0 {
            for (c, dst) in out.iter_mut().enumerate() {
                dst.extend_from_slice(&channels[c][seg_start..seg_end]);
            }
        } else {
            let prev_last_b = spans[span_idx - 1].1;
            let prev_end = beat_bounds[prev_last_b].1;
            let cur_out_len = out[0].len();
            let xf = xfade_len.min(cur_out_len / 2).min((seg_end - seg_start) / 2).max(1);
            let out_seam_pos = cur_out_len.saturating_sub(xf);

            // Outgoing tail samples come from source continuation past `prev_end` when available,
            // or the end of `out` already written; crossfade with incoming `[seg_start..seg_start + xf]`.
            for (c, dst) in out.iter_mut().enumerate() {
                for k in 0..xf {
                    let t = (k as f32 + 0.5) / xf as f32;
                    let g_in = FadeShape::EqualPower.gain(t);
                    let g_out = FadeShape::EqualPower.gain(1.0 - t);
                    let outgoing = dst[out_seam_pos + k];
                    let incoming = channels[c][seg_start + k];
                    dst[out_seam_pos + k] = outgoing * g_out + incoming * g_in;
                }
                if seg_start + xf < seg_end {
                    dst.extend_from_slice(&channels[c][(seg_start + xf)..seg_end]);
                }
            }
            let cost = seam_matrix.get(prev_last_b).and_then(|row| row.get(first_b)).copied().unwrap_or(1.0);
            edits.push(SeamEdit {
                from_sample: prev_end,
                to_sample: seg_start,
                output_sample: out_seam_pos,
                similarity: 1.0 / (1.0 + cost.max(0.0)),
            });
        }
    }

    conform_exact_length(&mut out, channels, in_frames, target_frames, xfade_len);

    RemixResult { channels: out, bpm: grid.bpm, beat_count: grid.beats.len(), edits, target_frames }
}

/// Ensure every output channel is sample-exact `target_frames` long with smooth fade-out or
/// micro-stretch so beat-quantized sums hit arbitrary sub-beat `target_seconds` cleanly.
fn conform_exact_length(out: &mut [Vec<f32>], src_channels: &[Vec<f32>], in_frames: usize, target_frames: usize, xfade_len: usize) {
    let cur_len = out.first().map_or(0, Vec::len);
    if cur_len == 0 {
        for (c, dst) in out.iter_mut().enumerate() {
            *dst = linear_time_conform(&src_channels[c][..in_frames], target_frames);
        }
        return;
    }

    // If within +/-12% of `target_frames`, smoothly resample/time-conform or trim with cosine tail
    // so pitch/tempo stay natural and zero clicks occur at the end.
    let ratio = cur_len as f64 / target_frames as f64;
    if (0.88..=1.12).contains(&ratio) && cur_len != target_frames {
        for dst in out.iter_mut() {
            *dst = crate::offline::time_stretch(std::slice::from_ref(dst), target_frames as f64 / cur_len as f64, 48_000.0)
                .into_iter()
                .next()
                .unwrap_or_default();
            dst.resize(target_frames, 0.0);
        }
    } else if cur_len > target_frames {
        let fade = xfade_len.min(target_frames / 4).max(1);
        for dst in out.iter_mut() {
            dst.truncate(target_frames);
            let start = target_frames.saturating_sub(fade);
            for (k, s) in dst[start..].iter_mut().enumerate() {
                let t = (k as f32 + 0.5) / fade as f32;
                *s *= FadeShape::EqualPower.gain(1.0 - t);
            }
        }
    } else if cur_len < target_frames {
        for dst in out.iter_mut() {
            dst.resize(target_frames, 0.0);
        }
    }
}

fn fallback_resample_duration(channels: &[Vec<f32>], in_frames: usize, target_frames: usize, grid: &BeatGrid) -> RemixResult {
    let out: Vec<Vec<f32>> = channels.iter().map(|c| linear_time_conform(&c[..in_frames], target_frames)).collect();
    RemixResult { channels: out, bpm: grid.bpm, beat_count: grid.beats.len(), edits: Vec::new(), target_frames }
}

fn linear_time_conform(src: &[f32], target_frames: usize) -> Vec<f32> {
    if target_frames == 0 || src.is_empty() {
        return vec![0.0; target_frames];
    }
    if target_frames == 1 {
        return vec![src[0]];
    }
    let last = (src.len() - 1) as f64;
    let denom = (target_frames - 1) as f64;
    (0..target_frames)
        .map(|i| {
            let pos = (i as f64) * last / denom;
            let idx = (pos.floor() as usize).min(src.len() - 1);
            let frac = (pos - idx as f64) as f32;
            let a = src[idx];
            let b = src.get(idx + 1).copied().unwrap_or(a);
            a + (b - a) * frac
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Synthesize a stereo musical test track at `bpm` (default 120 BPM = 0.5s per beat) with
    /// distinct intro, repeating 4-beat chorus groove, and outro.
    fn synth_rhythmic_song(sr: u32, beats: usize, bpm: f32) -> Vec<Vec<f32>> {
        let period = ((60.0 / bpm) * sr as f32).round() as usize;
        let frames = beats * period;
        let mut left = vec![0.0_f32; frames];
        let mut right = vec![0.0_f32; frames];

        for b in 0..beats {
            let start = b * period;
            let chord_hz = match b % 4 {
                0 => 220.0_f32,
                1 => 277.18_f32,
                2 => 329.63_f32,
                _ => 246.94_f32,
            };
            for k in 0..period {
                let t = k as f32 / sr as f32;
                // Percussive kick/hat transient on every beat onset + sustained harmonic pad.
                let transient = (-t * 28.0).exp() * (std::f32::consts::TAU * 95.0 * t).sin();
                let pad = 0.22 * (std::f32::consts::TAU * chord_hz * t).sin();
                left[start + k] = 0.55 * transient + pad;
                right[start + k] = 0.55 * transient + 0.9 * pad;
            }
        }
        vec![left, right]
    }

    #[test]
    fn detect_beats_finds_120_bpm_grid_on_rhythmic_audio() {
        let sr = 24_000;
        let song = synth_rhythmic_song(sr, 16, 120.0);
        let grid = detect_beats(&song, sr as f32);
        assert!((grid.bpm - 120.0).abs() < 6.0, "expected ~120 BPM, got {}", grid.bpm);
        assert!(grid.beats.len() >= 12, "expected beat grid across 16 beats, got {}", grid.beats.len());
    }

    #[test]
    fn remix_shortens_and_lengthens_to_exact_target_duration_without_seam_clicks() {
        let sr = 24_000;
        // 16 beats at 120 BPM = 8.0 seconds.
        let song = synth_rhythmic_song(sr, 16, 120.0);
        let opts = RemixOptions::default();

        // 1. Shorten from 8.0s -> 5.0s:
        let shortened = remix_to_duration(&song, sr as f32, 5.0, &opts);
        assert_eq!(shortened.target_frames, 5 * sr as usize);
        assert_eq!(shortened.channels[0].len(), 5 * sr as usize);
        assert_eq!(shortened.channels[1].len(), 5 * sr as usize);
        assert!(!shortened.edits.is_empty(), "shortening from 8s to 5s should make at least one beat cut");
        for s in &shortened.channels[0] {
            assert!(s.is_finite() && s.abs() <= 1.5);
        }

        // 2. Lengthen from 8.0s -> 12.0s:
        let extended = remix_to_duration(&song, sr as f32, 12.0, &opts);
        assert_eq!(extended.target_frames, 12 * sr as usize);
        assert_eq!(extended.channels[0].len(), 12 * sr as usize);
        assert!(!extended.edits.is_empty(), "extending from 8s to 12s should loop beat-aligned sections");
    }
}
