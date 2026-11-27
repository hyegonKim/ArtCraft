//! Acoustic lip-sync viseme analysis for 2D character mouth rigs.
//!
//! Maps decoded audio (`Pcm`) across movie frames to the standard 12-viseme mouth chart used in
//! 2D animation workflows (`neutral`, `ah`, `d`, `ee`, `f`, `l`, `m`, `oh`, `r`, `s`, `uh`, `woo`).
//! Analysis is pure and deterministic: it uses RMS energy normalized across the analysed clip,
//! zero-crossing rate, three-band one-pole spectral energy shares (low < 900 Hz, mid 900–2800 Hz,
//! high > 2800 Hz), frame-to-frame amplitude onset slope, and 1-frame jitter smoothing.

use anicraft_audio::Pcm;
use serde::{Deserialize, Serialize};

/// Maximum number of movie frames analysed in one auto lip-sync pass.
pub const MAX_LIPSYNC_FRAMES: u32 = 100_000;

/// Standard 12 mouth visemes in conventional Graphic symbol frame order (`0..12`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Viseme {
    #[default]
    Neutral,
    Ah,
    D,
    Ee,
    F,
    L,
    M,
    Oh,
    R,
    S,
    Uh,
    Woo,
}

impl Viseme {
    pub const ALL: [Viseme; 12] = [
        Viseme::Neutral,
        Viseme::Ah,
        Viseme::D,
        Viseme::Ee,
        Viseme::F,
        Viseme::L,
        Viseme::M,
        Viseme::Oh,
        Viseme::R,
        Viseme::S,
        Viseme::Uh,
        Viseme::Woo,
    ];

    pub const fn key(self) -> &'static str {
        match self {
            Viseme::Neutral => "neutral",
            Viseme::Ah => "ah",
            Viseme::D => "d",
            Viseme::Ee => "ee",
            Viseme::F => "f",
            Viseme::L => "l",
            Viseme::M => "m",
            Viseme::Oh => "oh",
            Viseme::R => "r",
            Viseme::S => "s",
            Viseme::Uh => "uh",
            Viseme::Woo => "woo",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Viseme::Neutral => "Neutral",
            Viseme::Ah => "Ah",
            Viseme::D => "D",
            Viseme::Ee => "Ee",
            Viseme::F => "F",
            Viseme::L => "L",
            Viseme::M => "M",
            Viseme::Oh => "Oh",
            Viseme::R => "R",
            Viseme::S => "S",
            Viseme::Uh => "Uh",
            Viseme::Woo => "Woo",
        }
    }

    /// Conventional 0-based frame index inside a 12-pose mouth Graphic symbol.
    pub const fn default_frame(self) -> u32 {
        match self {
            Viseme::Neutral => 0,
            Viseme::Ah => 1,
            Viseme::D => 2,
            Viseme::Ee => 3,
            Viseme::F => 4,
            Viseme::L => 5,
            Viseme::M => 6,
            Viseme::Oh => 7,
            Viseme::R => 8,
            Viseme::S => 9,
            Viseme::Uh => 10,
            Viseme::Woo => 11,
        }
    }

    pub fn from_key(s: &str) -> Option<Viseme> {
        let k = s.trim().to_ascii_lowercase();
        Viseme::ALL.into_iter().find(|v| v.key() == k)
    }

    fn label_aliases(self) -> &'static [&'static str] {
        match self {
            Viseme::Neutral => &["neutral", "rest", "closed", "idle"],
            Viseme::Ah => &["ah", "ai", "a"],
            Viseme::D => &["d", "cdgknrsthyz", "etc", "consonant"],
            Viseme::Ee => &["ee", "e", "i"],
            Viseme::F => &["f", "fv"],
            Viseme::L => &["l", "th"],
            Viseme::M => &["m", "mbp", "bmp"],
            Viseme::Oh => &["oh", "o"],
            Viseme::R => &["r", "cdgknrsthyz", "er"],
            Viseme::S => &["s", "cdgknrsthyz", "sz"],
            Viseme::Uh => &["uh", "u", "ai"],
            Viseme::Woo => &["woo", "wq", "w", "oo", "u"],
        }
    }
}

/// Builds a default 12-viseme -> 0-based symbol frame table for `sym`, matching keyframe labels
/// (such as the built-in `templates::MOUTHS` chart or viseme names) before falling back to frame
/// order.
pub fn default_mapping_for_symbol(sym: &crate::Symbol) -> [u32; 12] {
    let cap = sym.timeline.frame_count().max(1);
    let labelled: Vec<(String, u32)> = sym
        .timeline
        .layers
        .iter()
        .flat_map(|l| l.keyframes.iter())
        .filter(|k| !k.label.trim().is_empty())
        .map(|k| (k.label.trim().to_ascii_lowercase(), k.start.min(cap - 1)))
        .collect();
    let mut table = [0u32; 12];
    for (idx, v) in Viseme::ALL.into_iter().enumerate() {
        let by_label = v.label_aliases().iter().find_map(|alias| labelled.iter().find(|(lbl, _)| lbl == alias).map(|(_, f)| *f));
        let fallback =
            if cap >= 12 || v == Viseme::Neutral || cap == 1 { v.default_frame().min(cap - 1) } else { 1 + ((v.default_frame() - 1) % (cap - 1)) };
        if let Some(slot) = table.get_mut(idx) {
            *slot = by_label.unwrap_or(fallback);
        }
    }
    table
}

/// A contiguous span of frames sharing the same [`Viseme`], relative to the start of analysis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisemeRun {
    pub offset: u32,
    pub duration: u32,
    pub viseme: Viseme,
}

#[derive(Clone, Copy, Default)]
struct FrameFeatures {
    rms: f32,
    zcr: f32,
    low_share: f32,
    mid_share: f32,
    high_share: f32,
}

fn one_pole_alpha(cutoff_hz: f32, sample_rate: f32) -> f32 {
    let dt = 1.0 / sample_rate.max(1000.0);
    let rc = 1.0 / (2.0 * std::f32::consts::PI * cutoff_hz.max(1.0));
    (dt / (rc + dt)).clamp(0.001, 0.999)
}

fn extract_window(pcm: &Pcm, start_sample: usize, end_sample: usize) -> FrameFeatures {
    let total = pcm.frames();
    let s0 = start_sample.min(total);
    let s1 = end_sample.min(total);
    if s1 <= s0 {
        return FrameFeatures::default();
    }
    let rate = pcm.rate.max(1000) as f32;
    let alpha_low = one_pole_alpha(850.0, rate);
    let alpha_high = one_pole_alpha(2700.0, rate);

    let mut lp_low = 0.0_f32;
    let mut lp_high = 0.0_f32;
    let mut sum_sq = 0.0_f64;
    let mut low_e = 0.0_f64;
    let mut mid_e = 0.0_f64;
    let mut high_e = 0.0_f64;
    let mut crossings = 0usize;
    let mut prev = 0.0_f32;

    for idx in s0..s1 {
        let (l, r) = pcm.frame(idx);
        let x = 0.5 * (l + r);
        lp_low += alpha_low * (x - lp_low);
        lp_high += alpha_high * (x - lp_high);
        let band_low = lp_low;
        let band_mid = lp_high - lp_low;
        let band_high = x - lp_high;

        sum_sq += f64::from(x) * f64::from(x);
        low_e += f64::from(band_low) * f64::from(band_low);
        mid_e += f64::from(band_mid) * f64::from(band_mid);
        high_e += f64::from(band_high) * f64::from(band_high);

        if idx > s0 && ((x >= 0.0 && prev < 0.0) || (x < 0.0 && prev >= 0.0)) {
            crossings += 1;
        }
        prev = x;
    }

    let n = (s1 - s0) as f64;
    let rms = (sum_sq / n).sqrt() as f32;
    let zcr = if s1 - s0 > 1 { crossings as f32 / (s1 - s0 - 1) as f32 } else { 0.0 };
    let band_sum = (low_e + mid_e + high_e).max(1.0e-12);
    FrameFeatures { rms, zcr, low_share: (low_e / band_sum) as f32, mid_share: (mid_e / band_sum) as f32, high_share: (high_e / band_sum) as f32 }
}

fn classify_frame(feat: FrameFeatures, norm_rms: f32, prev_norm: f32, next_norm: f32) -> Viseme {
    if norm_rms < 0.06 || feat.rms < 1.0e-4 {
        return Viseme::Neutral;
    }
    let delta_up = norm_rms - prev_norm;
    let near_voiced = prev_norm > 0.25 || next_norm > 0.25;

    // Bilabial closure ('m', 'b', 'p' hold): quiet, low-frequency nasal/murmur adjacent to speech.
    if norm_rms < 0.18 && feat.low_share > 0.72 && feat.zcr < 0.10 && near_voiced {
        return Viseme::M;
    }
    // High-frequency sibilants ('s', 'z', 'sh', 'ch'): unvoiced high-band hiss.
    if feat.zcr > 0.32 || (feat.high_share > 0.48 && feat.low_share < 0.10) {
        return Viseme::S;
    }
    // Labiodental fricatives ('f', 'v'): airy broadband friction without strong vowel voicing.
    if norm_rms < 0.55 && feat.low_share < 0.14 && ((feat.zcr > 0.20 && feat.high_share > 0.24) || feat.high_share > 0.34) {
        return Viseme::F;
    }
    // Alveolar stop/tap onset ('d', 't', 'n'): crisp transient burst with mid/high content.
    if delta_up > 0.22 && (feat.mid_share + feat.high_share) > 0.36 && feat.zcr > 0.11 {
        return Viseme::D;
    }
    // Rounded back vowels ('oh', 'woo'): strong fundamental + low F1/F2 concentration.
    if feat.low_share > 0.80 {
        return if norm_rms > 0.48 { Viseme::Oh } else { Viseme::Woo };
    }
    // Wide open vowel ('ah'): high jaw-open energy with balanced F1/F2 mid presence.
    if norm_rms > 0.58 && feat.mid_share > 0.22 && feat.high_share < 0.24 {
        return Viseme::Ah;
    }
    // Bright front vowels & laterals ('ee', 'l'): prominent F2/F3 energy.
    if (feat.mid_share + feat.high_share) > 0.46 {
        return if feat.high_share > 0.12 || feat.zcr > 0.12 { Viseme::Ee } else { Viseme::L };
    }
    // Mid-central / rhotic vowels ('r', 'uh').
    if feat.mid_share > 0.30 { Viseme::R } else { Viseme::Uh }
}

/// Analyses `frame_count` movie frames starting at `start_frame` (at `fps` frames per second)
/// from `pcm` and returns one [`Viseme`] per frame.
pub fn analyze_visemes(pcm: &Pcm, fps: f32, start_frame: u32, frame_count: u32) -> Vec<Viseme> {
    let fps = if fps.is_finite() && fps > 0.0 { fps.clamp(1.0, 120.0) } else { 24.0 };
    let count = frame_count.min(MAX_LIPSYNC_FRAMES) as usize;
    if count == 0 {
        return Vec::new();
    }
    let samples_per_frame = f64::from(pcm.rate.max(1000)) / f64::from(fps);
    let pad = (samples_per_frame * 0.15).round() as usize;

    let mut features = Vec::with_capacity(count);
    let mut peak_rms = 0.0_f32;
    for i in 0..count {
        let f = f64::from(start_frame) + i as f64;
        let s0 = ((f * samples_per_frame).floor() as usize).saturating_sub(pad);
        let s1 = (((f + 1.0) * samples_per_frame).ceil() as usize).saturating_add(pad);
        let feat = extract_window(pcm, s0, s1);
        if feat.rms > peak_rms {
            peak_rms = feat.rms;
        }
        features.push(feat);
    }

    if peak_rms < 1.0e-4 {
        return vec![Viseme::Neutral; count];
    }

    let norms: Vec<f32> = features.iter().map(|f| (f.rms / peak_rms).clamp(0.0, 1.0)).collect();
    let mut raw = Vec::with_capacity(count);
    for (i, feat) in features.iter().copied().enumerate() {
        let cur = norms.get(i).copied().unwrap_or(0.0);
        let prev = i.checked_sub(1).and_then(|j| norms.get(j)).copied().unwrap_or(0.0);
        let next = norms.get(i + 1).copied().unwrap_or(0.0);
        raw.push(classify_frame(feat, cur, prev, next));
    }

    // Smooth isolated 1-frame jitter sandwiched between identical neighbours, while keeping
    // deliberate consonant closures/stops (`M`, `D`, `S`, `F`).
    let mut smoothed = raw.clone();
    for i in 1..count.saturating_sub(1) {
        let (Some(&prev), Some(&cur), Some(&next)) = (raw.get(i - 1), raw.get(i), raw.get(i + 1)) else {
            continue;
        };
        if prev == next
            && cur != prev
            && !matches!(cur, Viseme::M | Viseme::D | Viseme::S | Viseme::F)
            && let Some(slot) = smoothed.get_mut(i)
        {
            *slot = prev;
        }
    }
    smoothed
}

/// Coalesces per-frame visemes into contiguous `(offset, duration, viseme)` runs.
pub fn coalesce_runs(visemes: &[Viseme]) -> Vec<VisemeRun> {
    let mut runs: Vec<VisemeRun> = Vec::new();
    for (i, &v) in visemes.iter().enumerate() {
        if let Some(last) = runs.last_mut().filter(|r| r.viseme == v) {
            last.duration = last.duration.saturating_add(1);
        } else {
            runs.push(VisemeRun { offset: i as u32, duration: 1, viseme: v });
        }
    }
    runs
}

/// Convenience helper returning coalesced [`VisemeRun`]s for `pcm`.
pub fn analyze_runs(pcm: &Pcm, fps: f32, start_frame: u32, frame_count: u32) -> Vec<VisemeRun> {
    coalesce_runs(&analyze_visemes(pcm, fps, start_frame, frame_count))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn synth_tone(rate: u32, seconds: f32, freqs: &[(f32, f32)]) -> Pcm {
        let total = (rate as f32 * seconds) as usize;
        let samples: Vec<f32> = (0..total)
            .map(|i| {
                let t = i as f32 / rate as f32;
                freqs.iter().map(|&(hz, amp)| amp * (2.0 * std::f32::consts::PI * hz * t).sin()).sum::<f32>().clamp(-1.0, 1.0)
            })
            .collect();
        Pcm::new(rate, 1, samples).unwrap()
    }

    #[test]
    fn silence_maps_to_neutral_and_coalesces() {
        let pcm = Pcm::new(24_000, 1, vec![0.0; 12_000]).unwrap();
        let runs = analyze_runs(&pcm, 24.0, 0, 12);
        assert_eq!(runs, vec![VisemeRun { offset: 0, duration: 12, viseme: Viseme::Neutral }]);
    }

    #[test]
    fn distinguishes_sibilant_open_vowel_and_rounded_vowel() {
        let rate = 24_000_u32;
        let frame_len = 1_000usize; // 24 fps -> 1000 samples/frame
        let mut samples = Vec::with_capacity(frame_len * 12);
        // Frames 0..3: silence -> Neutral
        samples.extend(std::iter::repeat_n(0.0_f32, frame_len * 3));
        // Frames 3..6: high-frequency sibilant (~5500 Hz) -> S
        for i in 0..(frame_len * 3) {
            let t = i as f32 / rate as f32;
            samples.push(0.45 * (2.0 * std::f32::consts::PI * 5500.0 * t).sin());
        }
        // Frames 6..9: loud open vowel (650 Hz + 1250 Hz) -> Ah
        for i in 0..(frame_len * 3) {
            let t = i as f32 / rate as f32;
            samples.push(0.55 * (2.0 * std::f32::consts::PI * 650.0 * t).sin() + 0.40 * (2.0 * std::f32::consts::PI * 1250.0 * t).sin());
        }
        // Frames 9..12: rounded low back vowel (260 Hz) -> Woo / Oh
        for i in 0..(frame_len * 3) {
            let t = i as f32 / rate as f32;
            samples.push(0.38 * (2.0 * std::f32::consts::PI * 260.0 * t).sin());
        }
        let pcm = Pcm::new(rate, 1, samples).unwrap();
        let visemes = analyze_visemes(&pcm, 24.0, 0, 12);
        assert_eq!(visemes.first().copied(), Some(Viseme::Neutral));
        assert_eq!(visemes.get(4).copied(), Some(Viseme::S));
        assert_eq!(visemes.get(7).copied(), Some(Viseme::Ah));
        assert!(matches!(visemes.get(10).copied(), Some(Viseme::Woo | Viseme::Oh)));
    }

    #[test]
    fn front_vowel_maps_to_ee() {
        let pcm = synth_tone(24_000, 0.25, &[(320.0, 0.25), (2400.0, 0.35), (3200.0, 0.25)]);
        let visemes = analyze_visemes(&pcm, 24.0, 0, 6);
        assert!(visemes.contains(&Viseme::Ee));
    }

    #[test]
    fn maps_builtin_mouth_chart_template_labels() {
        let doc = (crate::templates::find("lip-sync").unwrap().build)();
        let mouth_sym = doc.library.first().unwrap();
        let table = default_mapping_for_symbol(mouth_sym);
        // MOUTHS = ["Rest", "AI", "E", "O", "U", "CDGKNRSThYZ", "FV", "L", "MBP", "WQ"]
        assert_eq!(table[Viseme::Neutral as usize], 0);
        assert_eq!(table[Viseme::Ah as usize], 1);
        assert_eq!(table[Viseme::Ee as usize], 2);
        assert_eq!(table[Viseme::Oh as usize], 3);
        assert_eq!(table[Viseme::Uh as usize], 4);
        assert_eq!(table[Viseme::D as usize], 5);
        assert_eq!(table[Viseme::F as usize], 6);
        assert_eq!(table[Viseme::L as usize], 7);
        assert_eq!(table[Viseme::M as usize], 8);
        assert_eq!(table[Viseme::Woo as usize], 9);
    }
}
