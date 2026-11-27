use craft_audio::AudioBuffer;
use serde_json::json;
use soundcraft_engine::Engine;
use soundcraft_mix::MixEngine;
use soundcraft_model::{ChannelFormat, Clip, Fade, FadeShape, FileFormat, Insert, TrackKind};

#[test]
fn aaf_and_omf_roundtrip_preserve_tracks_clips_gain_and_fades() {
    let dir = std::env::temp_dir().join(format!("soundcraft-audition-parity-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let aaf_path = dir.join("timeline.aaf");
    let omf_path = dir.join("timeline.omf");

    let mut src_engine = Engine::default();
    let sr = src_engine.session().sample_rate.hz();
    let frames = 4_800usize;
    let left: Vec<f32> = (0..frames).map(|i| 0.5 * (i as f32 * 0.05).sin()).collect();
    let right: Vec<f32> = (0..frames).map(|i| 0.4 * (i as f32 * 0.07).cos()).collect();
    let sid = soundcraft_engine::io::add_source(
        src_engine.session_mut(),
        "Dialogue_Take1",
        AudioBuffer { sample_rate: sr, channels: vec![left, right] },
        None,
        FileFormat::Wav,
    );
    let tid = src_engine.session_mut().add_track(TrackKind::Audio, ChannelFormat::Stereo, Some("DX 1"));
    {
        let s = src_engine.session_mut();
        s.track_mut(tid).unwrap().mixer.volume_db = -3.5;
        s.track_mut(tid).unwrap().mixer.pan = vec![-0.25, 0.75];
        let cid = s.new_clip_id();
        let mut clip = Clip::audio(cid, "DX_Clip_A", sid, 240, 1_200, 3_600);
        clip.gain_db = 2.5;
        clip.fade_in = Fade { len: 300, shape: FadeShape::EqualPower, curvature: 0.0 };
        clip.fade_out = Fade { len: 400, shape: FadeShape::SCurve, curvature: 0.0 };
        soundcraft_engine::edit::place_clip(s, tid, clip);
    }

    let aaf_res = src_engine.execute("file.export_aaf", &json!({"path": aaf_path.to_str().unwrap(), "embed_audio": true})).unwrap();
    assert_eq!(aaf_res["tracks"], 1);
    assert_eq!(aaf_res["clips"], 1);
    assert_eq!(aaf_res["embedded_media"], 1);

    let omf_res = src_engine.execute("file.export_omf", &json!({"path": omf_path.to_str().unwrap(), "embed_audio": true})).unwrap();
    assert_eq!(omf_res["format"], "omf");
    assert_eq!(omf_res["clips"], 1);

    // Import AAF into a fresh engine and verify track/clip/fade fidelity.
    let mut dst_aaf = Engine::default();
    let imp_aaf = dst_aaf.execute("file.import_aaf", &json!({"path": aaf_path.to_str().unwrap()})).unwrap();
    assert_eq!(imp_aaf["clips"], 1);
    let tr = dst_aaf.session().track_by_name("DX 1").unwrap();
    assert!((tr.mixer.volume_db - (-3.5)).abs() < 1e-4);
    assert!((tr.mixer.pan[0] - (-0.25)).abs() < 1e-4);
    assert!((tr.mixer.pan[1] - 0.75).abs() < 1e-4);
    let c = &tr.clips()[0];
    assert_eq!((c.start, c.length, c.source_offset()), (1_200, 3_600, 240));
    assert!((c.gain_db - 2.5).abs() < 1e-4);
    assert_eq!((c.fade_in.len, c.fade_out.len), (300, 400));

    // Import OMF via `file.import_session_data` at an offset and verify placement.
    let mut dst_omf = Engine::default();
    let imp_omf = dst_omf.execute("file.import_session_data", &json!({"path": omf_path.to_str().unwrap(), "at": 480})).unwrap();
    assert_eq!(imp_omf["clips"], 1);
    let c_omf = &dst_omf.session().track_by_name("DX 1").unwrap().clips()[0];
    assert_eq!(c_omf.start, 1_680);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn audiosuite_crosstalk_leveling_and_stem_separation_split_tracks() {
    let mut e = Engine::default();
    let sr = e.session().sample_rate.hz();
    let n = 8_192usize;
    let vocal_mid: Vec<f32> = (0..n).map(|i| 0.45 * (i as f32 * 0.08).sin()).collect();
    let side_fx: Vec<f32> = (0..n).map(|i| if i % 256 < 12 { 0.35 } else { 0.05 * (i as f32 * 0.21).cos() }).collect();
    let ch_l: Vec<f32> = vocal_mid.iter().zip(&side_fx).map(|(m, s)| m + s).collect();
    let ch_r: Vec<f32> = vocal_mid.iter().zip(&side_fx).map(|(m, s)| m - s).collect();
    let sid = soundcraft_engine::io::add_source(
        e.session_mut(),
        "MixClip",
        AudioBuffer { sample_rate: sr, channels: vec![ch_l, ch_r] },
        None,
        FileFormat::Wav,
    );
    let tid = e.session_mut().add_track(TrackKind::Audio, ChannelFormat::Stereo, Some("Full Mix"));
    let cid = e.session_mut().new_clip_id();
    soundcraft_engine::edit::place_clip(e.session_mut(), tid, Clip::audio(cid, "MixClip", sid, 0, 0, n as i64));

    // 1. Separate Crosstalk
    e.execute("audiosuite.process", &json!({"process": "separate_crosstalk", "clips": [cid.0], "params": {"suppression_db": 24.0}})).unwrap();
    let c1 = e.session().track(tid).unwrap().clips()[0].id;

    // 2. Audio Leveling
    e.execute("audiosuite.process", &json!({"process": "audio_leveler", "clips": [c1.0], "params": {"target_db": -16.0, "amount": 90.0}})).unwrap();
    let c2 = e.session().track(tid).unwrap().clips()[0].id;

    // 3. Stem Separation with multi-track split
    let split = e.execute("audiosuite.process", &json!({"process": "stem_separator", "clips": [c2.0], "params": {"split_to_tracks": 1.0}})).unwrap();
    assert_eq!(split["processed"], 1);
    assert_eq!(split["stem_tracks"].as_array().unwrap().len(), 3);
    assert!(e.session().tracks.iter().any(|t| t.name.contains("Vocals")));
    assert!(e.session().tracks.iter().any(|t| t.name.contains("Accompaniment")));
    assert!(e.session().tracks.iter().any(|t| t.name.contains("Ambience")));
}

#[test]
fn clip_looping_supports_1000_iterations_and_crossfades() {
    let mut e = Engine::default();
    let sr = e.session().sample_rate.hz();
    let sid = soundcraft_engine::io::add_source(
        e.session_mut(),
        "LoopSrc",
        AudioBuffer { sample_rate: sr, channels: vec![vec![0.25; 4_800]] },
        None,
        FileFormat::Wav,
    );
    let tid = e.session_mut().add_track(TrackKind::Audio, ChannelFormat::Mono, Some("LoopTrack"));
    let cid = e.session_mut().new_clip_id();
    soundcraft_engine::edit::place_clip(e.session_mut(), tid, Clip::audio(cid, "Beat", sid, 0, 0, 4_800));

    // Verify 750 loops (> 512 old cap, <= 1000)
    let r = e.execute("clip.loop", &json!({"clips": [cid.0], "count": 750})).unwrap();
    assert_eq!(r["loops"], 749);
    assert_eq!(e.session().track(tid).unwrap().clips().len(), 750);

    // Undo and loop with 10 ms s-curve crossfade
    assert!(e.undo());
    let xf = e.execute("clip.loop", &json!({"clips": [cid.0], "count": 4, "crossfade_ms": 10.0, "crossfade_shape": "s-curve"})).unwrap();
    assert_eq!(xf["loops"], 3);
    let clips = e.session().track(tid).unwrap().clips();
    assert_eq!(clips.len(), 4);
    assert_eq!(clips[0].fade_out.len, 480);
    assert_eq!(clips[1].fade_in.len, 480);
    assert_eq!(clips[1].fade_out.len, 480);
    assert_eq!(clips[1].start, 4_800 - 480);
}

#[test]
fn live_midi_audition_queues_request_and_renders_instrument_while_stopped() {
    let mut e = Engine::default();
    let tid = e.session_mut().add_track(TrackKind::Instrument, ChannelFormat::Stereo, Some("Synth"));
    e.session_mut().track_mut(tid).unwrap().instrument = Some(Insert::new("subtractive_synth"));
    let r = e.execute("midi.new_clip", &json!({"track": tid.0})).unwrap();
    let cid = r["clip"].as_u64().unwrap();

    let aud = e.execute("midi.audition", &json!({"clip": cid, "pitch": 69, "velocity": 110, "duration_ms": 200})).unwrap();
    assert_eq!(aud["pitch"], 69);
    assert_eq!(aud["track"], tid.0);
    assert!(e.transport_requests.iter().any(|req| matches!(
        req,
        soundcraft_engine::TransportRequest::MidiAudition { track: Some(t), pitch: 69, velocity: 110, duration_ms: 200 } if *t == tid
    )));

    // Render live MIDI through MixEngine with monitor_only = true (transport stopped!)
    let mut mix = MixEngine::new(e.session().sample_rate.as_f64() as f32, 512);
    mix.monitor_only = true;
    mix.push_live_midi(Some(tid), [0x90, 69, 110]);
    let mut out = vec![vec![0.0f32; 512]; 2];
    mix.render(e.session(), 0, 512, &mut out);
    let peak = out.iter().flat_map(|ch| ch.iter()).fold(0.0f32, |m, x| m.max(x.abs()));
    assert!(peak > 1e-3, "stopped instrument strip must render live auditioned MIDI note (peak={peak})");
}
