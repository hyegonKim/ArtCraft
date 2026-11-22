//! Logic Pro ↔ Final Cut Pro / FilmCraft FCPXML 1.9–1.13 audio interchange & Scoring to Picture.
//!
//! Bridges the video/audio post-production workflow between **FilmCraft** (Final Cut Pro
//! counterpart) and **SoundCraft** (Logic Pro / Pro Tools counterpart):
//! - **FCPXML Audio Roles Stem Import (`import_fcpxml_audio`)**: Parses FCPXML (`1.9`..=`1.13`)
//!   `<resources>`, `<sequence>` / `<spine>`, `<asset-clip>` / `<audio>` / `<video>` / `<clip>`,
//!   groups audio clips by FCPXML Audio Roles (`dialogue`, `effects`, `music`, plus subroles like
//!   `dialogue.dialogue-1`) onto dedicated [`TrackKind::Audio`] tracks, creates Role Summing Buses
//!   (`Dialogue Stem`, `Effects Stem`, `Music Stem`) with Aux return tracks, and imports clip gain,
//!   `<adjust-volume>` keyframe automation, `<adjust-panner>` pan, `<fadeIn>` / `<fadeOut>` curves,
//!   and timeline `<marker>` / `<chapter-marker>` scene markers.
//! - **FCPXML Audio Stem Export (`export_fcpxml_audio`)**: Exports a [`Session`]'s audio tracks,
//!   clips, fades, volume/pan automation, and markers back to FCPXML 1.11 (`<!DOCTYPE fcpxml>`) with
//!   `role="dialogue.*"`, `role="effects.*"`, or `role="music.*"` attributes.
//! - **Scoring to Picture (`smpte_lock_clips` & `beat_map_markers_to_tempo`)**:
//!   - [`smpte_lock_clips`]: Preserves the absolute SMPTE / sample positions of `time_locked` clips
//!     and scene markers across tempo changes while re-anchoring musical MIDI sequences.
//!   - [`beat_map_markers_to_tempo`]: Computes piecewise [`TempoEvent`]s so musical [`BarBeat`]
//!     downbeats/beats land sample-accurately on scene cut [`MemoryLocation`] markers.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use soundcraft_audio_io::FileFormat;
use soundcraft_time::{
    BarBeat, FrameRate, MAX_POSITION, SampleRate, Samples, TICKS_PER_QUARTER, TempoEvent,
    to_samples,
};

use crate::{
    AutoParam, BusId, ChannelFormat, Clip, Fade, FadeShape, MarkerId, MarkerKind, Route, Session,
    SessionError, Source, SourceId, TrackId, TrackKind,
};

/// Main Final Cut Pro / Logic Pro audio role category.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub enum AudioRoleCategory {
    Dialogue,
    Effects,
    Music,
    Video,
    Custom(String),
}

impl AudioRoleCategory {
    /// Parse main role from an FCPXML `role="dialogue.dialogue-1"` string.
    pub fn from_role_attr(role: &str) -> Self {
        let trimmed = role.trim();
        let main = trimmed.split('.').next().unwrap_or(trimmed).trim();
        match main.to_ascii_lowercase().as_str() {
            "dialogue" | "dialog" | "dx" => Self::Dialogue,
            "effects" | "sfx" | "fx" => Self::Effects,
            "music" | "mx" | "score" => Self::Music,
            "video" | "titles" => Self::Video,
            "" => Self::Dialogue,
            other => Self::Custom(other.to_string()),
        }
    }

    /// Canonical FCPXML main role identifier.
    pub fn fcpxml_id(&self) -> &str {
        match self {
            Self::Dialogue => "dialogue",
            Self::Effects => "effects",
            Self::Music => "music",
            Self::Video => "video",
            Self::Custom(s) => s.as_str(),
        }
    }

    /// Human-readable stem bus / aux name in SoundCraft.
    pub fn stem_bus_name(&self) -> String {
        match self {
            Self::Dialogue => "Dialogue Stem".to_string(),
            Self::Effects => "Effects Stem".to_string(),
            Self::Music => "Music Stem".to_string(),
            Self::Video => "Video Audio Stem".to_string(),
            Self::Custom(s) => format!("{s} Stem"),
        }
    }

    /// Track color associated with this role category.
    pub fn color(&self) -> [u8; 3] {
        match self {
            Self::Dialogue => [86, 132, 214], // Blue (Dialogue)
            Self::Effects => [230, 152, 62],  // Orange/Amber (Effects)
            Self::Music => [104, 186, 98],    // Green (Music)
            Self::Video => [150, 98, 206],    // Purple (Video)
            Self::Custom(_) => [94, 170, 214],
        }
    }
}

/// Summary returned by [`import_fcpxml_audio`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FcpxmlAudioImportSummary {
    /// Audio tracks created per `(main_role, subrole)`.
    pub role_tracks: Vec<(String, TrackId)>,
    /// Summing buses created per [`AudioRoleCategory`].
    pub stem_buses: Vec<(AudioRoleCategory, BusId, TrackId)>,
    /// Number of audio clips imported.
    pub clip_count: usize,
    /// Number of timeline markers imported.
    pub marker_count: usize,
}

#[derive(Debug, Clone)]
struct FcpxmlAsset {
    source_id: SourceId,
    name: String,
    channels: u16,
}

/// Returns `true` if `xml` looks like an FCPXML document (`<fcpxml ...>`).
pub fn is_fcpxml_document(xml: &str) -> bool {
    let head = xml.get(..4096).unwrap_or(xml);
    head.contains("<fcpxml")
}

/// Import FCPXML (`1.9`..=`1.13`) audio clips, Audio Roles (`dialogue`, `effects`, `music`),
/// volume/pan automation, fades, and scene markers into `session`.
pub fn import_fcpxml_audio(
    session: &mut Session,
    xml: &str,
) -> Result<FcpxmlAudioImportSummary, SessionError> {
    let root = parse_xml(xml).ok_or_else(|| SessionError::Format("invalid FCPXML XML".into()))?;
    let fcpxml = if root.tag == "fcpxml" {
        &root
    } else {
        root.find_descendant("fcpxml")
            .ok_or_else(|| SessionError::Format("missing <fcpxml> root element".into()))?
    };

    let sr = session.sample_rate;
    let mut format_fps: HashMap<String, FrameRate> = HashMap::new();
    let mut assets: HashMap<String, FcpxmlAsset> = HashMap::new();

    if let Some(resources) = fcpxml.child("resources") {
        for child in &resources.children {
            if child.tag == "format" {
                if let Some(id) = child.attr("id") {
                    let fps = child
                        .attr("frameDuration")
                        .and_then(parse_fcpxml_seconds)
                        .and_then(|sec| (sec > 0.0).then(|| frame_rate_from_duration(sec)))
                        .unwrap_or(session.frame_rate);
                    format_fps.insert(id.to_string(), fps);
                }
            } else if child.tag == "asset"
                && let Some(id) = child.attr("id")
            {
                let name = child
                    .attr("name")
                    .or_else(|| {
                        child
                            .child("media-rep")
                            .and_then(|m| m.attr("src"))
                            .and_then(|s| s.rsplit('/').next())
                    })
                    .unwrap_or(id)
                    .to_string();
                let src_url = child
                    .attr("src")
                    .or_else(|| child.child("media-rep").and_then(|m| m.attr("src")))
                    .unwrap_or(&name);
                let path = src_url.strip_prefix("file://").unwrap_or(src_url).to_string();
                let channels = child
                    .attr("audioChannels")
                    .and_then(|s| s.trim().parse::<u16>().ok())
                    .unwrap_or(2)
                    .clamp(1, 16);
                let asset_sr = child
                    .attr("audioRate")
                    .and_then(|s| s.trim().parse::<u32>().ok())
                    .unwrap_or(sr.hz());
                let dur_sec = child
                    .attr("duration")
                    .and_then(parse_fcpxml_seconds)
                    .unwrap_or(10.0)
                    .max(0.0);
                let frames = (dur_sec * f64::from(asset_sr)).round() as u64;
                let source_id = SourceId(session.alloc());
                session.sources.push(Source {
                    id: source_id,
                    name: name.clone(),
                    path,
                    channels,
                    frames,
                    sample_rate: asset_sr,
                    format: FileFormat::Wav,
                    time_reference: 0,
                    unsaved: false,
                });
                assets.insert(
                    id.to_string(),
                    FcpxmlAsset {
                        source_id,
                        name,
                        channels,
                    },
                );
            }
        }
    }

    let sequence = fcpxml
        .find_descendant("sequence")
        .ok_or_else(|| SessionError::Format("missing <sequence> in FCPXML".into()))?;
    if let Some(fmt_id) = sequence.attr("format")
        && let Some(&fps) = format_fps.get(fmt_id)
    {
        session.frame_rate = fps;
    }
    if let Some(tc_start_sec) = sequence.attr("tcStart").and_then(parse_fcpxml_seconds) {
        session.timecode_start = sr.samples(tc_start_sec);
    }

    let spine = sequence
        .child("spine")
        .ok_or_else(|| SessionError::Format("missing <spine> in FCPXML <sequence>".into()))?;

    let mut summary = FcpxmlAudioImportSummary::default();
    let mut stem_bus_map: BTreeMap<AudioRoleCategory, (BusId, TrackId)> = BTreeMap::new();
    let mut role_track_map: BTreeMap<String, TrackId> = BTreeMap::new();

    let mut spine_cursor_sec = 0.0_f64;
    for item in &spine.children {
        if item.tag == "gap" {
            let dur = item
                .attr("duration")
                .and_then(parse_fcpxml_seconds)
                .unwrap_or(0.0)
                .max(0.0);
            let gap_offset_timeline = item
                .attr("offset")
                .and_then(parse_fcpxml_seconds)
                .unwrap_or(spine_cursor_sec);
            let gap_start_media = item
                .attr("start")
                .and_then(parse_fcpxml_seconds)
                .unwrap_or(0.0);
            import_connected_items(
                session,
                item,
                gap_offset_timeline,
                gap_start_media,
                &assets,
                &mut stem_bus_map,
                &mut role_track_map,
                &mut summary,
            );
            spine_cursor_sec = gap_offset_timeline + dur;
        } else if is_clip_tag(&item.tag) {
            let timeline_offset = item
                .attr("offset")
                .and_then(parse_fcpxml_seconds)
                .unwrap_or(spine_cursor_sec);
            let dur = item
                .attr("duration")
                .and_then(parse_fcpxml_seconds)
                .unwrap_or(0.0)
                .max(0.0);
            import_clip_element(
                session,
                item,
                timeline_offset,
                &assets,
                &mut stem_bus_map,
                &mut role_track_map,
                &mut summary,
            );
            spine_cursor_sec = timeline_offset + dur;
        }
    }

    for (cat, (bus_id, aux_id)) in stem_bus_map {
        summary.stem_buses.push((cat, bus_id, aux_id));
    }
    for (role_key, track_id) in role_track_map {
        summary.role_tracks.push((role_key, track_id));
    }

    Ok(summary)
}

fn is_clip_tag(tag: &str) -> bool {
    matches!(tag, "asset-clip" | "audio" | "video" | "clip" | "ref-clip" | "sync-clip")
}

#[allow(clippy::too_many_arguments)]
fn import_connected_items(
    session: &mut Session,
    parent: &XmlNode,
    parent_timeline_offset_sec: f64,
    parent_media_start_sec: f64,
    assets: &HashMap<String, FcpxmlAsset>,
    stem_bus_map: &mut BTreeMap<AudioRoleCategory, (BusId, TrackId)>,
    role_track_map: &mut BTreeMap<String, TrackId>,
    summary: &mut FcpxmlAudioImportSummary,
) {
    for child in &parent.children {
        if is_clip_tag(&child.tag) {
            // In FCPXML, a connected clip's `offset` is relative to the parent clip's media `start`!
            let child_offset = child
                .attr("offset")
                .and_then(parse_fcpxml_seconds)
                .unwrap_or(parent_media_start_sec);
            let timeline_sec =
                (parent_timeline_offset_sec + (child_offset - parent_media_start_sec)).max(0.0);
            import_clip_element(
                session,
                child,
                timeline_sec,
                assets,
                stem_bus_map,
                role_track_map,
                summary,
            );
        } else if child.tag == "marker" || child.tag == "chapter-marker" {
            let m_start = child
                .attr("start")
                .and_then(parse_fcpxml_seconds)
                .unwrap_or(parent_media_start_sec);
            let timeline_sec =
                (parent_timeline_offset_sec + (m_start - parent_media_start_sec)).max(0.0);
            let sample_pos = session.sample_rate.samples(timeline_sec);
            let label = child.attr("value").unwrap_or("Scene Marker");
            let id = session.add_marker(label, MarkerKind::Marker, sample_pos, sample_pos);
            if let Some(note) = child.attr("note")
                && let Some(m) = session.markers.iter_mut().find(|m| m.id == id)
            {
                m.comments = note.to_string();
            }
            summary.marker_count += 1;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn import_clip_element(
    session: &mut Session,
    node: &XmlNode,
    timeline_offset_sec: f64,
    assets: &HashMap<String, FcpxmlAsset>,
    stem_bus_map: &mut BTreeMap<AudioRoleCategory, (BusId, TrackId)>,
    role_track_map: &mut BTreeMap<String, TrackId>,
    summary: &mut FcpxmlAudioImportSummary,
) {
    let media_start_sec = node
        .attr("start")
        .and_then(parse_fcpxml_seconds)
        .unwrap_or(0.0)
        .max(0.0);
    let dur_sec = node
        .attr("duration")
        .and_then(parse_fcpxml_seconds)
        .unwrap_or(0.0)
        .max(0.0);

    // Determine if this clip carries audio.
    let role_str = node
        .attr("audioRole")
        .or_else(|| node.attr("role"))
        .or_else(|| {
            node.child("audio-channel-source")
                .and_then(|a| a.attr("role"))
        })
        .unwrap_or("dialogue.dialogue-1");
    let category = AudioRoleCategory::from_role_attr(role_str);
    let has_audio = node.tag != "video" || node.attr("audioRole").is_some();

    if has_audio && dur_sec > 0.0 && category != AudioRoleCategory::Video {
        let sr = session.sample_rate;
        let start_samples = sr.samples(timeline_offset_sec).clamp(0, MAX_POSITION);
        let len_samples = sr.samples(dur_sec).clamp(1, MAX_POSITION);
        let offset_samples = sr.samples(media_start_sec).clamp(0, MAX_POSITION);

        let (source_id, default_name, channels) = node
            .attr("ref")
            .and_then(|r| assets.get(r))
            .map(|a| (a.source_id, a.name.clone(), a.channels))
            .unwrap_or_else(|| {
                let sid = SourceId(session.alloc());
                let name = node.attr("name").unwrap_or("Clip").to_string();
                session.sources.push(Source {
                    id: sid,
                    name: name.clone(),
                    path: name.clone(),
                    channels: 2,
                    frames: len_samples as u64,
                    sample_rate: sr.hz(),
                    format: FileFormat::Wav,
                    time_reference: 0,
                    unsaved: true,
                });
                (sid, name, 2)
            });

        let clip_name = node
            .attr("name")
            .filter(|s| !s.is_empty())
            .unwrap_or(&default_name)
            .to_string();
        let format = ChannelFormat::for_channels(usize::from(channels));

        // Ensure a Stem Summing Bus + Aux Track exists for this main role category.
        let (bus_id, _) = *stem_bus_map.entry(category.clone()).or_insert_with(|| {
            let stem_name = category.stem_bus_name();
            let b_id = session.add_bus(&stem_name, ChannelFormat::Stereo);
            let aux_id = session.add_track(TrackKind::Aux, ChannelFormat::Stereo, Some(&stem_name));
            if let Some(aux) = session.track_mut(aux_id) {
                aux.color = category.color();
                aux.mixer.input = Route::Bus(b_id);
                aux.mixer.output = Route::Main;
                aux.comments = format!("fcpxml.stem:{}", category.fcpxml_id());
            }
            (b_id, aux_id)
        });

        let normalized_role = normalize_role_key(role_str, &category);
        let track_id = *role_track_map
            .entry(normalized_role.clone())
            .or_insert_with(|| {
                let tid = session.add_track(TrackKind::Audio, format, Some(&normalized_role));
                if let Some(t) = session.track_mut(tid) {
                    t.color = category.color();
                    t.mixer.output = Route::Bus(bus_id);
                    t.comments = format!("fcpxml.role:{normalized_role}");
                }
                tid
            });

        let clip_id = session.new_clip_id();
        let mut clip = Clip::audio(
            clip_id,
            clip_name,
            source_id,
            offset_samples,
            start_samples,
            len_samples,
        );
        clip.color = Some(category.color());

        if node.attr("enabled") == Some("0") {
            clip.muted = true;
        }

        // Volume adjustment & keyframe animation (`<adjust-volume amount="-3dB">`)
        if let Some(vol_node) = node.child("adjust-volume").or_else(|| {
            node.child("audio-channel-source")
                .and_then(|acs| acs.child("adjust-volume"))
        }) {
            if let Some(amt) = vol_node.attr("amount").and_then(parse_fcpxml_db) {
                clip.gain_db = amt;
            }
            if let Some(param) = vol_node.child("param")
                && let Some(anim) = param.child("keyframeAnimation")
                && let Some(track) = session.track_mut(track_id)
            {
                let lane = track.lane_mut(&AutoParam::Volume);
                for kf in &anim.children {
                    if kf.tag == "keyframe"
                        && let Some(k_sec) = kf.attr("time").and_then(parse_fcpxml_seconds)
                        && let Some(val_db) = kf.attr("value").and_then(parse_fcpxml_db)
                    {
                        let rel_sec = (k_sec - media_start_sec).max(0.0);
                        let timeline_sample = sr.samples(timeline_offset_sec + rel_sec);
                        lane.set_point(timeline_sample, val_db);
                    }
                }
            }
        }

        // Pan adjustment (`<adjust-panner amount="-25">`)
        if let Some(pan_node) = node.child("adjust-panner")
            && let Some(amt_str) = pan_node.attr("amount")
            && let Ok(amt) = amt_str.trim().parse::<f32>()
            && let Some(track) = session.track_mut(track_id)
        {
            let pan_norm = (amt / 100.0).clamp(-1.0, 1.0);
            if let Some(first_pan) = track.mixer.pan.first_mut() {
                *first_pan = pan_norm;
            }
        }

        // Fades (`<fadeIn duration="..." type="..."/>`, `<fadeOut .../>`)
        if let Some(fi) = node.child("fadeIn")
            && let Some(fd_sec) = fi.attr("duration").and_then(parse_fcpxml_seconds)
        {
            clip.fade_in = Fade {
                len: sr.samples(fd_sec).clamp(0, len_samples),
                shape: parse_fcpxml_fade_shape(fi.attr("type").unwrap_or("easeInOut")),
                curvature: 0.0,
            };
        }
        if let Some(fo) = node.child("fadeOut")
            && let Some(fd_sec) = fo.attr("duration").and_then(parse_fcpxml_seconds)
        {
            clip.fade_out = Fade {
                len: sr.samples(fd_sec).clamp(0, len_samples),
                shape: parse_fcpxml_fade_shape(fo.attr("type").unwrap_or("easeInOut")),
                curvature: 0.0,
            };
        }

        clip.sanitize();
        if let Some(track) = session.track_mut(track_id)
            && let Some(pl) = track.playlist_mut()
        {
            pl.clips.push(clip);
            pl.sort();
            summary.clip_count += 1;
        }
    }

    // Recurse into connected clips and markers attached to this clip.
    import_connected_items(
        session,
        node,
        timeline_offset_sec,
        media_start_sec,
        assets,
        stem_bus_map,
        role_track_map,
        summary,
    );
}

fn normalize_role_key(raw: &str, category: &AudioRoleCategory) -> String {
    let trimmed = raw.trim();
    if trimmed.contains('.') {
        trimmed.to_string()
    } else {
        format!("{}.{}-1", category.fcpxml_id(), category.fcpxml_id())
    }
}

fn parse_fcpxml_fade_shape(t: &str) -> FadeShape {
    match t.trim().to_ascii_lowercase().as_str() {
        "linear" => FadeShape::Linear,
        "easein" => FadeShape::Exponential,
        "easeout" => FadeShape::Logarithmic,
        "easeinout" | "scurve" => FadeShape::SCurve,
        _ => FadeShape::EqualPower,
    }
}

fn fcpxml_fade_type(shape: FadeShape) -> &'static str {
    match shape {
        FadeShape::Linear => "linear",
        FadeShape::Exponential => "easeIn",
        FadeShape::Logarithmic => "easeOut",
        FadeShape::SCurve | FadeShape::EqualPower => "easeInOut",
    }
}

/// Export a SoundCraft [`Session`] back to an FCPXML 1.11 document with Audio Roles
/// (`dialogue`, `effects`, `music`), clip gain, volume automation keyframes, fades, and markers.
pub fn export_fcpxml_audio(session: &Session) -> String {
    let mut xml = String::with_capacity(4096);
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<!DOCTYPE fcpxml>\n");
    xml.push_str("<fcpxml version=\"1.11\">\n");
    xml.push_str("  <resources>\n");

    let (fps_num, fps_den) = session.frame_rate.rational();
    // frameDuration is 1 / fps = fps_den / fps_num seconds.
    let _ = writeln!(
        xml,
        "    <format id=\"r1\" name=\"FFVideoFormat1080p\" frameDuration=\"{fps_den}/{fps_num}s\" width=\"1920\" height=\"1080\"/>"
    );

    let sr = session.sample_rate;
    for src in &session.sources {
        let src_sr = src.sample_rate.max(1);
        let frames = src.frames.max(1);
        let _ = writeln!(
            xml,
            "    <asset id=\"a{}\" name=\"{}\" src=\"file://{}\" start=\"0s\" duration=\"{frames}/{src_sr}s\" hasAudio=\"1\" audioSources=\"1\" audioChannels=\"{}\" audioRate=\"{src_sr}\"/>",
            src.id.0,
            escape_xml(&src.name),
            escape_xml(&src.path),
            src.channels.max(1),
        );
    }
    xml.push_str("  </resources>\n");

    let total_samples = session.content_end().max(i64::from(sr.hz()));
    let tc_start_samples = session.timecode_start.max(0);
    let _ = writeln!(xml, "  <library>");
    let _ = writeln!(
        xml,
        "    <event name=\"{}\">",
        escape_xml(&session.name)
    );
    let _ = writeln!(
        xml,
        "      <project name=\"{}\">",
        escape_xml(&session.name)
    );
    let _ = writeln!(
        xml,
        "        <sequence format=\"r1\" duration=\"{total_samples}/{}s\" tcStart=\"{tc_start_samples}/{}s\" tcFormat=\"{}\" audioLayout=\"stereo\" audioRate=\"48k\">",
        sr.hz(),
        sr.hz(),
        if session.frame_rate.is_drop() { "DF" } else { "NDF" }
    );
    xml.push_str("          <spine>\n");
    let _ = writeln!(
        xml,
        "            <gap name=\"Timeline\" offset=\"0s\" start=\"0s\" duration=\"{total_samples}/{}s\">",
        sr.hz()
    );

    let mut lane_idx = 1_i32;
    for track in &session.tracks {
        if track.kind != TrackKind::Audio || track.hidden || track.inactive {
            continue;
        }
        let role = infer_track_fcpxml_role(session, track);
        let vol_lane = track.lane(&AutoParam::Volume);
        for clip in track.clips() {
            let Some(src_id) = clip.source() else {
                continue;
            };
            let offset_s = clip.start.max(0);
            let start_s = clip.source_offset().max(0);
            let dur_s = clip.length.max(1);
            let enabled_attr = if clip.muted || track.mixer.mute {
                " enabled=\"0\""
            } else {
                ""
            };
            let _ = writeln!(
                xml,
                "              <asset-clip name=\"{}\" lane=\"-{lane_idx}\" offset=\"{offset_s}/{}s\" ref=\"a{}\" start=\"{start_s}/{}s\" duration=\"{dur_s}/{}s\" role=\"{}\"{enabled_attr}>",
                escape_xml(&clip.name),
                sr.hz(),
                src_id.0,
                sr.hz(),
                sr.hz(),
                escape_xml(&role),
            );

            let net_db = clip.gain_db + track.mixer.volume_db;
            let clip_end = clip.end();
            let kfs_in_clip: Vec<_> = vol_lane
                .map(|l| {
                    l.points
                        .iter()
                        .filter(|p| p.at >= clip.start && p.at <= clip_end)
                        .collect()
                })
                .unwrap_or_default();

            if net_db.abs() > 1e-4 || !kfs_in_clip.is_empty() {
                let _ = writeln!(xml, "                <adjust-volume amount=\"{net_db:.2}dB\">");
                if !kfs_in_clip.is_empty() {
                    xml.push_str("                  <param name=\"volume\">\n");
                    xml.push_str("                    <keyframeAnimation>\n");
                    for kf in kfs_in_clip {
                        let rel = (kf.at - clip.start).max(0);
                        let media_pos = start_s.saturating_add(rel);
                        let _ = writeln!(
                            xml,
                            "                      <keyframe time=\"{media_pos}/{}s\" value=\"{:.2}dB\"/>",
                            sr.hz(),
                            kf.value
                        );
                    }
                    xml.push_str("                    </keyframeAnimation>\n");
                    xml.push_str("                  </param>\n");
                }
                xml.push_str("                </adjust-volume>\n");
            }

            if clip.fade_in.len > 0 {
                let _ = writeln!(
                    xml,
                    "                <fadeIn duration=\"{}/{}s\" type=\"{}\"/>",
                    clip.fade_in.len,
                    sr.hz(),
                    fcpxml_fade_type(clip.fade_in.shape)
                );
            }
            if clip.fade_out.len > 0 {
                let _ = writeln!(
                    xml,
                    "                <fadeOut duration=\"{}/{}s\" type=\"{}\"/>",
                    clip.fade_out.len,
                    sr.hz(),
                    fcpxml_fade_type(clip.fade_out.shape)
                );
            }

            xml.push_str("              </asset-clip>\n");
        }
        lane_idx += 1;
    }

    for marker in &session.markers {
        let start_s = marker.start.max(0);
        let dur_s = (marker.end - marker.start).max(1);
        let note_attr = if marker.comments.is_empty() {
            String::new()
        } else {
            format!(" note=\"{}\"", escape_xml(&marker.comments))
        };
        let _ = writeln!(
            xml,
            "              <marker start=\"{start_s}/{}s\" duration=\"{dur_s}/{}s\" value=\"{}\"{note_attr}/>",
            sr.hz(),
            sr.hz(),
            escape_xml(&marker.name)
        );
    }

    xml.push_str("            </gap>\n");
    xml.push_str("          </spine>\n");
    xml.push_str("        </sequence>\n");
    xml.push_str("      </project>\n");
    xml.push_str("    </event>\n");
    xml.push_str("  </library>\n");
    xml.push_str("</fcpxml>\n");
    xml
}

fn infer_track_fcpxml_role(session: &Session, track: &crate::Track) -> String {
    if let Some(rest) = track.comments.strip_prefix("fcpxml.role:") {
        let r = rest.trim();
        if !r.is_empty() {
            return r.to_string();
        }
    }
    if track.name.contains('.') {
        return track.name.clone();
    }
    let lower = track.name.to_ascii_lowercase();
    if lower.contains("dialogue") || lower.contains("dialog") || lower.contains("dx") || lower.contains("vox") {
        return "dialogue.dialogue-1".to_string();
    }
    if lower.contains("effect") || lower.contains("sfx") || lower.contains("foley") || lower.contains("amb") {
        return "effects.effects-1".to_string();
    }
    if lower.contains("music") || lower.contains("mx") || lower.contains("score") {
        return "music.music-1".to_string();
    }
    if let Route::Bus(bid) = track.mixer.output
        && let Some(bus) = session.bus(bid)
    {
        let cat = AudioRoleCategory::from_role_attr(&bus.name);
        return format!("{}.{}-1", cat.fcpxml_id(), cat.fcpxml_id());
    }
    "dialogue.dialogue-1".to_string()
}

// ============================================================================
// Scoring to Picture: SMPTE Lock & Beat Mapping
// ============================================================================

/// Logic Pro-style **SMPTE Lock** across a session tempo change:
///
/// - Clips with `clip.time_locked == true` keep their exact absolute sample/SMPTE start position
///   when the tempo at tick 0 is updated to `new_bpm`.
/// - Clips on musical (`ticks_timebase == true`) tracks that are **not** `time_locked` move with
///   their musical bar/beat tick position under the new tempo.
/// - Returns the number of clips whose timeline position was updated or preserved under SMPTE lock.
pub fn smpte_lock_clips(session: &mut Session, new_bpm: f64) -> Result<usize, SessionError> {
    let sr = session.sample_rate;
    let old_tempo = session.tempo.clone();

    // Capture the musical tick of every clip before changing the tempo map.
    let mut musical_positions: HashMap<u64, (i64, i64)> = HashMap::new();
    for track in &session.tracks {
        if track.ticks_timebase {
            for pl in &track.playlists {
                for clip in &pl.clips {
                    if !clip.time_locked {
                        let start_tick = old_tempo.samples_to_ticks(clip.start, sr);
                        let end_tick = old_tempo.samples_to_ticks(clip.end(), sr);
                        musical_positions.insert(clip.id.0, (start_tick, (end_tick - start_tick).max(1)));
                    }
                }
            }
        }
    }

    session
        .tempo
        .set_tempo(0, new_bpm)
        .map_err(|e| SessionError::Invalid(e.to_string()))?;

    let mut affected = 0_usize;
    for track in &mut session.tracks {
        for pl in &mut track.playlists {
            for clip in &mut pl.clips {
                if clip.time_locked {
                    // SMPTE-locked: `clip.start` in samples (and therefore SMPTE timecode) stays
                    // fixed!
                    affected += 1;
                } else if let Some(&(start_tick, len_ticks)) = musical_positions.get(&clip.id.0) {
                    let new_start = session.tempo.tick_to_samples(start_tick, sr);
                    let new_end = session.tempo.tick_to_samples(start_tick + len_ticks, sr);
                    clip.start = new_start.clamp(0, MAX_POSITION);
                    clip.length = (new_end - new_start).clamp(1, MAX_POSITION);
                    affected += 1;
                }
            }
            pl.sort();
        }
    }

    Ok(affected)
}

/// Logic Pro-style **Beat Mapping** (`beat_map_markers_to_tempo`):
///
/// Given a list of `(BarBeat, MarkerId)` pairs, computes and applies piecewise constant
/// [`TempoEvent`]s on `session.tempo` so that each musical [`BarBeat`] lands sample-accurately on
/// the corresponding scene hit [`MemoryLocation`].
pub fn beat_map_markers_to_tempo(
    session: &mut Session,
    anchors: &[(BarBeat, MarkerId)],
) -> Result<Vec<TempoEvent>, SessionError> {
    if anchors.is_empty() {
        return Ok(session.tempo.tempos().to_vec());
    }

    let sr: SampleRate = session.sample_rate;
    // Resolve `(tick, sample_pos)` for each anchor, plus an implicit origin `(0, 0)`.
    let mut resolved: Vec<(i64, Samples)> = Vec::with_capacity(anchors.len() + 1);
    resolved.push((0, 0));

    for &(bb, marker_id) in anchors {
        let tick = session.tempo.tick_at_bar_beat(bb);
        if tick <= 0 {
            return Err(SessionError::Invalid(
                "beat mapping target BarBeat must be after bar 1 beat 1".into(),
            ));
        }
        let marker = session
            .marker(marker_id)
            .ok_or_else(|| SessionError::Invalid(format!("unknown marker id {}", marker_id.0)))?;
        resolved.push((tick, marker.start));
    }

    resolved.sort_by_key(|&(t, _)| t);
    resolved.dedup_by_key(|p| p.0);

    let mut events = Vec::new();
    for pair in resolved.windows(2) {
        let (t0, s0) = pair[0];
        let (t1, s1) = pair[1];
        let dt_ticks = t1 - t0;
        let ds_samples = s1 - s0;
        if dt_ticks <= 0 || ds_samples <= 0 {
            return Err(SessionError::Invalid(
                "beat map markers must advance monotonically in both musical ticks and sample time"
                    .into(),
            ));
        }
        let delta_sec = sr.seconds(ds_samples);
        let quarters = dt_ticks as f64 / TICKS_PER_QUARTER as f64;
        let bpm = (quarters * 60.0 / delta_sec).clamp(5.0, 1000.0);
        session
            .tempo
            .set_tempo(t0, bpm)
            .map_err(|e| SessionError::Invalid(e.to_string()))?;
        events.push(TempoEvent { tick: t0, bpm });
    }

    Ok(events)
}

// ============================================================================
// Rational time & XML helpers
// ============================================================================

fn parse_fcpxml_seconds(s: &str) -> Option<f64> {
    let s = s.trim().strip_suffix('s').unwrap_or(s.trim()).trim();
    if let Some((num_s, den_s)) = s.split_once('/') {
        let num = num_s.trim().parse::<f64>().ok()?;
        let den = den_s.trim().parse::<f64>().ok()?;
        if den.abs() < 1e-12 {
            return None;
        }
        Some(num / den)
    } else {
        s.parse::<f64>().ok()
    }
}

fn parse_fcpxml_db(s: &str) -> Option<f32> {
    let s = s.trim();
    let num = s
        .strip_suffix("dB")
        .or_else(|| s.strip_suffix("db"))
        .unwrap_or(s)
        .trim();
    if num == "-inf" || num == "-INF" {
        return Some(-144.0);
    }
    num.parse::<f32>().ok().map(|v| v.clamp(-144.0, 12.0))
}

fn frame_rate_from_duration(frame_dur_sec: f64) -> FrameRate {
    let fps = 1.0 / frame_dur_sec;
    if (fps - 23.976).abs() < 0.01 {
        FrameRate::Fps23_976
    } else if (fps - 24.0).abs() < 0.01 {
        FrameRate::Fps24
    } else if (fps - 25.0).abs() < 0.01 {
        FrameRate::Fps25
    } else if (fps - 29.97).abs() < 0.015 {
        FrameRate::Fps29_97
    } else if (fps - 30.0).abs() < 0.01 {
        FrameRate::Fps30
    } else if (fps - 50.0).abs() < 0.02 {
        FrameRate::Fps50
    } else if (fps - 59.94).abs() < 0.025 {
        FrameRate::Fps59_94
    } else if (fps - 60.0).abs() < 0.02 {
        FrameRate::Fps60
    } else {
        let _ = to_samples(fps);
        FrameRate::Fps24
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

#[derive(Debug, Clone, Default)]
struct XmlNode {
    tag: String,
    attrs: Vec<(String, String)>,
    children: Vec<XmlNode>,
}

impl XmlNode {
    fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
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
    let mut root = XmlNode {
        tag: "__root__".to_string(),
        ..Default::default()
    };
    let mut stack: Vec<XmlNode> = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0_usize;

    while i < bytes.len() {
        let Some(rel_lt) = input[i..].find('<') else {
            break;
        };
        let lt = i + rel_lt;
        let rest = &input[lt..];
        if rest.starts_with("<!--") {
            let end = rest.find("-->").map(|e| lt + e + 3).unwrap_or(bytes.len());
            i = end;
            continue;
        }
        if rest.starts_with("<?") || rest.starts_with("<!") {
            let end = rest.find('>').map(|e| lt + e + 1).unwrap_or(bytes.len());
            i = end;
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
            let close_tag = closing.trim();
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
            continue;
        }

        let self_closing = inside.ends_with('/');
        let tag_body = if self_closing {
            inside[..inside.len() - 1].trim()
        } else {
            inside
        };
        let (tag_name, attrs) = parse_tag_and_attrs(tag_body);
        if tag_name.is_empty() {
            continue;
        }
        let node = XmlNode {
            tag: tag_name,
            attrs,
            children: Vec::new(),
        };
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

    while let Some(node) = stack.pop() {
        if let Some(parent) = stack.last_mut() {
            parent.children.push(node);
        } else {
            root.children.push(node);
        }
    }

    root.children.into_iter().next()
}

fn parse_tag_and_attrs(body: &str) -> (String, Vec<(String, String)>) {
    let mut chars = body.char_indices().peekable();
    let mut tag_end = body.len();
    for (idx, ch) in chars.by_ref() {
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

fn unescape_xml(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;
    use soundcraft_midi::{Note, Sequence};

    #[test]
    fn fcpxml_audio_roles_import_to_stems_and_roundtrip_export() {
        let fcpxml = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE fcpxml>
<fcpxml version="1.11">
  <resources>
    <format id="r1" name="FFVideoFormat1080p24" frameDuration="100/2400s" width="1920" height="1080"/>
    <asset id="a1" name="Boom_Mic" src="file:///media/boom.wav" duration="240/24s" audioChannels="1" audioRate="48000"/>
    <asset id="a2" name="Door_Slam" src="file:///media/slam.wav" duration="48/24s" audioChannels="2" audioRate="48000"/>
    <asset id="a3" name="Main_Score" src="file:///media/score.wav" duration="480/24s" audioChannels="2" audioRate="48000"/>
  </resources>
  <library>
    <event name="Reel 1">
      <project name="Scene 4 Mix">
        <sequence format="r1" duration="480/24s" tcStart="0s">
          <spine>
            <gap name="Gap" offset="0s" start="3600s" duration="480/24s">
              <asset-clip name="Hero Line" offset="3601s" ref="a1" start="0s" duration="96/24s" role="dialogue.dialogue-1">
                <adjust-volume amount="-3.0dB">
                  <param name="volume">
                    <keyframeAnimation>
                      <keyframe time="0s" value="-3.0dB"/>
                      <keyframe time="48/24s" value="0.0dB"/>
                    </keyframeAnimation>
                  </param>
                </adjust-volume>
                <fadeIn duration="12/24s" type="easeInOut"/>
                <fadeOut duration="12/24s" type="linear"/>
              </asset-clip>
              <asset-clip name="Door FX" offset="3605s" ref="a2" start="0s" duration="48/24s" role="effects.foley"/>
              <asset-clip name="Theme Cue" offset="3600s" ref="a3" start="0s" duration="240/24s" role="music.score"/>
              <marker start="3605s" duration="1/24s" value="Door Hit" note="Sync foley impact"/>
            </gap>
          </spine>
        </sequence>
      </project>
    </event>
  </library>
</fcpxml>"#;

        let mut session = Session::new("Scene 4", SampleRate::HZ_48000);
        let summary = import_fcpxml_audio(&mut session, fcpxml).unwrap();

        assert_eq!(summary.clip_count, 3);
        assert_eq!(summary.marker_count, 1);
        assert_eq!(summary.stem_buses.len(), 3); // Dialogue, Effects, Music stems
        assert_eq!(session.frame_rate, FrameRate::Fps24);

        let dx_track = session
            .tracks
            .iter()
            .find(|t| t.name == "dialogue.dialogue-1")
            .unwrap();
        assert_eq!(dx_track.clips().len(), 1);
        let dx_clip = &dx_track.clips()[0];
        assert_eq!(dx_clip.start, 48_000); // 1.0s at 48 kHz
        assert_eq!(dx_clip.length, 48_000 * 4); // 4.0s
        assert!((dx_clip.gain_db - (-3.0)).abs() < 1e-4);
        assert_eq!(dx_clip.fade_in.len, 24_000);
        assert_eq!(dx_clip.fade_in.shape, FadeShape::SCurve);
        assert_eq!(dx_clip.fade_out.shape, FadeShape::Linear);

        let vol_lane = dx_track.lane(&AutoParam::Volume).unwrap();
        assert_eq!(vol_lane.points.len(), 2);

        // Round-trip export to FCPXML 1.11
        let exported = export_fcpxml_audio(&session);
        assert!(is_fcpxml_document(&exported));
        assert!(exported.contains("role=\"dialogue.dialogue-1\""));
        assert!(exported.contains("role=\"effects.foley\""));
        assert!(exported.contains("role=\"music.score\""));
        assert!(exported.contains("value=\"Door Hit\""));
    }

    #[test]
    fn smpte_lock_and_beat_mapping_align_musical_grid_to_picture_markers() {
        let mut session = Session::new("Film Score", SampleRate::HZ_48000);

        // Add a time-locked SFX hit at exactly 3.0 seconds (144_000 samples) and a musical MIDI cue at Bar 2 Beat 1.
        let sfx_track = session.add_track(TrackKind::Audio, ChannelFormat::Stereo, Some("SFX"));
        let sfx_clip_id = session.new_clip_id();
        let mut sfx_clip = Clip::audio(sfx_clip_id, "Boom Hit", SourceId(1), 0, 144_000, 24_000);
        sfx_clip.time_locked = true;
        session
            .track_mut(sfx_track)
            .unwrap()
            .playlist_mut()
            .unwrap()
            .clips
            .push(sfx_clip);

        let midi_track = session.add_track(TrackKind::Midi, ChannelFormat::Mono, Some("Strings"));
        // At default 120 BPM 4/4, Bar 2 Beat 1 is at 2.0 seconds (96_000 samples).
        let midi_clip_id = session.new_clip_id();
        let mut seq = Sequence::default();
        seq.notes.push(Note {
            pitch: 60,
            velocity: 100,
            release_velocity: 0,
            start: 0,
            length: 960,
            channel: 0,
        });
        let midi_clip = Clip::midi(midi_clip_id, "Downbeat Chord", 96_000, 48_000, seq);
        session
            .track_mut(midi_track)
            .unwrap()
            .playlist_mut()
            .unwrap()
            .clips
            .push(midi_clip);

        // Change tempo from 120 BPM to 60 BPM with SMPTE Lock:
        // - The time-locked SFX hit stays at sample 144_000 (3.0s)
        // - The musical MIDI clip at Bar 2 Beat 1 moves from 96_000 (2.0s) to 192_000 (4.0s)
        let count = smpte_lock_clips(&mut session, 60.0).unwrap();
        assert_eq!(count, 2);
        assert_eq!(session.find_clip(sfx_clip_id).unwrap().1.start, 144_000);
        assert_eq!(session.find_clip(midi_clip_id).unwrap().1.start, 192_000);

        // Now add a scene cut marker at 3.2 seconds (153_600 samples) and beat-map Bar 3 Beat 1 to it!
        let cut_marker = session.add_marker("Scene Cut", MarkerKind::Marker, 153_600, 153_600);
        let events = beat_map_markers_to_tempo(
            &mut session,
            &[(
                BarBeat {
                    bar: 3,
                    beat: 1,
                    tick: 0,
                },
                cut_marker,
            )],
        )
        .unwrap();
        assert_eq!(events.len(), 1);
        // 2 bars of 4/4 = 8 beats in 3.2 seconds => 150.0 BPM!
        assert!((events[0].bpm - 150.0).abs() < 1e-6);
        assert_eq!(
            session.tempo.samples_at_bar_beat(
                BarBeat {
                    bar: 3,
                    beat: 1,
                    tick: 0
                },
                session.sample_rate
            ),
            153_600
        );
    }
}
