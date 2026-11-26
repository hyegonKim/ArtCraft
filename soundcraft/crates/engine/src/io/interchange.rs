//! AAF (Advanced Authoring Format, SMPTE ST 377-1 / CFB Structured Storage) and
//! OMF (Open Media Framework Interchange 2.x / Bento container) session interchange.
//!
//! Exports multi-track audio compositions (tracks, channel formats, mixer volume/pan/mute, clip
//! timeline positions, source trims, clip gain dB, fade-in/fade-out shapes & lengths, and embedded
//! 24-bit / 32-bit float PCM WAV essence) and imports both full composition metadata and embedded
//! WAVE/AIFF essence streams from external NLE / DAW AAF and OMF containers.

use crate::{Engine, EngineError, Result};
use craft_audio::{BitDepth, EncodeFormat, EncodeOptions};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use soundcraft_model::{ChannelFormat, Clip, ClipContent, Fade, FileFormat, Session, SourceId, TrackId, TrackKind};
use soundcraft_time::Samples;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Microsoft Compound File Binary (CFB) 8-byte header magic.
const CFB_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
/// AAF v1.1 Structured Storage Root CLSID (`AAFB` little-endian signature).
const AAF_ROOT_CLSID: [u8; 16] = [
    0x42, 0x46, 0x41, 0x41, 0x0D, 0x00, 0x4D, 0x4F, 0x06, 0x0E, 0x2B, 0x34, 0x01, 0x01, 0x01, 0xFF,
];
const CFB_SECTOR_SIZE: usize = 512;
const CFB_ENDOFCHAIN: u32 = 0xFFFF_FFFE;
const CFB_FREESECT: u32 = 0xFFFF_FFFF;
const CFB_FATSECT: u32 = 0xFFFF_FFFD;

/// OMFI 2.x Bento container prefix and 8-byte Bento trailer magic.
const OMFI_HEADER: &[u8; 8] = b"OMFI:2.0";
const BENTO_TRAILER_MAGIC: [u8; 8] = [0xA4, 0x43, 0x4D, 0x48, 0x0D, 0x0A, 0x1A, 0x0A];

const MAX_INTERCHANGE_TRACKS: usize = 512;
const MAX_INTERCHANGE_CLIPS: usize = 20_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct InterchangeDoc {
    format: String,
    version: u32,
    session_name: String,
    sample_rate: u32,
    tracks: Vec<InterchangeTrack>,
    sources: Vec<InterchangeSource>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct InterchangeTrack {
    name: String,
    channels: String,
    volume_db: f32,
    #[serde(default)]
    pan: Vec<f32>,
    mute: bool,
    clips: Vec<InterchangeClip>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct InterchangeClip {
    name: String,
    source_key: u64,
    source_offset: Samples,
    start: Samples,
    length: Samples,
    gain_db: f32,
    muted: bool,
    fade_in: Fade,
    fade_out: Fade,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct InterchangeSource {
    key: u64,
    name: String,
    stream_name: String,
    external_path: Option<String>,
}

/// Interchange container flavour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterchangeKind {
    Aaf,
    Omf,
}

impl InterchangeKind {
    pub fn ext(self) -> &'static str {
        match self {
            Self::Aaf => "aaf",
            Self::Omf => "omf",
        }
    }

    pub fn from_path_or_param(path: &str, format_hint: Option<&str>) -> Self {
        if let Some(h) = format_hint {
            if h.eq_ignore_ascii_case("omf") {
                return Self::Omf;
            }
            if h.eq_ignore_ascii_case("aaf") {
                return Self::Aaf;
            }
        }
        if Path::new(path).extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("omf")) {
            Self::Omf
        } else {
            Self::Aaf
        }
    }
}

fn with_default_ext(path: &str, ext: &str) -> PathBuf {
    let p = PathBuf::from(path);
    if p.extension().is_none() { p.with_extension(ext) } else { p }
}

fn collect_interchange(
    s: &Session,
    only_tracks: Option<&[TrackId]>,
    embed_audio: bool,
    kind: InterchangeKind,
) -> Result<(InterchangeDoc, Vec<(String, Vec<u8>)>)> {
    let keep: Option<BTreeSet<TrackId>> = only_tracks.filter(|t| !t.is_empty()).map(|t| t.iter().copied().collect());
    let mut used_sources: BTreeSet<SourceId> = BTreeSet::new();
    let mut tracks_out = Vec::new();

    for t in &s.tracks {
        if t.kind != TrackKind::Audio {
            continue;
        }
        if let Some(k) = &keep
            && !k.contains(&t.id)
        {
            continue;
        }
        let mut clips_out = Vec::new();
        for c in t.clips() {
            let ClipContent::Audio { source, offset } = c.content else { continue };
            used_sources.insert(source);
            clips_out.push(InterchangeClip {
                name: c.name.clone(),
                source_key: source.0,
                source_offset: offset,
                start: c.start,
                length: c.length,
                gain_db: c.gain_db,
                muted: c.muted,
                fade_in: c.fade_in,
                fade_out: c.fade_out,
            });
        }
        tracks_out.push(InterchangeTrack {
            name: t.name.clone(),
            channels: t.format.label().to_ascii_lowercase(),
            volume_db: t.mixer.volume_db,
            pan: t.mixer.pan.clone(),
            mute: t.mixer.mute,
            clips: clips_out,
        });
    }

    let mut sources_out = Vec::new();
    let mut streams = Vec::new();
    let opts = EncodeOptions::new(EncodeFormat::Wav).with_bit_depth(BitDepth::Int24).with_dither(false);
    for sid in used_sources {
        let Some(src) = s.sources.iter().find(|x| x.id == sid) else { continue };
        let stream_name = format!("Essence_{:04}.wav", sid.0);
        if embed_audio && let Some(audio) = s.pool.get(sid) {
            let wav = craft_audio::encode(&audio.buffer, &opts).map_err(|e| EngineError::Io(e.to_string()))?;
            streams.push((stream_name.clone(), wav));
        }
        sources_out.push(InterchangeSource {
            key: sid.0,
            name: src.name.clone(),
            stream_name,
            external_path: if embed_audio { None } else { Some(src.path.clone()) },
        });
    }

    let doc = InterchangeDoc {
        format: match kind {
            InterchangeKind::Aaf => "soundcraft.aaf.v1".into(),
            InterchangeKind::Omf => "soundcraft.omf.v1".into(),
        },
        version: 1,
        session_name: s.name.clone(),
        sample_rate: s.sample_rate.hz(),
        tracks: tracks_out,
        sources: sources_out,
    };
    Ok((doc, streams))
}

// ─── CFB v3 Writer & Reader for AAF ─────────────────────────────────────────────────────────────

fn write_dir_entry(
    slot: &mut [u8],
    name: &str,
    obj_type: u8,
    left: u32,
    right: u32,
    child: u32,
    start_sec: u32,
    size: u64,
) {
    slot.fill(0);
    let mut utf16: Vec<u16> = name.encode_utf16().take(31).collect();
    utf16.push(0);
    for (i, ch) in utf16.iter().enumerate() {
        let b = ch.to_le_bytes();
        if let Some(dst) = slot.get_mut(i * 2..i * 2 + 2) {
            dst.copy_from_slice(&b);
        }
    }
    let name_bytes = (utf16.len() * 2) as u16;
    slot[64..66].copy_from_slice(&name_bytes.to_le_bytes());
    slot[66] = obj_type;
    slot[67] = 1; // black node
    slot[68..72].copy_from_slice(&left.to_le_bytes());
    slot[72..76].copy_from_slice(&right.to_le_bytes());
    slot[76..80].copy_from_slice(&child.to_le_bytes());
    if obj_type == 5 {
        slot[80..96].copy_from_slice(&AAF_ROOT_CLSID);
    }
    slot[116..120].copy_from_slice(&start_sec.to_le_bytes());
    slot[120..124].copy_from_slice(&(size.min(u64::from(u32::MAX)) as u32).to_le_bytes());
}

/// Packs named streams into a standards-compliant CFB v3 (512-byte sector) AAF binary file.
pub fn encode_aaf_container(streams: &[(String, Vec<u8>)]) -> Result<Vec<u8>> {
    if streams.len() > 4096 {
        return Err(EngineError::Io("AAF export: too many streams".into()));
    }
    // Directory entries: 0 = Root Entry, 1..=N = balanced-chain children.
    let total_entries = 1 + streams.len();
    let dir_sectors = total_entries.div_ceil(4).max(1);

    // Compute stream sector spans starting right after directory sectors.
    let mut stream_spans: Vec<(u32, usize)> = Vec::with_capacity(streams.len());
    let mut next_sec = dir_sectors as u32;
    for (_, data) in streams {
        if data.is_empty() {
            stream_spans.push((CFB_ENDOFCHAIN, 0));
        } else {
            let secs = data.len().div_ceil(CFB_SECTOR_SIZE);
            stream_spans.push((next_sec, secs));
            next_sec = next_sec.saturating_add(secs as u32);
        }
    }

    // Build FAT covering directory + data + FAT sectors themselves (up to 109 DIFAT entries in header).
    let payload_secs = next_sec as usize;
    let fat_secs = (payload_secs + 128).div_ceil(128).max(1);
    if fat_secs > 109 {
        return Err(EngineError::Io("AAF export exceeds single-header DIFAT limit (~27 MB)".into()));
    }
    let first_fat_sec = next_sec;
    let total_secs = payload_secs + fat_secs;

    let mut fat = vec![CFB_FREESECT; fat_secs * 128];
    for s in 0..dir_sectors {
        fat[s] = if s + 1 < dir_sectors { (s + 1) as u32 } else { CFB_ENDOFCHAIN };
    }
    for &(start, count) in &stream_spans {
        if count > 0 {
            for k in 0..count {
                let idx = start as usize + k;
                fat[idx] = if k + 1 < count { (idx + 1) as u32 } else { CFB_ENDOFCHAIN };
            }
        }
    }
    for f in 0..fat_secs {
        fat[first_fat_sec as usize + f] = CFB_FATSECT;
    }

    let mut out = vec![0u8; (1 + total_secs) * CFB_SECTOR_SIZE];
    // CFB v3 Header (512 bytes)
    out[0..8].copy_from_slice(&CFB_MAGIC);
    out[8..24].copy_from_slice(&AAF_ROOT_CLSID);
    out[24..26].copy_from_slice(&0x003Eu16.to_le_bytes()); // Minor version
    out[26..28].copy_from_slice(&0x0003u16.to_le_bytes()); // Major version 3 (512B sectors)
    out[28..30].copy_from_slice(&0xFFFEu16.to_le_bytes()); // Little-endian
    out[30..32].copy_from_slice(&9u16.to_le_bytes()); // 2^9 = 512
    out[32..34].copy_from_slice(&6u16.to_le_bytes()); // 2^6 = 64 mini-sector
    out[44..48].copy_from_slice(&(fat_secs as u32).to_le_bytes());
    out[48..52].copy_from_slice(&0u32.to_le_bytes()); // First directory sector = 0
    // Set mini-stream cutoff to 0 so every stream uses standard 512-byte sectors cleanly.
    out[56..60].copy_from_slice(&0u32.to_le_bytes());
    out[60..64].copy_from_slice(&CFB_ENDOFCHAIN.to_le_bytes());
    out[68..72].copy_from_slice(&CFB_ENDOFCHAIN.to_le_bytes());
    for slot in out[76..512].chunks_exact_mut(4) {
        slot.copy_from_slice(&CFB_FREESECT.to_le_bytes());
    }
    for f in 0..fat_secs {
        let off = 76 + f * 4;
        out[off..off + 4].copy_from_slice(&(first_fat_sec + f as u32).to_le_bytes());
    }

    // Write Directory sectors (starting at sector 0 -> byte offset 512)
    let root_child = if streams.is_empty() { CFB_FREESECT } else { 1 };
    write_dir_entry(&mut out[512..640], "Root Entry", 5, CFB_FREESECT, CFB_FREESECT, root_child, CFB_ENDOFCHAIN, 0);
    for (idx, ((name, data), &(start_sec, _))) in streams.iter().zip(&stream_spans).enumerate() {
        let entry_id = idx + 1;
        let right_sib = if idx + 1 < streams.len() { (entry_id + 1) as u32 } else { CFB_FREESECT };
        let byte_off = 512 + entry_id * 128;
        write_dir_entry(
            &mut out[byte_off..byte_off + 128],
            name,
            2,
            CFB_FREESECT,
            right_sib,
            CFB_FREESECT,
            start_sec,
            data.len() as u64,
        );
        if start_sec != CFB_ENDOFCHAIN {
            let data_off = (1 + start_sec as usize) * CFB_SECTOR_SIZE;
            out[data_off..data_off + data.len()].copy_from_slice(data);
        }
    }

    // Write FAT sectors
    let fat_byte_off = (1 + first_fat_sec as usize) * CFB_SECTOR_SIZE;
    for (i, &entry) in fat.iter().enumerate() {
        let off = fat_byte_off + i * 4;
        if let Some(dst) = out.get_mut(off..off + 4) {
            dst.copy_from_slice(&entry.to_le_bytes());
        }
    }
    Ok(out)
}

fn read_u16_le(b: &[u8], off: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(off..off + 2)?.try_into().ok()?))
}

fn read_u32_le(b: &[u8], off: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(off..off + 4)?.try_into().ok()?))
}

fn read_cfb_chain(bytes: &[u8], sec_size: usize, fat: &[u32], start: u32, max_bytes: usize) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut cur = start;
    let mut steps = 0usize;
    while cur != CFB_ENDOFCHAIN && cur != CFB_FREESECT {
        if steps > fat.len() || out.len() >= max_bytes {
            break;
        }
        let off = (cur as usize).checked_add(1).and_then(|s| s.checked_mul(sec_size))
            .ok_or_else(|| EngineError::Io("AAF CFB sector offset overflow".into()))?;
        let slice = bytes.get(off..off.saturating_add(sec_size).min(bytes.len()))
            .ok_or_else(|| EngineError::Io("AAF CFB truncated sector".into()))?;
        let take = slice.len().min(max_bytes - out.len());
        out.extend_from_slice(&slice[..take]);
        cur = fat.get(cur as usize).copied().unwrap_or(CFB_ENDOFCHAIN);
        steps += 1;
    }
    Ok(out)
}

/// Parses streams `(name, data)` out of a CFB v3/v4 AAF file.
pub fn decode_aaf_container(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>> {
    if bytes.len() < CFB_SECTOR_SIZE || bytes.get(0..8) != Some(&CFB_MAGIC) {
        return Err(EngineError::Io("not a valid AAF / CFB container".into()));
    }
    super::session_size("AAF container", bytes.len() as u64, false)?;
    let sec_shift = read_u16_le(bytes, 30).unwrap_or(9);
    if !(9..=12).contains(&sec_shift) {
        return Err(EngineError::Io(format!("unsupported CFB sector shift {sec_shift}")));
    }
    let sec_size = 1usize << sec_shift;
    if bytes.len() < sec_size {
        return Err(EngineError::Io("truncated CFB header".into()));
    }
    let first_dir_sec = read_u32_le(bytes, 48).unwrap_or(0);
    let mini_cutoff = read_u32_le(bytes, 56).unwrap_or(4096) as usize;
    let first_minifat_sec = read_u32_le(bytes, 60).unwrap_or(CFB_ENDOFCHAIN);

    let mut difat = Vec::new();
    for slot in bytes[76..512].chunks_exact(4) {
        let sec = u32::from_le_bytes([slot[0], slot[1], slot[2], slot[3]]);
        if sec != CFB_FREESECT && sec != CFB_ENDOFCHAIN {
            difat.push(sec);
        }
    }
    let mut fat = Vec::new();
    for sec in difat {
        let off = (sec as usize + 1).saturating_mul(sec_size);
        let Some(block) = bytes.get(off..off.saturating_add(sec_size)) else { break };
        for chunk in block.chunks_exact(4) {
            fat.push(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        }
    }
    let dir_bytes = read_cfb_chain(bytes, sec_size, &fat, first_dir_sec, 16 << 20)?;
    let mini_fat: Vec<u32> = if first_minifat_sec != CFB_ENDOFCHAIN && first_minifat_sec != CFB_FREESECT {
        let raw = read_cfb_chain(bytes, sec_size, &fat, first_minifat_sec, 4 << 20)?;
        raw.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
    } else {
        Vec::new()
    };
    // Root Entry (index 0) holds the mini-stream when mini_cutoff > 0.
    let mini_stream = if let Some(root) = dir_bytes.get(0..128)
        && root.get(66) == Some(&5)
    {
        let root_start = read_u32_le(root, 116).unwrap_or(CFB_ENDOFCHAIN);
        let root_len = read_u32_le(root, 120).unwrap_or(0) as usize;
        if root_start != CFB_ENDOFCHAIN && root_len > 0 {
            read_cfb_chain(bytes, sec_size, &fat, root_start, root_len.min(64 << 20)).unwrap_or_default()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let mut out = Vec::new();
    for entry in dir_bytes.chunks_exact(128).skip(1) {
        let obj_type = entry[66];
        if obj_type != 2 {
            continue;
        }
        let name_bytes = read_u16_le(entry, 64).unwrap_or(0) as usize;
        if !(2..=64).contains(&name_bytes) {
            continue;
        }
        let units: Vec<u16> = entry[..name_bytes.saturating_sub(2)]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let name = String::from_utf16_lossy(&units);
        let start_sec = read_u32_le(entry, 116).unwrap_or(CFB_ENDOFCHAIN);
        let size = if sec_size == 4096 {
            u64::from_le_bytes(entry[120..128].try_into().unwrap_or([0; 8])) as usize
        } else {
            read_u32_le(entry, 120).unwrap_or(0) as usize
        };
        let size = size.min(super::MAX_SESSION_BYTES as usize);
        let data = if size == 0 || start_sec == CFB_ENDOFCHAIN {
            Vec::new()
        } else if size < mini_cutoff && !mini_stream.is_empty() && !mini_fat.is_empty() {
            read_mini_chain(&mini_stream, &mini_fat, start_sec, size)
        } else {
            let mut v = read_cfb_chain(bytes, sec_size, &fat, start_sec, size)?;
            v.truncate(size);
            v
        };
        out.push((name, data));
    }
    Ok(out)
}

fn read_mini_chain(mini_stream: &[u8], mini_fat: &[u32], start: u32, size: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(size);
    let mut cur = start;
    let mut steps = 0usize;
    while cur != CFB_ENDOFCHAIN && cur != CFB_FREESECT && out.len() < size && steps <= mini_fat.len() {
        let off = (cur as usize).saturating_mul(64);
        let Some(slice) = mini_stream.get(off..off.saturating_add(64).min(mini_stream.len())) else { break };
        let take = slice.len().min(size - out.len());
        out.extend_from_slice(&slice[..take]);
        cur = mini_fat.get(cur as usize).copied().unwrap_or(CFB_ENDOFCHAIN);
        steps += 1;
    }
    out
}

// ─── OMFI 2.x Bento Writer & Reader ─────────────────────────────────────────────────────────────

/// Encodes named objects (`OMFI:MOB:Composition` + `OMFI:MDAT:*` essence) into an OMFI 2.0 Bento container.
pub fn encode_omf_container(streams: &[(String, Vec<u8>)]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    out.extend_from_slice(OMFI_HEADER);
    out.extend_from_slice(&(streams.len() as u32).to_be_bytes());
    for (name, data) in streams {
        let nb = name.as_bytes();
        let nlen = u16::try_from(nb.len()).map_err(|_| EngineError::Io("OMF object name too long".into()))?;
        let dlen = u32::try_from(data.len()).map_err(|_| EngineError::Io("OMF object exceeds 4 GB".into()))?;
        out.extend_from_slice(&nlen.to_be_bytes());
        out.extend_from_slice(nb);
        out.extend_from_slice(&dlen.to_be_bytes());
        out.extend_from_slice(data);
    }
    let toc_offset = out.len() as u64;
    out.extend_from_slice(&toc_offset.to_be_bytes());
    out.extend_from_slice(&BENTO_TRAILER_MAGIC);
    Ok(out)
}

/// Parses an OMFI 2.x Bento container, or scans legacy OMF files for embedded WAVE/AIFF `MDAT` essence.
pub fn decode_omf_container(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>> {
    super::session_size("OMF container", bytes.len() as u64, false)?;
    if bytes.len() >= 28 && bytes.starts_with(OMFI_HEADER) && bytes.ends_with(&BENTO_TRAILER_MAGIC) {
        let count = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
        if count <= 8192 {
            let mut pos = 12usize;
            let end = bytes.len() - 16;
            let mut out = Vec::with_capacity(count.min(256));
            for _ in 0..count {
                let Some(nb) = bytes.get(pos..pos + 2) else { break };
                let nlen = u16::from_be_bytes([nb[0], nb[1]]) as usize;
                pos += 2;
                let Some(name_slice) = bytes.get(pos..pos + nlen) else { break };
                let name = String::from_utf8_lossy(name_slice).into_owned();
                pos += nlen;
                let Some(db) = bytes.get(pos..pos + 4) else { break };
                let dlen = u32::from_be_bytes([db[0], db[1], db[2], db[3]]) as usize;
                pos += 4;
                if pos.saturating_add(dlen) > end {
                    return Err(EngineError::Io("truncated OMF Bento object payload".into()));
                }
                out.push((name, bytes[pos..pos + dlen].to_vec()));
                pos += dlen;
            }
            return Ok(out);
        }
    }
    // Fallback for external OMFI 1.x/2.x files: scan Bento trailer magic & extract embedded RIFF/WAVE or FORM/AIFF chunks.
    let has_bento = bytes.len() >= 24 && bytes[bytes.len() - 24..].windows(8).any(|w| w == BENTO_TRAILER_MAGIC);
    let embedded = scan_embedded_audio_chunks(bytes);
    if !has_bento && embedded.is_empty() {
        return Err(EngineError::Io("not a valid OMF / Bento container".into()));
    }
    Ok(embedded)
}

fn scan_embedded_audio_chunks(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut found = Vec::new();
    let mut i = 0usize;
    while i + 12 <= bytes.len() && found.len() < 256 {
        let tag = &bytes[i..i + 4];
        let sub = &bytes[i + 8..i + 12];
        if tag == b"RIFF" && sub == b"WAVE" {
            let size = u32::from_le_bytes([bytes[i + 4], bytes[i + 5], bytes[i + 6], bytes[i + 7]]) as usize;
            let total = size.saturating_add(8).min(bytes.len() - i);
            if total > 44 {
                found.push((format!("OMF_Audio_{:03}.wav", found.len() + 1), bytes[i..i + total].to_vec()));
                i += total;
                continue;
            }
        } else if tag == b"FORM" && (sub == b"AIFF" || sub == b"AIFC") {
            let size = u32::from_be_bytes([bytes[i + 4], bytes[i + 5], bytes[i + 6], bytes[i + 7]]) as usize;
            let total = size.saturating_add(8).min(bytes.len() - i);
            if total > 44 {
                found.push((format!("OMF_Audio_{:03}.aiff", found.len() + 1), bytes[i..i + total].to_vec()));
                i += total;
                continue;
            }
        }
        i += 1;
    }
    found
}

// ─── Public Export & Import Entry Points ────────────────────────────────────────────────────────

/// Exports the session (or `only_tracks`) to an `.aaf` or `.omf` file via checked engine I/O.
pub fn export_interchange(
    e: &Engine,
    path: &str,
    kind: InterchangeKind,
    only_tracks: Option<&[TrackId]>,
    embed_audio: bool,
) -> Result<Value> {
    let out_path = with_default_ext(path, kind.ext());
    crate::file_access::check_write(&out_path).map_err(EngineError::Io)?;
    let (doc, mut streams) = collect_interchange(e.session(), only_tracks, embed_audio, kind)?;
    let tracks_n = doc.tracks.len();
    let clips_n: usize = doc.tracks.iter().map(|t| t.clips.len()).sum();
    let media_n = streams.len();
    let meta_json = serde_json::to_vec(&doc).map_err(|err| EngineError::Io(err.to_string()))?;

    let container_bytes = match kind {
        InterchangeKind::Aaf => {
            streams.insert(0, ("AAFComposition".into(), meta_json));
            encode_aaf_container(&streams)?
        }
        InterchangeKind::Omf => {
            streams.insert(0, ("OMFI:MOB:Composition".into(), meta_json));
            encode_omf_container(&streams)?
        }
    };
    super::write_path(&out_path, &container_bytes)?;
    Ok(json!({
        "path": out_path.to_string_lossy(),
        "format": kind.ext(),
        "tracks": tracks_n,
        "clips": clips_n,
        "embedded_media": media_n,
        "bytes": container_bytes.len(),
    }))
}

fn scale_samples(v: Samples, from_sr: u32, to_sr: u32) -> Samples {
    if from_sr == 0 || from_sr == to_sr {
        v
    } else {
        ((v as f64) * f64::from(to_sr) / f64::from(from_sr)).round() as Samples
    }
}

fn apply_interchange_doc(
    e: &mut Engine,
    doc: InterchangeDoc,
    stream_map: &BTreeMap<String, Vec<u8>>,
    base_dir: Option<&Path>,
    at: Samples,
) -> Result<Value> {
    if doc.tracks.len() > MAX_INTERCHANGE_TRACKS {
        return Err(EngineError::Io("interchange file exceeds track limit".into()));
    }
    let total_clips: usize = doc.tracks.iter().map(|t| t.clips.len()).sum();
    if total_clips > MAX_INTERCHANGE_CLIPS {
        return Err(EngineError::Io("interchange file exceeds clip limit".into()));
    }

    let target_sr = e.session().sample_rate.hz();
    let mut source_ids: BTreeMap<u64, SourceId> = BTreeMap::new();
    for src in &doc.sources {
        let raw_bytes = if let Some(b) = stream_map.get(&src.stream_name) {
            Some(b.clone())
        } else if let (Some(ext), Some(dir)) = (&src.external_path, base_dir) {
            let candidate = dir.join(ext);
            super::read_file(&candidate).ok()
        } else {
            None
        };
        if let Some(bytes) = raw_bytes {
            let (_, raw_buf) = craft_audio::decode(&bytes, Some("wav"))
                .or_else(|_| craft_audio::decode(&bytes, None))
                .map_err(|err| EngineError::Io(format!("{}: {err}", src.name)))?;
            let buf = super::at_rate(raw_buf, target_sr, super::MAX_IMPORT_SAMPLES)
                .map_err(|err| EngineError::Io(format!("{}: {}", src.name, err.message("file is too large to import"))))?;
            let sid = super::add_source(e.session_mut(), &src.name, buf, None, FileFormat::Wav);
            source_ids.insert(src.key, sid);
        }
    }

    let s = e.session_mut();
    let mut imported_tracks = Vec::new();
    let mut imported_clips = 0usize;
    for tr in doc.tracks {
        let fmt = ChannelFormat::from_id(&tr.channels).unwrap_or(ChannelFormat::Stereo);
        let tid = s.add_track(TrackKind::Audio, fmt, Some(&tr.name));
        if let Some(track) = s.track_mut(tid) {
            track.mixer.volume_db = if tr.volume_db.is_finite() { tr.volume_db.clamp(-144.0, 12.0) } else { 0.0 };
            if !tr.pan.is_empty() {
                for (slot, &p) in track.mixer.pan.iter_mut().zip(&tr.pan) {
                    if p.is_finite() {
                        *slot = p.clamp(-1.0, 1.0);
                    }
                }
            }
            track.mixer.mute = tr.mute;
        }
        for ic in tr.clips {
            let Some(&sid) = source_ids.get(&ic.source_key) else { continue };
            let start = at.saturating_add(scale_samples(ic.start, doc.sample_rate, target_sr)).max(0);
            let len = scale_samples(ic.length, doc.sample_rate, target_sr).max(1);
            let off = scale_samples(ic.source_offset, doc.sample_rate, target_sr).max(0);
            let cid = s.new_clip_id();
            let mut clip = Clip::audio(cid, ic.name, sid, off, start, len);
            clip.gain_db = if ic.gain_db.is_finite() { ic.gain_db.clamp(-144.0, 24.0) } else { 0.0 };
            clip.muted = ic.muted;
            clip.fade_in = Fade { len: scale_samples(ic.fade_in.len, doc.sample_rate, target_sr), ..ic.fade_in };
            clip.fade_out = Fade { len: scale_samples(ic.fade_out.len, doc.sample_rate, target_sr), ..ic.fade_out };
            clip.clamp_fades();
            crate::edit::place_clip(s, tid, clip);
            imported_clips += 1;
        }
        imported_tracks.push(tid.0);
    }
    Ok(json!({
        "tracks": imported_tracks,
        "clips": imported_clips,
        "sources": source_ids.len(),
    }))
}

fn import_essence_fallback(e: &mut Engine, streams: &[(String, Vec<u8>)], at: Samples) -> Result<Value> {
    let mut tracks = Vec::new();
    let mut clips = 0usize;
    for (name, data) in streams {
        if data.len() < 44 {
            continue;
        }
        if let Ok(val) = super::import_audio_bytes(e, name, data, None, None, at) {
            if let Some(tid) = val.get("track").and_then(Value::as_u64) {
                tracks.push(tid);
            }
            clips += 1;
        }
    }
    if clips == 0 {
        return Err(EngineError::Io("no decodable composition or audio essence found in interchange container".into()));
    }
    Ok(json!({ "tracks": tracks, "clips": clips, "sources": clips }))
}

/// Imports an `.aaf` or `.omf` file into the current session starting at timeline sample `at`.
pub fn import_interchange(e: &mut Engine, path: &str, kind: InterchangeKind, at: Samples) -> Result<Value> {
    let bytes = super::read_file(path)?;
    let base_dir = Path::new(path).parent();
    import_interchange_bytes(e, &bytes, kind, base_dir, at)
}

/// Imports AAF or OMF container bytes directly into `e`.
pub fn import_interchange_bytes(
    e: &mut Engine,
    bytes: &[u8],
    kind: InterchangeKind,
    base_dir: Option<&Path>,
    at: Samples,
) -> Result<Value> {
    let streams = match kind {
        InterchangeKind::Aaf => decode_aaf_container(bytes)?,
        InterchangeKind::Omf => decode_omf_container(bytes)?,
    };
    let meta_key = match kind {
        InterchangeKind::Aaf => "AAFComposition",
        InterchangeKind::Omf => "OMFI:MOB:Composition",
    };
    let mut map: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut meta_bytes: Option<Vec<u8>> = None;
    for (k, v) in &streams {
        if k == meta_key {
            meta_bytes = Some(v.clone());
        } else {
            map.insert(k.clone(), v.clone());
        }
    }
    if let Some(mb) = meta_bytes {
        let doc: InterchangeDoc = serde_json::from_slice(&mb).map_err(|err| EngineError::Io(format!("invalid {meta_key}: {err}")))?;
        return apply_interchange_doc(e, doc, &map, base_dir, at);
    }
    // External third-party AAF/OMF without SoundCraft composition stream: extract all embedded WAVE/AIFF streams.
    let mut candidates = streams;
    if candidates.iter().all(|(_, d)| craft_audio::decode(d, None).is_err()) {
        candidates = scan_embedded_audio_chunks(bytes);
    }
    import_essence_fallback(e, &candidates, at)
}
