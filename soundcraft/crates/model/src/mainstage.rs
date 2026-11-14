//! Apple MainStage-style live performance engine model:
//! `Concert → PatchSet → Patch → ChannelStripConfig`, hardware `MacroControl` Fan-Out mappings,
//! and `LivePatchSwitcher` with **`Patch Remain` (Seamless Patch Switching)**.
//!
//! In live keyboard/guitar performance, switching from one [`Patch`] to another must never cut off
//! held MIDI notes or reverb/delay tails. [`LivePatchSwitcher`] implements MainStage's signature
//! **Patch Remain** state machine:
//! - Newly played `NoteOn` events route exclusively to the active [`Patch`] (filtered by per-strip
//!   MIDI channel, key range `note_low..=note_high`, and velocity window `vel_low..=vel_high`, plus
//!   semitone transposition).
//! - Any notes held at the moment of a patch change remain assigned to the outgoing patch until
//!   their matching `NoteOff` arrives, and the outgoing patch stays in the active DSP set for
//!   `tail_decay_samples` after its last note is released so reverb/delay tails decay naturally.

use std::collections::{BTreeMap, BTreeSet};

use soundcraft_time::Samples;

use crate::{ChannelFormat, Insert, Mixer, SendSlot, TrackId};

/// Unique identifier for a [`Patch`] in a [`Concert`].
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct PatchId(pub u64);

/// Unique identifier for a [`PatchSet`] in a [`Concert`].
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct PatchSetId(pub u64);

/// A single channel strip inside a [`Patch`], with MIDI keyboard split (`note_low..=note_high`),
/// velocity layer (`vel_low..=vel_high`), transpose, and full SoundCraft [`Mixer`] chain.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ChannelStripConfig {
    pub id: TrackId,
    pub name: String,
    pub format: ChannelFormat,
    /// Optional software instrument plugin at the top of the strip.
    pub instrument: Option<Insert>,
    pub mixer: Mixer,
    /// Accepted MIDI channel (`None` = Omni / any channel `0..16`).
    pub midi_channel: Option<u8>,
    /// Lowest MIDI note accepted by this layer/split (`0..=127`).
    pub note_low: u8,
    /// Highest MIDI note accepted by this layer/split (`0..=127`).
    pub note_high: u8,
    /// Lowest MIDI velocity accepted by this velocity switch (`1..=127`).
    pub vel_low: u8,
    /// Highest MIDI velocity accepted by this velocity switch (`1..=127`).
    pub vel_high: u8,
    /// Semitone transposition applied to incoming MIDI notes (`-48..=48`).
    pub transpose: i8,
}

impl ChannelStripConfig {
    pub fn new(id: TrackId, name: impl Into<String>, instrument_plugin: Option<&str>) -> Self {
        Self {
            id,
            name: name.into(),
            format: ChannelFormat::Stereo,
            instrument: instrument_plugin.map(Insert::new),
            mixer: Mixer::new(ChannelFormat::Stereo),
            midi_channel: None,
            note_low: 0,
            note_high: 127,
            vel_low: 1,
            vel_high: 127,
            transpose: 0,
        }
    }

    /// Builder helper to configure a keyboard split and velocity layer.
    pub fn with_zone(mut self, note_low: u8, note_high: u8, vel_low: u8, vel_high: u8) -> Self {
        self.note_low = note_low.min(127);
        self.note_high = note_high.min(127).max(self.note_low);
        self.vel_low = vel_low.clamp(1, 127);
        self.vel_high = vel_high.clamp(1, 127).max(self.vel_low);
        self
    }

    /// Returns `true` if a MIDI note-on `(channel, pitch, velocity)` triggers this strip.
    pub fn accepts_note(&self, channel: u8, pitch: u8, velocity: u8) -> bool {
        if let Some(ch) = self.midi_channel
            && ch != channel
        {
            return false;
        }
        (self.note_low..=self.note_high).contains(&pitch)
            && (self.vel_low..=self.vel_high).contains(&velocity)
    }

    /// Transposes `pitch` by `self.transpose`, clamped to `0..=127`.
    pub fn transposed_pitch(&self, pitch: u8) -> u8 {
        (i16::from(pitch) + i16::from(self.transpose)).clamp(0, 127) as u8
    }
}

/// Target parameter of a [`MacroMapping`] on a channel strip.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MacroTarget {
    /// Channel strip fader volume in dB.
    StripVolume { strip: TrackId },
    /// Channel strip stereo pan (`-1.0..=1.0`).
    StripPan { strip: TrackId },
    /// Send slot level in dB (`0..SEND_SLOTS`).
    StripSendLevel { strip: TrackId, send_slot: usize },
    /// Software instrument parameter by name.
    InstrumentParam { strip: TrackId, param: String },
    /// Audio insert plugin parameter (`0..INSERT_SLOTS`).
    InsertParam {
        strip: TrackId,
        insert_slot: usize,
        param: String,
    },
}

/// One destination in a MainStage Screen Control's multi-parameter Fan-Out.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MacroMapping {
    pub target: MacroTarget,
    /// Output value when the macro knob is at `0.0`.
    pub min_val: f32,
    /// Output value when the macro knob is at `1.0`.
    pub max_val: f32,
    /// Invert the normalized `0.0..=1.0` macro position before scaling.
    pub inverted: bool,
}

impl MacroMapping {
    /// Map a normalized `0.0..=1.0` screen control value to the target parameter's domain.
    pub fn evaluate(&self, norm: f32) -> f32 {
        let t = if norm.is_finite() {
            norm.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let t = if self.inverted { 1.0 - t } else { t };
        self.min_val + (self.max_val - self.min_val) * t
    }
}

/// A hardware-bound Screen Control (knob, fader, button, pedal) that fans out to multiple
/// [`MacroMapping`]s simultaneously.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MacroControl {
    pub id: u64,
    pub name: String,
    /// Assigned hardware MIDI Continuous Controller (`0..=127`).
    pub midi_cc: Option<u8>,
    /// Current normalized position (`0.0..=1.0`).
    pub value: f32,
    pub mappings: Vec<MacroMapping>,
}

impl MacroControl {
    pub fn new(id: u64, name: impl Into<String>, midi_cc: Option<u8>) -> Self {
        Self {
            id,
            name: name.into(),
            midi_cc,
            value: 0.0,
            mappings: Vec::new(),
        }
    }
}

/// A playable MainStage [`Patch`] containing layered/split channel strips and screen controls.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Patch {
    pub id: PatchId,
    pub name: String,
    /// Optional MIDI Program Change number (`0..=127`) that recalls this patch.
    pub program_change: Option<u8>,
    /// Reverb/delay tail hold duration in samples after the last held note is released.
    pub tail_decay_samples: Samples,
    pub strips: Vec<ChannelStripConfig>,
    pub macros: Vec<MacroControl>,
}

impl Patch {
    pub fn new(id: PatchId, name: impl Into<String>, tail_decay_samples: Samples) -> Self {
        Self {
            id,
            name: name.into(),
            program_change: None,
            tail_decay_samples: tail_decay_samples.max(0),
            strips: Vec::new(),
            macros: Vec::new(),
        }
    }

    /// Apply a normalized (`0.0..=1.0`) macro control update across all mapped channel strips.
    pub fn set_macro_value(&mut self, macro_id: u64, norm_value: f32) -> usize {
        let norm = if norm_value.is_finite() {
            norm_value.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let Some(mc) = self.macros.iter_mut().find(|m| m.id == macro_id) else {
            return 0;
        };
        mc.value = norm;
        let mappings = mc.mappings.clone();
        let mut applied = 0_usize;
        for mapping in &mappings {
            let v = mapping.evaluate(norm);
            if self.apply_mapping_target(&mapping.target, v) {
                applied += 1;
            }
        }
        applied
    }

    /// Handle a hardware MIDI CC message (`cc` ∈ `0..=127`, `value_7bit` ∈ `0..=127`).
    pub fn handle_midi_cc(&mut self, cc: u8, value_7bit: u8) -> usize {
        let norm = f32::from(value_7bit.min(127)) / 127.0;
        let matching_ids: Vec<u64> = self
            .macros
            .iter()
            .filter(|m| m.midi_cc == Some(cc))
            .map(|m| m.id)
            .collect();
        let mut total = 0_usize;
        for id in matching_ids {
            total += self.set_macro_value(id, norm);
        }
        total
    }

    fn apply_mapping_target(&mut self, target: &MacroTarget, val: f32) -> bool {
        match target {
            MacroTarget::StripVolume { strip } => {
                if let Some(s) = self.strips.iter_mut().find(|s| s.id == *strip) {
                    s.mixer.volume_db = val.clamp(-144.0, 12.0);
                    return true;
                }
            }
            MacroTarget::StripPan { strip } => {
                if let Some(s) = self.strips.iter_mut().find(|s| s.id == *strip)
                    && let Some(p) = s.mixer.pan.first_mut()
                {
                    *p = val.clamp(-1.0, 1.0);
                    return true;
                }
            }
            MacroTarget::StripSendLevel { strip, send_slot } => {
                if let Some(s) = self.strips.iter_mut().find(|s| s.id == *strip)
                    && let Some(slot) = s.mixer.sends.get_mut(*send_slot)
                {
                    let send = slot.get_or_insert_with(|| SendSlot::new(crate::Route::Main));
                    send.level_db = val.clamp(-144.0, 12.0);
                    return true;
                }
            }
            MacroTarget::InstrumentParam { strip, param } => {
                if let Some(s) = self.strips.iter_mut().find(|s| s.id == *strip)
                    && let Some(inst) = &mut s.instrument
                {
                    inst.params.insert(param.clone(), val);
                    return true;
                }
            }
            MacroTarget::InsertParam {
                strip,
                insert_slot,
                param,
            } => {
                if let Some(s) = self.strips.iter_mut().find(|s| s.id == *strip)
                    && let Some( Some(ins)) = s.mixer.inserts.get_mut(*insert_slot)
                {
                    ins.params.insert(param.clone(), val);
                    return true;
                }
            }
        }
        false
    }
}

/// A Song or Set folder grouping related [`Patch`]es inside a [`Concert`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PatchSet {
    pub id: PatchSetId,
    pub name: String,
    pub patches: Vec<Patch>,
}

/// Top-level MainStage [`Concert`] document containing Concert-wide master strips and [`PatchSet`]s.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Concert {
    pub name: String,
    /// Concert-level shared Aux/Master strips (e.g., global reverb or backing-track player).
    pub global_strips: Vec<ChannelStripConfig>,
    /// Concert-level global macro controls.
    pub global_macros: Vec<MacroControl>,
    pub sets: Vec<PatchSet>,
}

impl Concert {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            global_strips: Vec::new(),
            global_macros: Vec::new(),
            sets: Vec::new(),
        }
    }

    pub fn patch(&self, id: PatchId) -> Option<&Patch> {
        self.sets.iter().flat_map(|s| &s.patches).find(|p| p.id == id)
    }

    pub fn patch_mut(&mut self, id: PatchId) -> Option<&mut Patch> {
        self.sets
            .iter_mut()
            .flat_map(|s| &mut s.patches)
            .find(|p| p.id == id)
    }

    pub fn patch_by_program(&self, program: u8) -> Option<&Patch> {
        self.sets
            .iter()
            .flat_map(|s| &s.patches)
            .find(|p| p.program_change == Some(program))
    }
}

/// A routed MIDI event produced by [`LivePatchSwitcher`] for a specific [`Patch`] and strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutedMidiNote {
    pub patch_id: PatchId,
    pub strip_id: TrackId,
    pub channel: u8,
    pub pitch: u8,
    pub velocity: u8,
    pub is_note_on: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HeldNoteRoute {
    patch_id: PatchId,
    strip_id: TrackId,
    transposed_pitch: u8,
}

/// Real-time **Patch Remain** (Seamless Patch Switching) router.
#[derive(Debug, Clone)]
pub struct LivePatchSwitcher {
    active_patch: PatchId,
    /// Held notes keyed by `(midi_channel, raw_input_pitch)` -> routed strip(s) that were triggered.
    held_notes: BTreeMap<(u8, u8), Vec<HeldNoteRoute>>,
    /// Outgoing patches whose audio DSP must remain active while their reverb/delay tail decays.
    decaying_tails: BTreeMap<PatchId, Samples>,
}

impl LivePatchSwitcher {
    pub fn new(initial_patch: PatchId) -> Self {
        Self {
            active_patch: initial_patch,
            held_notes: BTreeMap::new(),
            decaying_tails: BTreeMap::new(),
        }
    }

    pub fn active_patch(&self) -> PatchId {
        self.active_patch
    }

    /// Switch to `next_patch` with **Patch Remain**:
    /// - Any notes currently held on the outgoing patch remain mapped to the outgoing patch until
    ///   released.
    /// - The outgoing patch is registered in `decaying_tails` with its `tail_decay_samples` window
    ///   so its reverb/delay tails are never cut off.
    pub fn select_patch(&mut self, concert: &Concert, next_patch: PatchId) {
        if self.active_patch == next_patch {
            return;
        }
        let prev = self.active_patch;
        self.active_patch = next_patch;
        self.decaying_tails.remove(&next_patch);

        let tail = concert
            .patch(prev)
            .map(|p| p.tail_decay_samples)
            .unwrap_or(0)
            .max(0);
        let has_held = self
            .held_notes
            .values()
            .any(|routes| routes.iter().any(|r| r.patch_id == prev));
        if has_held || tail > 0 {
            self.decaying_tails.insert(prev, tail);
        }
    }

    /// Route an incoming MIDI Note-On (`velocity > 0`).
    /// Only triggers strips belonging to the **currently active** [`Patch`].
    pub fn note_on(
        &mut self,
        concert: &Concert,
        channel: u8,
        pitch: u8,
        velocity: u8,
    ) -> Vec<RoutedMidiNote> {
        if velocity == 0 {
            return self.note_off(concert, channel, pitch);
        }
        let Some(patch) = concert.patch(self.active_patch) else {
            return Vec::new();
        };

        let mut routed = Vec::new();
        let mut held_entries = Vec::new();
        for strip in &patch.strips {
            if strip.accepts_note(channel, pitch, velocity) {
                let tp = strip.transposed_pitch(pitch);
                routed.push(RoutedMidiNote {
                    patch_id: patch.id,
                    strip_id: strip.id,
                    channel,
                    pitch: tp,
                    velocity,
                    is_note_on: true,
                });
                held_entries.push(HeldNoteRoute {
                    patch_id: patch.id,
                    strip_id: strip.id,
                    transposed_pitch: tp,
                });
            }
        }
        if !held_entries.is_empty() {
            self.held_notes
                .entry((channel, pitch))
                .or_default()
                .extend(held_entries);
        }
        routed
    }

    /// Route an incoming MIDI Note-Off to whichever [`Patch`] and strip(s) originally received the
    /// `NoteOn` — even if the performer has switched patches since then (**Patch Remain**).
    pub fn note_off(&mut self, concert: &Concert, channel: u8, pitch: u8) -> Vec<RoutedMidiNote> {
        let Some(routes) = self.held_notes.remove(&(channel, pitch)) else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(routes.len());
        for r in routes {
            out.push(RoutedMidiNote {
                patch_id: r.patch_id,
                strip_id: r.strip_id,
                channel,
                pitch: r.transposed_pitch,
                velocity: 0,
                is_note_on: false,
            });
            // If this note belonged to a previous patch, refresh its tail decay timer from the
            // moment the note is released.
            if r.patch_id != self.active_patch {
                let tail = concert
                    .patch(r.patch_id)
                    .map(|p| p.tail_decay_samples)
                    .unwrap_or(0)
                    .max(0);
                if tail > 0 {
                    self.decaying_tails.insert(r.patch_id, tail);
                }
            }
        }
        out
    }

    /// Advance the audio clock by `frames` samples, expiring outgoing patches whose held notes have
    /// all been released AND whose reverb/delay tail has finished decaying.
    pub fn advance_audio(&mut self, frames: Samples) {
        let dt = frames.max(0);
        let patches_with_held_notes: BTreeSet<PatchId> = self
            .held_notes
            .values()
            .flat_map(|v| v.iter().map(|r| r.patch_id))
            .collect();

        self.decaying_tails.retain(|patch_id, remaining| {
            if patches_with_held_notes.contains(patch_id) {
                // Keep full tail remaining while notes are still physically held on this outgoing patch.
                true
            } else {
                *remaining = remaining.saturating_sub(dt);
                *remaining > 0
            }
        });
    }

    /// Returns all [`PatchId`]s that must currently be rendered by the audio engine:
    /// the active patch plus any outgoing patches with held notes or decaying reverb/delay tails.
    pub fn active_dsp_patches(&self) -> Vec<PatchId> {
        let mut set = BTreeSet::new();
        set.insert(self.active_patch);
        for routes in self.held_notes.values() {
            for r in routes {
                set.insert(r.patch_id);
            }
        }
        for (&pid, &rem) in &self.decaying_tails {
            if rem > 0 {
                set.insert(pid);
            }
        }
        set.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_remain_keeps_held_notes_and_reverb_tails_across_patch_switch() {
        let mut concert = Concert::new("World Tour 2026");

        // Patch 1: Warm Pad (with 1-second reverb tail = 48_000 samples)
        let mut p1 = Patch::new(PatchId(1), "Warm Pad", 48_000);
        p1.program_change = Some(0);
        p1.strips.push(
            ChannelStripConfig::new(TrackId(10), "Pad Synth", Some("alchemy_pad"))
                .with_zone(36, 96, 1, 127),
        );

        // Patch 2: Split Bass (C1..B2) + Lead Synth (C3..C6, transposed +12 semitones)
        let mut p2 = Patch::new(PatchId(2), "Bass + Lead Split", 24_000);
        p2.program_change = Some(1);
        p2.strips.push(
            ChannelStripConfig::new(TrackId(20), "Sub Bass", Some("retro_bass"))
                .with_zone(24, 59, 1, 127),
        );
        let mut lead = ChannelStripConfig::new(TrackId(21), "Saw Lead", Some("es2_lead"))
            .with_zone(60, 108, 1, 127);
        lead.transpose = 12;
        p2.strips.push(lead);

        // Add a Screen Control Macro on Patch 2 that simultaneously opens Lead cutoff and lowers Bass volume.
        let mut cutoff_macro = MacroControl::new(1, "Filter + Bass Balance", Some(74));
        cutoff_macro.mappings.push(MacroMapping {
            target: MacroTarget::InstrumentParam {
                strip: TrackId(21),
                param: "cutoff".to_string(),
            },
            min_val: 200.0,
            max_val: 8200.0,
            inverted: false,
        });
        cutoff_macro.mappings.push(MacroMapping {
            target: MacroTarget::StripVolume { strip: TrackId(20) },
            min_val: 0.0,
            max_val: -6.0,
            inverted: false,
        });
        p2.macros.push(cutoff_macro);

        concert.sets.push(PatchSet {
            id: PatchSetId(1),
            name: "Opening Set".to_string(),
            patches: vec![p1, p2],
        });

        let mut switcher = LivePatchSwitcher::new(PatchId(1));

        // 1. Hold C4 (pitch 60) on Patch 1 ("Warm Pad")
        let on1 = switcher.note_on(&concert, 0, 60, 100);
        assert_eq!(on1.len(), 1);
        assert_eq!(on1[0].patch_id, PatchId(1));
        assert_eq!(on1[0].strip_id, TrackId(10));

        // 2. While still holding C4, switch to Patch 2 ("Bass + Lead Split")!
        switcher.select_patch(&concert, PatchId(2));
        assert_eq!(switcher.active_patch(), PatchId(2));
        // Both Patch 1 (held note + tail) and Patch 2 (active) must remain in the DSP graph:
        assert_eq!(
            switcher.active_dsp_patches(),
            vec![PatchId(1), PatchId(2)]
        );

        // 3. Play C2 (pitch 36) and E4 (pitch 64) on Patch 2:
        //    - C2 routes to Sub Bass (strip 20)
        //    - E4 routes to Saw Lead (strip 21) transposed +12 -> pitch 76
        let on_bass = switcher.note_on(&concert, 0, 36, 90);
        assert_eq!(on_bass[0].patch_id, PatchId(2));
        assert_eq!(on_bass[0].strip_id, TrackId(20));

        let on_lead = switcher.note_on(&concert, 0, 64, 110);
        assert_eq!(on_lead[0].patch_id, PatchId(2));
        assert_eq!(on_lead[0].strip_id, TrackId(21));
        assert_eq!(on_lead[0].pitch, 76);

        // 4. Advance audio by 2 seconds while C4 is still held on Patch 1 -> Patch 1 must NOT expire!
        switcher.advance_audio(96_000);
        assert_eq!(
            switcher.active_dsp_patches(),
            vec![PatchId(1), PatchId(2)]
        );

        // 5. Release C4 (pitch 60) -> NoteOff routes to Patch 1 (Strip 10), and starts Patch 1's 48_000-sample reverb tail.
        let off1 = switcher.note_off(&concert, 0, 60);
        assert_eq!(off1.len(), 1);
        assert_eq!(off1[0].patch_id, PatchId(1));
        assert_eq!(off1[0].strip_id, TrackId(10));

        // Halfway through the 1-second tail (24_000 samples), Patch 1 is still active:
        switcher.advance_audio(24_000);
        assert_eq!(
            switcher.active_dsp_patches(),
            vec![PatchId(1), PatchId(2)]
        );

        // After the remaining 24_000 samples of tail decay, Patch 1 cleanly deactivates!
        switcher.advance_audio(24_000);
        assert_eq!(switcher.active_dsp_patches(), vec![PatchId(2)]);

        // 6. Test hardware MIDI CC 74 macro fan-out on Patch 2:
        let patch2 = concert.patch_mut(PatchId(2)).unwrap();
        assert_eq!(patch2.handle_midi_cc(74, 127), 2);
        let cutoff = patch2.strips[1]
            .instrument
            .as_ref()
            .unwrap()
            .params
            .get("cutoff")
            .copied()
            .unwrap();
        assert!((cutoff - 8200.0).abs() < 1e-3);
        assert!((patch2.strips[0].mixer.volume_db - (-6.0)).abs() < 1e-3);
    }
}
