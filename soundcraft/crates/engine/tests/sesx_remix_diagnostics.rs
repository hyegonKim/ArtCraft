//! Integration tests for `.sesx` multitrack import/export, beat-synchronous `Remix`, and
//! Audio Diagnostics (`DeClicker`, `DeClipper`, `Scan/Delete Silence`).

#![allow(clippy::unwrap_used)]

use craft_audio::AudioBuffer;
use serde_json::json;
use soundcraft_engine::Engine;
use soundcraft_model::{ChannelFormat, Clip, FileFormat, MarkerKind, Session, TrackKind};

#[test]
fn sesx_engine_export_and_import_roundtrips_tracks_clips_fades_and_markers() {
    let dir = std::env::temp_dir().join(format!("soundcraft-sesx-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let wav_path = dir.join("lead_vox.wav");
    let sesx_path = dir.join("episode.sesx");

    let s = Session { name: "Documentary Reel".to_string(), ..Default::default() };
    let sr = s.sample_rate.hz();
    let frames = sr as usize * 2;
    let tone: Vec<f32> = (0..frames).map(|i| 0.25 * (std::f32::consts::TAU * 440.0 * i as f32 / sr as f32).sin()).collect();
    let buf = AudioBuffer { sample_rate: sr, channels: vec![tone] };
    let fmt = craft_audio::encode_format_for("wav").unwrap();
    let wav_bytes = craft_audio::encode(&buf, &craft_audio::EncodeOptions::new(fmt)).unwrap();
    std::fs::write(&wav_path, &wav_bytes).unwrap();

    let mut e = Engine::new(s);
    e.execute("file.import_audio", &json!({"path": wav_path.to_string_lossy(), "at": 24_000})).unwrap();
    let tid = e.session().tracks[0].id;
    e.execute("mix.volume", &json!({"track": tid.0, "db": -3.0})).unwrap();
    e.execute("mix.pan", &json!({"track": tid.0, "pan": 0.35})).unwrap();
    e.execute("mix.solo", &json!({"track": tid.0, "value": true})).unwrap();
    let cid = e.session().tracks[0].clips()[0].id;
    e.session_mut().find_clip_mut(cid).unwrap().fade_in.len = 2_400;
    e.session_mut().find_clip_mut(cid).unwrap().fade_out.len = 4_800;
    e.session_mut().add_marker("Act II", MarkerKind::Marker, 48_000, 48_000);

    let exp = e.execute("file.export_sesx", &json!({"path": sesx_path.to_string_lossy(), "replace": true})).unwrap();
    assert!(exp["bytes"].as_u64().unwrap() > 200);

    let mut e2 = Engine::default();
    let imp = e2.execute("file.import_sesx", &json!({"path": sesx_path.to_string_lossy()})).unwrap();
    assert_eq!(imp["version"], "1.9");
    assert_eq!(imp["clips"], 1);
    assert_eq!(imp["markers"], 1);
    assert_eq!(imp["files_loaded"], 1);
    assert_eq!(e2.session().name, "Documentary Reel");

    let tr = e2.session().tracks.iter().find(|t| t.kind == TrackKind::Audio).unwrap();
    assert!(tr.mixer.solo);
    assert!((tr.mixer.volume_db - (-3.0)).abs() < 0.05);
    assert!((tr.mixer.pan[0] - 0.35).abs() < 0.02);
    assert_eq!(tr.clips()[0].start, 24_000);
    assert_eq!(tr.clips()[0].fade_in.len, 2_400);
    assert_eq!(tr.clips()[0].fade_out.len, 4_800);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn engine_remix_and_diagnostics_commands_execute_and_undo_cleanly() {
    let mut s = Session::default();
    let sr = s.sample_rate.hz();
    let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Music & Voice"));

    // Build an 8-second rhythmic clip at 120 BPM with 2 injected clicks, a clipped peak, and a trailing silence gap.
    let beat_samples = (sr / 2) as usize; // 120 BPM = 0.5s per beat
    let total_frames = beat_samples * 16; // 8.0s
    let mut sig = vec![0.0_f32; total_frames];
    for b in 0..14 {
        let base = b * beat_samples;
        for k in 0..beat_samples {
            let sec = k as f32 / sr as f32;
            let kick = (-sec * 25.0).exp() * (std::f32::consts::TAU * 100.0 * sec).sin();
            let chord = 0.3 * (std::f32::consts::TAU * 220.0 * sec).sin();
            sig[base + k] = 0.5 * kick + chord;
        }
    }
    // Inject impulsive clicks:
    sig[5_000] = 0.99;
    sig[5_001] = -0.97;
    // Inject a flat-top clipped plateau (6 samples at +0.98):
    for slot in &mut sig[12_000..12_006] {
        *slot = 0.98;
    }
    // Last 2 beats (`14..16`, 1.0 second) are pure 0.0 silence!

    let src = soundcraft_engine::io::add_source(&mut s, "cue", AudioBuffer { sample_rate: sr, channels: vec![sig] }, None, FileFormat::Wav);
    let cid = s.new_clip_id();
    s.track_mut(t).unwrap().playlist_mut().unwrap().clips.push(Clip::audio(cid, "cue", src, 0, 0, total_frames as i64));
    s.edit.selected_clips = vec![cid];

    let mut e = Engine::new(s);

    // 1. Scan diagnostics (read-only query)
    let scan = e.execute("diagnostics.scan", &json!({"clip_threshold_db": -0.3})).unwrap();
    assert!(scan["clicks"].as_u64().unwrap() >= 1, "scan={scan}");
    assert!(scan["clipped_peaks"].as_u64().unwrap() >= 1, "scan={scan}");
    assert!(scan["silence_regions"].as_u64().unwrap() >= 1, "scan={scan}");
    assert!(!e.can_undo(), "diagnostics.scan is a read-only query");

    // 2. DeClick + DeClip via engine commands
    let dc = e.execute("diagnostics.declick", &json!({"sensitivity": 65.0})).unwrap();
    assert_eq!(dc["processed"], 1);
    assert!(dc["clicks_repaired"].as_u64().unwrap() >= 1);

    e.session_mut().edit.selected_clips = vec![e.session().tracks[0].clips()[0].id];
    let dclip = e.execute("diagnostics.declip", &json!({"threshold_db": -0.3, "ceiling_db": -0.2})).unwrap();
    assert_eq!(dclip["processed"], 1);

    // 3. Delete Silence
    e.session_mut().edit.selected_clips = vec![e.session().tracks[0].clips()[0].id];
    let dsil = e.execute("diagnostics.delete_silence", &json!({"threshold_db": -48.0, "min_silence_ms": 200.0})).unwrap();
    assert_eq!(dsil["processed"], 1);
    assert!(dsil["frames_removed"].as_u64().unwrap() > (sr / 2) as u64);

    // 4. Remix to exact 5.0 seconds
    e.session_mut().edit.selected_clips = vec![e.session().tracks[0].clips()[0].id];
    let rmx = e.execute("clip.remix", &json!({"target_seconds": 5.0, "crossfade_ms": 50.0})).unwrap();
    assert_eq!(rmx["processed"], 1);
    assert_eq!(rmx["target_frames"], (sr * 5) as u64);
    assert_eq!(e.session().tracks[0].clips()[0].length, i64::from(sr) * 5);

    // Undo restores previous states cleanly
    e.execute("edit.undo", &json!({})).unwrap();
    assert!(e.session().tracks[0].clips()[0].length != i64::from(sr) * 5);
}
