//! Animated presentation export: GIF89a, H.264/MP4 (ISO BMFF) and VP9/WebM.
//!
//! Renders intra-slide object build animations and inter-slide transitions via
//! [`deckcraft_render::render_sequence`] and muxes the frame sequence into a playable media file
//! without external tools. All caller-supplied parameters are validated against fixed bounds.

use std::io::Cursor;

use deckcraft_model::Presentation;
use deckcraft_render::{RenderedSequence, SequenceOptions, render_sequence, sequence_dimensions};
use serde_json::Value;

use crate::cmd::bad;
use crate::gif_export::SIZE_PRESETS;
use crate::{EngineError, Result};

pub const MIN_DIM: u32 = 16;
pub const MAX_DIM: u32 = 3840;
pub const MIN_FPS: u32 = 1;
pub const MAX_FPS: u32 = 60;
pub const DEFAULT_VIDEO_WIDTH: u32 = 1280;
pub const DEFAULT_GIF_WIDTH: u32 = 640;
pub const DEFAULT_VIDEO_FPS: u32 = 24;
pub const DEFAULT_GIF_FPS: u32 = 15;
pub const DEFAULT_HOLD_SECONDS: f64 = 2.0;
pub const MIN_HOLD_SECONDS: f64 = 0.1;
pub const MAX_HOLD_SECONDS: f64 = 600.0;
pub const MAX_SLIDES: usize = 500;
/// Cap on unique rendered frames (`render_sequence` collapses identical hold ticks).
pub const MAX_SEQUENCE_FRAMES: usize = 2_400;
/// Cap on `unique_frames * width * height` (~480 HD frames or ~2,000 SD frames).
pub const MAX_SEQUENCE_PIXELS: u64 = 500_000_000;
const GIF_QUANTIZER_SPEED: i32 = 10;

/// Target container/codec for animated presentation export.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimatedFormat {
    Mp4,
    Webm,
    Gif,
}

impl AnimatedFormat {
    pub fn parse(s: &str) -> Option<Self> {
        if s.eq_ignore_ascii_case("mp4") || s.eq_ignore_ascii_case("m4v") {
            Some(Self::Mp4)
        } else if s.eq_ignore_ascii_case("webm") {
            Some(Self::Webm)
        } else if s.eq_ignore_ascii_case("gif") {
            Some(Self::Gif)
        } else {
            None
        }
    }

    pub fn from_path(path: &str) -> Option<Self> {
        let ext = std::path::Path::new(path).extension()?.to_str()?;
        Self::parse(ext)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Webm => "webm",
            Self::Gif => "gif",
        }
    }

    pub fn default_width(self) -> u32 {
        match self {
            Self::Mp4 | Self::Webm => DEFAULT_VIDEO_WIDTH,
            Self::Gif => DEFAULT_GIF_WIDTH,
        }
    }

    pub fn default_fps(self) -> u32 {
        match self {
            Self::Mp4 | Self::Webm => DEFAULT_VIDEO_FPS,
            Self::Gif => DEFAULT_GIF_FPS,
        }
    }
}

/// Options for [`encode_animated`].
#[derive(Clone, Debug, PartialEq)]
pub struct AnimatedExportOptions {
    pub format: AnimatedFormat,
    pub width: u32,
    pub height: Option<u32>,
    pub fps: u32,
    pub hold_seconds: f64,
    pub loop_forever: bool,
    pub include_hidden: bool,
    pub include_builds: bool,
    pub include_transitions: bool,
    pub slides: Option<Vec<usize>>,
}

impl AnimatedExportOptions {
    pub fn for_format(format: AnimatedFormat) -> Self {
        Self {
            format,
            width: format.default_width(),
            height: None,
            fps: format.default_fps(),
            hold_seconds: DEFAULT_HOLD_SECONDS,
            loop_forever: true,
            include_hidden: false,
            include_builds: true,
            include_transitions: true,
            slides: None,
        }
    }

    /// Parse command JSON parameters into validated [`AnimatedExportOptions`].
    pub fn from_params(p: &Value, default_format: AnimatedFormat, cmd: &str, slide_count: usize) -> Result<Self> {
        let format = match given(p, "format") {
            Some(Value::String(s)) => {
                AnimatedFormat::parse(s.trim()).ok_or_else(|| bad(cmd, format!("`format` must be mp4, webm or gif, not `{s}`")))?
            }
            Some(_) => return Err(bad(cmd, "`format` must be a string (mp4, webm or gif)")),
            None => default_format,
        };
        let mut o = Self::for_format(format);
        if let Some(v) = given(p, "width") {
            o.width = parse_width(v, cmd)?;
        }
        if let Some(v) = given(p, "height") {
            o.height = Some(parse_dim(v, "height", cmd)?);
        }
        if let Some(v) = given(p, "fps") {
            let fps = v
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .filter(|n| (MIN_FPS..=MAX_FPS).contains(n))
                .ok_or_else(|| bad(cmd, format!("`fps` must be an integer from {MIN_FPS} to {MAX_FPS}")))?;
            o.fps = fps;
        }
        if let Some(v) = given(p, "holdSeconds").or_else(|| given(p, "secondsPerSlide")) {
            let secs = v.as_f64().ok_or_else(|| bad(cmd, "`holdSeconds` must be a number"))?;
            if !(MIN_HOLD_SECONDS..=MAX_HOLD_SECONDS).contains(&secs) {
                return Err(bad(cmd, format!("`holdSeconds` must be from {MIN_HOLD_SECONDS} to {MAX_HOLD_SECONDS}")));
            }
            o.hold_seconds = secs;
        }
        if let Some(v) = given(p, "loop") {
            o.loop_forever = match v {
                Value::Bool(b) => *b,
                Value::String(s) if s.eq_ignore_ascii_case("forever") => true,
                Value::String(s) if s.eq_ignore_ascii_case("once") => false,
                _ => return Err(bad(cmd, "`loop` must be true, false, \"forever\" or \"once\"")),
            };
        }
        if let Some(v) = given(p, "includeHidden") {
            o.include_hidden = v.as_bool().ok_or_else(|| bad(cmd, "`includeHidden` must be true or false"))?;
        }
        if let Some(v) = given(p, "includeBuilds") {
            o.include_builds = v.as_bool().ok_or_else(|| bad(cmd, "`includeBuilds` must be true or false"))?;
        }
        if let Some(v) = given(p, "includeTransitions") {
            o.include_transitions = v.as_bool().ok_or_else(|| bad(cmd, "`includeTransitions` must be true or false"))?;
        }
        if let Some(v) = given(p, "slides") {
            let list = match v {
                Value::String(text) if !text.trim().is_empty() => crate::cmd::file::parse_slide_range(text, slide_count)?,
                Value::Array(arr) => {
                    if arr.is_empty() {
                        return Err(bad(cmd, "`slides` is empty"));
                    }
                    if arr.len() > MAX_SLIDES {
                        return Err(bad(cmd, format!("at most {MAX_SLIDES} slides can be exported at once")));
                    }
                    arr.iter()
                        .map(|x| x.as_u64().and_then(|i| usize::try_from(i).ok()))
                        .collect::<Option<Vec<usize>>>()
                        .ok_or_else(|| bad(cmd, "`slides` must be a list of slide indices (0, 1, 2…) or a range string"))?
                }
                _ => return Err(bad(cmd, "`slides` must be a list of slide indices or a range string (\"1-3, 5\")")),
            };
            o.slides = Some(list);
        }
        Ok(o)
    }

    fn validate_for(&self, doc: &Presentation, cmd: &str) -> Result<(u32, u32)> {
        if !(MIN_DIM..=MAX_DIM).contains(&self.width) {
            return Err(bad(cmd, format!("`width` must be from {MIN_DIM} to {MAX_DIM}")));
        }
        if let Some(h) = self.height
            && !(MIN_DIM..=MAX_DIM).contains(&h)
        {
            return Err(bad(cmd, format!("`height` must be from {MIN_DIM} to {MAX_DIM}")));
        }
        if !(MIN_FPS..=MAX_FPS).contains(&self.fps) {
            return Err(bad(cmd, format!("`fps` must be from {MIN_FPS} to {MAX_FPS}")));
        }
        if !(MIN_HOLD_SECONDS..=MAX_HOLD_SECONDS).contains(&self.hold_seconds) {
            return Err(bad(cmd, format!("`holdSeconds` must be from {MIN_HOLD_SECONDS} to {MAX_HOLD_SECONDS}")));
        }
        let total_slides = doc.slides.len();
        let indices: Vec<usize> = match &self.slides {
            Some(list) => {
                if let Some(bad_idx) = list.iter().find(|&&i| i >= total_slides) {
                    return Err(bad(cmd, format!("no slide {bad_idx} (the presentation has {total_slides})")));
                }
                list.clone()
            }
            None => (0..total_slides).collect(),
        };
        let visible_count = indices
            .into_iter()
            .filter(|&i| self.include_hidden || doc.slides.get(i).is_some_and(|s| !s.hidden))
            .count();
        if visible_count == 0 {
            return Err(bad(cmd, "no slides to export (all slides are hidden, or the presentation is empty)"));
        }
        if visible_count > MAX_SLIDES {
            return Err(bad(cmd, format!("at most {MAX_SLIDES} slides can be exported at once")));
        }

        let even = self.format != AnimatedFormat::Gif;
        let probe = SequenceOptions {
            width: self.width,
            height: self.height,
            fps: self.fps,
            hold_secs: self.hold_seconds,
            even_dimensions: even,
            ..Default::default()
        };
        let (mut w, mut h) = sequence_dimensions(doc, &probe);
        if even {
            w = w.max(MIN_DIM);
            h = h.max(MIN_DIM);
            if w % 2 != 0 {
                w += 1;
            }
            if h % 2 != 0 {
                h += 1;
            }
        }
        if w > MAX_DIM || h > MAX_DIM {
            return Err(bad(cmd, format!("frame size {w}×{h} exceeds maximum {MAX_DIM}×{MAX_DIM}")));
        }
        // Estimate an upper bound on unique frames before rendering so hostile requests fail fast.
        let est_frames_per_slide = 2usize.saturating_add(self.fps as usize * 2);
        let est_frames = visible_count.saturating_mul(est_frames_per_slide);
        if visible_count > 64 && est_frames > MAX_SEQUENCE_FRAMES * 2 {
            return Err(bad(cmd, "requested animation sequence is too long; choose fewer slides or a lower FPS"));
        }
        Ok((w, h))
    }
}

/// Encoded animated export payload and metadata.
#[derive(Clone, Debug)]
pub struct AnimatedExportOutput {
    pub format: AnimatedFormat,
    pub bytes: Vec<u8>,
    pub frames: usize,
    pub total_ticks: u64,
    pub duration_ms: u64,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

fn given<'a>(p: &'a Value, key: &str) -> Option<&'a Value> {
    p.get(key).filter(|v| !v.is_null())
}

fn parse_width(v: &Value, cmd: &str) -> Result<u32> {
    let err = || bad(cmd, format!("`width` must be a pixel integer from {MIN_DIM} to {MAX_DIM}, or small|medium|large|extraLarge"));
    match v {
        Value::String(s) => SIZE_PRESETS.iter().find(|(k, _)| k.eq_ignore_ascii_case(s.trim())).map(|(_, w)| *w).ok_or_else(err),
        Value::Number(n) => n.as_u64().and_then(|w| u32::try_from(w).ok()).filter(|w| (MIN_DIM..=MAX_DIM).contains(w)).ok_or_else(err),
        _ => Err(err()),
    }
}

fn parse_dim(v: &Value, name: &str, cmd: &str) -> Result<u32> {
    v.as_u64()
        .and_then(|d| u32::try_from(d).ok())
        .filter(|d| (MIN_DIM..=MAX_DIM).contains(d))
        .ok_or_else(|| bad(cmd, format!("`{name}` must be a pixel integer from {MIN_DIM} to {MAX_DIM}")))
}

/// Returns `true` when `file.export` with format `"gif"` explicitly asks for animated transitions
/// or intra-slide builds rather than the legacy 1-frame-per-slide static GIF.
pub fn wants_animated_gif(p: &Value) -> bool {
    given(p, "animate").and_then(Value::as_bool).unwrap_or(false)
        || given(p, "fps").is_some()
        || given(p, "holdSeconds").is_some()
        || given(p, "includeBuilds").is_some()
        || given(p, "includeTransitions").is_some()
}

/// Render slide builds + transitions and encode the sequence into GIF, MP4, or WebM.
pub fn encode_animated(doc: &Presentation, opts: &AnimatedExportOptions) -> Result<AnimatedExportOutput> {
    encode_animated_for(doc, opts, "file.exportVideo")
}

pub fn encode_animated_for(doc: &Presentation, opts: &AnimatedExportOptions, cmd: &str) -> Result<AnimatedExportOutput> {
    let (w, h) = opts.validate_for(doc, cmd)?;
    let seq_opts = SequenceOptions {
        width: w,
        height: Some(h),
        fps: opts.fps,
        hold_secs: opts.hold_seconds,
        include_hidden: opts.include_hidden,
        include_builds: opts.include_builds,
        include_transitions: opts.include_transitions,
        slides: opts.slides.clone(),
        even_dimensions: opts.format != AnimatedFormat::Gif,
        ..Default::default()
    };
    let seq = render_sequence(doc, &seq_opts);
    if seq.frames.is_empty() {
        return Err(bad(cmd, "no frames were rendered for export"));
    }
    if seq.frames.len() > MAX_SEQUENCE_FRAMES {
        return Err(bad(
            cmd,
            format!("sequence produced {} frames (limit {MAX_SEQUENCE_FRAMES}); lower FPS or export fewer slides", seq.frames.len()),
        ));
    }
    let total_px = (seq.frames.len() as u64).saturating_mul(u64::from(seq.width)).saturating_mul(u64::from(seq.height));
    if total_px > MAX_SEQUENCE_PIXELS {
        return Err(bad(
            cmd,
            format!("{} frames at {}×{} exceeds memory budget; choose a smaller resolution or lower FPS", seq.frames.len(), seq.width, seq.height),
        ));
    }

    let bytes = match opts.format {
        AnimatedFormat::Gif => encode_gif_sequence(&seq, opts.loop_forever)?,
        AnimatedFormat::Mp4 => encode_mp4_sequence(&seq)?,
        AnimatedFormat::Webm => encode_webm_sequence(&seq)?,
    };
    let total_ticks = seq.total_ticks();
    let duration_ms = total_ticks.saturating_mul(1000) / u64::from(seq.fps.max(1));
    Ok(AnimatedExportOutput {
        format: opts.format,
        bytes,
        frames: seq.frames.len(),
        total_ticks,
        duration_ms,
        width: seq.width,
        height: seq.height,
        fps: seq.fps,
    })
}

fn straight_opaque_rgba(img: &deckcraft_render::Image, w: u32, h: u32) -> Vec<u8> {
    let len = (w as usize).saturating_mul(h as usize).saturating_mul(4);
    if img.width != w || img.height != h || img.pixels.len() != len {
        return vec![255; len];
    }
    let mut px = img.pixels.clone();
    for p in px.as_chunks_mut::<4>().0 {
        let back = 255 - p[3];
        p[0] = p[0].saturating_add(back);
        p[1] = p[1].saturating_add(back);
        p[2] = p[2].saturating_add(back);
        p[3] = 255;
    }
    px
}

fn encode_gif_sequence(seq: &RenderedSequence, loop_forever: bool) -> Result<Vec<u8>> {
    use image::codecs::gif::{GifEncoder, Repeat};

    let enc_err = |e: image::ImageError| EngineError::Other(format!("GIF encoding failed: {e}"));
    let mut bytes = Vec::new();
    {
        let mut enc = GifEncoder::new_with_speed(&mut bytes, GIF_QUANTIZER_SPEED);
        if loop_forever {
            enc.set_repeat(Repeat::Infinite).map_err(enc_err)?;
        }
        for frame in &seq.frames {
            let rgba = straight_opaque_rgba(&frame.image, seq.width, seq.height);
            let buffer = image::RgbaImage::from_raw(seq.width, seq.height, rgba)
                .ok_or_else(|| EngineError::Other("GIF export: invalid frame buffer".into()))?;
            // GIF delays are stored in centiseconds (10 ms units); clamp to >= 20 ms so browsers do
            // not penalise 0/10 ms delays to 100 ms.
            let ms = frame.duration_ms(seq.fps).max(20);
            let centis = ((ms + 5) / 10).max(2);
            let delay = image::Delay::from_numer_denom_ms(centis * 10, 1);
            enc.encode_frame(image::Frame::from_parts(buffer, 0, 0, delay)).map_err(enc_err)?;
        }
    }
    Ok(bytes)
}

fn encode_mp4_sequence(seq: &RenderedSequence) -> Result<Vec<u8>> {
    use craft_media::isobmff::{AvcConfig, Brand, Mp4Writer, SampleEntry, TrackConfig, WriteSample, WriterOptions};
    use craft_video::h264::{Encoder, EncoderConfig, PacketFormat, Preset, YuvFrame};

    let (w, h, fps) = (seq.width, seq.height, seq.fps.max(1));
    let width_u16 = u16::try_from(w).map_err(|_| EngineError::Other(format!("MP4 width {w} exceeds u16")))?;
    let height_u16 = u16::try_from(h).map_err(|_| EngineError::Other(format!("MP4 height {h} exceeds u16")))?;

    let mut cfg = EncoderConfig::new(w, h, fps, 1);
    cfg.preset = Preset::Speed;
    cfg.bframes = 0;
    cfg.keyint = fps.saturating_mul(2).max(1);
    cfg.format = PacketFormat::LengthPrefixed;
    cfg.threads = 2;

    let mut enc = Encoder::new(cfg).map_err(|e| EngineError::Other(format!("H.264 encoder init failed: {e}")))?;
    let avc_cfg = AvcConfig::parse(&enc.avcc()).map_err(|e| EngineError::Other(format!("H.264 avcC parse failed: {e}")))?;
    let entry = SampleEntry::avc(avc_cfg, width_u16, height_u16);

    let mut wopts = WriterOptions::new(Brand::Mp4);
    wopts.movie_timescale = fps.saturating_mul(1000).max(1000);
    let mut mux = Mp4Writer::new(Cursor::new(Vec::new()), wopts)
        .map_err(|e| EngineError::Other(format!("MP4 muxer init failed: {e}")))?;
    let track = mux
        .add_track(TrackConfig::new(entry, fps))
        .map_err(|e| EngineError::Other(format!("MP4 track init failed: {e}")))?;

    let y_stride = w as usize;
    let uv_stride = (w as usize).div_ceil(2);
    let mut pts: i64 = 0;
    let mut pending_durations = std::collections::VecDeque::with_capacity(seq.frames.len());

    for frame in &seq.frames {
        let rgba = straight_opaque_rgba(&frame.image, w, h);
        let (y, u, v) = craft_video::vp9::rgba_to_yuv420(&rgba, w, h, false);
        let yuv = YuvFrame { y: &y, u: &u, v: &v, y_stride, uv_stride };
        let dur = frame.ticks.max(1);
        pending_durations.push_back(dur);
        let pkts = enc.encode(&yuv, pts).map_err(|e| EngineError::Other(format!("H.264 encode failed: {e}")))?;
        pts = pts.saturating_add(i64::from(dur));
        for pkt in pkts {
            let sample_dur = pending_durations.pop_front().unwrap_or(1);
            mux.write_sample(
                track,
                WriteSample { data: &pkt.data, duration: sample_dur, composition_offset: 0, is_sync: pkt.keyframe },
            )
            .map_err(|e| EngineError::Other(format!("MP4 write failed: {e}")))?;
        }
    }
    let tail = enc.flush();
    for pkt in tail {
        let sample_dur = pending_durations.pop_front().unwrap_or(1);
        mux.write_sample(
            track,
            WriteSample { data: &pkt.data, duration: sample_dur, composition_offset: 0, is_sync: pkt.keyframe },
        )
        .map_err(|e| EngineError::Other(format!("MP4 flush write failed: {e}")))?;
    }

    let cursor = mux.finish_faststart().map_err(|e| EngineError::Other(format!("MP4 finalize failed: {e}")))?;
    Ok(cursor.into_inner())
}

fn encode_webm_sequence(seq: &RenderedSequence) -> Result<Vec<u8>> {
    use craft_media::matroska::{MkvWriter, MuxOptions, TrackKind, TrackSpec};
    use craft_video::vp9::{EncoderConfig, Vp9Encoder};

    let (w, h, fps) = (seq.width, seq.height, seq.fps.max(1));
    let vcfg = EncoderConfig {
        width: w,
        height: h,
        quality: 75,
        full_range: false,
        keyframe_interval: fps.saturating_mul(2).max(1),
        target_kbps: None,
        frame_rate: f64::from(fps),
        loop_filter: true,
    };
    let mut enc = Vp9Encoder::new(vcfg);
    let frame_ns = (1_000_000_000u64 / u64::from(fps)).max(1);
    let mut tspec = TrackSpec::new(TrackKind::Video, "V_VP9");
    tspec.codec_private = enc.vpcc();
    tspec.default_duration_ns = Some(frame_ns);
    tspec.video_size = Some((w, h));
    let mopts = MuxOptions { doc_type: "webm".into(), writing_app: "DeckCraft".into(), ..Default::default() };
    let mut mux = MkvWriter::new(Cursor::new(Vec::new()), vec![tspec], mopts)
        .map_err(|e| EngineError::Other(format!("WebM muxer init failed: {e}")))?;

    let mut pts_ns: i64 = 0;
    for (idx, frame) in seq.frames.iter().enumerate() {
        let rgba = straight_opaque_rgba(&frame.image, w, h);
        let (y, u, v) = craft_video::vp9::rgba_to_yuv420(&rgba, w, h, false);
        let ef = enc.encode_frame(&y, &u, &v, idx == 0);
        if ef.is_empty() {
            return Err(EngineError::Other("VP9 encoder returned an empty packet".into()));
        }
        let ticks = frame.ticks.max(1);
        let dur_ns = u64::from(ticks).saturating_mul(frame_ns);
        let explicit_dur = (ticks != 1).then_some(dur_ns);
        mux.write_frame(0, pts_ns, ef.key, &ef.data, explicit_dur)
            .map_err(|e| EngineError::Other(format!("WebM write failed: {e}")))?;
        pts_ns = pts_ns.saturating_add(i64::try_from(dur_ns).unwrap_or(i64::MAX));
    }

    let cursor = mux.finish().map_err(|e| EngineError::Other(format!("WebM finalize failed: {e}")))?;
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::sync::Arc;

    use image::AnimationDecoder;
    use serde_json::json;

    use crate::Session;

    fn animated_session() -> Session {
        let mut s = Session::new();
        s.execute("file.new", &json!({"blank": true})).unwrap();
        s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
        s.execute("design.background", &json!({"color": "#D82828"})).unwrap();
        let r = s.execute("shape.insert", &json!({"preset": "rect", "rect": [80, 100, 260, 180]})).unwrap();
        let id = r["id"].as_u64().unwrap();
        s.execute("shape.fill", &json!({"ids": [id], "color": "#FFD830"})).unwrap();

        s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
        s.execute("design.background", &json!({"color": "#2060D8"})).unwrap();
        let r2 = s.execute("shape.insert", &json!({"preset": "roundRect", "rect": [520, 180, 280, 180]})).unwrap();
        let id2 = r2["id"].as_u64().unwrap();
        s.execute("shape.fill", &json!({"ids": [id2], "color": "#30E080"})).unwrap();
        s.execute("transition.set", &json!({"kind": "push", "option": "l", "duration": 500})).unwrap();
        s.execute(
            "animation.add",
            &json!({"ids": [id2], "class": "entrance", "effect": "fade", "start": "afterPrevious", "duration": 300}),
        )
        .unwrap();
        s
    }

    #[test]
    fn exports_animated_gif_with_transition_and_build_frames() {
        let mut s = animated_session();
        let dir = std::env::temp_dir().join(format!("deckcraft-anim-gif-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("show.gif");

        let res = s
            .execute(
                "file.exportGif",
                &json!({"path": path.to_string_lossy(), "width": 160, "fps": 10, "holdSeconds": 0.4}),
            )
            .unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let _ = std::fs::remove_dir_all(&dir);

        assert!(bytes.starts_with(b"GIF89a"));
        assert_eq!(res["width"], 160);
        assert_eq!(res["height"], 90);
        let frame_count = res["frames"].as_u64().unwrap() as usize;
        assert!(frame_count >= 7, "animated GIF must include transition + build frames, got {frame_count}");

        let dec = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(&bytes)).unwrap();
        let decoded: Vec<_> = dec.into_frames().collect::<std::result::Result<Vec<_>, _>>().unwrap();
        assert_eq!(decoded.len(), frame_count);
        assert_eq!(decoded[0].buffer().dimensions(), (160, 90));
        // Mid-transition frame must differ from both initial red slide and final blue slide.
        let first_px = decoded.first().unwrap().buffer().as_raw();
        let last_px = decoded.last().unwrap().buffer().as_raw();
        assert!(decoded[1..decoded.len() - 1].iter().any(|f| f.buffer().as_raw() != first_px && f.buffer().as_raw() != last_px));
    }

    #[test]
    fn exports_playable_mp4_and_webm_with_probed_headers_and_decoded_frames() {
        let mut s = animated_session();
        let dir = std::env::temp_dir().join(format!("deckcraft-anim-vid-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // 1. H.264 / MP4
        let mp4_path = dir.join("deck.mp4");
        let mp4_res = s
            .execute(
                "file.exportVideo",
                &json!({"path": mp4_path.to_string_lossy(), "format": "mp4", "width": 320, "fps": 12, "holdSeconds": 0.5}),
            )
            .unwrap();
        let mp4_bytes = std::fs::read(&mp4_path).unwrap();
        assert!(mp4_bytes.len() > 256);
        assert_eq!(&mp4_bytes[4..8], b"ftyp", "MP4 must begin with an ISO BMFF ftyp box");
        assert!(mp4_bytes.windows(4).any(|w| w == b"moov"), "MP4 must contain moov box");
        assert!(mp4_bytes.windows(4).any(|w| w == b"mdat"), "MP4 must contain mdat box");
        assert_eq!((mp4_res["width"].as_u64(), mp4_res["height"].as_u64()), (Some(320), Some(180)));

        let mp4_arc = Arc::new(mp4_bytes);
        let mp4_info = deckcraft_media::probe_headers(&mp4_arc).expect("probe MP4 headers");
        let mp4_vid = mp4_info.video.as_ref().expect("probed MP4 must report a video track");
        assert!(mp4_vid.decodable);
        assert_eq!((mp4_vid.width, mp4_vid.height), (320, 180));
        assert!(mp4_info.duration_ms > 500, "MP4 duration_ms was {}", mp4_info.duration_ms);
        let mut mp4_dec = deckcraft_media::VideoDecoder::open(mp4_arc).expect("open MP4 in VideoDecoder");
        let frame0 = mp4_dec.next_frame().expect("decode MP4").expect("first MP4 frame");
        assert_eq!((frame0.width, frame0.height), (320, 180));
        assert_eq!(frame0.rgba.len(), 320 * 180 * 4);

        // 2. VP9 / WebM
        let webm_path = dir.join("deck.webm");
        let webm_res = s
            .execute(
                "file.export",
                &json!({"path": webm_path.to_string_lossy(), "width": 320, "fps": 12, "holdSeconds": 0.5}),
            )
            .unwrap();
        let webm_bytes = std::fs::read(&webm_path).unwrap();
        let _ = std::fs::remove_dir_all(&dir);

        assert!(webm_bytes.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]), "WebM must start with EBML magic");
        assert!(webm_bytes.windows(4).any(|w| w == b"webm"), "EBML DocType must be webm");
        assert_eq!((webm_res["width"].as_u64(), webm_res["height"].as_u64()), (Some(320), Some(180)));

        let webm_arc = Arc::new(webm_bytes);
        let webm_info = deckcraft_media::probe_headers(&webm_arc).expect("probe WebM headers");
        let webm_vid = webm_info.video.as_ref().expect("probed WebM must report a video track");
        assert!(webm_vid.decodable);
        assert_eq!((webm_vid.width, webm_vid.height), (320, 180));
        let mut webm_dec = deckcraft_media::VideoDecoder::open(webm_arc).expect("open WebM in VideoDecoder");
        let wframe0 = webm_dec.next_frame().expect("decode WebM").expect("first WebM frame");
        assert_eq!((wframe0.width, wframe0.height), (320, 180));
        assert_eq!(wframe0.rgba.len(), 320 * 180 * 4);
    }

    #[test]
    fn session_saves_and_reopens_odp_presentation() {
        let mut s = Session::new();
        crate::sample::open_sample(&mut s).unwrap();
        let slide_count = s.doc().unwrap().doc.slides.len();
        let saved = s.execute("file.saveBytes", &json!({"format": "odp"})).unwrap();
        let b64 = saved["data"].as_str().unwrap().to_string();
        s.execute("file.openBytes", &json!({"name": "sample.odp", "data": b64})).unwrap();
        assert_eq!(s.doc().unwrap().doc.slides.len(), slide_count);
    }
}
