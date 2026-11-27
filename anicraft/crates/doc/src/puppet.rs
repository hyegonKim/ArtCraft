//! Puppet rigging and procedural character animation behaviors:
//! - **Head-Turn 5-Pose Parallax Rig**: `Frontal`, `LeftQuarter`, `RightQuarter`, `Upward`, and
//!   `Downward` pose offsets with per-feature parallax weights (nose, eyes, brows, mouth, ears,
//!   front/back hair, contour).
//! - **Swap Set Triggers**: named trigger groups with key bindings, default choice, and `Latch` vs
//!   `Hold` keyframe stamping on Graphic symbol instances.
//! - **Procedural Character Behaviors**: `AutoBlink` (periodic blink interval + duration + jitter),
//!   `Breathe` (rhythmic chest/torso scale & lift around a pivot), and `WalkCycle` (hip, stride,
//!   foot arc, and counter-swinging arm procedural IK / transform keyframes).

use crate::{Easing, Element, ElementKind, Keyframe, Layer, LayerId, LoopMode, SymbolId, Timeline, Tween, TweenKind, evaluate_layer_elements};
use anicraft_geom::{Affine, Decomposed, Vec2, lerp};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Maximum puppet rigs stored in one document.
pub const MAX_PUPPET_RIGS: usize = 256;
/// Maximum swap sets per puppet rig.
pub const MAX_SWAP_SETS: usize = 128;
/// Maximum choices per swap set.
pub const MAX_SWAP_CHOICES: usize = 128;
/// Maximum feature bindings per head-turn rig.
pub const MAX_FEATURE_BINDINGS: usize = 256;

/// Canonical 5-pose head-turn rig views.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadPose {
    #[default]
    Frontal,
    LeftQuarter,
    RightQuarter,
    Upward,
    Downward,
}

impl HeadPose {
    pub const ALL: [HeadPose; 5] = [HeadPose::Frontal, HeadPose::LeftQuarter, HeadPose::RightQuarter, HeadPose::Upward, HeadPose::Downward];

    pub fn key(self) -> &'static str {
        match self {
            HeadPose::Frontal => "frontal",
            HeadPose::LeftQuarter => "left_quarter",
            HeadPose::RightQuarter => "right_quarter",
            HeadPose::Upward => "upward",
            HeadPose::Downward => "downward",
        }
    }

    pub fn from_key(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().replace(['-', ' '], "_").as_str() {
            "frontal" | "front" | "center" => Some(Self::Frontal),
            "left_quarter" | "left" | "leftquarter" => Some(Self::LeftQuarter),
            "right_quarter" | "right" | "rightquarter" => Some(Self::RightQuarter),
            "upward" | "up" => Some(Self::Upward),
            "downward" | "down" => Some(Self::Downward),
            _ => None,
        }
    }
}

/// Facial feature categories with calibrated 2.5D head-turn parallax weights.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FacialFeature {
    #[default]
    Nose,
    LeftEye,
    RightEye,
    Mouth,
    LeftBrow,
    RightBrow,
    LeftEar,
    RightEar,
    HairFront,
    HairBack,
    HeadContour,
}

impl FacialFeature {
    pub const ALL: [FacialFeature; 11] = [
        FacialFeature::Nose,
        FacialFeature::LeftEye,
        FacialFeature::RightEye,
        FacialFeature::Mouth,
        FacialFeature::LeftBrow,
        FacialFeature::RightBrow,
        FacialFeature::LeftEar,
        FacialFeature::RightEar,
        FacialFeature::HairFront,
        FacialFeature::HairBack,
        FacialFeature::HeadContour,
    ];

    /// Default parallax weight relative to the head turn offset (`1.0` = full nose lead,
    /// negative = counter-movement for ears/back-of-head).
    pub fn default_parallax_weight(self) -> f32 {
        match self {
            FacialFeature::Nose => 1.15,
            FacialFeature::HairFront => 1.0,
            FacialFeature::LeftBrow | FacialFeature::RightBrow => 0.90,
            FacialFeature::LeftEye | FacialFeature::RightEye => 0.85,
            FacialFeature::Mouth => 0.80,
            FacialFeature::HeadContour => 0.35,
            FacialFeature::LeftEar | FacialFeature::RightEar => -0.60,
            FacialFeature::HairBack => -0.45,
        }
    }

    pub fn from_key(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().replace(['-', ' '], "_").as_str() {
            "nose" => Some(Self::Nose),
            "left_eye" | "lefteye" => Some(Self::LeftEye),
            "right_eye" | "righteye" => Some(Self::RightEye),
            "mouth" => Some(Self::Mouth),
            "left_brow" | "leftbrow" | "left_eyebrow" => Some(Self::LeftBrow),
            "right_brow" | "rightbrow" | "right_eyebrow" => Some(Self::RightBrow),
            "left_ear" | "leftear" => Some(Self::LeftEar),
            "right_ear" | "rightear" => Some(Self::RightEar),
            "hair_front" | "hairfront" | "bangs" => Some(Self::HairFront),
            "hair_back" | "hairback" => Some(Self::HairBack),
            "head_contour" | "head" | "face" | "contour" => Some(Self::HeadContour),
            _ => None,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            FacialFeature::Nose => "nose",
            FacialFeature::LeftEye => "left_eye",
            FacialFeature::RightEye => "right_eye",
            FacialFeature::Mouth => "mouth",
            FacialFeature::LeftBrow => "left_brow",
            FacialFeature::RightBrow => "right_brow",
            FacialFeature::LeftEar => "left_ear",
            FacialFeature::RightEar => "right_ear",
            FacialFeature::HairFront => "hair_front",
            FacialFeature::HairBack => "hair_back",
            FacialFeature::HeadContour => "head_contour",
        }
    }
}

/// Transform delta for one head pose relative to the neutral frontal rig.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PoseOffset {
    pub offset: Vec2,
    pub scale: Vec2,
    pub rotation_deg: f32,
}

impl Default for PoseOffset {
    fn default() -> Self {
        Self { offset: Vec2::ZERO, scale: Vec2::new(1.0, 1.0), rotation_deg: 0.0 }
    }
}

impl PoseOffset {
    pub fn sanitized(self) -> Self {
        let clean = |v: f32, lo: f32, hi: f32, def: f32| {
            if v.is_finite() { v.clamp(lo, hi) } else { def }
        };
        Self {
            offset: Vec2::new(clean(self.offset.x, -2000.0, 2000.0, 0.0), clean(self.offset.y, -2000.0, 2000.0, 0.0)),
            scale: Vec2::new(clean(self.scale.x, 0.05, 10.0, 1.0), clean(self.scale.y, 0.05, 10.0, 1.0)),
            rotation_deg: clean(self.rotation_deg, -180.0, 180.0, 0.0),
        }
    }

    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            offset: self.offset.lerp(other.offset, t),
            scale: self.scale.lerp(other.scale, t),
            rotation_deg: lerp(self.rotation_deg, other.rotation_deg, t),
        }
    }
}

/// Associates a timeline layer (and optional symbol/element) with a facial feature and parallax weight.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FeatureParallax {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<LayerId>,
    pub feature: FacialFeature,
    pub weight: f32,
}

impl FeatureParallax {
    pub fn new(name: impl Into<String>, layer: Option<LayerId>, feature: FacialFeature) -> Self {
        Self { name: name.into(), layer, feature, weight: feature.default_parallax_weight() }
    }

    pub fn sanitized(&self) -> Self {
        let w = if self.weight.is_finite() { self.weight.clamp(-4.0, 4.0) } else { self.feature.default_parallax_weight() };
        Self { name: self.name.chars().take(128).collect(), layer: self.layer, feature: self.feature, weight: w }
    }
}

/// 5-Pose Head-Turn Parallax Rig (`Frontal`, `LeftQuarter`, `RightQuarter`, `Upward`, `Downward`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HeadTurnRig {
    pub frontal: PoseOffset,
    pub left_quarter: PoseOffset,
    pub right_quarter: PoseOffset,
    pub upward: PoseOffset,
    pub downward: PoseOffset,
    #[serde(default)]
    pub pivot: Vec2,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<FeatureParallax>,
}

impl Default for HeadTurnRig {
    fn default() -> Self {
        Self {
            frontal: PoseOffset::default(),
            left_quarter: PoseOffset { offset: Vec2::new(-28.0, 0.0), scale: Vec2::new(0.94, 1.0), rotation_deg: -4.0 },
            right_quarter: PoseOffset { offset: Vec2::new(28.0, 0.0), scale: Vec2::new(0.94, 1.0), rotation_deg: 4.0 },
            upward: PoseOffset { offset: Vec2::new(0.0, -20.0), scale: Vec2::new(1.0, 0.95), rotation_deg: 0.0 },
            downward: PoseOffset { offset: Vec2::new(0.0, 20.0), scale: Vec2::new(1.0, 0.95), rotation_deg: 0.0 },
            pivot: Vec2::ZERO,
            features: Vec::new(),
        }
    }
}

impl HeadTurnRig {
    pub fn pose(&self, which: HeadPose) -> PoseOffset {
        match which {
            HeadPose::Frontal => self.frontal,
            HeadPose::LeftQuarter => self.left_quarter,
            HeadPose::RightQuarter => self.right_quarter,
            HeadPose::Upward => self.upward,
            HeadPose::Downward => self.downward,
        }
    }

    pub fn set_pose(&mut self, which: HeadPose, offset: PoseOffset) {
        let clean = offset.sanitized();
        match which {
            HeadPose::Frontal => self.frontal = clean,
            HeadPose::LeftQuarter => self.left_quarter = clean,
            HeadPose::RightQuarter => self.right_quarter = clean,
            HeadPose::Upward => self.upward = clean,
            HeadPose::Downward => self.downward = clean,
        }
    }

    /// Evaluates blended head-turn pose for continuous `yaw` (`-1` left .. `+1` right) and
    /// `pitch` (`-1` downward .. `+1` upward).
    pub fn evaluate_pose(&self, yaw: f32, pitch: f32) -> PoseOffset {
        let y = if yaw.is_finite() { yaw.clamp(-1.0, 1.0) } else { 0.0 };
        let p = if pitch.is_finite() { pitch.clamp(-1.0, 1.0) } else { 0.0 };

        let horiz = if y < 0.0 { self.frontal.lerp(self.left_quarter, -y) } else { self.frontal.lerp(self.right_quarter, y) };
        let vert = if p < 0.0 { self.frontal.lerp(self.downward, -p) } else { self.frontal.lerp(self.upward, p) };

        // Combine horizontal and vertical deltas relative to the frontal rest pose.
        let base = self.frontal;
        PoseOffset {
            offset: base.offset + (horiz.offset - base.offset) + (vert.offset - base.offset),
            scale: Vec2::new(
                (base.scale.x * (horiz.scale.x / base.scale.x.max(1e-3)) * (vert.scale.x / base.scale.x.max(1e-3))).clamp(0.05, 10.0),
                (base.scale.y * (horiz.scale.y / base.scale.y.max(1e-3)) * (vert.scale.y / base.scale.y.max(1e-3))).clamp(0.05, 10.0),
            ),
            rotation_deg: base.rotation_deg + (horiz.rotation_deg - base.rotation_deg) + (vert.rotation_deg - base.rotation_deg),
        }
        .sanitized()
    }

    /// Computes the delta `Affine` transform for a specific facial feature at `(yaw, pitch)`.
    pub fn feature_delta(&self, weight: f32, feature: FacialFeature, yaw: f32, pitch: f32) -> Affine {
        let pose = self.evaluate_pose(yaw, pitch);
        let w = if weight.is_finite() { weight.clamp(-4.0, 4.0) } else { 1.0 };
        let shift = pose.offset * w;
        // Foreshortening on lateral features (eyes/ears compress slightly on the turned-toward side).
        let side_squash = match feature {
            FacialFeature::LeftEye | FacialFeature::LeftBrow | FacialFeature::LeftEar if yaw < 0.0 => 1.0 - 0.12 * (-yaw).clamp(0.0, 1.0),
            FacialFeature::RightEye | FacialFeature::RightBrow | FacialFeature::RightEar if yaw > 0.0 => 1.0 - 0.12 * yaw.clamp(0.0, 1.0),
            _ => 1.0,
        };
        let sx = lerp(1.0, pose.scale.x, w.abs().min(1.0)) * side_squash;
        let sy = lerp(1.0, pose.scale.y, w.abs().min(1.0));
        let rot = (pose.rotation_deg * w.clamp(-1.5, 1.5)).to_radians();
        let pv = if self.pivot.is_finite() { self.pivot } else { Vec2::ZERO };

        Affine::translate(pv.x + shift.x, pv.y + shift.y).mul(&Affine::rotate(rot)).mul(&Affine::scale(sx, sy)).mul(&Affine::translate(-pv.x, -pv.y))
    }

    /// Applies the evaluated `(yaw, pitch)` head-turn pose onto bound feature layers at `frame`.
    /// Returns the number of layers updated.
    pub fn apply_at_frame(&self, tl: &mut Timeline, frame: u32, yaw: f32, pitch: f32) -> usize {
        let mut updated = 0usize;
        for f in &self.features {
            let Some(lid) = f.layer else { continue };
            let delta = self.feature_delta(f.weight, f.feature, yaw, pitch);
            let Some(layer) = tl.layer_mut(lid) else { continue };
            let base_els = layer.keyframes.first().map(|k| k.elements.clone()).unwrap_or_default();
            if base_els.is_empty() {
                continue;
            }
            let kf_idx = ensure_keyframe_at(layer, frame);
            if let Some(kf) = layer.keyframe_mut(kf_idx) {
                kf.elements = base_els
                    .iter()
                    .map(|el| {
                        let mut c = (**el).clone();
                        c.transform = delta.mul(&el.transform);
                        Arc::new(c)
                    })
                    .collect();
                updated += 1;
            }
            layer.normalize();
        }
        updated
    }

    pub fn sanitized(&self) -> Self {
        let pv = if self.pivot.is_finite() { self.pivot } else { Vec2::ZERO };
        Self {
            frontal: self.frontal.sanitized(),
            left_quarter: self.left_quarter.sanitized(),
            right_quarter: self.right_quarter.sanitized(),
            upward: self.upward.sanitized(),
            downward: self.downward.sanitized(),
            pivot: pv,
            features: self.features.iter().take(MAX_FEATURE_BINDINGS).map(FeatureParallax::sanitized).collect(),
        }
    }
}

/// Swap-set trigger activation behavior (`Hold` while pressed vs `Latch` toggle).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerMode {
    /// Active only for the event's duration, reverting automatically to `default_choice`.
    #[default]
    Hold,
    /// Stays latched on the triggered choice until another choice or a second press toggles it back.
    Latch,
}

impl TriggerMode {
    pub fn key(self) -> &'static str {
        match self {
            TriggerMode::Hold => "hold",
            TriggerMode::Latch => "latch",
        }
    }

    pub fn from_key(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "hold" => Some(Self::Hold),
            "latch" | "toggle" => Some(Self::Latch),
            _ => None,
        }
    }
}

/// One choice inside a [`SwapSet`] (e.g., hand pose `"Point"`, key `"1"`, frame `2`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwapChoice {
    pub name: String,
    pub key: String,
    /// 0-based Graphic symbol `first_frame` stamped when this choice is active.
    #[serde(default)]
    pub frame: u32,
    /// Optional replacement Library symbol ID when choices swap separate symbols.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<SymbolId>,
}

impl SwapChoice {
    pub fn sanitized(&self) -> Self {
        Self {
            name: self.name.chars().take(128).collect(),
            key: self.key.trim().chars().take(16).collect(),
            frame: self.frame.min(100_000),
            symbol: self.symbol,
        }
    }
}

/// Named mutually-exclusive swap trigger group (e.g., `"Right Hand"`, `"Brows"`, `"Props"`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwapSet {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<LayerId>,
    #[serde(default)]
    pub mode: TriggerMode,
    #[serde(default)]
    pub default_choice: usize,
    #[serde(default)]
    pub choices: Vec<SwapChoice>,
}

/// Input trigger press event on the timeline.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggerEvent {
    pub frame: u32,
    #[serde(default = "one_u32")]
    pub duration: u32,
    /// Matches either `SwapChoice::key` (case-insensitive) or `SwapChoice::name`.
    pub trigger: String,
}

fn one_u32() -> u32 {
    1
}

impl SwapSet {
    pub fn sanitized(&self) -> Self {
        let choices: Vec<SwapChoice> = self.choices.iter().take(MAX_SWAP_CHOICES).map(SwapChoice::sanitized).collect();
        let default_choice = if choices.is_empty() { 0 } else { self.default_choice.min(choices.len() - 1) };
        Self { name: self.name.chars().take(128).collect(), layer: self.layer, mode: self.mode, default_choice, choices }
    }

    pub fn resolve_choice(&self, trigger: &str) -> Option<usize> {
        let q = trigger.trim();
        if q.is_empty() {
            return None;
        }
        self.choices.iter().position(|c| c.key.eq_ignore_ascii_case(q) || c.name.eq_ignore_ascii_case(q))
    }

    /// Evaluates choice index per frame across `0..total_frames` under `Hold` or `Latch` semantics,
    /// then stamps `SingleFrame` symbol keyframes onto `layer`. Returns stamped keyframe count.
    pub fn stamp_triggers(&self, layer: &mut Layer, events: &[TriggerEvent], total_frames: u32) -> usize {
        if self.choices.is_empty() || total_frames == 0 {
            return 0;
        }
        let n = total_frames.min(100_000) as usize;
        let def = self.default_choice.min(self.choices.len() - 1);
        let mut active = vec![def; n];

        let mut sorted: Vec<&TriggerEvent> = events.iter().collect();
        sorted.sort_by_key(|e| e.frame);

        match self.mode {
            TriggerMode::Hold => {
                for ev in sorted {
                    let Some(choice_idx) = self.resolve_choice(&ev.trigger) else { continue };
                    let start = (ev.frame as usize).min(n);
                    let end = (ev.frame.saturating_add(ev.duration.max(1)) as usize).min(n);
                    for slot in active.iter_mut().take(end).skip(start) {
                        *slot = choice_idx;
                    }
                }
            }
            TriggerMode::Latch => {
                let mut current = def;
                let mut ev_idx = 0usize;
                for (f, slot) in active.iter_mut().enumerate() {
                    while let Some(ev) = sorted.get(ev_idx) {
                        if (ev.frame as usize) > f {
                            break;
                        }
                        if (ev.frame as usize) == f
                            && let Some(choice_idx) = self.resolve_choice(&ev.trigger)
                        {
                            // Pressing an already-latched trigger unlatches back to default.
                            current = if current == choice_idx && choice_idx != def { def } else { choice_idx };
                        }
                        ev_idx += 1;
                    }
                    *slot = current;
                }
            }
        }

        // Compress runs of identical choices into clean keyframe spans.
        let template_elements = layer.keyframes.first().map(|k| k.elements.clone()).unwrap_or_default();
        if template_elements.is_empty() {
            return 0;
        }

        let mut runs: Vec<(u32, u32, usize)> = Vec::new();
        for (f, &choice_idx) in active.iter().enumerate() {
            let f = f as u32;
            match runs.last_mut() {
                Some((_, dur, prev)) if *prev == choice_idx => *dur += 1,
                _ => runs.push((f, 1, choice_idx)),
            }
        }

        let mut keyframes = Vec::with_capacity(runs.len());
        for (start, duration, choice_idx) in runs {
            let Some(choice) = self.choices.get(choice_idx) else { continue };
            let mut kf = Keyframe::new(start, duration);
            kf.label = choice.name.clone();
            kf.elements = template_elements
                .iter()
                .map(|el| {
                    let mut c = (**el).clone();
                    if let ElementKind::Symbol { symbol, loop_mode, first_frame, .. } = &mut c.kind {
                        if let Some(sym_override) = choice.symbol {
                            *symbol = sym_override;
                        }
                        *loop_mode = LoopMode::SingleFrame;
                        *first_frame = choice.frame;
                    }
                    Arc::new(c)
                })
                .collect();
            keyframes.push(Arc::new(kf));
        }

        let count = keyframes.len();
        if !keyframes.is_empty() {
            layer.keyframes = keyframes;
            layer.normalize();
        }
        count
    }
}

/// Procedural `AutoBlink` behavior parameters.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutoBlinkConfig {
    /// Mean interval between blinks in frames (e.g., `72` = 3 s at 24 fps).
    pub interval_frames: u32,
    /// Closed-eye duration in frames (e.g., `3` frames).
    pub duration_frames: u32,
    /// Relative interval randomness in `0..=0.8`.
    pub jitter: f32,
    /// Open-eye Graphic symbol frame index (default `0`).
    pub open_frame: u32,
    /// Closed-eye Graphic symbol frame index (default `1`).
    pub closed_frame: u32,
    /// Deterministic PRNG seed.
    pub seed: u64,
}

impl Default for AutoBlinkConfig {
    fn default() -> Self {
        Self { interval_frames: 72, duration_frames: 3, jitter: 0.25, open_frame: 0, closed_frame: 1, seed: 0xA11C_EB11 }
    }
}

fn splitmix64(state: &mut u64) -> f32 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    // Uniform in -1.0..=1.0
    ((z >> 40) as f32 / ((1u64 << 24) as f32)) * 2.0 - 1.0
}

/// Stamps periodic blink keyframes across `start_frame..end_frame` on `layer`.
/// Works on both Graphic symbol instances (switching `SingleFrame` between `open_frame` and
/// `closed_frame`) and plain vector/group artwork (squashing vertical eye scale on blink frames).
pub fn apply_auto_blink(layer: &mut Layer, cfg: &AutoBlinkConfig, start_frame: u32, end_frame: u32) -> usize {
    if end_frame <= start_frame {
        return 0;
    }
    let base_els = evaluate_layer_elements(layer, start_frame)
        .map(|(_, e)| e.into_owned())
        .or_else(|| layer.keyframes.first().map(|k| k.elements.clone()))
        .unwrap_or_default();
    if base_els.is_empty() {
        return 0;
    }

    let interval = cfg.interval_frames.clamp(4, 10_000);
    let dur = cfg.duration_frames.clamp(1, interval.saturating_sub(1).max(1));
    let jitter = if cfg.jitter.is_finite() { cfg.jitter.clamp(0.0, 0.8) } else { 0.0 };
    let mut rng = cfg.seed ^ layer.id.0;

    // Compute blink start frames within [start_frame, end_frame).
    let mut blinks: Vec<(u32, u32)> = Vec::new();
    let mut cursor = start_frame.saturating_add((interval / 2).max(2));
    while cursor < end_frame {
        let delta = (interval as f32 * jitter * splitmix64(&mut rng)).round() as i64;
        let at = (i64::from(cursor) + delta).clamp(i64::from(start_frame) + 1, i64::from(end_frame) - 1) as u32;
        let b_end = at.saturating_add(dur).min(end_frame);
        if blinks.last().is_none_or(|&(_, prev_end)| at > prev_end) {
            blinks.push((at, b_end));
        }
        cursor = cursor.saturating_add(interval);
    }
    // Guarantee at least one blink on short spans if the span has >= 4 frames.
    if blinks.is_empty() && end_frame.saturating_sub(start_frame) >= 4 {
        let mid = start_frame + (end_frame - start_frame) / 2;
        blinks.push((mid, (mid + dur).min(end_frame)));
    }

    let make_state = |closed: bool| -> Vec<Arc<Element>> {
        base_els
            .iter()
            .map(|el| {
                let mut c = (**el).clone();
                match &mut c.kind {
                    ElementKind::Symbol { loop_mode, first_frame, .. } => {
                        *loop_mode = LoopMode::SingleFrame;
                        *first_frame = if closed { cfg.closed_frame } else { cfg.open_frame };
                    }
                    _ if closed => {
                        let center = el.transform.translation();
                        c.transform = Affine::translate(center.x, center.y)
                            .mul(&Affine::scale(1.04, 0.12))
                            .mul(&Affine::translate(-center.x, -center.y))
                            .mul(&el.transform);
                    }
                    _ => {}
                }
                Arc::new(c)
            })
            .collect()
    };

    let mut keyframes: Vec<Arc<Keyframe>> = layer.keyframes.iter().filter(|k| k.end() <= start_frame || k.start >= end_frame).cloned().collect();

    let mut pos = start_frame;
    let mut stamped = 0usize;
    for (b_start, b_end) in blinks {
        if b_start > pos {
            let mut open_kf = Keyframe::new(pos, b_start - pos);
            open_kf.elements = make_state(false);
            keyframes.push(Arc::new(open_kf));
            stamped += 1;
        }
        let mut closed_kf = Keyframe::new(b_start, (b_end - b_start).max(1));
        closed_kf.label = "blink".into();
        closed_kf.elements = make_state(true);
        keyframes.push(Arc::new(closed_kf));
        stamped += 1;
        pos = b_end;
    }
    if pos < end_frame {
        let mut tail_kf = Keyframe::new(pos, end_frame - pos);
        tail_kf.elements = make_state(false);
        keyframes.push(Arc::new(tail_kf));
        stamped += 1;
    }

    layer.keyframes = keyframes;
    layer.normalize();
    stamped
}

/// Procedural `Breathe` chest/torso scale & pivot behavior parameters.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BreatheConfig {
    /// Full inhale-exhale period in frames (e.g., `72` = 3 s at 24 fps, 20 breaths/min).
    pub period_frames: u32,
    /// Peak horizontal chest expansion factor (e.g., `1.03` = +3 %).
    pub scale_x: f32,
    /// Peak vertical torso expansion factor (e.g., `1.05` = +5 %).
    pub scale_y: f32,
    /// Peak upward chest lift in pixels at full inhale.
    pub lift_px: f32,
    /// Origin pivot in stage/layer space (typically waist/hip base so torso expands upward).
    pub pivot: Option<Vec2>,
    /// Phase offset in degrees.
    pub phase_deg: f32,
}

impl Default for BreatheConfig {
    fn default() -> Self {
        Self { period_frames: 72, scale_x: 1.03, scale_y: 1.05, lift_px: 4.0, pivot: None, phase_deg: 0.0 }
    }
}

/// Generates smooth classic-tweened breathing keyframes around `pivot` across `start_frame..=end_frame`.
pub fn apply_breathe(layer: &mut Layer, cfg: &BreatheConfig, start_frame: u32, end_frame: u32) -> usize {
    if end_frame <= start_frame {
        return 0;
    }
    let base_els = evaluate_layer_elements(layer, start_frame)
        .map(|(_, e)| e.into_owned())
        .or_else(|| layer.keyframes.first().map(|k| k.elements.clone()))
        .unwrap_or_default();
    if base_els.is_empty() {
        return 0;
    }

    let period = cfg.period_frames.clamp(8, 10_000);
    let sx_amp = if cfg.scale_x.is_finite() { cfg.scale_x.clamp(0.5, 2.0) - 1.0 } else { 0.03 };
    let sy_amp = if cfg.scale_y.is_finite() { cfg.scale_y.clamp(0.5, 2.0) - 1.0 } else { 0.05 };
    let lift = if cfg.lift_px.is_finite() { cfg.lift_px.clamp(-200.0, 200.0) } else { 4.0 };
    let phase0 = if cfg.phase_deg.is_finite() { cfg.phase_deg.to_radians() } else { 0.0 };

    let default_pivot = base_els.first().map_or(Vec2::ZERO, |e| e.transform.translation());
    let pivot = cfg.pivot.filter(|p| p.is_finite()).unwrap_or(default_pivot);

    // Place 4 quarter-cycle keyframes per breath period for clean sinusoidal classic tweens.
    let step = (period / 4).max(2);
    let mut key_times: Vec<u32> = Vec::new();
    let mut f = start_frame;
    while f < end_frame {
        key_times.push(f);
        f = f.saturating_add(step);
    }
    key_times.push(end_frame);

    let mut keyframes: Vec<Arc<Keyframe>> = layer.keyframes.iter().filter(|k| k.end() <= start_frame || k.start > end_frame).cloned().collect();

    let count = key_times.len();
    for (idx, &at) in key_times.iter().enumerate() {
        let next_at = key_times.get(idx + 1).copied();
        let duration = next_at.map_or(1, |nx| nx.saturating_sub(at).max(1));
        let angle = phase0 + ((at.saturating_sub(start_frame)) as f32 / period as f32) * std::f32::consts::TAU;
        // 0..1 smooth breath wave (0 at rest, 1 at peak inhale)
        let breath = 0.5 * (1.0 - angle.cos());
        let sx = 1.0 + sx_amp * breath;
        let sy = 1.0 + sy_amp * breath;
        let dy = -lift * breath;

        let breath_xf = Affine::translate(pivot.x, pivot.y + dy).mul(&Affine::scale(sx, sy)).mul(&Affine::translate(-pivot.x, -pivot.y));

        let mut kf = Keyframe::new(at, duration);
        if next_at.is_some() {
            kf.tween = Some(Tween::new(Easing::Classic(0.0), TweenKind::Classic));
        }
        kf.elements = base_els
            .iter()
            .map(|el| {
                let mut c = (**el).clone();
                c.transform = breath_xf.mul(&el.transform);
                Arc::new(c)
            })
            .collect();
        keyframes.push(Arc::new(kf));
    }

    layer.keyframes = keyframes;
    layer.normalize();
    count
}

/// Procedural `WalkCycle` locomotion parameters.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WalkCycleConfig {
    /// Frames per full 2-step stride cycle (e.g., `24` frames = 1 s at 24 fps).
    pub cycle_frames: u32,
    /// Horizontal foot travel amplitude in pixels.
    pub stride_length: f32,
    /// Peak foot lift height during swing phase in pixels.
    pub step_height: f32,
    /// Vertical pelvis/body bounce amplitude in pixels (oscillates at `2×` stride frequency).
    pub hip_bob: f32,
    /// Counter-phase shoulder/arm swing amplitude in degrees.
    pub arm_swing_deg: f32,
    /// Forward lean angle of torso in degrees.
    pub torso_lean_deg: f32,
    /// `1.0` walks right (`+X`), `-1.0` walks left (`-X`), `0.0` walks in place.
    pub root_speed_per_cycle: f32,
}

impl Default for WalkCycleConfig {
    fn default() -> Self {
        Self {
            cycle_frames: 24,
            stride_length: 48.0,
            step_height: 22.0,
            hip_bob: 6.0,
            arm_swing_deg: 28.0,
            torso_lean_deg: 3.5,
            root_speed_per_cycle: 0.0,
        }
    }
}

/// Optional multi-layer character rig targets for [`apply_walk_cycle`].
#[derive(Clone, Copy, Debug, Default)]
pub struct WalkRigLayers {
    pub hip: Option<LayerId>,
    pub left_leg: Option<LayerId>,
    pub right_leg: Option<LayerId>,
    pub left_arm: Option<LayerId>,
    pub right_arm: Option<LayerId>,
}

/// Analytical 2-bone limb inverse kinematics solver returning `(hip_or_shoulder_rad, knee_or_elbow_rad)`.
pub fn solve_two_bone_limb_ik(root: Vec2, target: Vec2, len_upper: f32, len_lower: f32, bend_sign: f32) -> (f32, f32) {
    let l1 = len_upper.abs().max(1e-3);
    let l2 = len_lower.abs().max(1e-3);
    let diff = target - root;
    let dist = diff.length().clamp((l1 - l2).abs() + 1e-3, (l1 + l2) - 1e-3);
    let base_angle = diff.y.atan2(diff.x);

    let cos_upper = ((l1 * l1 + dist * dist - l2 * l2) / (2.0 * l1 * dist)).clamp(-1.0, 1.0);
    let cos_knee = ((l1 * l1 + l2 * l2 - dist * dist) / (2.0 * l1 * l2)).clamp(-1.0, 1.0);
    let sign = if bend_sign >= 0.0 { 1.0 } else { -1.0 };

    let upper_angle = base_angle - sign * cos_upper.acos();
    let knee_angle = sign * (std::f32::consts::PI - cos_knee.acos());
    (upper_angle, knee_angle)
}

/// Evaluates foot offset `(dx, dy)` and leg swing angle (radians) at normalized cycle phase `u` (`0..1`).
pub fn evaluate_foot_stride(u: f32, stride_length: f32, step_height: f32) -> (Vec2, f32) {
    let phase = u.rem_euclid(1.0);
    let half_stride = stride_length * 0.5;
    if phase < 0.5 {
        // Stance phase (0..0.5): foot plants forward (+half_stride) and slides back (-half_stride) on ground.
        let s = phase * 2.0;
        let dx = lerp(half_stride, -half_stride, s);
        let leg_rot = (-dx / stride_length.abs().max(40.0)) * 0.45;
        (Vec2::new(dx, 0.0), leg_rot)
    } else {
        // Swing phase (0.5..1.0): foot lifts in a parabolic/sinusoidal arc from back to front.
        let s = (phase - 0.5) * 2.0;
        let dx = lerp(-half_stride, half_stride, s);
        let dy = -step_height * (s * std::f32::consts::PI).sin();
        let leg_rot = (-dx / stride_length.abs().max(40.0)) * 0.45;
        (Vec2::new(dx, dy), leg_rot)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WalkPart {
    Body,
    LeftLeg,
    RightLeg,
    LeftArm,
    RightArm,
}

fn stamp_walk_part(layer: &mut Layer, part: WalkPart, cfg: &WalkCycleConfig, start_frame: u32, end_frame: u32) -> usize {
    let base_els = evaluate_layer_elements(layer, start_frame)
        .map(|(_, e)| e.into_owned())
        .or_else(|| layer.keyframes.first().map(|k| k.elements.clone()))
        .unwrap_or_default();
    if base_els.is_empty() || end_frame <= start_frame {
        return 0;
    }

    let cycle = cfg.cycle_frames.clamp(4, 10_000);
    // 8 pose keys per stride cycle (contact, down, passing, up for both left & right steps).
    let step = (cycle / 8).max(1);
    let mut key_times = Vec::new();
    let mut f = start_frame;
    while f < end_frame {
        key_times.push(f);
        f = f.saturating_add(step);
    }
    key_times.push(end_frame);

    let mut keyframes: Vec<Arc<Keyframe>> = layer.keyframes.iter().filter(|k| k.end() <= start_frame || k.start > end_frame).cloned().collect();

    let count = key_times.len();
    for (idx, &at) in key_times.iter().enumerate() {
        let next_at = key_times.get(idx + 1).copied();
        let duration = next_at.map_or(1, |nx| nx.saturating_sub(at).max(1));
        let cycles_elapsed = (at.saturating_sub(start_frame)) as f32 / cycle as f32;
        let u = cycles_elapsed.rem_euclid(1.0);
        let root_x = cycles_elapsed * cfg.root_speed_per_cycle;
        // Hip bobs twice per full 2-step cycle (lowest at contact+down, highest at passing).
        let bob = -cfg.hip_bob * (u * std::f32::consts::TAU * 2.0).cos();

        let (offset, rot_rad) = match part {
            WalkPart::Body => {
                let sway = (u * std::f32::consts::TAU).sin() * 1.8;
                (Vec2::new(root_x, bob), (cfg.torso_lean_deg + sway).to_radians())
            }
            WalkPart::LeftLeg => {
                let (foot, ang) = evaluate_foot_stride(u, cfg.stride_length, cfg.step_height);
                // Refine swing angle via 2-bone analytical limb IK from virtual hip above foot.
                let limb_len = cfg.stride_length.abs().max(40.0);
                let (ik_upper, ik_knee) =
                    solve_two_bone_limb_ik(Vec2::ZERO, Vec2::new(foot.x, limb_len * 1.4 + foot.y), limb_len * 0.8, limb_len * 0.8, 1.0);
                let blended_rot = (ang * 0.65) + ((ik_upper - std::f32::consts::FRAC_PI_2 + ik_knee * 0.25) * 0.35);
                (Vec2::new(root_x + foot.x, bob * 0.35 + foot.y), blended_rot)
            }
            WalkPart::RightLeg => {
                let u_opp = (u + 0.5).rem_euclid(1.0);
                let (foot, ang) = evaluate_foot_stride(u_opp, cfg.stride_length, cfg.step_height);
                let limb_len = cfg.stride_length.abs().max(40.0);
                let (ik_upper, ik_knee) =
                    solve_two_bone_limb_ik(Vec2::ZERO, Vec2::new(foot.x, limb_len * 1.4 + foot.y), limb_len * 0.8, limb_len * 0.8, 1.0);
                let blended_rot = (ang * 0.65) + ((ik_upper - std::f32::consts::FRAC_PI_2 + ik_knee * 0.25) * 0.35);
                (Vec2::new(root_x + foot.x, bob * 0.35 + foot.y), blended_rot)
            }
            WalkPart::LeftArm => {
                // Left arm swings forward when right leg steps forward (counter-phase to left leg).
                let swing = -(u * std::f32::consts::TAU).cos() * cfg.arm_swing_deg;
                (Vec2::new(root_x, bob * 0.8), swing.to_radians())
            }
            WalkPart::RightArm => {
                let swing = (u * std::f32::consts::TAU).cos() * cfg.arm_swing_deg;
                (Vec2::new(root_x, bob * 0.8), swing.to_radians())
            }
        };

        let mut kf = Keyframe::new(at, duration);
        if next_at.is_some() {
            kf.tween = Some(Tween::new(Easing::Linear, TweenKind::Classic));
        }
        kf.elements = base_els
            .iter()
            .map(|el| {
                let mut c = (**el).clone();
                let mut d: Decomposed = el.transform.decompose();
                d.tx += offset.x;
                d.ty += offset.y;
                d.rotation += rot_rad;
                c.transform = d.compose();
                Arc::new(c)
            })
            .collect();
        keyframes.push(Arc::new(kf));
    }

    layer.keyframes = keyframes;
    layer.normalize();
    count
}

/// Applies a procedural walk cycle across `start_frame..=end_frame` to a multi-layer character rig
/// (or a single body layer if limb layers are omitted). Returns `(layers_updated, keyframes_stamped)`.
pub fn apply_walk_cycle(
    tl: &mut Timeline,
    rig: &WalkRigLayers,
    fallback_layer: LayerId,
    cfg: &WalkCycleConfig,
    start_frame: u32,
    end_frame: u32,
) -> (usize, usize) {
    let mut parts: Vec<(LayerId, WalkPart)> = Vec::new();
    if let Some(id) = rig.hip {
        parts.push((id, WalkPart::Body));
    }
    if let Some(id) = rig.left_leg {
        parts.push((id, WalkPart::LeftLeg));
    }
    if let Some(id) = rig.right_leg {
        parts.push((id, WalkPart::RightLeg));
    }
    if let Some(id) = rig.left_arm {
        parts.push((id, WalkPart::LeftArm));
    }
    if let Some(id) = rig.right_arm {
        parts.push((id, WalkPart::RightArm));
    }
    if parts.is_empty() {
        // Auto-detect named limb layers in the timeline if present, else animate `fallback_layer`.
        for l in &tl.layers {
            let low = l.name.to_ascii_lowercase();
            if low.contains("left") && (low.contains("leg") || low.contains("foot")) {
                parts.push((l.id, WalkPart::LeftLeg));
            } else if low.contains("right") && (low.contains("leg") || low.contains("foot")) {
                parts.push((l.id, WalkPart::RightLeg));
            } else if low.contains("left") && (low.contains("arm") || low.contains("hand")) {
                parts.push((l.id, WalkPart::LeftArm));
            } else if low.contains("right") && (low.contains("arm") || low.contains("hand")) {
                parts.push((l.id, WalkPart::RightArm));
            } else if low.contains("torso") || low.contains("body") || low.contains("hip") {
                parts.push((l.id, WalkPart::Body));
            }
        }
        if parts.is_empty() {
            parts.push((fallback_layer, WalkPart::Body));
        }
    }

    let mut layers_updated = 0usize;
    let mut total_kfs = 0usize;
    for (lid, part) in parts {
        if let Some(layer) = tl.layer_mut(lid) {
            let n = stamp_walk_part(layer, part, cfg, start_frame, end_frame);
            if n > 0 {
                layers_updated += 1;
                total_kfs += n;
            }
        }
    }
    (layers_updated, total_kfs)
}

/// Persistent Puppet Rig definition stored in `Document::puppets`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PuppetRig {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub head_turn: HeadTurnRig,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub swap_sets: Vec<SwapSet>,
}

impl PuppetRig {
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        Self { id, name: name.into(), head_turn: HeadTurnRig::default(), swap_sets: Vec::new() }
    }

    pub fn sanitized(&self) -> Self {
        let name: String = self.name.trim().chars().take(128).collect();
        Self {
            id: self.id.max(1),
            name: if name.is_empty() { "Puppet".into() } else { name },
            head_turn: self.head_turn.sanitized(),
            swap_sets: self.swap_sets.iter().take(MAX_SWAP_SETS).map(SwapSet::sanitized).collect(),
        }
    }
}

fn ensure_keyframe_at(layer: &mut Layer, frame: u32) -> usize {
    if let Some(i) = layer.keyframe_starting_at(frame) {
        return i;
    }
    if let Some(i) = layer.keyframe_index_at(frame) {
        let orig = layer.keyframes.get(i).map(|k| (**k).clone());
        if let Some(mut right) = orig {
            let old_end = right.end();
            if let Some(left) = layer.keyframe_mut(i) {
                left.duration = (frame - left.start).max(1);
            }
            right.start = frame;
            right.duration = old_end.saturating_sub(frame).max(1);
            layer.keyframes.insert(i + 1, Arc::new(right));
            return i + 1;
        }
    }
    if let Some(last) = layer.keyframes.last_mut()
        && last.end() < frame
    {
        Arc::make_mut(last).duration = frame - last.start;
    }
    let mut fresh = Keyframe::new(frame, 1);
    if let Some(prev) = layer.keyframes.last() {
        fresh.elements = prev.elements.clone();
    }
    layer.keyframes.push(Arc::new(fresh));
    layer.keyframes.len() - 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ColorEffect, ElementId, Paint, Rgba, Shape};
    use anicraft_geom::Path;

    fn symbol_layer(id: u64, sym: SymbolId, frames: u32) -> Layer {
        let mut l = Layer::new(LayerId(id), "Hand");
        let mut kf = Keyframe::new(0, frames);
        kf.elements.push(Arc::new(Element {
            id: ElementId(100 + id),
            name: String::new(),
            transform: Affine::translate(200.0, 300.0),
            color: ColorEffect::default(),
            kind: ElementKind::Symbol {
                symbol: sym,
                loop_mode: LoopMode::SingleFrame,
                first_frame: 0,
                blend: Default::default(),
                filters: Vec::new(),
                events: Vec::new(),
            },
        }));
        l.keyframes = vec![Arc::new(kf)];
        l
    }

    #[test]
    fn head_turn_5_pose_parallax_moves_nose_opposite_ears() {
        let mut rig = HeadTurnRig::default();
        rig.features.push(FeatureParallax::new("Nose", Some(LayerId(1)), FacialFeature::Nose));
        rig.features.push(FeatureParallax::new("LeftEar", Some(LayerId(2)), FacialFeature::LeftEar));

        let nose_xf = rig.feature_delta(FacialFeature::Nose.default_parallax_weight(), FacialFeature::Nose, 1.0, 0.0);
        let ear_xf = rig.feature_delta(FacialFeature::LeftEar.default_parallax_weight(), FacialFeature::LeftEar, 1.0, 0.0);
        // Turning right (+1.0): nose shifts +X, ear counter-shifts -X.
        assert!(nose_xf.translation().x > 25.0);
        assert!(ear_xf.translation().x < -10.0);

        let up_pose = rig.evaluate_pose(0.0, 1.0);
        let down_pose = rig.evaluate_pose(0.0, -1.0);
        assert!(up_pose.offset.y < 0.0 && down_pose.offset.y > 0.0);
    }

    #[test]
    fn swap_set_hold_and_latch_stamp_symbol_keyframes() {
        let swap_hold = SwapSet {
            name: "Hand".into(),
            layer: Some(LayerId(1)),
            mode: TriggerMode::Hold,
            default_choice: 0,
            choices: vec![
                SwapChoice { name: "Relaxed".into(), key: "1".into(), frame: 0, symbol: None },
                SwapChoice { name: "Point".into(), key: "2".into(), frame: 3, symbol: None },
                SwapChoice { name: "Wave".into(), key: "3".into(), frame: 5, symbol: None },
            ],
        };
        let mut l_hold = symbol_layer(1, SymbolId(10), 20);
        let events = vec![TriggerEvent { frame: 4, duration: 5, trigger: "2".into() }];
        let n = swap_hold.stamp_triggers(&mut l_hold, &events, 20);
        assert_eq!(n, 3, "0..4 Relaxed, 4..9 Point, 9..20 Relaxed");
        let frame_at = |l: &Layer, f: u32| match &l.keyframe_at(f).unwrap().elements[0].kind {
            ElementKind::Symbol { first_frame, .. } => *first_frame,
            _ => 999,
        };
        assert_eq!(frame_at(&l_hold, 2), 0);
        assert_eq!(frame_at(&l_hold, 6), 3);
        assert_eq!(frame_at(&l_hold, 12), 0);

        let mut swap_latch = swap_hold;
        swap_latch.mode = TriggerMode::Latch;
        let mut l_latch = symbol_layer(1, SymbolId(10), 20);
        let latch_events =
            vec![TriggerEvent { frame: 4, duration: 1, trigger: "2".into() }, TriggerEvent { frame: 14, duration: 1, trigger: "2".into() }];
        let n_latch = swap_latch.stamp_triggers(&mut l_latch, &latch_events, 20);
        assert_eq!(n_latch, 3, "latches at 4, unlatches back to default at 14");
        assert_eq!(frame_at(&l_latch, 10), 3);
        assert_eq!(frame_at(&l_latch, 16), 0);
    }

    #[test]
    fn procedural_auto_blink_breathe_and_walk_cycle_generate_keyframes() {
        let mut eye = symbol_layer(1, SymbolId(10), 120);
        let kfs = apply_auto_blink(&mut eye, &AutoBlinkConfig::default(), 0, 120);
        assert!(kfs >= 3);
        assert!(eye.keyframes.iter().any(|k| k.label == "blink"));

        let mut torso = Layer::new(LayerId(2), "Torso");
        let mut tkf = Keyframe::new(0, 72);
        tkf.elements.push(Arc::new(Element {
            id: ElementId(202),
            name: "chest".into(),
            transform: Affine::translate(400.0, 350.0),
            color: ColorEffect::default(),
            kind: ElementKind::Shape(Shape::filled(Path::rect(-40.0, -80.0, 80.0, 120.0), Paint::Solid(Rgba::BLACK))),
        }));
        torso.keyframes = vec![Arc::new(tkf)];
        let b_kfs = apply_breathe(&mut torso, &BreatheConfig::default(), 0, 72);
        assert!(b_kfs >= 5);

        let mut tl = Timeline { layers: vec![Arc::new(torso)] };
        let (updated, w_kfs) = apply_walk_cycle(&mut tl, &WalkRigLayers::default(), LayerId(2), &WalkCycleConfig::default(), 0, 24);
        assert_eq!(updated, 1);
        assert!(w_kfs >= 8);
    }
}
