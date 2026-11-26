//! Restoration & intelligent separation processors:
//! - [`CrosstalkSeparator`] (`separate_crosstalk`): multi-channel microphone bleed suppressor.
//! - [`AudioLeveler`] (`audio_leveler`): speech/dialogue loudness rider with noise-floor freeze.
//! - [`StemSeparator`] (`stem_separator`): realtime zero-allocation Vocal / Accompaniment / Ambience
//!   stem separator and mixer with lossless unity reconstruction.

use crate::biquad::{Biquad, FilterType};
use crate::params::{Params, choice, lin, log, param_plumbing};
use crate::util::{Smoother, flush, frames_in, sane_channels, sane_sr, time_coef};
use crate::{Category, ParamInfo, Plugin, PluginInfo, Unit, db_to_gain, gain_to_db};

// ─── Separate Crosstalk ─────────────────────────────────────────────────────────────────────────

static XTALK_PARAMS: [ParamInfo; 5] = [
    lin("sensitivity", "Sensitivity", 0.0, 100.0, 60.0, Unit::Percent),
    lin("suppression", "Suppression", 0.0, 48.0, 18.0, Unit::Db),
    log("attack", "Attack", 0.5, 100.0, 5.0, Unit::Ms),
    log("release", "Release", 10.0, 1000.0, 120.0, Unit::Ms),
    lin("threshold", "Active Floor", -72.0, -12.0, -48.0, Unit::Db),
];

pub static XTALK_INFO: PluginInfo = PluginInfo {
    id: "separate_crosstalk",
    name: "Separate Crosstalk",
    short_name: "XTalk",
    category: Category::Dynamics,
    params: &XTALK_PARAMS,
    is_instrument: false,
    audiosuite: true,
};

/// Multi-channel microphone bleed detector and suppressor.
///
/// Compares per-channel speech envelopes at each frame; when one channel dominates over another by
/// the sensitivity margin and exceeds the active floor, the quieter channel is attenuated as bleed.
pub struct CrosstalkSeparator {
    p: Params,
    sr: f32,
    channels: usize,
    env: Vec<f32>,
    gain_db: Vec<f32>,
    env_att: f32,
    env_rel: f32,
    att: f32,
    rel: f32,
    gr_meter: f32,
}

impl Default for CrosstalkSeparator {
    fn default() -> Self {
        Self::new()
    }
}

impl CrosstalkSeparator {
    pub fn new() -> Self {
        let mut s = Self {
            p: Params::new(&XTALK_PARAMS),
            sr: 48_000.0,
            channels: 2,
            env: vec![0.0; 2],
            gain_db: vec![0.0; 2],
            env_att: 0.0,
            env_rel: 0.0,
            att: 0.0,
            rel: 0.0,
            gr_meter: 0.0,
        };
        s.update();
        s
    }

    fn update(&mut self) {
        self.env_att = time_coef(2.0, self.sr);
        self.env_rel = time_coef(35.0, self.sr);
        self.att = time_coef(self.p.v(2), self.sr);
        self.rel = time_coef(self.p.v(3), self.sr);
    }

    #[inline]
    fn target_cut_db(dom_db: f32, ch_db: f32, floor_db: f32, margin_db: f32, max_cut: f32) -> f32 {
        if dom_db <= floor_db {
            return 0.0;
        }
        let diff = dom_db - ch_db;
        if diff <= margin_db {
            return 0.0;
        }
        let scale = ((diff - margin_db) / 12.0).clamp(0.0, 1.0);
        max_cut * scale
    }
}

impl Plugin for CrosstalkSeparator {
    param_plumbing!(XTALK_INFO);

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, channels: usize) {
        self.sr = sane_sr(sample_rate);
        self.channels = sane_channels(channels);
        self.env.resize(self.channels, 0.0);
        self.gain_db.resize(self.channels, 0.0);
        self.reset();
        self.update();
    }

    fn reset(&mut self) {
        self.env.fill(0.0);
        self.gain_db.fill(0.0);
        self.gr_meter = 0.0;
    }

    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        let ch = self.channels.min(io.len());
        let n = frames_in(io, frames, ch);
        if ch < 2 || n == 0 {
            self.gr_meter = 0.0;
            return;
        }
        let sens = self.p.v(0).clamp(0.0, 100.0);
        let max_cut = self.p.v(1).clamp(0.0, 48.0);
        let floor_db = self.p.v(4);
        // Higher sensitivity catches smaller inter-channel level differences (from 14 dB down to 2 dB).
        let margin_db = 14.0 - 0.12 * sens;
        let mut peak_gr = 0.0f32;

        for i in 0..n {
            let mut dom = 0.0f32;
            for c in 0..ch {
                let x = io.get(c).and_then(|v| v.get(i)).copied().unwrap_or(0.0).abs();
                let x = if x.is_finite() { x } else { 0.0 };
                if let Some(e) = self.env.get_mut(c) {
                    let k = if x > *e { self.env_att } else { self.env_rel };
                    *e = flush(x + (*e - x) * k);
                    if *e > dom {
                        dom = *e;
                    }
                }
            }
            let dom_db = gain_to_db(dom);
            for c in 0..ch {
                let ch_env = self.env.get(c).copied().unwrap_or(0.0);
                let target = Self::target_cut_db(dom_db, gain_to_db(ch_env), floor_db, margin_db, max_cut);
                let cur = self.gain_db.get_mut(c);
                let gr = if let Some(g) = cur {
                    let k = if target > *g { self.att } else { self.rel };
                    *g = flush(target + (*g - target) * k);
                    *g
                } else {
                    0.0
                };
                if gr > peak_gr {
                    peak_gr = gr;
                }
                if let Some(s) = io.get_mut(c).and_then(|v| v.get_mut(i)) {
                    *s *= db_to_gain(-gr);
                }
            }
        }
        self.gr_meter = peak_gr;
    }

    fn gain_reduction_db(&self) -> f32 {
        self.gr_meter
    }
}

// ─── Audio Leveling ─────────────────────────────────────────────────────────────────────────────

static LEVELER_PARAMS: [ParamInfo; 6] = [
    lin("target", "Target Level", -36.0, -6.0, -18.0, Unit::Db),
    lin("amount", "Amount", 0.0, 100.0, 80.0, Unit::Percent),
    lin("max_boost", "Max Boost", 0.0, 24.0, 12.0, Unit::Db),
    lin("max_cut", "Max Cut", 0.0, 36.0, 18.0, Unit::Db),
    lin("gate", "Noise Gate", -80.0, -24.0, -54.0, Unit::Db),
    log("speed", "Response", 50.0, 2000.0, 300.0, Unit::Ms),
];

pub static LEVELER_INFO: PluginInfo = PluginInfo {
    id: "audio_leveler",
    name: "Audio Leveling",
    short_name: "Leveler",
    category: Category::Dynamics,
    params: &LEVELER_PARAMS,
    is_instrument: false,
    audiosuite: true,
};

/// Automatic speech & programme loudness rider with low-level gate freeze and peak protection.
pub struct AudioLeveler {
    p: Params,
    sr: f32,
    channels: usize,
    det_hp: Vec<Biquad>,
    rms_sq: f32,
    gain_db: f32,
    rms_coef: f32,
    up_coef: f32,
    down_coef: f32,
    gr_meter: f32,
}

impl Default for AudioLeveler {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioLeveler {
    pub fn new() -> Self {
        let mut l = Self {
            p: Params::new(&LEVELER_PARAMS),
            sr: 48_000.0,
            channels: 2,
            det_hp: Vec::new(),
            rms_sq: 0.0,
            gain_db: 0.0,
            rms_coef: 0.0,
            up_coef: 0.0,
            down_coef: 0.0,
            gr_meter: 0.0,
        };
        l.update();
        l
    }

    fn update(&mut self) {
        let resp = self.p.v(5).clamp(50.0, 2000.0);
        self.rms_coef = time_coef(3.5, self.sr);
        self.up_coef = time_coef(resp * 0.35, self.sr);
        self.down_coef = time_coef((resp * 0.06).max(5.0), self.sr);
        for hp in &mut self.det_hp {
            hp.set(FilterType::HighPass, 110.0, std::f32::consts::FRAC_1_SQRT_2, 0.0, self.sr);
        }
    }

    #[inline]
    fn desired_gain_db(&self, rms_db: f32) -> f32 {
        let gate_db = self.p.v(4);
        if rms_db < gate_db {
            // Below the noise gate floor, decay gently toward unity instead of boosting room tone.
            return 0.0;
        }
        let target = self.p.v(0);
        let amount = self.p.v(1).clamp(0.0, 100.0) * 0.01;
        let boost = self.p.v(2).clamp(0.0, 24.0);
        let cut = self.p.v(3).clamp(0.0, 36.0);
        ((target - rms_db) * amount).clamp(-cut, boost)
    }
}

impl Plugin for AudioLeveler {
    param_plumbing!(LEVELER_INFO);

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, channels: usize) {
        self.sr = sane_sr(sample_rate);
        self.channels = sane_channels(channels);
        self.det_hp = vec![Biquad::default(); self.channels];
        self.reset();
        self.update();
    }

    fn reset(&mut self) {
        self.det_hp.iter_mut().for_each(Biquad::reset);
        self.rms_sq = 0.0;
        self.gain_db = 0.0;
        self.gr_meter = 0.0;
    }

    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        let ch = self.channels.min(io.len());
        let n = frames_in(io, frames, ch);
        if ch == 0 || n == 0 {
            return;
        }
        let ceiling = db_to_gain(-0.5);
        let mut max_cut = 0.0f32;
        for i in 0..n {
            let mut power = 0.0f32;
            let mut raw_peak = 0.0f32;
            for c in 0..ch {
                let x = io.get(c).and_then(|v| v.get(i)).copied().unwrap_or(0.0);
                let x = if x.is_finite() { x } else { 0.0 };
                raw_peak = raw_peak.max(x.abs());
                let w = self.det_hp.get_mut(c).map_or(x, |f| f.process_sample(x));
                power += w * w;
            }
            power /= ch as f32;
            self.rms_sq = flush(power + (self.rms_sq - power) * self.rms_coef);
            let rms_db = gain_to_db(self.rms_sq.max(0.0).sqrt());
            let want = self.desired_gain_db(rms_db);
            let coef = if want < self.gain_db { self.down_coef } else { self.up_coef };
            self.gain_db = flush(want + (self.gain_db - want) * coef);
            let mut g = db_to_gain(self.gain_db);
            if raw_peak * g > ceiling && raw_peak > 1.0e-9 {
                g = ceiling / raw_peak;
            }
            if self.gain_db < 0.0 {
                max_cut = max_cut.max(-self.gain_db);
            }
            for c in 0..ch {
                if let Some(s) = io.get_mut(c).and_then(|v| v.get_mut(i)) {
                    *s *= g;
                }
            }
        }
        self.gr_meter = max_cut;
    }

    fn gain_reduction_db(&self) -> f32 {
        self.gr_meter
    }
}

// ─── Stem Separation ────────────────────────────────────────────────────────────────────────────

static STEM_MODES: [&str; 4] = ["Stem Mix", "Vocals Solo", "Accompaniment Solo", "Ambience Solo"];

static STEM_PARAMS: [ParamInfo; 4] = [
    choice("mode", "Output", &STEM_MODES, 0),
    lin("vocals", "Vocals", -48.0, 12.0, 0.0, Unit::Db),
    lin("accompaniment", "Accompaniment", -48.0, 12.0, 0.0, Unit::Db),
    lin("ambience", "Ambience", -48.0, 12.0, 0.0, Unit::Db),
];

pub static STEM_INFO: PluginInfo = PluginInfo {
    id: "stem_separator",
    name: "Stem Separation",
    short_name: "Stems",
    category: Category::Other,
    params: &STEM_PARAMS,
    is_instrument: false,
    audiosuite: true,
};

/// Realtime Vocal / Accompaniment / Ambience separator.
///
/// Uses Mid/Side spatial coherence combined with vocal-formant band isolation and fast/slow
/// transient-vs-sustain envelope masks. Because `accompaniment = input - vocal - ambience`,
/// setting all three stem gains to `0 dB` in `Stem Mix` mode reconstructs the input losslessly.
pub struct StemSeparator {
    p: Params,
    sr: f32,
    channels: usize,
    mid_hp: Biquad,
    mid_lp: Biquad,
    side_hp: Biquad,
    fast_mid: f32,
    slow_mid: f32,
    fast_side: f32,
    slow_side: f32,
    fast_c: f32,
    slow_c: f32,
    tail_c: f32,
    g_voc: Smoother,
    g_acc: Smoother,
    g_amb: Smoother,
}

impl Default for StemSeparator {
    fn default() -> Self {
        Self::new()
    }
}

impl StemSeparator {
    pub fn new() -> Self {
        let mut s = Self {
            p: Params::new(&STEM_PARAMS),
            sr: 48_000.0,
            channels: 2,
            mid_hp: Biquad::default(),
            mid_lp: Biquad::default(),
            side_hp: Biquad::default(),
            fast_mid: 0.0,
            slow_mid: 0.0,
            fast_side: 0.0,
            slow_side: 0.0,
            fast_c: 0.0,
            slow_c: 0.0,
            tail_c: 0.0,
            g_voc: Smoother::new(1.0),
            g_acc: Smoother::new(1.0),
            g_amb: Smoother::new(1.0),
        };
        s.update();
        s
    }

    fn gain_or_mute(db: f32) -> f32 {
        if db <= -47.9 { 0.0 } else { db_to_gain(db) }
    }

    fn update(&mut self) {
        let q = std::f32::consts::FRAC_1_SQRT_2;
        self.mid_hp.set(FilterType::HighPass, 175.0, q, 0.0, self.sr);
        self.mid_lp.set(FilterType::LowPass, 5200.0, q, 0.0, self.sr);
        self.side_hp.set(FilterType::HighPass, 220.0, q, 0.0, self.sr);
        self.fast_c = time_coef(3.0, self.sr);
        self.slow_c = time_coef(45.0, self.sr);
        self.tail_c = time_coef(140.0, self.sr);
        self.g_voc.set_time(15.0, self.sr);
        self.g_acc.set_time(15.0, self.sr);
        self.g_amb.set_time(15.0, self.sr);

        let mode = self.p.choice(0);
        let gv = Self::gain_or_mute(self.p.v(1));
        let ga = Self::gain_or_mute(self.p.v(2));
        let gm = Self::gain_or_mute(self.p.v(3));
        let (tv, ta, tm) = match mode {
            1 => (gv, 0.0, 0.0),
            2 => (0.0, ga, 0.0),
            3 => (0.0, 0.0, gm),
            _ => (gv, ga, gm),
        };
        self.g_voc.set(tv);
        self.g_acc.set(ta);
        self.g_amb.set(tm);
    }

    #[inline]
    fn split_stereo(&mut self, l: f32, r: f32) -> ([f32; 2], [f32; 2], [f32; 2]) {
        let mid = 0.5 * (l + r);
        let side = 0.5 * (l - r);
        let v_band = self.mid_lp.process_sample(self.mid_hp.process_sample(mid));
        let s_band = self.side_hp.process_sample(side);

        let am = v_band.abs();
        let as_ = side.abs();
        self.fast_mid = flush(am + (self.fast_mid - am) * self.fast_c);
        self.slow_mid = flush(am + (self.slow_mid - am) * self.slow_c);
        self.fast_side = flush(as_ + (self.fast_side - as_) * self.fast_c);
        self.slow_side = flush(as_ + (self.slow_side - as_) * self.tail_c);

        // Center focus + sustain mask (suppresses sharp center drum transients from vocals).
        let center = self.fast_mid / (self.fast_mid + self.fast_side + 1.0e-7);
        let sustain = (self.slow_mid / (self.fast_mid + 1.0e-7)).clamp(0.0, 1.0);
        let voc_mask = (center * center * (0.35 + 0.65 * sustain)).clamp(0.0, 1.0);
        let voc = v_band * voc_mask;

        // Diffuse tail mask on high-passed side component for room ambience.
        let diffuse = (self.slow_side / (self.fast_side + 1.0e-7)).clamp(0.0, 1.0);
        let amb_s = s_band * (0.65 * diffuse);

        let voc_lr = [voc, voc];
        let amb_lr = [amb_s, -amb_s];
        let acc_lr = [l - voc_lr[0] - amb_lr[0], r - voc_lr[1] - amb_lr[1]];
        (voc_lr, acc_lr, amb_lr)
    }
}

impl Plugin for StemSeparator {
    param_plumbing!(STEM_INFO);

    fn prepare(&mut self, sample_rate: f32, _max_block: usize, channels: usize) {
        self.sr = sane_sr(sample_rate);
        self.channels = sane_channels(channels);
        self.reset();
        self.update();
        self.g_voc.snap();
        self.g_acc.snap();
        self.g_amb.snap();
    }

    fn reset(&mut self) {
        self.mid_hp.reset();
        self.mid_lp.reset();
        self.side_hp.reset();
        self.fast_mid = 0.0;
        self.slow_mid = 0.0;
        self.fast_side = 0.0;
        self.slow_side = 0.0;
    }

    fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
        let ch = self.channels.min(io.len());
        let n = frames_in(io, frames, ch);
        if ch == 0 || n == 0 {
            return;
        }
        if ch == 1 {
            for i in 0..n {
                let x = io.first().and_then(|v| v.get(i)).copied().unwrap_or(0.0);
                let x = if x.is_finite() { x } else { 0.0 };
                let (voc, acc, amb) = self.split_stereo(x, x);
                let gv = self.g_voc.next();
                let ga = self.g_acc.next();
                let gm = self.g_amb.next();
                if let Some(s) = io.first_mut().and_then(|v| v.get_mut(i)) {
                    *s = voc[0] * gv + acc[0] * ga + amb[0] * gm;
                }
            }
            return;
        }
        for i in 0..n {
            let l = io.first().and_then(|v| v.get(i)).copied().unwrap_or(0.0);
            let r = io.get(1).and_then(|v| v.get(i)).copied().unwrap_or(0.0);
            let l = if l.is_finite() { l } else { 0.0 };
            let r = if r.is_finite() { r } else { 0.0 };
            let (voc, acc, amb) = self.split_stereo(l, r);
            let gv = self.g_voc.next();
            let ga = self.g_acc.next();
            let gm = self.g_amb.next();
            if let Some(sl) = io.first_mut().and_then(|v| v.get_mut(i)) {
                *sl = voc[0] * gv + acc[0] * ga + amb[0] * gm;
            }
            if let Some(sr) = io.get_mut(1).and_then(|v| v.get_mut(i)) {
                *sr = voc[1] * gv + acc[1] * ga + amb[1] * gm;
            }
        }
    }
}
