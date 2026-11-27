//! Multitrack XML (`.sesx` version `1.9`) session import and export.
//!
//! Parses and writes `<sesx version="1.9">` XML multitrack documents:
//! - `<session appBuild="..." appVersion="..." sampleRate="..." bits="..." audioChannelType="..." duration="..." name="...">`
//! - `<files>` (`<file id="..." name="..." absolutePath="..." relativePath="..." sampleRate="..." duration="..." channelType="..."/>`)
//! - `<tracks>` (`<audioTrack>` with volume, pan, mute, solo, nested `<trackParameters>` / `<trackAudioParameters>`)
//! - `<audioClip clipAutoCrossfade="..." sourceInPoint="..." sourceOutPoint="..." startPoint="..." endPoint="...">`
//!   with `<fadeIn>` / `<fadeOut>` curves and clip gain/mute
//! - `<markers>` (`<marker id="..." name="..." startPoint="..." endPoint="..." type="cue"/>`)

use std::collections::HashMap;
use std::fmt::Write as _;

use soundcraft_time::{MAX_POSITION, SampleRate};

use crate::{
    BitDepthSetting, ChannelFormat, Clip, ClipContent, Fade, FadeShape, FileFormat, MarkerKind, Session, SessionError, Source, SourceId, TrackId,
    TrackKind,
};

/// Canonical `.sesx` schema version written on export.
pub const SESX_VERSION: &str = "1.9";

/// Metadata for a `<file>` entry referenced by a `.sesx` document so the engine can resolve
/// relative or absolute media paths into the in-memory audio pool when available on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SesxFileRef {
    pub file_id: String,
    pub source_id: SourceId,
    pub name: String,
    pub absolute_path: String,
    pub relative_path: String,
    pub channels: u16,
    pub sample_rate: u32,
}

/// Summary of a `.sesx` multitrack import operation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SesxImportSummary {
    pub version: String,
    pub sample_rate: u32,
    pub bit_depth: u16,
    pub files: Vec<SesxFileRef>,
    pub tracks: Vec<TrackId>,
    pub clip_count: usize,
    pub marker_count: usize,
}

/// Returns `true` when `xml` starts with or contains a `<sesx` document root.
pub fn is_sesx_document(xml: &str) -> bool {
    let head = xml.get(..4096).unwrap_or(xml);
    head.contains("<sesx")
}

/// Import a `<sesx version="1.9">` XML multitrack session into `session`.
pub fn import_sesx(session: &mut Session, xml: &str) -> Result<SesxImportSummary, SessionError> {
    let root = parse_xml(xml).ok_or_else(|| SessionError::Format("invalid .sesx XML".into()))?;
    let sesx = if root.tag == "sesx" {
        &root
    } else {
        root.find_descendant("sesx").ok_or_else(|| SessionError::Format("missing <sesx> root element".into()))?
    };

    let version = sesx.attr("version").unwrap_or(SESX_VERSION).to_string();
    let sess_node = sesx.child("session").ok_or_else(|| SessionError::Format("missing <session> inside <sesx>".into()))?;

    apply_session_header(session, sess_node)?;

    let mut summary =
        SesxImportSummary { version, sample_rate: session.sample_rate.hz(), bit_depth: bit_depth_bits(session.bit_depth), ..Default::default() };

    let files_map = import_sesx_files(session, sesx, sess_node, &mut summary);
    import_sesx_tracks(session, sess_node, &files_map, &mut summary);
    import_sesx_markers(session, sesx, sess_node, &mut summary);

    Ok(summary)
}

fn apply_session_header(session: &mut Session, sess_node: &XmlNode) -> Result<(), SessionError> {
    if let Some(sr_str) = sess_node.attr("sampleRate")
        && let Some(sr_val) = parse_u32_flexible(sr_str)
        && sr_val > 0
    {
        session.sample_rate = SampleRate::new(sr_val).map_err(|err| SessionError::Format(format!("invalid .sesx sampleRate `{sr_str}`: {err}")))?;
    }
    if let Some(bits_str) = sess_node.attr("bits").or_else(|| sess_node.attr("bitDepth")) {
        session.bit_depth = match bits_str.trim() {
            "16" => BitDepthSetting::Int16,
            "32" | "32f" => BitDepthSetting::Float32,
            _ => BitDepthSetting::Int24,
        };
    }
    let maybe_name = sess_node
        .attr("name")
        .filter(|s| !s.trim().is_empty())
        .or_else(|| sess_node.child("name").map(|n| n.text.as_str()).filter(|s| !s.trim().is_empty()));
    if let Some(name) = maybe_name {
        session.name = name.trim().to_string();
    }
    Ok(())
}

fn import_sesx_files(session: &mut Session, sesx: &XmlNode, sess_node: &XmlNode, summary: &mut SesxImportSummary) -> HashMap<String, SesxFileRef> {
    let mut map = HashMap::new();
    let files_parent = sesx.child("files").or_else(|| sess_node.child("files"));
    let Some(files_node) = files_parent else {
        return map;
    };

    let default_sr = session.sample_rate.hz();
    for child in &files_node.children {
        if child.tag != "file" {
            continue;
        }
        let Some(id) = child.attr("id").map(str::trim).filter(|s| !s.is_empty()) else {
            continue;
        };
        let abs_path = child.attr("absolutePath").unwrap_or("").trim().to_string();
        let rel_path = child.attr("relativePath").unwrap_or("").trim().to_string();
        let chosen_path = if !abs_path.is_empty() {
            abs_path.clone()
        } else if !rel_path.is_empty() {
            rel_path.clone()
        } else {
            child.attr("name").unwrap_or(id).trim().to_string()
        };
        let fallback_name = chosen_path.rsplit(['/', '\\']).next().filter(|s| !s.is_empty()).unwrap_or(id);
        let name = child.attr("name").map(str::trim).filter(|s| !s.is_empty()).unwrap_or(fallback_name).to_string();
        let file_sr = child.attr("sampleRate").and_then(parse_u32_flexible).filter(|&v| v > 0).unwrap_or(default_sr);
        let channels =
            parse_channel_count(child.attr("channelType").or_else(|| child.attr("audioChannelType")).or_else(|| child.attr("channels")), 2);
        let frames =
            child.attr("duration").or_else(|| child.attr("sampleCount")).and_then(parse_i64_flexible).unwrap_or(i64::from(file_sr) * 10).max(0)
                as u64;
        let format = infer_file_format(&chosen_path, child.attr("mediaHandler"));
        let source_id = SourceId(session.alloc());
        session.sources.push(Source {
            id: source_id,
            name: name.clone(),
            path: chosen_path,
            channels,
            frames,
            sample_rate: file_sr,
            format,
            time_reference: 0,
            unsaved: false,
        });
        let info = SesxFileRef {
            file_id: id.to_string(),
            source_id,
            name,
            absolute_path: abs_path,
            relative_path: rel_path,
            channels,
            sample_rate: file_sr,
        };
        summary.files.push(info.clone());
        map.insert(id.to_string(), info);
    }
    map
}

fn import_sesx_tracks(session: &mut Session, sess_node: &XmlNode, files_map: &HashMap<String, SesxFileRef>, summary: &mut SesxImportSummary) {
    let tracks_container = sess_node.child("tracks").unwrap_or(sess_node);
    let sess_channel_default = parse_channel_count(sess_node.attr("audioChannelType"), 2);

    for node in &tracks_container.children {
        if node.tag == "audioTrack" {
            import_one_audio_track(session, node, files_map, sess_channel_default, summary);
        } else if node.tag == "masterTrack" {
            apply_master_track_params(session, node);
        }
    }
}

fn import_one_audio_track(
    session: &mut Session,
    node: &XmlNode,
    files_map: &HashMap<String, SesxFileRef>,
    default_channels: u16,
    summary: &mut SesxImportSummary,
) {
    let name = extract_track_name(node);
    let ch_attr = node
        .attr("audioChannelType")
        .or_else(|| node.attr("channelType"))
        .or_else(|| node.child("trackAudioParameters").and_then(|p| p.attr("audioChannelType")));
    let channels = parse_channel_count(ch_attr, default_channels);
    let ch_format = ChannelFormat::for_channels(usize::from(channels));
    let track_id = session.add_track(TrackKind::Audio, ch_format, Some(&name));

    let (vol_db, pan_norm, muted, solo) = extract_track_mixer(node);
    if let Some(track) = session.track_mut(track_id) {
        track.mixer.volume_db = vol_db;
        if let Some(first_pan) = track.mixer.pan.first_mut() {
            *first_pan = pan_norm;
        }
        track.mixer.mute = muted;
        track.mixer.solo = solo;
        if node.attr("visible") == Some("false") {
            track.hidden = true;
        }
    }

    let mut auto_xfade_clips: Vec<bool> = Vec::new();
    for child in &node.children {
        if child.tag == "audioClip"
            && let Some((clip, auto_xfade)) = parse_audio_clip(session, child, files_map)
            && let Some(track) = session.track_mut(track_id)
            && let Some(pl) = track.playlist_mut()
        {
            pl.clips.push(clip);
            auto_xfade_clips.push(auto_xfade);
            summary.clip_count += 1;
        }
    }

    if let Some(track) = session.track_mut(track_id)
        && let Some(pl) = track.playlist_mut()
    {
        pl.sort();
        if auto_xfade_clips.iter().any(|&x| x) {
            apply_overlap_auto_crossfades(&mut pl.clips);
        }
    }
    summary.tracks.push(track_id);
}

fn apply_master_track_params(session: &mut Session, node: &XmlNode) {
    let (vol_db, pan_norm, muted, _) = extract_track_mixer(node);
    if let Some(master) = session.tracks.iter_mut().find(|t| t.kind == TrackKind::Master) {
        master.mixer.volume_db = vol_db;
        if let Some(first_pan) = master.mixer.pan.first_mut() {
            *first_pan = pan_norm;
        }
        master.mixer.mute = muted;
    }
}

fn extract_track_name(node: &XmlNode) -> String {
    if let Some(n) = node.attr("name").map(str::trim).filter(|s| !s.is_empty()) {
        return n.to_string();
    }
    if let Some(tp) = node.child("trackParameters") {
        if let Some(n) = tp.attr("name").map(str::trim).filter(|s| !s.is_empty()) {
            return n.to_string();
        }
        if let Some(name_el) = tp.child("name")
            && !name_el.text.trim().is_empty()
        {
            return name_el.text.trim().to_string();
        }
    }
    let idx = node.attr("index").unwrap_or("1");
    format!("Audio {idx}")
}

fn extract_track_mixer(node: &XmlNode) -> (f32, f32, bool, bool) {
    let mut vol_db = 0.0_f32;
    let mut pan_norm = 0.0_f32;
    let mut muted = parse_bool_attr(node.attr("mute")).unwrap_or(false);
    let mut solo = parse_bool_attr(node.attr("solo")).unwrap_or(false);

    if let Some(v_str) = node.attr("volumeDb")
        && let Some(db) = parse_f32_flexible(v_str)
    {
        vol_db = db.clamp(-144.0, 12.0);
    } else if let Some(v_str) = node.attr("volume") {
        vol_db = parse_gain_or_db(v_str);
    }

    if let Some(p_str) = node.attr("pan")
        && let Some(p) = parse_f32_flexible(p_str)
    {
        pan_norm = if p.abs() > 1.0 { (p / 100.0).clamp(-1.0, 1.0) } else { p.clamp(-1.0, 1.0) };
    }

    if let Some(tap) = node.child("trackAudioParameters") {
        if let Some(m) = parse_bool_attr(tap.attr("mute")) {
            muted = m;
        }
        if let Some(s) = parse_bool_attr(tap.attr("solo")) {
            solo = s;
        }
        apply_component_params(tap, &mut vol_db, &mut pan_norm, &mut muted);
    }
    apply_component_params(node, &mut vol_db, &mut pan_norm, &mut muted);

    (vol_db, pan_norm, muted, solo)
}

fn apply_component_params(parent: &XmlNode, vol_db: &mut f32, pan_norm: &mut f32, muted: &mut bool) {
    for child in &parent.children {
        if child.tag != "component" {
            continue;
        }
        for param in &child.children {
            if param.tag != "parameter" {
                continue;
            }
            let pname = param.attr("name").unwrap_or("").trim().to_ascii_lowercase();
            let Some(val_str) = param.attr("parameterValue").or_else(|| param.attr("value")) else {
                continue;
            };
            match pname.as_str() {
                "volume" | "static gain" | "gain" => {
                    *vol_db = parse_gain_or_db(val_str);
                }
                "pan" => {
                    if let Some(p) = parse_f32_flexible(val_str) {
                        *pan_norm = (p / 100.0).clamp(-1.0, 1.0);
                    }
                }
                "mute" => {
                    if let Some(b) = parse_bool_attr(Some(val_str)) {
                        *muted = b;
                    }
                }
                _ => {}
            }
        }
    }
}

fn parse_audio_clip(session: &mut Session, node: &XmlNode, files_map: &HashMap<String, SesxFileRef>) -> Option<(Clip, bool)> {
    let sr = session.sample_rate.hz();
    let start_pt = node.attr("startPoint").and_then(|s| parse_samples_at_sr(s, sr, sr)).unwrap_or(0).clamp(0, MAX_POSITION);
    let src_ref = node.attr("fileID").and_then(|id| files_map.get(id.trim()));
    let file_sr = src_ref.map_or(sr, |r| r.sample_rate);

    let src_in = node.attr("sourceInPoint").and_then(|s| parse_samples_at_sr(s, file_sr, sr)).unwrap_or(0).clamp(0, MAX_POSITION);
    let src_out = node.attr("sourceOutPoint").and_then(|s| parse_samples_at_sr(s, file_sr, sr));

    let end_pt = node
        .attr("endPoint")
        .and_then(|s| parse_samples_at_sr(s, sr, sr))
        .or_else(|| src_out.map(|so| start_pt.saturating_add((so - src_in).max(1))))
        .unwrap_or(start_pt + i64::from(sr));

    let length = (end_pt - start_pt).clamp(1, MAX_POSITION);
    let (source_id, default_name) = match src_ref {
        Some(info) => (info.source_id, info.name.clone()),
        None => {
            let sid = SourceId(session.alloc());
            let fallback = node.attr("name").unwrap_or("Clip").trim().to_string();
            session.sources.push(Source {
                id: sid,
                name: fallback.clone(),
                path: fallback.clone(),
                channels: 2,
                frames: (src_in.saturating_add(length)) as u64,
                sample_rate: sr,
                format: FileFormat::Wav,
                time_reference: 0,
                unsaved: true,
            });
            (sid, fallback)
        }
    };

    let clip_name = node.attr("name").map(str::trim).filter(|s| !s.is_empty()).unwrap_or(&default_name).to_string();

    let clip_id = session.new_clip_id();
    let mut clip = Clip::audio(clip_id, clip_name, source_id, src_in, start_pt, length);
    clip.time_locked = parse_bool_attr(node.attr("lockedInTime")).unwrap_or(false);
    clip.muted = parse_bool_attr(node.attr("mute")).unwrap_or(false) || parse_bool_attr(node.attr("enabled")).is_some_and(|en| !en);

    let mut clip_vol_db = 0.0_f32;
    let mut clip_pan = 0.0_f32;
    let mut clip_muted = clip.muted;
    if let Some(g_str) = node.attr("gainDb")
        && let Some(db) = parse_f32_flexible(g_str)
    {
        clip_vol_db = db.clamp(-144.0, 24.0);
    } else if let Some(v_str) = node.attr("volume") {
        clip_vol_db = parse_gain_or_db(v_str);
    }
    apply_component_params(node, &mut clip_vol_db, &mut clip_pan, &mut clip_muted);
    clip.gain_db = clip_vol_db;
    clip.muted = clip_muted;

    if let Some(fi) = node.child("fadeIn") {
        clip.fade_in = parse_sesx_fade(fi, length);
    }
    if let Some(fo) = node.child("fadeOut") {
        clip.fade_out = parse_sesx_fade(fo, length);
    }
    clip.sanitize();

    let auto_xfade = parse_bool_attr(node.attr("clipAutoCrossfade")).unwrap_or(true);
    Some((clip, auto_xfade))
}

fn parse_sesx_fade(node: &XmlNode, clip_len: i64) -> Fade {
    let start = node.attr("startPoint").and_then(parse_i64_flexible).unwrap_or(0);
    let len = if let Some(end) = node.attr("endPoint").and_then(parse_i64_flexible) {
        (end - start).max(0)
    } else {
        node.attr("duration").and_then(parse_i64_flexible).unwrap_or(0).max(0)
    };
    let shape = match node.attr("shape").or_else(|| node.attr("type")).unwrap_or("cosine").trim().to_ascii_lowercase().as_str() {
        "linear" => FadeShape::Linear,
        "logarithmic" | "log" | "easeout" => FadeShape::Logarithmic,
        "exponential" | "exp" | "easein" => FadeShape::Exponential,
        "scurve" | "easeinout" => FadeShape::SCurve,
        _ => FadeShape::EqualPower,
    };
    Fade { len: len.clamp(0, clip_len), shape, curvature: 0.0 }
}

/// When `clipAutoCrossfade="true"` and two adjacent clips overlap on the same track without
/// explicit fades, set equal-power fade-out/fade-in over the overlap span.
fn apply_overlap_auto_crossfades(clips: &mut [Clip]) {
    for idx in 1..clips.len() {
        let (left_slice, right_slice) = clips.split_at_mut(idx);
        let (Some(left), Some(right)) = (left_slice.last_mut(), right_slice.first_mut()) else {
            continue;
        };
        let overlap = left.end() - right.start;
        if overlap > 0 {
            let xfade_len = overlap.min(left.length).min(right.length);
            if left.fade_out.len == 0 {
                left.fade_out = Fade { len: xfade_len, shape: FadeShape::EqualPower, curvature: 0.0 };
            }
            if right.fade_in.len == 0 {
                right.fade_in = Fade { len: xfade_len, shape: FadeShape::EqualPower, curvature: 0.0 };
            }
        }
    }
}

fn import_sesx_markers(session: &mut Session, sesx: &XmlNode, sess_node: &XmlNode, summary: &mut SesxImportSummary) {
    for container in [sess_node.child("markers"), sesx.child("markers")].into_iter().flatten() {
        for m in &container.children {
            if m.tag != "marker" {
                continue;
            }
            let start = m.attr("startPoint").and_then(parse_i64_flexible).unwrap_or(0).clamp(0, MAX_POSITION);
            let end = m.attr("endPoint").and_then(parse_i64_flexible).unwrap_or(start).clamp(start, MAX_POSITION);
            let name = m.attr("name").map(str::trim).filter(|s| !s.is_empty()).unwrap_or("Cue Marker");
            let kind = if end > start || m.attr("type") == Some("subclip") { MarkerKind::Selection } else { MarkerKind::Marker };
            let marker_id = session.add_marker(name, kind, start, end);
            if let Some(comment) = m.attr("comment").or_else(|| m.attr("description"))
                && let Some(slot) = session.markers.iter_mut().find(|mk| mk.id == marker_id)
            {
                slot.comments = comment.to_string();
            }
            summary.marker_count += 1;
        }
    }
}

/// Export `session` to an Audition-compatible `<sesx version="1.9">` XML multitrack document.
pub fn export_sesx(session: &Session) -> String {
    let mut xml = String::with_capacity(4096);
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\" ?>\n");
    xml.push_str("<!DOCTYPE sesx>\n");
    let _ = writeln!(xml, "<sesx version=\"{SESX_VERSION}\">");

    let sr = session.sample_rate.hz();
    let bits = bit_depth_bits(session.bit_depth);
    let duration = session.content_end().max(i64::from(sr));
    let master_ch_type = session.tracks.iter().find(|t| t.kind == TrackKind::Master).map_or("stereo", |t| channel_type_str(t.channels()));

    let _ = writeln!(
        xml,
        "  <session appBuild=\"27.0.0.1\" appVersion=\"27.0\" audioChannelType=\"{master_ch_type}\" bits=\"{bits}\" duration=\"{duration}\" name=\"{}\" sampleRate=\"{sr}\">",
        escape_xml(&session.name)
    );
    let _ = writeln!(xml, "    <name>{}</name>", escape_xml(&session.name));

    write_sesx_tracks(&mut xml, session);
    write_sesx_markers(&mut xml, session);

    xml.push_str("  </session>\n");
    write_sesx_files(&mut xml, session, sr);
    xml.push_str("</sesx>\n");
    xml
}

fn write_sesx_tracks(xml: &mut String, session: &Session) {
    xml.push_str("    <tracks>\n");
    let mut track_idx = 1_usize;
    let mut clip_uid = 0_usize;

    for track in &session.tracks {
        if track.kind == TrackKind::Audio {
            let ch_type = channel_type_str(track.channels());
            let lin_vol = db_to_linear_sesx(track.mixer.volume_db);
            let pan_norm = track.mixer.pan.first().copied().unwrap_or(0.0).clamp(-1.0, 1.0);
            let pan_pct = pan_norm * 100.0;
            let visible = !track.hidden;
            let _ = writeln!(
                xml,
                "      <audioTrack audioChannelType=\"{ch_type}\" id=\"{}\" index=\"{track_idx}\" mute=\"{}\" name=\"{}\" pan=\"{pan_norm:.4}\" select=\"false\" solo=\"{}\" visible=\"{visible}\" volume=\"{lin_vol:.6}\">",
                10000 + track.id.0,
                track.mixer.mute,
                escape_xml(&track.name),
                track.mixer.solo
            );
            let _ = writeln!(xml, "        <trackParameters><name>{}</name></trackParameters>", escape_xml(&track.name));
            let _ = writeln!(
                xml,
                "        <trackAudioParameters audioChannelType=\"{ch_type}\" mute=\"{}\" solo=\"{}\">",
                track.mixer.mute, track.mixer.solo
            );
            let _ = writeln!(xml, "          <component componentID=\"Audition.Fader\" name=\"volume\">");
            let _ = writeln!(xml, "            <parameter index=\"0\" name=\"volume\" parameterValue=\"{lin_vol:.6}\"/>");
            let _ = writeln!(xml, "            <parameter index=\"1\" name=\"pan\" parameterValue=\"{pan_pct:.2}\"/>");
            xml.push_str("          </component>\n");
            xml.push_str("        </trackAudioParameters>\n");

            for clip in track.clips() {
                if let ClipContent::Audio { source, offset } = clip.content {
                    write_sesx_clip(xml, clip, source, offset, clip_uid);
                    clip_uid += 1;
                }
            }
            xml.push_str("      </audioTrack>\n");
            track_idx += 1;
        } else if track.kind == TrackKind::Master {
            let ch_type = channel_type_str(track.channels());
            let lin_vol = db_to_linear_sesx(track.mixer.volume_db);
            let pan_norm = track.mixer.pan.first().copied().unwrap_or(0.0).clamp(-1.0, 1.0);
            let _ = writeln!(
                xml,
                "      <masterTrack audioChannelType=\"{ch_type}\" id=\"90000\" index=\"{track_idx}\" mute=\"{}\" name=\"{}\" pan=\"{pan_norm:.4}\" volume=\"{lin_vol:.6}\"/>",
                track.mixer.mute,
                escape_xml(&track.name)
            );
        }
    }
    xml.push_str("    </tracks>\n");
}

fn write_sesx_clip(xml: &mut String, clip: &Clip, source: SourceId, offset: i64, uid: usize) {
    let start_pt = clip.start.max(0);
    let end_pt = clip.end().max(start_pt + 1);
    let src_in = offset.max(0);
    let src_out = src_in.saturating_add(clip.length.max(1));
    let lin_gain = db_to_linear_sesx(clip.gain_db);
    let _ = writeln!(
        xml,
        "        <audioClip clipAutoCrossfade=\"true\" endPoint=\"{end_pt}\" fileID=\"{}\" id=\"{uid}\" lockedInTime=\"{}\" mute=\"{}\" name=\"{}\" sourceInPoint=\"{src_in}\" sourceOutPoint=\"{src_out}\" startPoint=\"{start_pt}\" volume=\"{lin_gain:.6}\" zOrder=\"0\">",
        source.0,
        clip.time_locked,
        clip.muted,
        escape_xml(&clip.name)
    );
    if clip.fade_in.len > 0 {
        let _ = writeln!(
            xml,
            "          <fadeIn crossFadeLinkType=\"linkedAsymmetric\" endPoint=\"{}\" shape=\"{}\" startPoint=\"0\"/>",
            clip.fade_in.len,
            sesx_fade_shape_str(clip.fade_in.shape)
        );
    }
    if clip.fade_out.len > 0 {
        let fo_start = (clip.length - clip.fade_out.len).max(0);
        let _ = writeln!(
            xml,
            "          <fadeOut crossFadeLinkType=\"linkedAsymmetric\" endPoint=\"{}\" shape=\"{}\" startPoint=\"{fo_start}\"/>",
            clip.length,
            sesx_fade_shape_str(clip.fade_out.shape)
        );
    }
    if clip.gain_db.abs() > 1e-4 {
        xml.push_str("          <component componentID=\"Audition.ClipGain\" name=\"ClipGain\">\n");
        let _ = writeln!(xml, "            <parameter index=\"0\" name=\"static gain\" parameterValue=\"{lin_gain:.6}\"/>");
        xml.push_str("          </component>\n");
    }
    xml.push_str("        </audioClip>\n");
}

fn write_sesx_markers(xml: &mut String, session: &Session) {
    if session.markers.is_empty() {
        return;
    }
    xml.push_str("    <markers>\n");
    for m in &session.markers {
        let start_pt = m.start.max(0);
        let end_pt = m.end.max(start_pt);
        let mtype = if end_pt > start_pt { "subclip" } else { "cue" };
        if m.comments.is_empty() {
            let _ = writeln!(
                xml,
                "      <marker endPoint=\"{end_pt}\" id=\"{}\" name=\"{}\" startPoint=\"{start_pt}\" type=\"{mtype}\"/>",
                m.id.0,
                escape_xml(&m.name)
            );
        } else {
            let _ = writeln!(
                xml,
                "      <marker comment=\"{}\" endPoint=\"{end_pt}\" id=\"{}\" name=\"{}\" startPoint=\"{start_pt}\" type=\"{mtype}\"/>",
                escape_xml(&m.comments),
                m.id.0,
                escape_xml(&m.name)
            );
        }
    }
    xml.push_str("    </markers>\n");
}

fn write_sesx_files(xml: &mut String, session: &Session, default_sr: u32) {
    xml.push_str("  <files>\n");
    for src in &session.sources {
        let sr = if src.sample_rate > 0 { src.sample_rate } else { default_sr };
        let ch_type = channel_type_str(usize::from(src.channels.max(1)));
        let handler = match src.format {
            FileFormat::Aiff => "AmioAiff",
            FileFormat::Flac => "AmioFlac",
            FileFormat::Mp3 => "AmioMp3",
            FileFormat::Ogg => "AmioOgg",
            FileFormat::Wav => "AmioWav",
            FileFormat::Aac | FileFormat::Alac | FileFormat::Caf => "AmioCoreAudio",
            FileFormat::Other => "AmioWav",
        };
        let rel = src.path.rsplit(['/', '\\']).next().unwrap_or(&src.name);
        let _ = writeln!(
            xml,
            "    <file absolutePath=\"{}\" channelType=\"{ch_type}\" duration=\"{}\" id=\"{}\" mediaHandler=\"{handler}\" name=\"{}\" relativePath=\"Imported Files/{}\" sampleRate=\"{sr}\"/>",
            escape_xml(&src.path),
            src.frames,
            src.id.0,
            escape_xml(&src.name),
            escape_xml(rel)
        );
    }
    xml.push_str("  </files>\n");
}

fn bit_depth_bits(bd: BitDepthSetting) -> u16 {
    match bd {
        BitDepthSetting::Int16 => 16,
        BitDepthSetting::Int24 => 24,
        BitDepthSetting::Float32 => 32,
    }
}

fn channel_type_str(channels: usize) -> &'static str {
    match channels {
        0 | 1 => "mono",
        2 => "stereo",
        _ => "fivePointOne",
    }
}

fn parse_channel_count(raw: Option<&str>, default: u16) -> u16 {
    let Some(s) = raw else {
        return default;
    };
    match s.trim().to_ascii_lowercase().as_str() {
        "mono" | "1" => 1,
        "stereo" | "2" => 2,
        "fivepointone" | "5.1" | "6" => 6,
        other => other.parse::<u16>().ok().map_or(default, |c| c.clamp(1, 16)),
    }
}

fn infer_file_format(path: &str, handler: Option<&str>) -> FileFormat {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".aif") || lower.ends_with(".aiff") {
        return FileFormat::Aiff;
    }
    if lower.ends_with(".flac") {
        return FileFormat::Flac;
    }
    if lower.ends_with(".mp3") {
        return FileFormat::Mp3;
    }
    if lower.ends_with(".ogg") {
        return FileFormat::Ogg;
    }
    if let Some(h) = handler {
        let hl = h.to_ascii_lowercase();
        if hl.contains("aiff") {
            return FileFormat::Aiff;
        }
        if hl.contains("flac") {
            return FileFormat::Flac;
        }
        if hl.contains("mp3") {
            return FileFormat::Mp3;
        }
        if hl.contains("ogg") {
            return FileFormat::Ogg;
        }
    }
    FileFormat::Wav
}

fn sesx_fade_shape_str(shape: FadeShape) -> &'static str {
    match shape {
        FadeShape::Linear => "linear",
        FadeShape::EqualPower => "cosine",
        FadeShape::SCurve => "scurve",
        FadeShape::Exponential => "exponential",
        FadeShape::Logarithmic => "logarithmic",
    }
}

fn db_to_linear_sesx(db: f32) -> f32 {
    if !db.is_finite() || db <= -144.0 { 0.0 } else { 10.0_f32.powf(db.clamp(-144.0, 24.0) / 20.0) }
}

fn parse_gain_or_db(raw: &str) -> f32 {
    let trimmed = raw.trim();
    if let Some(stripped) = trimmed.strip_suffix("dB").or_else(|| trimmed.strip_suffix("db")) {
        return stripped.trim().parse::<f32>().ok().map_or(0.0, |v| v.clamp(-144.0, 24.0));
    }
    let Some(lin) = parse_f32_flexible(trimmed) else {
        return 0.0;
    };
    if lin <= 0.0 { -144.0 } else { (20.0 * lin.log10()).clamp(-144.0, 24.0) }
}

fn parse_samples_at_sr(raw: &str, from_sr: u32, to_sr: u32) -> Option<i64> {
    let val = parse_i64_flexible(raw)?;
    if from_sr == 0 || from_sr == to_sr { Some(val) } else { Some(((val as f64) * f64::from(to_sr) / f64::from(from_sr)).round() as i64) }
}

fn parse_u32_flexible(raw: &str) -> Option<u32> {
    let s = raw.trim();
    s.parse::<u32>().ok().or_else(|| s.parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0).map(|v| v.round() as u32))
}

fn parse_i64_flexible(raw: &str) -> Option<i64> {
    let s = raw.trim();
    s.parse::<i64>().ok().or_else(|| s.parse::<f64>().ok().filter(|v| v.is_finite()).map(|v| v.round() as i64))
}

fn parse_f32_flexible(raw: &str) -> Option<f32> {
    raw.trim().parse::<f32>().ok().filter(|v| v.is_finite())
}

fn parse_bool_attr(raw: Option<&str>) -> Option<bool> {
    match raw?.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    }
}

fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

fn unescape_xml(s: &str) -> String {
    s.replace("&quot;", "\"").replace("&apos;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

#[derive(Debug, Clone, Default)]
struct XmlNode {
    tag: String,
    attrs: Vec<(String, String)>,
    text: String,
    children: Vec<XmlNode>,
}

impl XmlNode {
    fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    fn child(&self, tag: &str) -> Option<&XmlNode> {
        self.children.iter().find(|c| c.tag == tag)
    }

    fn find_descendant(&self, tag: &str) -> Option<&XmlNode> {
        if self.tag == tag {
            return Some(self);
        }
        for c in &self.children {
            if let Some(found) = c.find_descendant(tag) {
                return Some(found);
            }
        }
        None
    }
}

fn parse_xml(input: &str) -> Option<XmlNode> {
    let mut root = XmlNode { tag: "__root__".to_string(), ..Default::default() };
    let mut stack: Vec<XmlNode> = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0_usize;

    while i < bytes.len() {
        let Some(rel_lt) = input[i..].find('<') else {
            break;
        };
        let lt = i + rel_lt;
        if lt > i
            && let Some(top) = stack.last_mut()
        {
            let txt = unescape_xml(&input[i..lt]);
            if !txt.trim().is_empty() {
                top.text.push_str(txt.trim());
            }
        }
        let rest = &input[lt..];
        if rest.starts_with("<!--") {
            i = rest.find("-->").map_or(bytes.len(), |e| lt + e + 3);
            continue;
        }
        if rest.starts_with("<?") || rest.starts_with("<!") {
            i = rest.find('>').map_or(bytes.len(), |e| lt + e + 1);
            continue;
        }
        let Some(rel_gt) = rest.find('>') else {
            break;
        };
        let inside = rest[1..rel_gt].trim();
        i = lt + rel_gt + 1;
        if inside.is_empty() {
            continue;
        }
        if let Some(closing) = inside.strip_prefix('/') {
            pop_xml_node(&mut root, &mut stack, closing.trim());
            continue;
        }
        push_xml_node(&mut root, &mut stack, inside);
    }

    while let Some(node) = stack.pop() {
        if let Some(parent) = stack.last_mut() {
            parent.children.push(node);
        } else {
            root.children.push(node);
        }
    }
    root.children.into_iter().next()
}

fn pop_xml_node(root: &mut XmlNode, stack: &mut Vec<XmlNode>, close_tag: &str) {
    if let Some(node) = stack.pop() {
        if node.tag == close_tag || stack.is_empty() {
            if let Some(parent) = stack.last_mut() {
                parent.children.push(node);
            } else {
                root.children.push(node);
            }
        } else if let Some(parent) = stack.last_mut() {
            parent.children.push(node);
        }
    }
}

fn push_xml_node(root: &mut XmlNode, stack: &mut Vec<XmlNode>, inside: &str) {
    let self_closing = inside.ends_with('/');
    let tag_body = if self_closing { inside[..inside.len() - 1].trim() } else { inside };
    let (tag_name, attrs) = parse_tag_and_attrs(tag_body);
    if tag_name.is_empty() {
        return;
    }
    let node = XmlNode { tag: tag_name, attrs, text: String::new(), children: Vec::new() };
    if self_closing {
        if let Some(parent) = stack.last_mut() {
            parent.children.push(node);
        } else {
            root.children.push(node);
        }
    } else if stack.len() < 128 {
        stack.push(node);
    }
}

fn parse_tag_and_attrs(body: &str) -> (String, Vec<(String, String)>) {
    let mut tag_end = body.len();
    for (idx, ch) in body.char_indices() {
        if ch.is_whitespace() {
            tag_end = idx;
            break;
        }
    }
    let tag = body[..tag_end].trim().to_string();
    let mut attrs = Vec::new();
    let mut rest = body[tag_end..].trim();

    while !rest.is_empty() {
        let Some(eq_idx) = rest.find('=') else {
            break;
        };
        let key = rest[..eq_idx].trim().to_string();
        rest = rest[eq_idx + 1..].trim_start();
        let Some(quote) = rest.chars().next() else {
            break;
        };
        if quote != '"' && quote != '\'' {
            break;
        }
        rest = &rest[quote.len_utf8()..];
        let Some(end_q) = rest.find(quote) else {
            break;
        };
        let val = unescape_xml(&rest[..end_q]);
        attrs.push((key, val));
        rest = rest[end_q + quote.len_utf8()..].trim_start();
    }
    (tag, attrs)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn sesx_v1_9_imports_tracks_clips_fades_mixer_markers_and_roundtrips() {
        let sesx_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="no" ?>
<!DOCTYPE sesx>
<sesx version="1.9">
  <session appBuild="27.0.0.12" appVersion="27.0" audioChannelType="stereo" bits="24" duration="480000" name="Podcast Episode 12" sampleRate="48000">
    <tracks>
      <audioTrack id="10001" index="1" select="true" visible="true">
        <trackParameters>
          <name>Host Mic</name>
        </trackParameters>
        <trackAudioParameters audioChannelType="mono" mute="false" solo="true">
          <component componentID="Audition.Fader" name="volume">
            <parameter index="0" name="volume" parameterValue="0.500000"/>
            <parameter index="1" name="pan" parameterValue="-25.00"/>
          </component>
        </trackAudioParameters>
        <audioClip clipAutoCrossfade="true" endPoint="144000" fileID="1" id="0" lockedInTime="true" name="Intro Line" sourceInPoint="24000" sourceOutPoint="120000" startPoint="48000" zOrder="0">
          <fadeIn crossFadeLinkType="linkedAsymmetric" endPoint="4800" shape="cosine" startPoint="0"/>
          <fadeOut crossFadeLinkType="linkedAsymmetric" endPoint="96000" shape="linear" startPoint="84000"/>
        </audioClip>
      </audioTrack>
      <audioTrack audioChannelType="stereo" id="10002" index="2" mute="true" name="Music Bed" pan="0.4" solo="false" volume="0.707946">
        <audioClip clipAutoCrossfade="true" endPoint="192000" fileID="2" id="1" name="Bed Part A" sourceInPoint="0" sourceOutPoint="192000" startPoint="0"/>
        <audioClip clipAutoCrossfade="true" endPoint="336000" fileID="2" id="2" name="Bed Part B" sourceInPoint="48000" sourceOutPoint="216000" startPoint="168000"/>
      </audioTrack>
    </tracks>
    <markers>
      <marker comment="Ad break cue" endPoint="96000" id="1" name="Sponsor Read" startPoint="96000" type="cue"/>
      <marker endPoint="240000" id="2" name="Highlight Range" startPoint="192000" type="subclip"/>
    </markers>
  </session>
  <files>
    <file absolutePath="/audio/host.wav" channelType="mono" duration="240000" id="1" mediaHandler="AmioWav" name="host.wav" relativePath="Imported Files/host.wav" sampleRate="48000"/>
    <file absolutePath="/audio/music_bed.flac" channelType="stereo" duration="480000" id="2" mediaHandler="AmioFlac" name="music_bed.flac" relativePath="Imported Files/music_bed.flac" sampleRate="48000"/>
  </files>
</sesx>"#;

        assert!(is_sesx_document(sesx_xml));
        let mut session = Session::new("Untitled", SampleRate::HZ_44100);
        let summary = import_sesx(&mut session, sesx_xml).unwrap();

        assert_eq!(summary.version, "1.9");
        assert_eq!(session.name, "Podcast Episode 12");
        assert_eq!(session.sample_rate.hz(), 48_000);
        assert_eq!(session.bit_depth, BitDepthSetting::Int24);
        assert_eq!(summary.files.len(), 2);
        assert_eq!(summary.tracks.len(), 2);
        assert_eq!(summary.clip_count, 3);
        assert_eq!(summary.marker_count, 2);

        let host = session.tracks.iter().find(|t| t.name == "Host Mic").unwrap();
        assert_eq!(host.channels(), 1);
        assert!(host.mixer.solo);
        assert!(!host.mixer.mute);
        assert!((host.mixer.volume_db - (-6.0206)).abs() < 0.05);
        assert!((host.mixer.pan[0] - (-0.25)).abs() < 1e-3);

        let intro = &host.clips()[0];
        assert_eq!(intro.start, 48_000);
        assert_eq!(intro.length, 96_000);
        assert!(intro.time_locked);
        assert_eq!(intro.fade_in.len, 4_800);
        assert_eq!(intro.fade_in.shape, FadeShape::EqualPower);
        assert_eq!(intro.fade_out.len, 12_000);
        assert_eq!(intro.fade_out.shape, FadeShape::Linear);
        assert_eq!(intro.content, ClipContent::Audio { source: summary.files[0].source_id, offset: 24_000 });

        let bed = session.tracks.iter().find(|t| t.name == "Music Bed").unwrap();
        assert!(bed.mixer.mute);
        assert!((bed.mixer.pan[0] - 0.4).abs() < 1e-3);
        assert_eq!(bed.clips()[0].fade_out.len, 24_000);
        assert_eq!(bed.clips()[1].fade_in.len, 24_000);

        verify_sesx_roundtrip(&session);
    }

    fn verify_sesx_roundtrip(session: &Session) {
        let exported = export_sesx(session);
        assert!(is_sesx_document(&exported));
        assert!(exported.contains("<sesx version=\"1.9\">"));

        let mut roundtripped = Session::new("Blank", SampleRate::HZ_44100);
        let rt_summary = import_sesx(&mut roundtripped, &exported).unwrap();
        assert_eq!(roundtripped.name, "Podcast Episode 12");
        assert_eq!(rt_summary.tracks.len(), 2);
        assert_eq!(rt_summary.clip_count, 3);
        assert_eq!(rt_summary.marker_count, 2);
        let rt_host = roundtripped.tracks.iter().find(|t| t.name == "Host Mic").unwrap();
        assert!(rt_host.mixer.solo);
        assert!((rt_host.mixer.pan[0] - (-0.25)).abs() < 1e-3);
    }
}
