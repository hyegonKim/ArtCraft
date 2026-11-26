use super::{SoundApp, Value, json};

/// The speed ramp's choices in the AudioSuite window: param id, label, default, and (value, name) pairs.
pub(super) const SPEED_RAMP_CHOICES: [(&str, &str, f32, &[(f32, &str)]); 3] = [
    ("change", "Change", 0.0, &[(0.0, "Slow Down"), (1.0, "Speed Up")]),
    ("selection", "Selection", 0.0, &[(0.0, "Fit To"), (1.0, "Extend")]),
    ("fades", "Fades", 1.0, &[(1.0, "On"), (0.0, "Off")]),
];

/// Match Loudness controls: the integrated-loudness target (ITU-R BS.1770 gated loudness, LUFS)
/// with one-click delivery presets, and the true-peak ceiling the look-ahead limiter holds (dBTP).
pub(super) fn loudness_controls(ui: &mut egui::Ui, params: &mut std::collections::BTreeMap<String, f32>) {
    ui.label(egui::RichText::new("Match Loudness (ITU-R BS.1770 integrated loudness)").strong());
    ui.horizontal_wrapped(|ui| {
        for preset in soundcraft_dsp::LoudnessPreset::ALL {
            if ui.button(preset.label()).clicked() {
                params.insert("target_lufs".into(), preset.target_lufs());
                params.insert("true_peak_db".into(), preset.max_true_peak_dbtp());
            }
        }
    });
    let target_lufs = params.entry("target_lufs".into()).or_insert(-14.0);
    let mut lufs_wide = f64::from(*target_lufs);
    crate::widgets::FineSlider::new(&mut lufs_wide, -36.0..=0.0).text("Target Loudness (LUFS)").default_value(-14.0).show(ui);
    *target_lufs = lufs_wide as f32;
    let tp = params.entry("true_peak_db".into()).or_insert(-1.0);
    let mut tp_wide = f64::from(*tp);
    crate::widgets::FineSlider::new(&mut tp_wide, -6.0..=0.0).text("Max True Peak (dBTP)").default_value(-1.0).show(ui);
    *tp = tp_wide as f32;
}

/// Turn Normalize's loudness mode on or off. Off also drops the loudness targets, so Render sends a
/// peak normalize again.
pub(super) fn set_lufs_mode(params: &mut std::collections::BTreeMap<String, f32>, on: bool) {
    params.insert("lufs_mode".into(), if on { 1.0 } else { 0.0 });
    if !on {
        params.remove("target_lufs");
        params.remove("true_peak_db");
    }
}

fn silence_controls(ui: &mut egui::Ui, params: &mut std::collections::BTreeMap<String, f32>, with_shorten: bool) {
    for (id, label, min, max, def) in [
        ("threshold_db", "Threshold dB", -96.0, 0.0, -48.0),
        ("min_length_ms", "Min silence ms", 0.0, 4000.0, 50.0),
        ("pad_before_ms", "Pad before ms", 0.0, 500.0, 5.0),
        ("pad_after_ms", "Pad after ms", 0.0, 2000.0, 20.0),
    ] {
        let v = params.entry(id.into()).or_insert(def as f32);
        let mut wide = f64::from(*v);
        crate::widgets::FineSlider::new(&mut wide, min..=max).text(label).default_value(def).show(ui);
        *v = wide as f32;
    }
    if with_shorten {
        let v = params.entry("shorten_ms".into()).or_insert(100.0);
        let mut wide = f64::from(*v);
        crate::widgets::FineSlider::new(&mut wide, 0.0..=2000.0).text("Shorten silence to ms").default_value(100.0).show(ui);
        *v = wide as f32;
    }
}

/// Process-parameter window for the chosen AudioSuite process, with its controls and Render action.
pub(super) fn audiosuite_window(app: &mut SoundApp, ctx: &egui::Context) {
    let Some(process) = app.ui.audiosuite.clone() else { return };
    let mut open = true;
    let raw_title = soundcraft_dsp::plugin_info(&process).map_or_else(|| process.replace('_', " "), |p| p.name.to_string());
    let title = crate::i18n::tr(&raw_title);
    let as_hdr = crate::i18n::tr("AudioSuite");
    egui::Window::new(format!("{as_hdr} · {title}")).open(&mut open).default_width(360.0).show(ctx, |ui| {
        let key = egui::Id::unique(("as_params", process.clone()));
        let mut params: std::collections::BTreeMap<String, f32> = ui.ctx().memory(|m| m.data.get_temp(key)).unwrap_or_default();
        if let Some(info) = soundcraft_dsp::plugin_info(&process) {
            for p in info.params {
                let v = params.entry(p.id.to_string()).or_insert(p.default);
                match p.unit {
                    soundcraft_dsp::Unit::Choice => {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(crate::i18n::tr(p.name));
                            for (idx, &label) in p.choices.iter().enumerate() {
                                ui.radio_value(v, idx as f32, crate::i18n::tr(label));
                            }
                        });
                    }
                    soundcraft_dsp::Unit::Toggle => {
                        let mut on = *v >= 0.5;
                        if ui.checkbox(&mut on, crate::i18n::tr(p.name)).changed() {
                            *v = if on { 1.0 } else { 0.0 };
                        }
                    }
                    _ => {
                        let mut wide = f64::from(*v);
                        crate::widgets::FineSlider::new(&mut wide, f64::from(p.min)..=f64::from(p.max))
                            .text(crate::i18n::tr(p.name))
                            .default_value(f64::from(p.default))
                            .show(ui);
                        *v = wide as f32;
                    }
                }
            }
            if process == "stem_separator" {
                let split = params.entry("split_to_tracks".into()).or_insert(0.0);
                let mut on = *split >= 0.5;
                if ui.checkbox(&mut on, crate::i18n::tr("Split into Vocal / Accompaniment / Ambience tracks")).changed() {
                    *split = if on { 1.0 } else { 0.0 };
                }
            }
        } else {
            match process.as_str() {
                "normalize" => {
                    let mut use_lufs = params.get("lufs_mode").is_some_and(|v| *v >= 0.5);
                    if ui.checkbox(&mut use_lufs, "Normalize loudness (LUFS) instead of peak").changed() {
                        set_lufs_mode(&mut params, use_lufs);
                    }
                    if use_lufs {
                        loudness_controls(ui, &mut params);
                    } else {
                        let v = params.entry("target_db".into()).or_insert(-0.1);
                        let mut wide = f64::from(*v);
                        crate::widgets::FineSlider::new(&mut wide, -24.0..=0.0).text("Peak dB").default_value(-0.1).show(ui);
                        *v = wide as f32;
                    }
                }
                "match_loudness" | "lufs_normalize" => {
                    loudness_controls(ui, &mut params);
                }
                "delete_silence" => {
                    silence_controls(ui, &mut params, false);
                }
                "shorten_silence" => {
                    silence_controls(ui, &mut params, true);
                }
                "time_stretch" => {
                    let v = params.entry("ratio".into()).or_insert(1.0);
                    let mut wide = f64::from(*v);
                    crate::widgets::FineSlider::new(&mut wide, 0.25..=4.0).text("Length ratio").default_value(1.0).show(ui);
                    *v = wide as f32;
                }
                "pitch_shift" => {
                    let v = params.entry("semitones".into()).or_insert(0.0);
                    let mut wide = f64::from(*v);
                    crate::widgets::FineSlider::new(&mut wide, -24.0..=24.0).text("Semitones").default_value(0.0).show(ui);
                    *v = wide as f32;
                }
                "speed_ramp" => {
                    for (id, label, default, choices) in SPEED_RAMP_CHOICES {
                        let v = params.entry(id.to_string()).or_insert(default);
                        ui.horizontal(|ui| {
                            ui.label(label);
                            for &(value, name) in choices {
                                ui.radio_value(v, value, name);
                            }
                        });
                    }
                }
                _ => {
                    ui.label("No parameters.");
                }
            }
        }
        ui.ctx().memory_mut(|m| m.data.insert_temp(key, params.clone()));
        let hint = if soundcraft_dsp::is_generator(&process) {
            "Fills the selection on the selected tracks with a new clip."
        } else {
            "Processes the selected clips and replaces them with rendered audio."
        };
        ui.label(egui::RichText::new(hint).small());
        if ui.button(egui::RichText::new(crate::i18n::tr("Render")).strong()).clicked() {
            let p: serde_json::Map<String, Value> = params.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
            let _ = app.run("audiosuite.process", json!({"process": process, "params": p}));
        }
    });
    if !open {
        app.ui.audiosuite = None;
    }
}
