//! Timelapse video & animated GIF export (`File > Export > Export Timelapse…`, Photoshop 28.0).
//!
//! Encodes captured RGBA8 canvas history snapshots at `2560 (2K)` or `1920 (1080p)` max long-edge
//! resolution and `High` / `Medium` / `Low` quality into:
//! - `.mp4`: ISO Base Media File Format (`isom` / `mp41`) with Motion-JPEG intra samples (`jpeg`).
//! - `.webm`: Matroska / WebM EBML container (`V_MJPEG` intra keyframes).
//! - `.avi`: RIFF `AVI ` Motion-JPEG (`MJPG`) stream with `idx1` keyframe index.
//! - `.gif`: Multi-frame looping `GIF89a` (`NETSCAPE2.0`) via `photocraft-codecs`.

use photocraft_codecs::{self as codecs, ChannelLayout, EncodeOptions, Format, Image, SampleType as CSample};

use crate::IoError;

/// Output long-edge resolution preset for Timelapse recording and export.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TimelapseResolution {
    /// 2560 px long edge (2K).
    Res2560,
    /// 1920 px long edge (1080p).
    #[default]
    Res1920,
}

impl TimelapseResolution {
    /// Maximum pixel count along the longest canvas edge.
    #[must_use]
    pub const fn long_edge(self) -> u32 {
        match self {
            Self::Res2560 => 2560,
            Self::Res1920 => 1920,
        }
    }

    /// Canonical identifier used in commands and preferences.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Res2560 => "2560",
            Self::Res1920 => "1920",
        }
    }

    /// UI display label matching Photoshop 28.0 (`2560 (2K)` / `1920 (1080p)`).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Res2560 => "2560 (2K)",
            Self::Res1920 => "1920 (1080p)",
        }
    }

    /// Parses a command/preference string (`"2560"`, `"2k"`, `"1920"`, `"1080p"`, etc.).
    #[must_use]
    pub fn parse_str(s: &str) -> Self {
        let lower = s.trim().to_ascii_lowercase();
        if lower.contains("2560") || lower.contains("2k") {
            Self::Res2560
        } else {
            Self::Res1920
        }
    }
}

/// Compression quality preset for Timelapse recording and export.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TimelapseQuality {
    /// High quality (JPEG Q92).
    #[default]
    High,
    /// Medium quality (JPEG Q78).
    Medium,
    /// Low quality (JPEG Q58).
    Low,
}

impl TimelapseQuality {
    /// Corresponding baseline JPEG quality factor (`1..=100`).
    #[must_use]
    pub const fn jpeg_quality(self) -> u8 {
        match self {
            Self::High => 92,
            Self::Medium => 78,
            Self::Low => 58,
        }
    }

    /// Canonical identifier used in commands and preferences.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }

    /// Parses a command/preference string (`"high"`, `"medium"`, `"low"`).
    #[must_use]
    pub fn parse_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "low" => Self::Low,
            "medium" | "med" => Self::Medium,
            _ => Self::High,
        }
    }
}

/// Container format for Timelapse export.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TimelapseFormat {
    /// ISO BMFF `.mp4`.
    #[default]
    Mp4,
    /// WebM / Matroska `.webm`.
    Webm,
    /// RIFF Motion-JPEG `.avi`.
    Avi,
    /// Animated `.gif`.
    Gif,
}

impl TimelapseFormat {
    /// Infers the container format from a file path or extension.
    #[must_use]
    pub fn from_path_or_ext(name_or_ext: &str) -> Self {
        let ext = name_or_ext
            .rsplit(['.', '/', '\\'])
            .next()
            .unwrap_or(name_or_ext)
            .to_ascii_lowercase();
        match ext.as_str() {
            "webm" => Self::Webm,
            "avi" => Self::Avi,
            "gif" => Self::Gif,
            _ => Self::Mp4,
        }
    }
}

/// One RGBA8 frame fed to [`encode_timelapse_frames`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimelapseRawFrame {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Straight-alpha `RGBA8` pixels (`width * height * 4` bytes).
    pub rgba8: Vec<u8>,
}

/// Computes even output `(width, height)` fitting within `res.long_edge()` while preserving
/// aspect ratio.
#[must_use]
pub fn target_dimensions(src_w: u32, src_h: u32, res: TimelapseResolution) -> (u32, u32) {
    let w = src_w.max(2);
    let h = src_h.max(2);
    let max_edge = res.long_edge();
    let long = w.max(h);
    let (scaled_w, scaled_h) = if long > max_edge {
        let scale = max_edge as f64 / long as f64;
        (
            ((w as f64 * scale).round() as u32).max(2),
            ((h as f64 * scale).round() as u32).max(2),
        )
    } else {
        (w, h)
    };
    let even_w = (scaled_w & !1).max(2);
    let even_h = (scaled_h & !1).max(2);
    (even_w, even_h)
}

/// Encodes a sequence of RGBA8 timelapse snapshots into the requested video/animation container.
pub fn encode_timelapse_frames(
    frames: &[TimelapseRawFrame],
    res: TimelapseResolution,
    quality: TimelapseQuality,
    fps: u32,
    format: TimelapseFormat,
) -> Result<Vec<u8>, IoError> {
    let Some(first) = frames.first() else {
        return Err(IoError::Unsupported("timelapse has no recorded frames to export".into()));
    };
    let fps = fps.clamp(1, 120);
    let (out_w, out_h) = target_dimensions(first.width, first.height, res);

    let rgb_frames: Vec<Vec<u8>> = crate::pixels::par_map(frames.iter().collect(), |frame| {
        scale_and_matte_rgb8(frame, out_w, out_h)
    });

    match format {
        TimelapseFormat::Gif => encode_animated_gif(&rgb_frames, out_w, out_h, fps),
        TimelapseFormat::Mp4 => {
            let jpegs = encode_jpeg_frames(&rgb_frames, out_w, out_h, quality)?;
            Ok(mux_mp4_mjpeg(&jpegs, out_w, out_h, fps))
        }
        TimelapseFormat::Avi => {
            let jpegs = encode_jpeg_frames(&rgb_frames, out_w, out_h, quality)?;
            Ok(mux_avi_mjpeg(&jpegs, out_w, out_h, fps))
        }
        TimelapseFormat::Webm => {
            let jpegs = encode_jpeg_frames(&rgb_frames, out_w, out_h, quality)?;
            Ok(mux_webm_mjpeg(&jpegs, out_w, out_h, fps))
        }
    }
}

/// Bilinearly resamples `frame` to `(out_w, out_h)` and composites translucent pixels over a clean
/// dark-neutral canvas backdrop (`RGB8`).
fn scale_and_matte_rgb8(frame: &TimelapseRawFrame, out_w: u32, out_h: u32) -> Vec<u8> {
    let sw = frame.width.max(1) as usize;
    let sh = frame.height.max(1) as usize;
    let dw = out_w.max(1) as usize;
    let dh = out_h.max(1) as usize;
    let mut out = vec![0u8; dw * dh * 3];

    let sample_rgba = |x: usize, y: usize| -> [f32; 4] {
        let idx = (y.min(sh - 1) * sw + x.min(sw - 1)) * 4;
        if let Some(px) = frame.rgba8.get(idx..idx + 4) {
            [px[0] as f32, px[1] as f32, px[2] as f32, px[3] as f32 / 255.0]
        } else {
            [255.0, 255.0, 255.0, 1.0]
        }
    };

    for dy in 0..dh {
        let fy = ((dy as f32 + 0.5) * (sh as f32 / dh as f32) - 0.5).clamp(0.0, (sh - 1) as f32);
        let y0 = fy.floor() as usize;
        let y1 = (y0 + 1).min(sh - 1);
        let wy = fy - y0 as f32;
        for dx in 0..dw {
            let fx = ((dx as f32 + 0.5) * (sw as f32 / dw as f32) - 0.5).clamp(0.0, (sw - 1) as f32);
            let x0 = fx.floor() as usize;
            let x1 = (x0 + 1).min(sw - 1);
            let wx = fx - x0 as f32;

            let p00 = sample_rgba(x0, y0);
            let p10 = sample_rgba(x1, y0);
            let p01 = sample_rgba(x0, y1);
            let p11 = sample_rgba(x1, y1);

            let mut c = [0.0f32; 4];
            for k in 0..4 {
                let top = p00[k] * (1.0 - wx) + p10[k] * wx;
                let bot = p01[k] * (1.0 - wx) + p11[k] * wx;
                c[k] = top * (1.0 - wy) + bot * wy;
            }
            let a = c[3].clamp(0.0, 1.0);
            let dst = (dy * dw + dx) * 3;
            if let Some(rgb) = out.get_mut(dst..dst + 3) {
                rgb[0] = (c[0] * a + 255.0 * (1.0 - a)).round().clamp(0.0, 255.0) as u8;
                rgb[1] = (c[1] * a + 255.0 * (1.0 - a)).round().clamp(0.0, 255.0) as u8;
                rgb[2] = (c[2] * a + 255.0 * (1.0 - a)).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

fn encode_jpeg_frames(
    rgb_frames: &[Vec<u8>],
    w: u32,
    h: u32,
    quality: TimelapseQuality,
) -> Result<Vec<Vec<u8>>, IoError> {
    let opts = EncodeOptions {
        jpeg_quality: quality.jpeg_quality(),
        embed_icc: false,
        embed_metadata: false,
        ..EncodeOptions::default()
    };
    let encoded: Vec<Result<Vec<u8>, IoError>> = crate::pixels::par_map(rgb_frames.iter().collect(), |rgb| {
        let img = Image::from_raw(w, h, ChannelLayout::Rgb, CSample::U8, rgb.clone())?;
        Ok(codecs::encode(&img, Format::Jpeg, &opts)?)
    });
    encoded.into_iter().collect()
}

fn mp4_box(fourcc: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let total = (8usize.saturating_add(payload.len())) as u32;
    let mut out = Vec::with_capacity(8 + payload.len());
    out.extend_from_slice(&total.to_be_bytes());
    out.extend_from_slice(fourcc);
    out.extend_from_slice(payload);
    out
}

/// Muxes JPEG intra frames into a valid ISO BMFF `.mp4` (`ftyp` + `mdat` + `moov`).
fn mux_mp4_mjpeg(jpegs: &[Vec<u8>], width: u32, height: u32, fps: u32) -> Vec<u8> {
    let mut ftyp_payload = Vec::with_capacity(24);
    ftyp_payload.extend_from_slice(b"isom");
    ftyp_payload.extend_from_slice(&0x0000_0200u32.to_be_bytes());
    ftyp_payload.extend_from_slice(b"isomiso2mp41qt  ");
    let ftyp = mp4_box(b"ftyp", &ftyp_payload);

    let mdat_header_offset = ftyp.len();
    let mdat_data_start = mdat_header_offset + 8;
    let total_jpeg_bytes: usize = jpegs.iter().map(Vec::len).sum();
    let mut offsets = Vec::with_capacity(jpegs.len());
    let mut sizes = Vec::with_capacity(jpegs.len());
    let mut mdat_payload = Vec::with_capacity(total_jpeg_bytes);
    for jpg in jpegs {
        offsets.push((mdat_data_start + mdat_payload.len()) as u32);
        sizes.push(jpg.len() as u32);
        mdat_payload.extend_from_slice(jpg);
    }
    let mdat = mp4_box(b"mdat", &mdat_payload);

    let timescale = fps.saturating_mul(1000).max(1000);
    let sample_delta = 1000u32;
    let duration = (jpegs.len() as u32).saturating_mul(sample_delta);

    // mvhd (version 0, 100-byte payload)
    let mut mvhd_p = Vec::with_capacity(100);
    mvhd_p.extend_from_slice(&0u32.to_be_bytes()); // version + flags
    mvhd_p.extend_from_slice(&0u32.to_be_bytes()); // creation_time
    mvhd_p.extend_from_slice(&0u32.to_be_bytes()); // modification_time
    mvhd_p.extend_from_slice(&timescale.to_be_bytes());
    mvhd_p.extend_from_slice(&duration.to_be_bytes());
    mvhd_p.extend_from_slice(&0x0001_0000u32.to_be_bytes()); // rate 1.0
    mvhd_p.extend_from_slice(&0x0100u16.to_be_bytes()); // volume 1.0
    mvhd_p.extend_from_slice(&[0u8; 10]); // reserved
    for &m in &[0x0001_0000u32, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000] {
        mvhd_p.extend_from_slice(&m.to_be_bytes());
    }
    mvhd_p.extend_from_slice(&[0u8; 24]); // pre_defined
    mvhd_p.extend_from_slice(&2u32.to_be_bytes()); // next_track_ID
    let mvhd = mp4_box(b"mvhd", &mvhd_p);

    // tkhd (version 0, flags = 3 enabled + in_movie)
    let mut tkhd_p = Vec::with_capacity(84);
    tkhd_p.extend_from_slice(&0x0000_0003u32.to_be_bytes());
    tkhd_p.extend_from_slice(&0u32.to_be_bytes());
    tkhd_p.extend_from_slice(&0u32.to_be_bytes());
    tkhd_p.extend_from_slice(&1u32.to_be_bytes()); // track_ID
    tkhd_p.extend_from_slice(&0u32.to_be_bytes());
    tkhd_p.extend_from_slice(&duration.to_be_bytes());
    tkhd_p.extend_from_slice(&[0u8; 8]);
    tkhd_p.extend_from_slice(&0u16.to_be_bytes()); // layer
    tkhd_p.extend_from_slice(&0u16.to_be_bytes()); // alternate_group
    tkhd_p.extend_from_slice(&0u16.to_be_bytes()); // volume (video = 0)
    tkhd_p.extend_from_slice(&0u16.to_be_bytes());
    for &m in &[0x0001_0000u32, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000] {
        tkhd_p.extend_from_slice(&m.to_be_bytes());
    }
    tkhd_p.extend_from_slice(&(width.min(65535) << 16).to_be_bytes());
    tkhd_p.extend_from_slice(&(height.min(65535) << 16).to_be_bytes());
    let tkhd = mp4_box(b"tkhd", &tkhd_p);

    // mdhd
    let mut mdhd_p = Vec::with_capacity(24);
    mdhd_p.extend_from_slice(&0u32.to_be_bytes());
    mdhd_p.extend_from_slice(&0u32.to_be_bytes());
    mdhd_p.extend_from_slice(&0u32.to_be_bytes());
    mdhd_p.extend_from_slice(&timescale.to_be_bytes());
    mdhd_p.extend_from_slice(&duration.to_be_bytes());
    mdhd_p.extend_from_slice(&0x55C4u16.to_be_bytes()); // "und"
    mdhd_p.extend_from_slice(&0u16.to_be_bytes());
    let mdhd = mp4_box(b"mdhd", &mdhd_p);

    // hdlr
    let mut hdlr_p = Vec::with_capacity(45);
    hdlr_p.extend_from_slice(&0u32.to_be_bytes());
    hdlr_p.extend_from_slice(&0u32.to_be_bytes());
    hdlr_p.extend_from_slice(b"vide");
    hdlr_p.extend_from_slice(&[0u8; 12]);
    hdlr_p.extend_from_slice(b"PhotoCraft Timelapse\0");
    let hdlr = mp4_box(b"hdlr", &hdlr_p);

    // vmhd
    let mut vmhd_p = Vec::with_capacity(12);
    vmhd_p.extend_from_slice(&0x0000_0001u32.to_be_bytes());
    vmhd_p.extend_from_slice(&[0u8; 8]);
    let vmhd = mp4_box(b"vmhd", &vmhd_p);

    // dinf -> dref -> url
    let url_box = mp4_box(b"url ", &0x0000_0001u32.to_be_bytes());
    let mut dref_p = Vec::with_capacity(8 + url_box.len());
    dref_p.extend_from_slice(&0u32.to_be_bytes());
    dref_p.extend_from_slice(&1u32.to_be_bytes());
    dref_p.extend_from_slice(&url_box);
    let dinf = mp4_box(b"dinf", &mp4_box(b"dref", &dref_p));

    // stsd (`jpeg` VisualSampleEntry, 86 bytes total)
    let mut jpeg_entry = Vec::with_capacity(78);
    jpeg_entry.extend_from_slice(&[0u8; 6]); // reserved
    jpeg_entry.extend_from_slice(&1u16.to_be_bytes()); // data_reference_index
    jpeg_entry.extend_from_slice(&[0u8; 16]); // pre_defined + reserved
    jpeg_entry.extend_from_slice(&(width.min(65535) as u16).to_be_bytes());
    jpeg_entry.extend_from_slice(&(height.min(65535) as u16).to_be_bytes());
    jpeg_entry.extend_from_slice(&0x0048_0000u32.to_be_bytes()); // horiz 72 dpi
    jpeg_entry.extend_from_slice(&0x0048_0000u32.to_be_bytes()); // vert 72 dpi
    jpeg_entry.extend_from_slice(&0u32.to_be_bytes());
    jpeg_entry.extend_from_slice(&1u16.to_be_bytes()); // frame_count = 1
    let mut compressor = [0u8; 32];
    let cname = b"PhotoCraft MJPEG";
    compressor[0] = cname.len() as u8;
    compressor[1..1 + cname.len()].copy_from_slice(cname);
    jpeg_entry.extend_from_slice(&compressor);
    jpeg_entry.extend_from_slice(&0x0018u16.to_be_bytes()); // depth = 24
    jpeg_entry.extend_from_slice(&0xFFFFu16.to_be_bytes()); // pre_defined = -1
    let jpeg_box = mp4_box(b"jpeg", &jpeg_entry);

    let mut stsd_p = Vec::with_capacity(8 + jpeg_box.len());
    stsd_p.extend_from_slice(&0u32.to_be_bytes());
    stsd_p.extend_from_slice(&1u32.to_be_bytes());
    stsd_p.extend_from_slice(&jpeg_box);
    let stsd = mp4_box(b"stsd", &stsd_p);

    // stts
    let mut stts_p = Vec::with_capacity(16);
    stts_p.extend_from_slice(&0u32.to_be_bytes());
    stts_p.extend_from_slice(&1u32.to_be_bytes());
    stts_p.extend_from_slice(&(jpegs.len() as u32).to_be_bytes());
    stts_p.extend_from_slice(&sample_delta.to_be_bytes());
    let stts = mp4_box(b"stts", &stts_p);

    // stss (every frame is an intra keyframe)
    let mut stss_p = Vec::with_capacity(8 + jpegs.len() * 4);
    stss_p.extend_from_slice(&0u32.to_be_bytes());
    stss_p.extend_from_slice(&(jpegs.len() as u32).to_be_bytes());
    for i in 1..=jpegs.len() as u32 {
        stss_p.extend_from_slice(&i.to_be_bytes());
    }
    let stss = mp4_box(b"stss", &stss_p);

    // stsc (1 sample per chunk)
    let mut stsc_p = Vec::with_capacity(20);
    stsc_p.extend_from_slice(&0u32.to_be_bytes());
    stsc_p.extend_from_slice(&1u32.to_be_bytes());
    stsc_p.extend_from_slice(&1u32.to_be_bytes()); // first_chunk
    stsc_p.extend_from_slice(&1u32.to_be_bytes()); // samples_per_chunk
    stsc_p.extend_from_slice(&1u32.to_be_bytes()); // sample_description_index
    let stsc = mp4_box(b"stsc", &stsc_p);

    // stsz
    let mut stsz_p = Vec::with_capacity(12 + sizes.len() * 4);
    stsz_p.extend_from_slice(&0u32.to_be_bytes());
    stsz_p.extend_from_slice(&0u32.to_be_bytes()); // variable sample sizes
    stsz_p.extend_from_slice(&(sizes.len() as u32).to_be_bytes());
    for &sz in &sizes {
        stsz_p.extend_from_slice(&sz.to_be_bytes());
    }
    let stsz = mp4_box(b"stsz", &stsz_p);

    // stco
    let mut stco_p = Vec::with_capacity(8 + offsets.len() * 4);
    stco_p.extend_from_slice(&0u32.to_be_bytes());
    stco_p.extend_from_slice(&(offsets.len() as u32).to_be_bytes());
    for &off in &offsets {
        stco_p.extend_from_slice(&off.to_be_bytes());
    }
    let stco = mp4_box(b"stco", &stco_p);

    let mut stbl_p = Vec::new();
    stbl_p.extend_from_slice(&stsd);
    stbl_p.extend_from_slice(&stts);
    stbl_p.extend_from_slice(&stss);
    stbl_p.extend_from_slice(&stsc);
    stbl_p.extend_from_slice(&stsz);
    stbl_p.extend_from_slice(&stco);
    let stbl = mp4_box(b"stbl", &stbl_p);

    let mut minf_p = Vec::new();
    minf_p.extend_from_slice(&vmhd);
    minf_p.extend_from_slice(&dinf);
    minf_p.extend_from_slice(&stbl);
    let minf = mp4_box(b"minf", &minf_p);

    let mut mdia_p = Vec::new();
    mdia_p.extend_from_slice(&mdhd);
    mdia_p.extend_from_slice(&hdlr);
    mdia_p.extend_from_slice(&minf);
    let mdia = mp4_box(b"mdia", &mdia_p);

    let mut trak_p = Vec::new();
    trak_p.extend_from_slice(&tkhd);
    trak_p.extend_from_slice(&mdia);
    let trak = mp4_box(b"trak", &trak_p);

    let mut moov_p = Vec::new();
    moov_p.extend_from_slice(&mvhd);
    moov_p.extend_from_slice(&trak);
    let moov = mp4_box(b"moov", &moov_p);

    let mut out = Vec::with_capacity(ftyp.len() + mdat.len() + moov.len());
    out.extend_from_slice(&ftyp);
    out.extend_from_slice(&mdat);
    out.extend_from_slice(&moov);
    out
}

/// Muxes JPEG frames into a standard RIFF `AVI ` (`MJPG`) video container.
fn mux_avi_mjpeg(jpegs: &[Vec<u8>], width: u32, height: u32, fps: u32) -> Vec<u8> {
    let usec_per_frame = (1_000_000u32 / fps.max(1)).max(1);
    let max_bytes = jpegs.iter().map(Vec::len).max().unwrap_or(0) as u32;
    let total_frames = jpegs.len() as u32;

    // avih (56 bytes)
    let mut avih = Vec::with_capacity(56);
    avih.extend_from_slice(&usec_per_frame.to_le_bytes());
    avih.extend_from_slice(&(max_bytes.saturating_mul(fps)).to_le_bytes());
    avih.extend_from_slice(&0u32.to_le_bytes());
    avih.extend_from_slice(&0x10u32.to_le_bytes()); // AVIF_HASINDEX
    avih.extend_from_slice(&total_frames.to_le_bytes());
    avih.extend_from_slice(&0u32.to_le_bytes());
    avih.extend_from_slice(&1u32.to_le_bytes()); // streams = 1
    avih.extend_from_slice(&max_bytes.to_le_bytes());
    avih.extend_from_slice(&width.to_le_bytes());
    avih.extend_from_slice(&height.to_le_bytes());
    avih.extend_from_slice(&[0u8; 16]);

    // strh (56 bytes)
    let mut strh = Vec::with_capacity(56);
    strh.extend_from_slice(b"vids");
    strh.extend_from_slice(b"MJPG");
    strh.extend_from_slice(&0u32.to_le_bytes());
    strh.extend_from_slice(&0u32.to_le_bytes());
    strh.extend_from_slice(&0u32.to_le_bytes());
    strh.extend_from_slice(&1u32.to_le_bytes()); // dwScale = 1
    strh.extend_from_slice(&fps.to_le_bytes()); // dwRate = fps
    strh.extend_from_slice(&0u32.to_le_bytes());
    strh.extend_from_slice(&total_frames.to_le_bytes());
    strh.extend_from_slice(&max_bytes.to_le_bytes());
    strh.extend_from_slice(&10000u32.to_le_bytes());
    strh.extend_from_slice(&0u32.to_le_bytes());
    strh.extend_from_slice(&0u16.to_le_bytes());
    strh.extend_from_slice(&0u16.to_le_bytes());
    strh.extend_from_slice(&(width.min(65535) as u16).to_le_bytes());
    strh.extend_from_slice(&(height.min(65535) as u16).to_le_bytes());

    // strf (BITMAPINFOHEADER, 40 bytes)
    let mut strf = Vec::with_capacity(40);
    strf.extend_from_slice(&40u32.to_le_bytes());
    strf.extend_from_slice(&width.to_le_bytes());
    strf.extend_from_slice(&height.to_le_bytes());
    strf.extend_from_slice(&1u16.to_le_bytes());
    strf.extend_from_slice(&24u16.to_le_bytes());
    strf.extend_from_slice(b"MJPG");
    strf.extend_from_slice(&(width.saturating_mul(height).saturating_mul(3)).to_le_bytes());
    strf.extend_from_slice(&[0u8; 16]);

    let riff_chunk = |tag: &[u8; 4], data: &[u8]| -> Vec<u8> {
        let mut c = Vec::with_capacity(8 + data.len() + (data.len() & 1));
        c.extend_from_slice(tag);
        c.extend_from_slice(&(data.len() as u32).to_le_bytes());
        c.extend_from_slice(data);
        if data.len() & 1 == 1 {
            c.push(0);
        }
        c
    };
    let riff_list = |list_type: &[u8; 4], body: &[u8]| -> Vec<u8> {
        let mut p = Vec::with_capacity(4 + body.len());
        p.extend_from_slice(list_type);
        p.extend_from_slice(body);
        riff_chunk(b"LIST", &p)
    };

    let mut strl_body = Vec::new();
    strl_body.extend_from_slice(&riff_chunk(b"strh", &strh));
    strl_body.extend_from_slice(&riff_chunk(b"strf", &strf));
    let strl = riff_list(b"strl", &strl_body);

    let mut hdrl_body = Vec::new();
    hdrl_body.extend_from_slice(&riff_chunk(b"avih", &avih));
    hdrl_body.extend_from_slice(&strl);
    let hdrl = riff_list(b"hdrl", &hdrl_body);

    let mut movi_body = Vec::new();
    let mut idx1_body = Vec::with_capacity(jpegs.len() * 16);
    for jpg in jpegs {
        let offset_from_movi = 4u32.saturating_add(movi_body.len() as u32);
        idx1_body.extend_from_slice(b"00dc");
        idx1_body.extend_from_slice(&0x10u32.to_le_bytes()); // AVIIF_KEYFRAME
        idx1_body.extend_from_slice(&offset_from_movi.to_le_bytes());
        idx1_body.extend_from_slice(&(jpg.len() as u32).to_le_bytes());
        movi_body.extend_from_slice(&riff_chunk(b"00dc", jpg));
    }
    let movi = riff_list(b"movi", &movi_body);
    let idx1 = riff_chunk(b"idx1", &idx1_body);

    let mut avi_body = Vec::with_capacity(4 + hdrl.len() + movi.len() + idx1.len());
    avi_body.extend_from_slice(b"AVI ");
    avi_body.extend_from_slice(&hdrl);
    avi_body.extend_from_slice(&movi);
    avi_body.extend_from_slice(&idx1);
    riff_chunk(b"RIFF", &avi_body)
}

fn ebml_vint(len: usize) -> Vec<u8> {
    let val = len as u64;
    if val < 0x7F {
        vec![0x80 | val as u8]
    } else if val < 0x3FFF {
        vec![0x40 | ((val >> 8) as u8), val as u8]
    } else if val < 0x1F_FFFF {
        vec![0x20 | ((val >> 16) as u8), (val >> 8) as u8, val as u8]
    } else if val < 0x0FFF_FFFF {
        vec![0x10 | ((val >> 24) as u8), (val >> 16) as u8, (val >> 8) as u8, val as u8]
    } else {
        let mut out = Vec::with_capacity(9);
        out.push(0x01);
        out.extend_from_slice(&val.to_be_bytes());
        out
    }
}

fn ebml_el(id: &[u8], data: &[u8]) -> Vec<u8> {
    let vint = ebml_vint(data.len());
    let mut out = Vec::with_capacity(id.len() + vint.len() + data.len());
    out.extend_from_slice(id);
    out.extend_from_slice(&vint);
    out.extend_from_slice(data);
    out
}

fn ebml_uint(id: &[u8], v: u64) -> Vec<u8> {
    let be = v.to_be_bytes();
    let first = be.iter().position(|&b| b != 0).unwrap_or(7);
    ebml_el(id, &be[first..])
}

/// Muxes intra frames into a valid EBML WebM/Matroska container.
fn mux_webm_mjpeg(jpegs: &[Vec<u8>], width: u32, height: u32, fps: u32) -> Vec<u8> {
    let mut ebml_head = Vec::new();
    ebml_head.extend_from_slice(&ebml_uint(&[0x42, 0x86], 1)); // EBMLVersion
    ebml_head.extend_from_slice(&ebml_uint(&[0x42, 0xF7], 1)); // EBMLReadVersion
    ebml_head.extend_from_slice(&ebml_uint(&[0x42, 0xF2], 4)); // EBMLMaxIDLength
    ebml_head.extend_from_slice(&ebml_uint(&[0x42, 0xF3], 8)); // EBMLMaxSizeLength
    ebml_head.extend_from_slice(&ebml_el(&[0x42, 0x82], b"webm")); // DocType
    ebml_head.extend_from_slice(&ebml_uint(&[0x42, 0x87], 4)); // DocTypeVersion
    ebml_head.extend_from_slice(&ebml_uint(&[0x42, 0x85], 2)); // DocTypeReadVersion
    let header = ebml_el(&[0x1A, 0x45, 0xDF, 0xA3], &ebml_head);

    let frame_ms = (1000u64 / u64::from(fps.max(1))).max(1);
    let total_ms = (jpegs.len() as f64) * (1000.0 / f64::from(fps.max(1)));

    let mut info_body = Vec::new();
    info_body.extend_from_slice(&ebml_uint(&[0x2A, 0xD7, 0xB1], 1_000_000)); // TimecodeScale = 1ms
    info_body.extend_from_slice(&ebml_el(&[0x4D, 0x80], b"PhotoCraft 2027")); // MuxingApp
    info_body.extend_from_slice(&ebml_el(&[0x57, 0x41], b"PhotoCraft 2027")); // WritingApp
    info_body.extend_from_slice(&ebml_el(&[0x44, 0x89], &total_ms.to_be_bytes())); // Duration
    let info = ebml_el(&[0x15, 0x49, 0xA9, 0x66], &info_body);

    let mut video_body = Vec::new();
    video_body.extend_from_slice(&ebml_uint(&[0xB0], u64::from(width))); // PixelWidth
    video_body.extend_from_slice(&ebml_uint(&[0xBA], u64::from(height))); // PixelHeight
    let video = ebml_el(&[0xE0], &video_body);

    let mut track_entry = Vec::new();
    track_entry.extend_from_slice(&ebml_uint(&[0xD7], 1)); // TrackNumber
    track_entry.extend_from_slice(&ebml_uint(&[0x73, 0xC5], 1)); // TrackUID
    track_entry.extend_from_slice(&ebml_uint(&[0x83], 1)); // TrackType = video
    track_entry.extend_from_slice(&ebml_el(&[0x86], b"V_MJPEG")); // CodecID
    track_entry.extend_from_slice(&video);
    let tracks = ebml_el(&[0x16, 0x54, 0xAE, 0x6B], &ebml_el(&[0xAE], &track_entry));

    let mut clusters = Vec::new();
    for (idx, jpg) in jpegs.iter().enumerate() {
        let tc = (idx as u64).saturating_mul(frame_ms);
        let mut cluster_body = Vec::with_capacity(jpg.len() + 16);
        cluster_body.extend_from_slice(&ebml_uint(&[0xE7], tc)); // Timecode
        let mut sb = Vec::with_capacity(4 + jpg.len());
        sb.push(0x81); // Track 1 VINT
        sb.extend_from_slice(&0i16.to_be_bytes()); // relative timecode 0
        sb.push(0x80); // Keyframe flag
        sb.extend_from_slice(jpg);
        cluster_body.extend_from_slice(&ebml_el(&[0xA3], &sb)); // SimpleBlock
        clusters.extend_from_slice(&ebml_el(&[0x1F, 0x43, 0xB6, 0x75], &cluster_body));
    }

    let mut seg_body = Vec::with_capacity(info.len() + tracks.len() + clusters.len());
    seg_body.extend_from_slice(&info);
    seg_body.extend_from_slice(&tracks);
    seg_body.extend_from_slice(&clusters);
    let segment = ebml_el(&[0x18, 0x53, 0x80, 0x67], &seg_body);

    let mut out = Vec::with_capacity(header.len() + segment.len());
    out.extend_from_slice(&header);
    out.extend_from_slice(&segment);
    out
}

/// Encodes RGB8 frames into a looping `GIF89a` animation using `photocraft-codecs`'s palette + LZW
/// encoder per frame.
fn encode_animated_gif(rgb_frames: &[Vec<u8>], w: u32, h: u32, fps: u32) -> Result<Vec<u8>, IoError> {
    let opts = EncodeOptions::default();
    let single_gifs: Vec<Result<Vec<u8>, IoError>> = crate::pixels::par_map(rgb_frames.iter().collect(), |rgb| {
        let img = Image::from_raw(w, h, ChannelLayout::Rgb, CSample::U8, rgb.clone())?;
        Ok(codecs::encode(&img, Format::Gif, &opts)?)
    });
    let gifs: Vec<Vec<u8>> = single_gifs.into_iter().collect::<Result<_, _>>()?;
    if gifs.len() == 1 {
        return Ok(gifs.into_iter().next().unwrap_or_default());
    }

    let delay_cs = (100u32 / fps.max(1)).clamp(1, 65535) as u16;
    let mut out = Vec::new();
    // GIF89a header + Logical Screen Descriptor (no Global Color Table: each frame carries its own
    // Local Color Table so every frame preserves full 256-color fidelity).
    out.extend_from_slice(b"GIF89a");
    out.extend_from_slice(&(w.min(65535) as u16).to_le_bytes());
    out.extend_from_slice(&(h.min(65535) as u16).to_le_bytes());
    out.push(0x70); // 8-bit color resolution, no GCT
    out.push(0); // background color index
    out.push(0); // pixel aspect ratio

    // NETSCAPE2.0 infinite-loop application extension
    out.extend_from_slice(&[
        0x21, 0xFF, 0x0B, b'N', b'E', b'T', b'S', b'C', b'A', b'P', b'E', b'2', b'.', b'0', 0x03, 0x01, 0x00, 0x00, 0x00,
    ]);

    for gif in &gifs {
        append_gif_frame(&mut out, gif, delay_cs)?;
    }
    out.push(0x3B); // GIF trailer
    Ok(out)
}

fn append_gif_frame(out: &mut Vec<u8>, single_gif: &[u8], delay_cs: u16) -> Result<(), IoError> {
    if single_gif.len() < 14 || (!single_gif.starts_with(b"GIF89a") && !single_gif.starts_with(b"GIF87a")) {
        return Err(IoError::Unsupported("invalid intermediate GIF frame".into()));
    }
    let packed = single_gif[10];
    let has_gct = (packed & 0x80) != 0;
    let gct_bits = packed & 0x07;
    let gct_len = if has_gct { 3usize << (gct_bits + 1) } else { 0 };
    let mut pos = 13usize.saturating_add(gct_len);
    let gct_slice = if has_gct { single_gif.get(13..13 + gct_len) } else { None };

    // Skip any extension blocks before Image Descriptor (0x2C).
    while let Some(&intro) = single_gif.get(pos) {
        if intro == 0x2C {
            break;
        }
        if intro == 0x21 {
            pos += 2;
            while let Some(&sub_len) = single_gif.get(pos) {
                pos += 1;
                if sub_len == 0 {
                    break;
                }
                pos = pos.saturating_add(sub_len as usize);
            }
        } else {
            return Err(IoError::Unsupported("unexpected block in intermediate GIF".into()));
        }
    }

    let Some(desc) = single_gif.get(pos..pos + 10) else {
        return Err(IoError::Unsupported("truncated GIF image descriptor".into()));
    };
    pos += 10;
    let img_packed = desc[9];
    let has_lct = (img_packed & 0x80) != 0;
    let lct_bits = img_packed & 0x07;
    let lct_len = if has_lct { 3usize << (lct_bits + 1) } else { 0 };
    let lct_slice = if has_lct { single_gif.get(pos..pos + lct_len) } else { None };
    pos = pos.saturating_add(lct_len);

    // Graphic Control Extension (disposal 1 = do not dispose, delay = delay_cs)
    out.extend_from_slice(&[0x21, 0xF9, 0x04, 0x04]);
    out.extend_from_slice(&delay_cs.to_le_bytes());
    out.extend_from_slice(&[0x00, 0x00]);

    // Image Descriptor + Local Color Table
    out.extend_from_slice(&desc[..9]);
    if let Some(lct) = lct_slice {
        out.push(img_packed);
        out.extend_from_slice(lct);
    } else if let Some(gct) = gct_slice {
        out.push((img_packed & 0x78) | 0x80 | gct_bits);
        out.extend_from_slice(gct);
    } else {
        out.push(img_packed);
    }

    // LZW minimum code size + data sub-blocks up to 0x00 terminator
    let Some(&lzw_min) = single_gif.get(pos) else {
        return Err(IoError::Unsupported("missing GIF LZW code size".into()));
    };
    out.push(lzw_min);
    pos += 1;
    while let Some(&sub_len) = single_gif.get(pos) {
        out.push(sub_len);
        pos += 1;
        if sub_len == 0 {
            break;
        }
        let Some(block) = single_gif.get(pos..pos + sub_len as usize) else {
            return Err(IoError::Unsupported("truncated GIF LZW sub-block".into()));
        };
        out.extend_from_slice(block);
        pos += sub_len as usize;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn sample_frames() -> Vec<TimelapseRawFrame> {
        (0..4)
            .map(|i| {
                let (w, h) = (64u32, 36u32);
                let mut rgba8 = vec![0u8; (w * h * 4) as usize];
                for (px_idx, px) in rgba8.chunks_exact_mut(4).enumerate() {
                    px[0] = ((px_idx + i * 40) % 256) as u8;
                    px[1] = ((px_idx * 3 + i * 25) % 256) as u8;
                    px[2] = (200usize.saturating_sub(i * 30)) as u8;
                    px[3] = 255;
                }
                TimelapseRawFrame { width: w, height: h, rgba8 }
            })
            .collect()
    }

    #[test]
    fn target_dimensions_respect_2560_2k_and_1920_1080p_even_bounds() {
        assert_eq!(target_dimensions(3840, 2160, TimelapseResolution::Res2560), (2560, 1440));
        assert_eq!(target_dimensions(3840, 2160, TimelapseResolution::Res1920), (1920, 1080));
        assert_eq!(target_dimensions(65, 37, TimelapseResolution::Res1920), (64, 36));
    }

    #[test]
    fn encodes_valid_mp4_webm_avi_and_looping_gif_containers() {
        let frames = sample_frames();

        let mp4 = encode_timelapse_frames(&frames, TimelapseResolution::Res1920, TimelapseQuality::High, 24, TimelapseFormat::Mp4).unwrap();
        assert_eq!(&mp4[4..12], b"ftypisom");
        assert!(mp4.windows(4).any(|w| w == b"moov"));
        assert!(mp4.windows(4).any(|w| w == b"mdat"));

        let avi = encode_timelapse_frames(&frames, TimelapseResolution::Res2560, TimelapseQuality::Medium, 30, TimelapseFormat::Avi).unwrap();
        assert_eq!(&avi[0..4], b"RIFF");
        assert_eq!(&avi[8..12], b"AVI ");
        assert!(avi.windows(4).any(|w| w == b"MJPG"));

        let webm = encode_timelapse_frames(&frames, TimelapseResolution::Res1920, TimelapseQuality::Low, 15, TimelapseFormat::Webm).unwrap();
        assert_eq!(&webm[0..4], &[0x1A, 0x45, 0xDF, 0xA3]);
        assert!(webm.windows(4).any(|w| w == b"webm"));

        let gif = encode_timelapse_frames(&frames, TimelapseResolution::Res1920, TimelapseQuality::High, 12, TimelapseFormat::Gif).unwrap();
        assert!(gif.starts_with(b"GIF89a"));
        assert_eq!(gif.last().copied(), Some(0x3B));
        let mut count = 0usize;
        let imported = crate::import_gif_frames("timelapse.gif", &gif, &mut |_, _| {
            count += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(imported.frames, 4);
        assert_eq!(count, 4);
    }
}
