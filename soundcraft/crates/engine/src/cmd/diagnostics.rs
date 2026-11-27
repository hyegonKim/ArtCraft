//! Beat-synchronous `Remix` duration retiming and Audio Diagnostics (`DeClicker`, `DeClipper`,
//! `Scan/Delete Silence`) engine commands.

use super::{CommandSpec, always, bad, clip_ids_param, f64_or, has_selection};
use crate::{Engine, Result, cmd};
use craft_audio::AudioBuffer;
use serde_json::{Value, json};
use soundcraft_dsp::diagnostics::{
    SilenceScanOptions, delete_silence_regions, detect_clicks, detect_clipped_regions, repair_clicks, repair_clipping, scan_silence_regions,
};
use soundcraft_dsp::remix::{RemixOptions, remix_to_duration};
use soundcraft_model::{Clip, ClipContent, ClipId, FileFormat, TrackId};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "clip.remix",
            "Remix Duration...",
            ["Clip"],
            None,
            "{target_seconds, crossfade_ms?: 60, min_segment_beats?: 4, preserve_intro_beats?: 2, preserve_outro_beats?: 2, clips?} — beat-synchronously cuts or loops selected audio clips with equal-power crossfades to hit `target_seconds`",
            has_selection,
            cmd_remix
        ),
        cmd!(
            query "diagnostics.scan",
            "Scan Audio Diagnostics",
            [],
            None,
            "{clips?, click_sensitivity?: 60, max_click_samples?: 32, clip_threshold_db?: -0.5, silence_threshold_db?: -48, min_silence_ms?: 100} — scans selected (or all active) audio clips for impulsive clicks, flat-top clipped peaks, and silence intervals",
            always,
            cmd_scan
        ),
        cmd!(
            "diagnostics.declick",
            "Repair Clicks",
            [],
            None,
            "{clips?, sensitivity?: 60, max_click_samples?: 32} — detects and repairs impulsive clicks on selected audio clips via bidirectional AR-median interpolation",
            has_selection,
            cmd_declick
        ),
        cmd!(
            "diagnostics.declip",
            "Repair Clipping",
            [],
            None,
            "{clips?, threshold_db?: -0.5, ceiling_db?: -0.3} — reconstructs clipped flat-top peaks on selected audio clips via Cubic Hermite spline interpolation",
            has_selection,
            cmd_declip
        ),
        cmd!(
            "diagnostics.delete_silence",
            "Scan and Delete Silence",
            [],
            None,
            "{clips?, threshold_db?: -48, min_silence_ms?: 100, pad_before_ms?: 5, pad_after_ms?: 20} — scans and excises silent intervals from selected audio clips",
            has_selection,
            cmd_delete_silence
        ),
    ]
}

fn render_raw_clip_audio(e: &Engine, track: TrackId, id: ClipId, clip: &Clip) -> Vec<Vec<f32>> {
    let mut tmp = e.session().clone();
    if let Some(c) = tmp.find_clip_mut(id) {
        c.fade_in = Default::default();
        c.fade_out = Default::default();
        c.gain_db = 0.0;
        c.gain_env.clear();
    }
    if let Some(pl) = tmp.track_mut(track).and_then(|t| t.playlist_mut()) {
        pl.clips.retain(|c| c.id == id);
    }
    soundcraft_mix::render_clips(&tmp, track, clip.range())
}

fn replace_clip_audio(e: &mut Engine, track: TrackId, old_id: ClipId, old_clip: &Clip, suffix: &str, channels: Vec<Vec<f32>>) -> Option<ClipId> {
    let frames = channels.first().map_or(0, Vec::len);
    if frames == 0 {
        return None;
    }
    let sr = e.session().sample_rate.hz();
    let new_name = format!("{}-{suffix}", old_clip.name);
    let s = e.session_mut();
    let src = crate::io::add_source(s, &new_name, AudioBuffer { sample_rate: sr, channels }, None, FileFormat::Wav);
    let mut nc = old_clip.clone();
    nc.name = new_name;
    nc.content = ClipContent::Audio { source: src, offset: 0 };
    nc.length = i64::try_from(frames).unwrap_or(old_clip.length);
    nc.stretch = 1.0;
    nc.gain_db = 0.0;
    nc.gain_env.clear();
    nc.clamp_fades();
    if let Some(pl) = s.track_mut(track).and_then(|t| t.playlist_mut()) {
        pl.clips.retain(|c| c.id != old_id);
    }
    crate::edit::place_clip(s, track, nc);
    Some(old_id)
}

fn selected_audio_clips(e: &Engine, p: &Value, cmd_id: &str) -> Result<Vec<(TrackId, Clip)>> {
    let ids = clip_ids_param(e, p);
    let mut out = Vec::new();
    for id in ids {
        if let Some((track, clip)) = e.session().find_clip(id)
            && clip.is_audio()
        {
            out.push((track, clip.clone()));
        }
    }
    if out.is_empty() {
        return Err(bad(cmd_id, "select audio clips first"));
    }
    Ok(out)
}

fn cmd_remix(e: &mut Engine, p: &Value) -> Result<Value> {
    let target_sec = p
        .get("target_seconds")
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite() && *v > 0.0)
        .ok_or_else(|| bad("clip.remix", "`target_seconds` (> 0) required"))?;
    let sr = e.session().sample_rate;
    let srf = sr.as_f64() as f32;
    let target_frames = ((target_sec * sr.as_f64()).round() as usize).min(soundcraft_dsp::offline::MAX_OFFLINE_SAMPLES);
    if target_frames == 0 {
        return Err(bad("clip.remix", "`target_seconds` produces zero samples"));
    }

    let opts = RemixOptions {
        crossfade_ms: f64_or(p, "crossfade_ms", 60.0).clamp(5.0, 500.0) as f32,
        min_segment_beats: f64_or(p, "min_segment_beats", 4.0).clamp(1.0, 16.0) as usize,
        preserve_intro_beats: f64_or(p, "preserve_intro_beats", 2.0).clamp(1.0, 16.0) as usize,
        preserve_outro_beats: f64_or(p, "preserve_outro_beats", 2.0).clamp(1.0, 16.0) as usize,
    };

    let targets = selected_audio_clips(e, p, "clip.remix")?;
    let mut processed = 0_usize;
    let mut last_bpm = 120.0_f32;
    let mut total_edits = 0_usize;
    let mut last_beats = 0_usize;

    for (track, clip) in targets {
        let ch = e.session().track(track).map_or(1, |t| t.channels());
        crate::io::refuse_longer(e.session(), "clip.remix", &clip.name, "remix", clip.length, crate::io::render_limit(ch))?;
        crate::io::refuse_longer(
            e.session(),
            "clip.remix",
            &clip.name,
            "remix",
            i64::try_from(target_frames).unwrap_or(i64::MAX),
            crate::io::render_limit(ch),
        )?;
        let audio = render_raw_clip_audio(e, track, clip.id, &clip);
        let res = remix_to_duration(&audio, srf, target_sec, &opts);
        last_bpm = res.bpm;
        last_beats = res.beat_count;
        total_edits += res.edits.len();
        if replace_clip_audio(e, track, clip.id, &clip, "remix", res.channels).is_some() {
            processed += 1;
        }
    }

    Ok(json!({
        "processed": processed,
        "target_seconds": target_sec,
        "target_frames": target_frames,
        "bpm": last_bpm,
        "beats": last_beats,
        "edits": total_edits,
    }))
}

fn cmd_scan(e: &mut Engine, p: &Value) -> Result<Value> {
    let click_sens = f64_or(p, "click_sensitivity", 60.0).clamp(0.0, 100.0) as f32;
    let max_click = f64_or(p, "max_click_samples", 32.0).clamp(1.0, 256.0) as usize;
    let clip_thr_db = f64_or(p, "clip_threshold_db", -0.5).clamp(-24.0, 0.0) as f32;
    let silence_opts = SilenceScanOptions {
        threshold_db: f64_or(p, "silence_threshold_db", -48.0).clamp(-144.0, 0.0) as f32,
        min_silence_ms: f64_or(p, "min_silence_ms", 100.0).clamp(1.0, 10_000.0) as f32,
        pad_before_ms: 5.0,
        pad_after_ms: 20.0,
    };

    let s = e.session();
    let srf = s.sample_rate.as_f64() as f32;
    let explicit_ids = clip_ids_param(e, p);
    let mut clips_to_scan: Vec<(TrackId, Clip)> = Vec::new();
    if !explicit_ids.is_empty() {
        for id in explicit_ids {
            if let Some((t, c)) = s.find_clip(id)
                && c.is_audio()
            {
                clips_to_scan.push((t, c.clone()));
            }
        }
    } else {
        for track in &s.tracks {
            for clip in track.clips() {
                if clip.is_audio() {
                    clips_to_scan.push((track.id, clip.clone()));
                }
            }
        }
    }

    let mut total_clicks = 0_usize;
    let mut total_clipped = 0_usize;
    let mut total_silence = 0_usize;
    let mut per_clip = Vec::new();

    for (track_id, clip) in clips_to_scan {
        let ch_count = e.session().track(track_id).map_or(1, |t| t.channels());
        if clip.length <= 0 || usize::try_from(clip.length).unwrap_or(usize::MAX) > crate::io::render_limit(ch_count) {
            continue;
        }
        let audio = render_raw_clip_audio(e, track_id, clip.id, &clip);
        let clicks = detect_clicks(&audio, srf, click_sens, max_click);
        let clipped = detect_clipped_regions(&audio, clip_thr_db, 2);
        let silences = scan_silence_regions(&audio, srf, &silence_opts);

        total_clicks += clicks.len();
        total_clipped += clipped.len();
        total_silence += silences.len();
        per_clip.push(json!({
            "clip_id": clip.id.0,
            "name": clip.name,
            "clicks": clicks.len(),
            "clipped_peaks": clipped.len(),
            "silence_regions": silences.len(),
        }));
    }

    Ok(json!({
        "clicks": total_clicks,
        "clipped_peaks": total_clipped,
        "silence_regions": total_silence,
        "clips": per_clip,
    }))
}

fn cmd_declick(e: &mut Engine, p: &Value) -> Result<Value> {
    let sens = f64_or(p, "sensitivity", 60.0).clamp(0.0, 100.0) as f32;
    let max_click = f64_or(p, "max_click_samples", 32.0).clamp(1.0, 256.0) as usize;
    let srf = e.session().sample_rate.as_f64() as f32;
    let targets = selected_audio_clips(e, p, "diagnostics.declick")?;

    let mut processed = 0_usize;
    let mut repaired_total = 0_usize;
    for (track, clip) in targets {
        let ch = e.session().track(track).map_or(1, |t| t.channels());
        crate::io::refuse_longer(e.session(), "diagnostics.declick", &clip.name, "declick", clip.length, crate::io::render_limit(ch))?;
        let mut audio = render_raw_clip_audio(e, track, clip.id, &clip);
        let regions = repair_clicks(&mut audio, srf, sens, max_click);
        repaired_total += regions.len();
        if replace_clip_audio(e, track, clip.id, &clip, "declick", audio).is_some() {
            processed += 1;
        }
    }
    Ok(json!({"processed": processed, "clicks_repaired": repaired_total}))
}

fn cmd_declip(e: &mut Engine, p: &Value) -> Result<Value> {
    let thr_db = f64_or(p, "threshold_db", -0.5).clamp(-24.0, 0.0) as f32;
    let ceil_db = f64_or(p, "ceiling_db", -0.3).clamp(-24.0, 6.0) as f32;
    let targets = selected_audio_clips(e, p, "diagnostics.declip")?;

    let mut processed = 0_usize;
    let mut repaired_total = 0_usize;
    for (track, clip) in targets {
        let ch = e.session().track(track).map_or(1, |t| t.channels());
        crate::io::refuse_longer(e.session(), "diagnostics.declip", &clip.name, "declip", clip.length, crate::io::render_limit(ch))?;
        let mut audio = render_raw_clip_audio(e, track, clip.id, &clip);
        let regions = repair_clipping(&mut audio, thr_db, ceil_db);
        repaired_total += regions.len();
        if replace_clip_audio(e, track, clip.id, &clip, "declip", audio).is_some() {
            processed += 1;
        }
    }
    Ok(json!({"processed": processed, "clipped_regions_repaired": repaired_total}))
}

fn cmd_delete_silence(e: &mut Engine, p: &Value) -> Result<Value> {
    let opts = SilenceScanOptions {
        threshold_db: f64_or(p, "threshold_db", -48.0).clamp(-144.0, 0.0) as f32,
        min_silence_ms: f64_or(p, "min_silence_ms", 100.0).clamp(1.0, 10_000.0) as f32,
        pad_before_ms: f64_or(p, "pad_before_ms", 5.0).clamp(0.0, 2000.0) as f32,
        pad_after_ms: f64_or(p, "pad_after_ms", 20.0).clamp(0.0, 2000.0) as f32,
    };
    let srf = e.session().sample_rate.as_f64() as f32;
    let targets = selected_audio_clips(e, p, "diagnostics.delete_silence")?;

    let mut processed = 0_usize;
    let mut deleted_regions = 0_usize;
    let mut frames_removed = 0_usize;

    for (track, clip) in targets {
        let ch = e.session().track(track).map_or(1, |t| t.channels());
        crate::io::refuse_longer(e.session(), "diagnostics.delete_silence", &clip.name, "delete silence", clip.length, crate::io::render_limit(ch))?;
        let audio = render_raw_clip_audio(e, track, clip.id, &clip);
        let before_frames = audio.first().map_or(0, Vec::len);
        let (compacted, regions) = delete_silence_regions(&audio, srf, &opts);
        let after_frames = compacted.first().map_or(0, Vec::len);
        deleted_regions += regions.len();
        frames_removed += before_frames.saturating_sub(after_frames);
        if after_frames > 0 && replace_clip_audio(e, track, clip.id, &clip, "nosil", compacted).is_some() {
            processed += 1;
        }
    }
    Ok(json!({
        "processed": processed,
        "silence_regions_deleted": deleted_regions,
        "frames_removed": frames_removed,
    }))
}
