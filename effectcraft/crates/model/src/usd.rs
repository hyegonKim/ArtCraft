//! OpenUSD ASCII (`.usda` / `.usd` with `#usda` header) and `.usdz` package importer.
//!
//! Parses hierarchical `Xform`, `Scope`, and `Mesh` prims (n-gon fan triangulation, normals,
//! `primvars:st` / `texCoords` with indexed primvars, `GeomSubset` per-face material bindings,
//! and `xformOp:*` transforms) together with `Material` / `Shader` networks (`UsdPreviewSurface`
//! and `UsdUVTexture`), plus uncompressed and deflated `.usdz` archives within bounded memory
//! and [`MAX_IMPORT_TEXTURE_PIXELS`].

use std::collections::HashMap;
use std::sync::Arc;

use crate::{
    AlphaMode, MAX_IMPORT_TEXTURE_PIXELS, Mat4, Material, Mesh, Model, ModelError, Node, Primitive,
    Result, TexRef, Texture, Wrap, perr, quat_matrix, vec3,
};

const MAX_USD_DEPTH: usize = 128;
const MAX_USDA_TOKENS: usize = 8_000_000;
const MAX_USD_PRIMS: usize = 65_536;
const MAX_USD_VERTICES: usize = 4_000_000;
const MAX_USDZ_ENTRIES: usize = 4_096;
const MAX_USDZ_TOTAL_BYTES: usize = 256 * 1024 * 1024;

/// Parse an OpenUSD `.usda`, ASCII `.usd`, or `.usdz` archive.
pub fn parse(bytes: &[u8], resolve: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<Model> {
    if is_zip_magic(bytes) {
        return parse_usdz(bytes);
    }
    if bytes.starts_with(b"PXR-USDC") {
        return Err(ModelError::Unsupported(
            "binary USD Crate (.usdc) is not supported; use ASCII .usda or .usdz".into(),
        ));
    }
    if starts_with_usda(bytes) {
        return parse_usda(bytes, resolve);
    }
    Err(perr("usd: expected `#usda` header or `.usdz` archive"))
}

pub(crate) fn is_zip_magic(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06")
}

pub(crate) fn starts_with_usda(bytes: &[u8]) -> bool {
    let b = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let trimmed = trim_ascii_start(b);
    trimmed.starts_with(b"#usda")
}

fn trim_ascii_start(mut b: &[u8]) -> &[u8] {
    while let Some((&first, rest)) = b.split_first() {
        if first.is_ascii_whitespace() {
            b = rest;
        } else {
            break;
        }
    }
    b
}

// ---------------------------------------------------------------------------
// USDZ ZIP Container + Bounded RFC 1951 Inflate
// ---------------------------------------------------------------------------

/// Parse a `.usdz` package: extracts the root `.usda`/`.usd` stage and resolves embedded textures.
pub fn parse_usdz(bytes: &[u8]) -> Result<Model> {
    let entries = read_zip_entries(bytes)?;
    if entries.is_empty() {
        return Err(perr("usdz: archive contains no files"));
    }
    let stage_idx = entries
        .iter()
        .position(|(name, _)| {
            let ext = name.rsplit('.').next().unwrap_or("");
            ext.eq_ignore_ascii_case("usda")
                || ext.eq_ignore_ascii_case("usd")
                || ext.eq_ignore_ascii_case("usdc")
        })
        .ok_or_else(|| perr("usdz: archive contains no `.usda` or `.usd` root stage"))?;

    let (stage_name, stage_bytes) = &entries[stage_idx];
    if stage_bytes.starts_with(b"PXR-USDC") {
        return Err(ModelError::Unsupported(format!(
            "usdz root stage `{stage_name}` uses binary Crate (.usdc); export with ASCII `.usda` root stage"
        )));
    }
    let stage_dir = stage_name.rsplit_once('/').map_or("", |(d, _)| d);
    let mut map: HashMap<String, &[u8]> = HashMap::with_capacity(entries.len());
    for (k, v) in &entries {
        map.insert(k.to_ascii_lowercase(), v.as_slice());
    }
    parse_usda(stage_bytes, &|uri| resolve_in_archive(&map, stage_dir, uri))
}

fn resolve_in_archive(map: &HashMap<String, &[u8]>, base_dir: &str, uri: &str) -> Option<Vec<u8>> {
    let clean = normalize_archive_path(base_dir, uri)?;
    if let Some(b) = map.get(&clean.to_ascii_lowercase()) {
        return Some((*b).to_vec());
    }
    let direct = normalize_archive_path("", uri)?;
    map.get(&direct.to_ascii_lowercase()).map(|b| (*b).to_vec())
}

fn normalize_archive_path(base: &str, rel: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    if !rel.starts_with('/') && !base.is_empty() {
        for seg in base.split(['/', '\\']) {
            if !seg.is_empty() && seg != "." {
                parts.push(seg);
            }
        }
    }
    for seg in rel.split(['/', '\\']) {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    Some(parts.join("/"))
}

fn read_u16_le(b: &[u8], off: usize) -> Option<u16> {
    let s = b.get(off..off.checked_add(2)?)?;
    Some(u16::from_le_bytes([s[0], s[1]]))
}

fn read_u32_le(b: &[u8], off: usize) -> Option<u32> {
    let s = b.get(off..off.checked_add(4)?)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn read_zip_entries(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>> {
    if !is_zip_magic(bytes) {
        return Err(perr("usdz: invalid ZIP header"));
    }
    if let Some(cd_entries) = try_read_central_directory(bytes) {
        return cd_entries;
    }
    read_local_headers(bytes)
}

fn try_read_central_directory(bytes: &[u8]) -> Option<Result<Vec<(String, Vec<u8>)>>> {
    if bytes.len() < 22 {
        return None;
    }
    let min_off = bytes.len().saturating_sub(65_557);
    let mut eocd = None;
    for pos in (min_off..=bytes.len() - 22).rev() {
        if bytes.get(pos..pos + 4) == Some(b"PK\x05\x06") {
            let comment_len = read_u16_le(bytes, pos + 20)? as usize;
            if pos + 22 + comment_len <= bytes.len() {
                eocd = Some(pos);
                break;
            }
        }
    }
    let eocd = eocd?;
    let total_entries = read_u16_le(bytes, eocd + 10)? as usize;
    let cd_size = read_u32_le(bytes, eocd + 12)? as usize;
    let cd_offset = read_u32_le(bytes, eocd + 16)? as usize;
    if total_entries > MAX_USDZ_ENTRIES {
        return Some(Err(perr("usdz: too many archive entries")));
    }
    let cd_end = cd_offset.checked_add(cd_size)?;
    if cd_end > bytes.len() {
        return Some(Err(perr("usdz: truncated central directory")));
    }
    Some(extract_from_cd(bytes, cd_offset, cd_end, total_entries))
}

fn extract_from_cd(
    bytes: &[u8],
    mut pos: usize,
    cd_end: usize,
    expected: usize,
) -> Result<Vec<(String, Vec<u8>)>> {
    let mut out = Vec::with_capacity(expected.min(64));
    let mut budget = MAX_USDZ_TOTAL_BYTES;
    for _ in 0..expected {
        if pos >= cd_end {
            break;
        }
        if bytes.get(pos..pos.saturating_add(4)) != Some(b"PK\x01\x02") {
            return Err(perr("usdz: malformed central directory entry"));
        }
        let method = read_u16_le(bytes, pos + 10).ok_or_else(|| perr("usdz: truncated cd"))?;
        let comp_size = read_u32_le(bytes, pos + 20).ok_or_else(|| perr("usdz: truncated cd"))? as usize;
        let uncomp_size = read_u32_le(bytes, pos + 24).ok_or_else(|| perr("usdz: truncated cd"))? as usize;
        let name_len = read_u16_le(bytes, pos + 28).ok_or_else(|| perr("usdz: truncated cd"))? as usize;
        let extra_len = read_u16_le(bytes, pos + 30).ok_or_else(|| perr("usdz: truncated cd"))? as usize;
        let comment_len = read_u16_le(bytes, pos + 32).ok_or_else(|| perr("usdz: truncated cd"))? as usize;
        let lfh_off = read_u32_le(bytes, pos + 42).ok_or_else(|| perr("usdz: truncated cd"))? as usize;
        let name_start = pos.checked_add(46).ok_or_else(|| perr("usdz: cd overflow"))?;
        let name_end = name_start.checked_add(name_len).ok_or_else(|| perr("usdz: cd overflow"))?;
        let next = name_end
            .checked_add(extra_len)
            .and_then(|x| x.checked_add(comment_len))
            .ok_or_else(|| perr("usdz: cd overflow"))?;
        if next > cd_end {
            return Err(perr("usdz: central directory entry past end"));
        }
        let raw_name = std::str::from_utf8(bytes.get(name_start..name_end).unwrap_or_default())
            .map_err(|_| perr("usdz: non-UTF-8 entry name"))?;
        pos = next;
        if raw_name.ends_with('/') {
            continue;
        }
        let Some(norm_name) = normalize_archive_path("", raw_name) else {
            continue;
        };
        let data = extract_lfh_payload(bytes, lfh_off, method, comp_size, uncomp_size, &mut budget)?;
        out.push((norm_name, data));
    }
    Ok(out)
}

fn read_local_headers(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>> {
    let mut pos = 0usize;
    let mut out = Vec::new();
    let mut budget = MAX_USDZ_TOTAL_BYTES;
    while pos.saturating_add(4) <= bytes.len() {
        let sig = bytes.get(pos..pos + 4).unwrap_or_default();
        if sig == b"PK\x01\x02" || sig == b"PK\x05\x06" {
            break;
        }
        if sig != b"PK\x03\x04" {
            return Err(perr("usdz: malformed local file header"));
        }
        if out.len() >= MAX_USDZ_ENTRIES {
            return Err(perr("usdz: too many archive entries"));
        }
        let flags = read_u16_le(bytes, pos + 6).ok_or_else(|| perr("usdz: truncated header"))?;
        if (flags & 0x08) != 0 {
            return Err(perr("usdz: streaming data descriptor requires central directory"));
        }
        let method = read_u16_le(bytes, pos + 8).ok_or_else(|| perr("usdz: truncated header"))?;
        let comp_size = read_u32_le(bytes, pos + 18).ok_or_else(|| perr("usdz: truncated header"))? as usize;
        let uncomp_size = read_u32_le(bytes, pos + 22).ok_or_else(|| perr("usdz: truncated header"))? as usize;
        let name_len = read_u16_le(bytes, pos + 26).ok_or_else(|| perr("usdz: truncated header"))? as usize;
        let extra_len = read_u16_le(bytes, pos + 28).ok_or_else(|| perr("usdz: truncated header"))? as usize;
        let name_start = pos.checked_add(30).ok_or_else(|| perr("usdz: header overflow"))?;
        let name_end = name_start.checked_add(name_len).ok_or_else(|| perr("usdz: header overflow"))?;
        let data_start = name_end.checked_add(extra_len).ok_or_else(|| perr("usdz: header overflow"))?;
        let data_end = data_start.checked_add(comp_size).ok_or_else(|| perr("usdz: data overflow"))?;
        if data_end > bytes.len() {
            return Err(perr("usdz: truncated entry payload"));
        }
        let raw_name = std::str::from_utf8(bytes.get(name_start..name_end).unwrap_or_default())
            .map_err(|_| perr("usdz: non-UTF-8 entry name"))?;
        if !raw_name.ends_with('/')
            && let Some(norm_name) = normalize_archive_path("", raw_name)
        {
            let data = extract_lfh_payload(bytes, pos, method, comp_size, uncomp_size, &mut budget)?;
            out.push((norm_name, data));
        }
        pos = data_end;
    }
    Ok(out)
}

fn extract_lfh_payload(
    bytes: &[u8],
    lfh_off: usize,
    method: u16,
    comp_size: usize,
    uncomp_size: usize,
    budget: &mut usize,
) -> Result<Vec<u8>> {
    if bytes.get(lfh_off..lfh_off.saturating_add(4)) != Some(b"PK\x03\x04") {
        return Err(perr("usdz: bad local file header offset"));
    }
    let name_len = read_u16_le(bytes, lfh_off + 26).ok_or_else(|| perr("usdz: truncated lfh"))? as usize;
    let extra_len = read_u16_le(bytes, lfh_off + 28).ok_or_else(|| perr("usdz: truncated lfh"))? as usize;
    let data_start = lfh_off
        .checked_add(30)
        .and_then(|x| x.checked_add(name_len))
        .and_then(|x| x.checked_add(extra_len))
        .ok_or_else(|| perr("usdz: lfh offset overflow"))?;
    let data_end = data_start.checked_add(comp_size).ok_or_else(|| perr("usdz: payload overflow"))?;
    let payload = bytes.get(data_start..data_end).ok_or_else(|| perr("usdz: truncated entry data"))?;
    if uncomp_size > *budget {
        return Err(perr("usdz: decompressed size exceeds import budget"));
    }
    let out = match method {
        0 => {
            if payload.len() > *budget {
                return Err(perr("usdz: stored entry exceeds import budget"));
            }
            payload.to_vec()
        }
        8 => inflate_bounded(payload, *budget)?,
        other => {
            return Err(ModelError::Unsupported(format!(
                "usdz compression method {other} (only Stored and Deflate are supported)"
            )));
        }
    };
    *budget = budget.saturating_sub(out.len());
    Ok(out)
}

struct BitReader<'a> {
    src: &'a [u8],
    pos: usize,
    buf: u64,
    bits: u8,
}

impl<'a> BitReader<'a> {
    fn new(src: &'a [u8]) -> Self {
        Self { src, pos: 0, buf: 0, bits: 0 }
    }

    fn read_bits(&mut self, count: u8) -> Result<u32> {
        while self.bits < count {
            let b = *self.src.get(self.pos).ok_or_else(|| perr("usdz: truncated deflate stream"))?;
            self.pos += 1;
            self.buf |= u64::from(b) << self.bits;
            self.bits += 8;
        }
        let mask = if count == 32 { u64::MAX } else { (1u64 << count) - 1 };
        let val = (self.buf & mask) as u32;
        self.buf >>= count;
        self.bits -= count;
        Ok(val)
    }

    fn align_byte(&mut self) {
        let drop_bits = self.bits % 8;
        self.buf >>= drop_bits;
        self.bits -= drop_bits;
    }

    fn read_u16_aligned(&mut self) -> Result<u16> {
        self.align_byte();
        let lo = self.read_bits(8)? as u16;
        let hi = self.read_bits(8)? as u16;
        Ok(lo | (hi << 8))
    }
}

struct Huff {
    counts: [u16; 16],
    symbols: Vec<u16>,
}

fn build_huff(lengths: &[u8]) -> Result<Huff> {
    let mut counts = [0u16; 16];
    for &l in lengths {
        if l > 15 {
            return Err(perr("usdz: invalid deflate code length"));
        }
        if l > 0 {
            counts[l as usize] = counts[l as usize].saturating_add(1);
        }
    }
    let mut offsets = [0u16; 16];
    let mut total = 0u16;
    for bits in 1..=15 {
        offsets[bits] = total;
        total = total.saturating_add(counts[bits]);
    }
    let mut symbols = vec![0u16; total as usize];
    for (sym, &l) in lengths.iter().enumerate() {
        if l > 0 {
            let idx = offsets[l as usize] as usize;
            if let Some(slot) = symbols.get_mut(idx) {
                *slot = sym as u16;
            }
            offsets[l as usize] = offsets[l as usize].saturating_add(1);
        }
    }
    Ok(Huff { counts, symbols })
}

fn decode_sym(h: &Huff, br: &mut BitReader<'_>) -> Result<u16> {
    let mut code = 0i32;
    let mut first = 0i32;
    let mut index = 0usize;
    for bits in 1..=15 {
        let b = br.read_bits(1)? as i32;
        code = (code << 1) | b;
        let count = i32::from(h.counts[bits]);
        if code - count < first {
            let pos = index.checked_add((code - first) as usize).ok_or_else(|| perr("usdz: huffman overflow"))?;
            return h.symbols.get(pos).copied().ok_or_else(|| perr("usdz: invalid huffman symbol"));
        }
        index = index.saturating_add(count as usize);
        first = (first + count) << 1;
    }
    Err(perr("usdz: invalid deflate huffman code"))
}

fn fixed_trees() -> Result<(Huff, Huff)> {
    let mut lit = vec![0u8; 288];
    lit[0..144].fill(8);
    lit[144..256].fill(9);
    lit[256..280].fill(7);
    lit[280..288].fill(8);
    let dist = vec![5u8; 32];
    Ok((build_huff(&lit)?, build_huff(&dist)?))
}

fn dynamic_trees(br: &mut BitReader<'_>) -> Result<(Huff, Huff)> {
    const ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
    let hlit = br.read_bits(5)? as usize + 257;
    let hdist = br.read_bits(5)? as usize + 1;
    let hclen = br.read_bits(4)? as usize + 4;
    if hlit > 286 || hdist > 30 {
        return Err(perr("usdz: invalid dynamic huffman header"));
    }
    let mut cl_lens = [0u8; 19];
    for &slot in ORDER.iter().take(hclen) {
        cl_lens[slot] = br.read_bits(3)? as u8;
    }
    let cl_huff = build_huff(&cl_lens)?;
    let total = hlit + hdist;
    let mut lens = Vec::with_capacity(total);
    while lens.len() < total {
        let sym = decode_sym(&cl_huff, br)?;
        match sym {
            0..=15 => lens.push(sym as u8),
            16 => {
                let prev = *lens.last().ok_or_else(|| perr("usdz: repeat with no previous length"))?;
                let rep = br.read_bits(2)? as usize + 3;
                for _ in 0..rep {
                    if lens.len() >= total {
                        return Err(perr("usdz: code length overflow"));
                    }
                    lens.push(prev);
                }
            }
            17 => {
                let rep = br.read_bits(3)? as usize + 3;
                for _ in 0..rep {
                    if lens.len() >= total {
                        return Err(perr("usdz: code length overflow"));
                    }
                    lens.push(0);
                }
            }
            18 => {
                let rep = br.read_bits(7)? as usize + 11;
                for _ in 0..rep {
                    if lens.len() >= total {
                        return Err(perr("usdz: code length overflow"));
                    }
                    lens.push(0);
                }
            }
            _ => return Err(perr("usdz: invalid code-length symbol")),
        }
    }
    Ok((build_huff(&lens[..hlit])?, build_huff(&lens[hlit..])?))
}

fn inflate_bounded(src: &[u8], max_out: usize) -> Result<Vec<u8>> {
    let mut br = BitReader::new(src);
    let mut out = Vec::new();
    loop {
        let bfinal = br.read_bits(1)?;
        let btype = br.read_bits(2)?;
        match btype {
            0 => {
                let len = br.read_u16_aligned()? as usize;
                let nlen = br.read_u16_aligned()?;
                if (len as u16) != !nlen {
                    return Err(perr("usdz: corrupt stored deflate block"));
                }
                if out.len().saturating_add(len) > max_out {
                    return Err(perr("usdz: inflated data exceeds import limit"));
                }
                for _ in 0..len {
                    out.push(br.read_bits(8)? as u8);
                }
            }
            1 | 2 => {
                let (lit_h, dist_h) = if btype == 1 { fixed_trees()? } else { dynamic_trees(&mut br)? };
                inflate_huff_block(&mut br, &lit_h, &dist_h, max_out, &mut out)?;
            }
            _ => return Err(perr("usdz: invalid deflate block type")),
        }
        if bfinal != 0 {
            break;
        }
    }
    Ok(out)
}

fn inflate_huff_block(
    br: &mut BitReader<'_>,
    lit_h: &Huff,
    dist_h: &Huff,
    max_out: usize,
    out: &mut Vec<u8>,
) -> Result<()> {
    const LEN_BASE: [u16; 29] = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
        131, 163, 195, 227, 258,
    ];
    const LEN_EXTRA: [u8; 29] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
    ];
    const DIST_BASE: [u16; 30] = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
        2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
    ];
    const DIST_EXTRA: [u8; 30] = [
        0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
        13, 13,
    ];
    loop {
        let sym = decode_sym(lit_h, br)?;
        if sym < 256 {
            if out.len() >= max_out {
                return Err(perr("usdz: inflated data exceeds import limit"));
            }
            out.push(sym as u8);
        } else if sym == 256 {
            return Ok(());
        } else {
            let li = (sym - 257) as usize;
            let base = *LEN_BASE.get(li).ok_or_else(|| perr("usdz: invalid length code"))? as usize;
            let extra = *LEN_EXTRA.get(li).unwrap_or(&0);
            let length = base + br.read_bits(extra)? as usize;
            let di = decode_sym(dist_h, br)? as usize;
            let dbase = *DIST_BASE.get(di).ok_or_else(|| perr("usdz: invalid distance code"))? as usize;
            let dextra = *DIST_EXTRA.get(di).unwrap_or(&0);
            let dist = dbase + br.read_bits(dextra)? as usize;
            if dist == 0 || dist > out.len() {
                return Err(perr("usdz: back-reference before start of buffer"));
            }
            if out.len().saturating_add(length) > max_out {
                return Err(perr("usdz: inflated data exceeds import limit"));
            }
            for _ in 0..length {
                let b = out[out.len() - dist];
                out.push(b);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// USDA Tokenizer
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Ident(String),
    Str(String),
    Asset(String),
    SdfPath(String),
    Num(f64),
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Eq,
}

fn tokenize_usda(text: &str) -> Result<Vec<Tok>> {
    let bytes = text.as_bytes();
    let mut pos = 0usize;
    let mut out = Vec::new();
    while pos < bytes.len() {
        if out.len() >= MAX_USDA_TOKENS {
            return Err(perr("usda: token count exceeds import limit"));
        }
        let b = bytes[pos];
        if b.is_ascii_whitespace() || b == b',' || b == b';' {
            pos += 1;
            continue;
        }
        if b == b'#' {
            while pos < bytes.len() && bytes[pos] != b'\n' {
                pos += 1;
            }
            continue;
        }
        match b {
            b'(' => {
                out.push(Tok::LParen);
                pos += 1;
            }
            b')' => {
                out.push(Tok::RParen);
                pos += 1;
            }
            b'{' => {
                out.push(Tok::LBrace);
                pos += 1;
            }
            b'}' => {
                out.push(Tok::RBrace);
                pos += 1;
            }
            b'[' => {
                out.push(Tok::LBracket);
                pos += 1;
            }
            b']' => {
                out.push(Tok::RBracket);
                pos += 1;
            }
            b'=' => {
                out.push(Tok::Eq);
                pos += 1;
            }
            b'"' | b'\'' => {
                let (s, next) = lex_string(text, pos, b)?;
                out.push(Tok::Str(s));
                pos = next;
            }
            b'@' => {
                let (a, next) = lex_asset(text, pos)?;
                out.push(Tok::Asset(a));
                pos = next;
            }
            b'<' => {
                let (p, next) = lex_sdf_path(text, pos)?;
                out.push(Tok::SdfPath(p));
                pos = next;
            }
            _ if starts_number(bytes, pos) => {
                let (n, next) = lex_number(text, pos)?;
                out.push(Tok::Num(n));
                pos = next;
            }
            _ => {
                let (id, next) = lex_ident(text, pos)?;
                out.push(Tok::Ident(id));
                pos = next;
            }
        }
    }
    Ok(out)
}

fn starts_number(bytes: &[u8], pos: usize) -> bool {
    let b = bytes[pos];
    if b.is_ascii_digit() {
        return true;
    }
    if (b == b'-' || b == b'+')
        && let Some(&n) = bytes.get(pos + 1)
    {
        return n.is_ascii_digit()
            || (n == b'.' && bytes.get(pos + 2).is_some_and(u8::is_ascii_digit));
    }
    b == b'.' && bytes.get(pos + 1).is_some_and(u8::is_ascii_digit)
}

fn lex_number(text: &str, start: usize) -> Result<(f64, usize)> {
    let bytes = text.as_bytes();
    let mut end = start;
    if matches!(bytes.get(end), Some(b'-' | b'+')) {
        end += 1;
    }
    while bytes.get(end).is_some_and(|c| c.is_ascii_digit() || *c == b'.') {
        end += 1;
    }
    if matches!(bytes.get(end), Some(b'e' | b'E')) {
        end += 1;
        if matches!(bytes.get(end), Some(b'-' | b'+')) {
            end += 1;
        }
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
    }
    let slice = text.get(start..end).ok_or_else(|| perr("usda: invalid number slice"))?;
    let val = slice.parse::<f64>().map_err(|_| perr(format!("usda: invalid number `{slice}`")))?;
    if !val.is_finite() {
        return Err(perr("usda: non-finite number"));
    }
    Ok((val, end))
}

fn lex_ident(text: &str, start: usize) -> Result<(String, usize)> {
    let bytes = text.as_bytes();
    let mut end = start;
    while let Some(&c) = bytes.get(end) {
        if c.is_ascii_whitespace()
            || matches!(c, b'(' | b')' | b'{' | b'}' | b'[' | b']' | b'=' | b',' | b';' | b'"' | b'\'' | b'@' | b'<' | b'>' | b'#')
        {
            break;
        }
        let ch_len = text.get(end..).and_then(|s| s.chars().next()).map_or(1, char::len_utf8);
        end += ch_len;
    }
    if end == start {
        return Err(perr("usda: unexpected character"));
    }
    let id = text.get(start..end).ok_or_else(|| perr("usda: invalid identifier"))?;
    Ok((id.to_string(), end))
}

fn lex_string(text: &str, start: usize, quote: u8) -> Result<(String, usize)> {
    let bytes = text.as_bytes();
    let triple = bytes.get(start..start.saturating_add(3)) == Some(&[quote, quote, quote]);
    let mut pos = if triple { start + 3 } else { start + 1 };
    let mut out = String::new();
    while pos < bytes.len() {
        if triple && bytes.get(pos..pos.saturating_add(3)) == Some(&[quote, quote, quote]) {
            return Ok((out, pos + 3));
        }
        if !triple && bytes[pos] == quote {
            return Ok((out, pos + 1));
        }
        if bytes[pos] == b'\\' {
            pos += 1;
            let esc = *bytes.get(pos).ok_or_else(|| perr("usda: unterminated string escape"))?;
            match esc {
                b'n' => out.push('\n'),
                b'r' => out.push('\r'),
                b't' => out.push('\t'),
                b'\\' => out.push('\\'),
                b'"' => out.push('"'),
                b'\'' => out.push('\''),
                other => out.push(char::from(other)),
            }
            pos += 1;
        } else {
            let ch = text.get(pos..).and_then(|s| s.chars().next()).ok_or_else(|| perr("usda: bad utf-8"))?;
            out.push(ch);
            pos += ch.len_utf8();
        }
    }
    Err(perr("usda: unterminated string literal"))
}

fn lex_asset(text: &str, start: usize) -> Result<(String, usize)> {
    let bytes = text.as_bytes();
    let triple = bytes.get(start..start.saturating_add(3)) == Some(b"@@@");
    let content_start = if triple { start + 3 } else { start + 1 };
    let mut pos = content_start;
    while pos < bytes.len() {
        if triple && bytes.get(pos..pos.saturating_add(3)) == Some(b"@@@") {
            let s = text.get(content_start..pos).unwrap_or_default().trim().to_string();
            return Ok((s, pos + 3));
        }
        if !triple && bytes[pos] == b'@' {
            let s = text.get(content_start..pos).unwrap_or_default().trim().to_string();
            return Ok((s, pos + 1));
        }
        pos += 1;
    }
    Err(perr("usda: unterminated `@asset@` path"))
}

fn lex_sdf_path(text: &str, start: usize) -> Result<(String, usize)> {
    let bytes = text.as_bytes();
    let mut pos = start + 1;
    while pos < bytes.len() {
        match bytes[pos] {
            b'>' => {
                let s = text.get(start + 1..pos).unwrap_or_default().trim().to_string();
                return Ok((s, pos + 1));
            }
            b'\n' | b'\r' => break,
            _ => pos += 1,
        }
    }
    Err(perr("usda: unterminated `<path>` reference"))
}

// ---------------------------------------------------------------------------
// USDA AST & Recursive-Descent Stage Parser
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum UsdVal {
    Num(f64),
    Bool(bool),
    Str(String),
    Asset(String),
    SdfPath(String),
    Tuple(Vec<f64>),
    ArrayNum(Vec<f64>),
    ArrayTuple(Vec<Vec<f64>>),
    ArrayStr(Vec<String>),
    NoneVal,
}

impl UsdVal {
    fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Num(x) => Some(*x),
            Self::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            Self::Num(n) => Some(*n != 0.0),
            Self::Str(s) => match s.as_str() {
                "true" | "1" => Some(true),
                "false" | "0" => Some(false),
                _ => None,
            },
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) | Self::Asset(s) | Self::SdfPath(s) => Some(s.as_str()),
            _ => None,
        }
    }

    fn as_vec3(&self) -> Option<[f64; 3]> {
        match self {
            Self::Tuple(v) if v.len() >= 3 => Some([v[0], v[1], v[2]]),
            _ => None,
        }
    }

    fn as_quat_xyzw(&self) -> Option<[f64; 4]> {
        // USD `quatf` / `quatd` is `(real, i, j, k)` = `(w, x, y, z)`.
        match self {
            Self::Tuple(v) if v.len() >= 4 => Some([v[1], v[2], v[3], v[0]]),
            _ => None,
        }
    }

    fn as_mat4_col(&self) -> Option<Mat4> {
        // USD `matrix4d` is `((r0c0..r0c3), ..., (r3c0..r3c3))` in row-vector convention (`v' = v * M`),
        // so EffectCraft's column-vector matrix is `M^T`.
        match self {
            Self::ArrayTuple(rows) if rows.len() == 4 && rows.iter().all(|r| r.len() == 4) => {
                let mut m = [[0.0; 4]; 4];
                for r in 0..4 {
                    for c in 0..4 {
                        m[c][r] = rows[r][c];
                    }
                }
                Some(Mat4(m))
            }
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
struct UsdProp {
    name: String,
    val: UsdVal,
}

#[derive(Clone, Debug)]
struct UsdPrim {
    specifier: String,
    prim_type: String,
    name: String,
    path: String,
    props: Vec<UsdProp>,
    children: Vec<UsdPrim>,
}

struct UsdaStage {
    up_axis: String,
    prims: Vec<UsdPrim>,
}

struct Parser<'a> {
    toks: &'a [Tok],
    pos: usize,
    prim_count: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&'a Tok> {
        self.toks.get(self.pos)
    }

    fn next(&mut self) -> Option<&'a Tok> {
        let t = self.toks.get(self.pos)?;
        self.pos += 1;
        Some(t)
    }

    fn parse_stage(&mut self) -> Result<UsdaStage> {
        let mut up_axis = "Y".to_string();
        if self.peek() == Some(&Tok::LParen) {
            let meta = self.parse_metadata(0)?;
            for p in meta {
                if p.name == "upAxis"
                    && let Some(s) = p.val.as_str()
                {
                    up_axis = s.to_ascii_uppercase();
                }
            }
        }
        let mut prims = Vec::new();
        while self.pos < self.toks.len() {
            if let Some(prim) = self.parse_prim_or_stmt("", 0)? {
                prims.push(prim);
            }
        }
        Ok(UsdaStage { up_axis, prims })
    }

    fn parse_metadata(&mut self, depth: usize) -> Result<Vec<UsdProp>> {
        if depth > MAX_USD_DEPTH {
            return Err(perr("usda: metadata nesting depth exceeded"));
        }
        if self.next() != Some(&Tok::LParen) {
            return Err(perr("usda: expected `(`"));
        }
        let mut props = Vec::new();
        while let Some(t) = self.peek() {
            if *t == Tok::RParen {
                self.pos += 1;
                return Ok(props);
            }
            if *t == Tok::LParen {
                let _ = self.parse_metadata(depth + 1)?;
                continue;
            }
            if *t == Tok::LBrace {
                self.skip_group(&Tok::LBrace, &Tok::RBrace, depth + 1)?;
                continue;
            }
            let Some(Tok::Ident(key)) = self.next() else {
                continue;
            };
            if self.peek() == Some(&Tok::Eq) {
                self.pos += 1;
                let val = self.parse_value(depth + 1)?;
                props.push(UsdProp { name: key.clone(), val });
            }
        }
        Err(perr("usda: unterminated `(` metadata block"))
    }

    fn skip_group(&mut self, open: &Tok, close: &Tok, depth: usize) -> Result<()> {
        if depth > MAX_USD_DEPTH {
            return Err(perr("usda: nesting depth exceeded"));
        }
        if self.next() != Some(open) {
            return Err(perr("usda: expected group open"));
        }
        while let Some(t) = self.peek() {
            if t == close {
                self.pos += 1;
                return Ok(());
            }
            if t == open {
                self.skip_group(open, close, depth + 1)?;
            } else if *t == Tok::LParen {
                self.skip_group(&Tok::LParen, &Tok::RParen, depth + 1)?;
            } else if *t == Tok::LBracket {
                self.skip_group(&Tok::LBracket, &Tok::RBracket, depth + 1)?;
            } else if *t == Tok::LBrace {
                self.skip_group(&Tok::LBrace, &Tok::RBrace, depth + 1)?;
            } else {
                self.pos += 1;
            }
        }
        Err(perr("usda: unterminated group"))
    }

    fn parse_prim_or_stmt(&mut self, parent_path: &str, depth: usize) -> Result<Option<UsdPrim>> {
        if depth > MAX_USD_DEPTH {
            return Err(perr("usda: prim hierarchy exceeds maximum depth"));
        }
        if let Some(Tok::Ident(w)) = self.peek()
            && matches!(w.as_str(), "def" | "over" | "class")
        {
            return self.parse_prim(parent_path, depth).map(Some);
        }
        let _ = self.parse_property(depth)?;
        Ok(None)
    }

    fn parse_prim(&mut self, parent_path: &str, depth: usize) -> Result<UsdPrim> {
        self.prim_count = self.prim_count.saturating_add(1);
        if self.prim_count > MAX_USD_PRIMS {
            return Err(perr("usda: prim count exceeds import limit"));
        }
        let Some(Tok::Ident(specifier)) = self.next().cloned() else {
            return Err(perr("usda: expected prim specifier"));
        };
        let mut prim_type = String::new();
        if let Some(Tok::Ident(t)) = self.peek().cloned() {
            self.pos += 1;
            prim_type = t;
        }
        let name = match self.next() {
            Some(Tok::Str(s)) => s.clone(),
            _ => return Err(perr("usda: expected quoted prim name")),
        };
        let path = format!("{parent_path}/{name}");
        if self.peek() == Some(&Tok::LParen) {
            let _ = self.parse_metadata(depth + 1)?;
        }
        if self.next() != Some(&Tok::LBrace) {
            return Err(perr("usda: expected `{` opening prim body"));
        }
        let mut props = Vec::new();
        let mut children = Vec::new();
        while let Some(t) = self.peek() {
            if *t == Tok::RBrace {
                self.pos += 1;
                return Ok(UsdPrim { specifier, prim_type, name, path, props, children });
            }
            if let Some(Tok::Ident(w)) = self.peek()
                && matches!(w.as_str(), "def" | "over" | "class")
            {
                children.push(self.parse_prim(&path, depth + 1)?);
            } else if let Some(prop) = self.parse_property(depth + 1)? {
                props.push(prop);
            }
        }
        Err(perr("usda: unterminated `{` prim body"))
    }

    fn parse_property(&mut self, depth: usize) -> Result<Option<UsdProp>> {
        if depth > MAX_USD_DEPTH {
            return Err(perr("usda: property nesting depth exceeded"));
        }
        // Property declarations have optional qualifiers (`custom`, `uniform`, `prepend`, `rel`, ...),
        // a type (possibly with `[]`), a property name, optional `(metadata)`, and optional `= value`.
        let mut last_ident = String::new();
        while let Some(t) = self.peek() {
            match t {
                Tok::RBrace => return Ok(None),
                Tok::Ident(s) => {
                    last_ident = s.clone();
                    self.pos += 1;
                }
                Tok::LBracket => {
                    self.skip_group(&Tok::LBracket, &Tok::RBracket, depth + 1)?;
                }
                Tok::LParen => {
                    let _ = self.parse_metadata(depth + 1)?;
                }
                Tok::Eq => {
                    self.pos += 1;
                    let mut val = self.parse_value(depth + 1)?;
                    if self.peek() == Some(&Tok::LParen) {
                        let _ = self.parse_metadata(depth + 1)?;
                    }
                    // Time-sampled block `{ 0: val0, 10: val1 }`: evaluate first sample.
                    if matches!(val, UsdVal::NoneVal) {
                        val = UsdVal::NoneVal;
                    }
                    return Ok((!last_ident.is_empty()).then_some(UsdProp { name: last_ident, val }));
                }
                Tok::LBrace => {
                    self.skip_group(&Tok::LBrace, &Tok::RBrace, depth + 1)?;
                    return Ok(None);
                }
                _ => {
                    self.pos += 1;
                    return Ok(None);
                }
            }
        }
        Ok(None)
    }

    fn parse_value(&mut self, depth: usize) -> Result<UsdVal> {
        if depth > MAX_USD_DEPTH {
            return Err(perr("usda: value nesting depth exceeded"));
        }
        match self.next().cloned() {
            Some(Tok::Num(n)) => Ok(UsdVal::Num(n)),
            Some(Tok::Str(s)) => Ok(UsdVal::Str(s)),
            Some(Tok::Asset(a)) => Ok(UsdVal::Asset(a)),
            Some(Tok::SdfPath(p)) => Ok(UsdVal::SdfPath(p)),
            Some(Tok::Ident(id)) => match id.as_str() {
                "true" => Ok(UsdVal::Bool(true)),
                "false" => Ok(UsdVal::Bool(false)),
                "None" => Ok(UsdVal::NoneVal),
                _ => Ok(UsdVal::Str(id)),
            },
            Some(Tok::LParen) => self.parse_tuple_or_matrix(depth + 1),
            Some(Tok::LBracket) => self.parse_array(depth + 1),
            Some(Tok::LBrace) => self.parse_time_samples(depth + 1),
            _ => Err(perr("usda: expected property value")),
        }
    }

    fn parse_tuple_or_matrix(&mut self, depth: usize) -> Result<UsdVal> {
        if depth > MAX_USD_DEPTH {
            return Err(perr("usda: tuple nesting depth exceeded"));
        }
        if self.peek() == Some(&Tok::LParen) {
            let mut rows = Vec::new();
            while let Some(t) = self.peek() {
                if *t == Tok::RParen {
                    self.pos += 1;
                    return Ok(UsdVal::ArrayTuple(rows));
                }
                if self.next() != Some(&Tok::LParen) {
                    return Err(perr("usda: expected matrix row `(`"));
                }
                rows.push(self.read_num_tuple()?);
            }
            return Err(perr("usda: unterminated matrix tuple"));
        }
        Ok(UsdVal::Tuple(self.read_num_tuple()?))
    }

    fn read_num_tuple(&mut self) -> Result<Vec<f64>> {
        let mut vals = Vec::new();
        while let Some(t) = self.peek() {
            if *t == Tok::RParen {
                self.pos += 1;
                return Ok(vals);
            }
            match self.next() {
                Some(Tok::Num(n)) => vals.push(*n),
                _ => return Err(perr("usda: expected number in tuple")),
            }
        }
        Err(perr("usda: unterminated tuple"))
    }

    fn parse_array(&mut self, depth: usize) -> Result<UsdVal> {
        if depth > MAX_USD_DEPTH {
            return Err(perr("usda: array nesting depth exceeded"));
        }
        match self.peek() {
            Some(Tok::RBracket) => {
                self.pos += 1;
                Ok(UsdVal::ArrayNum(Vec::new()))
            }
            Some(Tok::LParen) => {
                let mut tuples = Vec::new();
                while let Some(t) = self.peek() {
                    if *t == Tok::RBracket {
                        self.pos += 1;
                        return Ok(UsdVal::ArrayTuple(tuples));
                    }
                    if tuples.len() >= MAX_USD_VERTICES {
                        return Err(perr("usda: tuple array exceeds vertex limit"));
                    }
                    if self.next() != Some(&Tok::LParen) {
                        return Err(perr("usda: expected `(` in tuple array"));
                    }
                    tuples.push(self.read_num_tuple()?);
                }
                Err(perr("usda: unterminated tuple array"))
            }
            Some(Tok::Num(_)) => {
                let mut nums = Vec::new();
                while let Some(t) = self.peek() {
                    if *t == Tok::RBracket {
                        self.pos += 1;
                        return Ok(UsdVal::ArrayNum(nums));
                    }
                    if nums.len() >= MAX_USD_VERTICES * 3 {
                        return Err(perr("usda: numeric array exceeds limit"));
                    }
                    match self.next() {
                        Some(Tok::Num(n)) => nums.push(*n),
                        _ => return Err(perr("usda: expected number in array")),
                    }
                }
                Err(perr("usda: unterminated numeric array"))
            }
            _ => {
                let mut items = Vec::new();
                while let Some(t) = self.peek() {
                    if *t == Tok::RBracket {
                        self.pos += 1;
                        return Ok(UsdVal::ArrayStr(items));
                    }
                    if let Some(Tok::Str(s) | Tok::Asset(s) | Tok::SdfPath(s) | Tok::Ident(s)) = self.next() {
                        items.push(s.clone());
                    }
                }
                Err(perr("usda: unterminated array"))
            }
        }
    }

    fn parse_time_samples(&mut self, depth: usize) -> Result<UsdVal> {
        if depth > MAX_USD_DEPTH {
            return Err(perr("usda: timeSamples nesting depth exceeded"));
        }
        let mut first_val = UsdVal::NoneVal;
        let mut got_first = false;
        while let Some(t) = self.peek() {
            if *t == Tok::RBrace {
                self.pos += 1;
                return Ok(first_val);
            }
            // `<frame>:` comes through as `Num` + `Ident(":")` or `Ident("0:")`
            self.pos += 1;
            if let Some(Tok::Ident(s)) = self.peek()
                && s == ":"
            {
                self.pos += 1;
            }
            let v = self.parse_value(depth + 1)?;
            if !got_first {
                first_val = v;
                got_first = true;
            }
        }
        Err(perr("usda: unterminated timeSamples `{`"))
    }
}

// ---------------------------------------------------------------------------
// Stage -> EffectCraft Model Conversion
// ---------------------------------------------------------------------------

/// Parse an ASCII `.usda` stage with an external/archive asset resolver.
pub fn parse_usda(bytes: &[u8], resolve: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<Model> {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    if !starts_with_usda(bytes) {
        return Err(perr("usda: missing `#usda` header"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| perr("usda: file is not valid UTF-8"))?;
    let toks = tokenize_usda(text)?;
    let mut parser = Parser { toks: &toks, pos: 0, prim_count: 0 };
    let stage = parser.parse_stage()?;

    let mut ctx = BuildCtx {
        resolve,
        pixels_left: MAX_IMPORT_TEXTURE_PIXELS,
        decoded_images: HashMap::new(),
        textures: Vec::new(),
        texture_variants: HashMap::new(),
        materials: Vec::new(),
        material_by_path: HashMap::new(),
        meshes: Vec::new(),
        nodes: Vec::new(),
        warnings: Vec::new(),
        total_vertices: 0,
    };

    collect_materials(&stage.prims, &mut ctx)?;

    let mut roots = Vec::new();
    for prim in &stage.prims {
        if prim.specifier == "class" {
            continue;
        }
        if let Some(idx) = build_prim_node(prim, None, &mut ctx, 0)? {
            roots.push(idx);
        }
    }

    if stage.up_axis == "Z" && !roots.is_empty() {
        let s = std::f64::consts::FRAC_1_SQRT_2;
        let z_up_root = Node {
            name: "ZUpToYUp".into(),
            rotation: [-s, 0.0, 0.0, s],
            children: roots,
            ..Node::default()
        };
        ctx.nodes.push(z_up_root);
        roots = vec![ctx.nodes.len() - 1];
    }

    Ok(Model {
        meshes: ctx.meshes,
        nodes: ctx.nodes,
        roots,
        materials: ctx.materials,
        textures: ctx.textures,
        warnings: ctx.warnings,
        ..Model::default()
    })
}

struct BuildCtx<'a> {
    resolve: &'a dyn Fn(&str) -> Option<Vec<u8>>,
    pixels_left: u64,
    decoded_images: HashMap<String, Option<Arc<Texture>>>,
    textures: Vec<Arc<Texture>>,
    texture_variants: HashMap<(String, [Wrap; 2]), usize>,
    materials: Vec<Material>,
    material_by_path: HashMap<String, usize>,
    meshes: Vec<Mesh>,
    nodes: Vec<Node>,
    warnings: Vec<String>,
    total_vertices: usize,
}

impl BuildCtx<'_> {
    fn load_texture(&mut self, uri: &str, wrap: [Wrap; 2]) -> Option<TexRef> {
        if uri.is_empty() {
            return None;
        }
        let key = (uri.to_string(), wrap);
        if let Some(&idx) = self.texture_variants.get(&key) {
            return Some(TexRef { texture: idx, uv_set: 0 });
        }
        let base_img = if let Some(cached) = self.decoded_images.get(uri) {
            cached.clone()
        } else {
            let decoded = match (self.resolve)(uri) {
                Some(bytes) => match Texture::decode_within(&bytes, &mut self.pixels_left) {
                    Ok(t) => Some(Arc::new(t)),
                    Err(e) => {
                        let w = format!("texture `{uri}` was left out: {e}");
                        log::warn!("model: {w}");
                        self.warnings.push(w);
                        None
                    }
                },
                None => {
                    let w = format!("texture `{uri}` was not found");
                    log::warn!("model: {w}");
                    self.warnings.push(w);
                    None
                }
            };
            self.decoded_images.insert(uri.to_string(), decoded.clone());
            decoded
        }?;

        let tex_arc = if base_img.wrap == wrap {
            base_img
        } else {
            let cost = u64::from(base_img.width).saturating_mul(u64::from(base_img.height));
            if cost > self.pixels_left {
                let w = format!("texture `{uri}` wrap variant exceeds pixel budget");
                log::warn!("model: {w}");
                self.warnings.push(w);
                return None;
            }
            self.pixels_left -= cost;
            Arc::new(Texture { wrap, ..(*base_img).clone() })
        };
        let idx = self.textures.len();
        self.textures.push(tex_arc);
        self.texture_variants.insert(key, idx);
        Some(TexRef { texture: idx, uv_set: 0 })
    }
}

fn find_prop<'p>(prim: &'p UsdPrim, name: &str) -> Option<&'p UsdVal> {
    prim.props.iter().rev().find(|p| p.name == name).map(|p| &p.val)
}

fn collect_materials(prims: &[UsdPrim], ctx: &mut BuildCtx<'_>) -> Result<()> {
    for prim in prims {
        if prim.specifier == "class" {
            continue;
        }
        if prim.prim_type == "Material" {
            let mat = build_usd_material(prim, ctx);
            let idx = ctx.materials.len();
            ctx.materials.push(mat);
            ctx.material_by_path.insert(prim.path.clone(), idx);
        }
        collect_materials(&prim.children, ctx)?;
    }
    Ok(())
}

fn collect_shaders<'p>(prim: &'p UsdPrim, out: &mut HashMap<String, &'p UsdPrim>) {
    for ch in &prim.children {
        if ch.prim_type == "Shader" {
            out.insert(ch.path.clone(), ch);
        }
        collect_shaders(ch, out);
    }
}

fn shader_path_from_conn(conn: &str) -> &str {
    conn.split_once(".outputs:").map_or(conn, |(p, _)| p)
}

fn parse_wrap(v: Option<&UsdVal>) -> Wrap {
    match v.and_then(UsdVal::as_str).unwrap_or("repeat") {
        "clamp" | "black" => Wrap::Clamp,
        "mirror" => Wrap::Mirror,
        _ => Wrap::Repeat,
    }
}

fn resolve_uv_texture(
    shader_map: &HashMap<String, &UsdPrim>,
    conn: &str,
    ctx: &mut BuildCtx<'_>,
) -> Option<TexRef> {
    let sh = *shader_map.get(shader_path_from_conn(conn))?;
    let id = find_prop(sh, "info:id").and_then(UsdVal::as_str).unwrap_or("");
    if !id.is_empty() && id != "UsdUVTexture" {
        return None;
    }
    let file = find_prop(sh, "inputs:file").and_then(UsdVal::as_str)?;
    let ws = parse_wrap(find_prop(sh, "inputs:wrapS"));
    let wt = parse_wrap(find_prop(sh, "inputs:wrapT"));
    ctx.load_texture(file, [ws, wt])
}

fn build_usd_material(mat_prim: &UsdPrim, ctx: &mut BuildCtx<'_>) -> Material {
    let mut shaders = HashMap::new();
    collect_shaders(mat_prim, &mut shaders);

    let surface_conn = find_prop(mat_prim, "outputs:surface.connect")
        .or_else(|| find_prop(mat_prim, "outputs:surface"))
        .and_then(UsdVal::as_str);

    let surface = surface_conn
        .and_then(|c| shaders.get(shader_path_from_conn(c)).copied())
        .or_else(|| {
            shaders.values().copied().find(|s| {
                find_prop(s, "info:id").and_then(UsdVal::as_str) == Some("UsdPreviewSurface")
            })
        });

    let mut mat = Material {
        name: mat_prim.name.clone(),
        base_color: [0.18, 0.18, 0.18, 1.0],
        metallic: 0.0,
        roughness: 0.5,
        ..Material::default()
    };
    let Some(surf) = surface else {
        return mat;
    };

    if let Some(c) = find_prop(surf, "inputs:diffuseColor").and_then(UsdVal::as_vec3) {
        mat.base_color = [c[0] as f32, c[1] as f32, c[2] as f32, 1.0];
    }
    if let Some(conn) = find_prop(surf, "inputs:diffuseColor.connect").and_then(UsdVal::as_str)
        && let Some(tref) = resolve_uv_texture(&shaders, conn, ctx)
    {
        mat.base_color_tex = Some(tref);
        if find_prop(surf, "inputs:diffuseColor").is_none() {
            mat.base_color = [1.0, 1.0, 1.0, 1.0];
        }
    }

    if let Some(m) = find_prop(surf, "inputs:metallic").and_then(UsdVal::as_f64) {
        mat.metallic = (m as f32).clamp(0.0, 1.0);
    }
    if let Some(r) = find_prop(surf, "inputs:roughness").and_then(UsdVal::as_f64) {
        mat.roughness = (r as f32).clamp(0.0, 1.0);
    }
    apply_usd_mr_textures(surf, &shaders, ctx, &mut mat);

    if let Some(e) = find_prop(surf, "inputs:emissiveColor").and_then(UsdVal::as_vec3) {
        mat.emissive = [e[0] as f32, e[1] as f32, e[2] as f32];
    }
    if let Some(conn) = find_prop(surf, "inputs:emissiveColor.connect").and_then(UsdVal::as_str)
        && let Some(tref) = resolve_uv_texture(&shaders, conn, ctx)
    {
        mat.emissive_tex = Some(tref);
        if find_prop(surf, "inputs:emissiveColor").is_none() {
            mat.emissive = [1.0, 1.0, 1.0];
        }
    }
    if let Some(conn) = find_prop(surf, "inputs:normal.connect").and_then(UsdVal::as_str) {
        mat.normal_tex = resolve_uv_texture(&shaders, conn, ctx);
    }
    if let Some(conn) = find_prop(surf, "inputs:occlusion.connect").and_then(UsdVal::as_str) {
        mat.occlusion_tex = resolve_uv_texture(&shaders, conn, ctx);
    }

    let opacity = find_prop(surf, "inputs:opacity")
        .and_then(UsdVal::as_f64)
        .map_or(1.0, |o| (o as f32).clamp(0.0, 1.0));
    let threshold = find_prop(surf, "inputs:opacityThreshold")
        .and_then(UsdVal::as_f64)
        .map_or(0.0, |t| (t as f32).clamp(0.0, 1.0));
    mat.base_color[3] = opacity;
    if threshold > 0.0 {
        mat.alpha_mode = AlphaMode::Mask;
        mat.alpha_cutoff = threshold;
    } else if opacity < 0.999 || find_prop(surf, "inputs:opacity.connect").is_some() {
        mat.alpha_mode = AlphaMode::Blend;
    }
    mat
}

fn apply_usd_mr_textures(
    surf: &UsdPrim,
    shaders: &HashMap<String, &UsdPrim>,
    ctx: &mut BuildCtx<'_>,
    mat: &mut Material,
) {
    let m_conn = find_prop(surf, "inputs:metallic.connect").and_then(UsdVal::as_str);
    let r_conn = find_prop(surf, "inputs:roughness.connect").and_then(UsdVal::as_str);
    if m_conn.is_none() && r_conn.is_none() {
        return;
    }
    let m_tex = m_conn.and_then(|c| resolve_uv_texture(shaders, c, ctx));
    let r_tex = r_conn.and_then(|c| resolve_uv_texture(shaders, c, ctx));
    let chan_of = |conn: Option<&str>| -> usize {
        match conn.and_then(|c| c.rsplit_once(".outputs:")).map(|(_, ch)| ch) {
            Some("g") => 1,
            Some("b") => 2,
            Some("a") => 3,
            _ => 0,
        }
    };
    let (m_ch, r_ch) = (chan_of(m_conn), chan_of(r_conn));
    let base_ref = r_tex.or(m_tex);
    let Some(bref) = base_ref else { return };
    let Some(base_img) = ctx.textures.get(bref.texture).cloned() else { return };
    let (w, h) = (base_img.width, base_img.height);
    let m_img = m_tex.and_then(|t| ctx.textures.get(t.texture)).filter(|t| t.width == w && t.height == h);
    let r_img = r_tex.and_then(|t| ctx.textures.get(t.texture)).filter(|t| t.width == w && t.height == h);
    let mut packed = Vec::with_capacity(base_img.data.len());
    for i in 0..base_img.data.len() {
        let rough = r_img.map_or(mat.roughness, |img| img.data[i][r_ch]);
        let metal = m_img.map_or(mat.metallic, |img| img.data[i][m_ch]);
        packed.push([1.0, rough, metal, 1.0]);
    }
    let idx = ctx.textures.len();
    ctx.textures.push(Arc::new(Texture { width: w, height: h, data: packed, wrap: base_img.wrap }));
    mat.metallic_roughness_tex = Some(TexRef { texture: idx, uv_set: 0 });
    mat.metallic = 1.0;
    mat.roughness = 1.0;
}

fn bound_material(prim: &UsdPrim, ctx: &BuildCtx<'_>) -> Option<usize> {
    let path = find_prop(prim, "material:binding")
        .or_else(|| find_prop(prim, "material:binding.connect"))
        .and_then(UsdVal::as_str)?;
    ctx.material_by_path.get(path).copied()
}

fn build_prim_node(
    prim: &UsdPrim,
    inherited_mat: Option<usize>,
    ctx: &mut BuildCtx<'_>,
    depth: usize,
) -> Result<Option<usize>> {
    if depth > MAX_USD_DEPTH {
        return Err(perr("usda: scene hierarchy depth exceeded"));
    }
    if matches!(prim.prim_type.as_str(), "Material" | "Shader" | "GeomSubset") || prim.specifier == "class" {
        return Ok(None);
    }
    let active_mat = bound_material(prim, ctx).or(inherited_mat);
    let (translation, rotation, scale, matrix) = eval_xform_ops(prim);

    let mesh_idx = match prim.prim_type.as_str() {
        "Mesh" => build_usd_mesh(prim, active_mat, ctx)?,
        "Cube" => Some(build_usd_cube(prim, active_mat, ctx)),
        "Sphere" => Some(build_usd_sphere(prim, active_mat, ctx)),
        _ => None,
    };

    let mut children = Vec::new();
    for ch in &prim.children {
        if let Some(cid) = build_prim_node(ch, active_mat, ctx, depth + 1)? {
            children.push(cid);
        }
    }

    let idx = ctx.nodes.len();
    ctx.nodes.push(Node {
        name: prim.name.clone(),
        translation,
        rotation,
        scale,
        matrix,
        mesh: mesh_idx,
        children,
        ..Node::default()
    });
    Ok(Some(idx))
}

fn eval_xform_ops(prim: &UsdPrim) -> ([f64; 3], [f64; 4], [f64; 3], Option<Mat4>) {
    let order: Vec<String> = match find_prop(prim, "xformOpOrder") {
        Some(UsdVal::ArrayStr(list)) => list.clone(),
        _ => prim
            .props
            .iter()
            .filter(|p| p.name.starts_with("xformOp:"))
            .map(|p| p.name.clone())
            .collect(),
    };
    if order.is_empty() {
        return ([0.0; 3], [0.0, 0.0, 0.0, 1.0], [1.0; 3], None);
    }
    // Simple TRS fast-path when no custom matrix or inverted pivot is used.
    if is_simple_trs(prim, &order) {
        let t = find_prop(prim, "xformOp:translate").and_then(UsdVal::as_vec3).unwrap_or([0.0; 3]);
        let q = find_prop(prim, "xformOp:orient")
            .and_then(UsdVal::as_quat_xyzw)
            .or_else(|| find_prop(prim, "xformOp:rotateXYZ").and_then(UsdVal::as_vec3).map(euler_xyz_quat))
            .unwrap_or([0.0, 0.0, 0.0, 1.0]);
        let s = find_prop(prim, "xformOp:scale").and_then(UsdVal::as_vec3).unwrap_or([1.0; 3]);
        return (t, q, s, None);
    }

    let mut acc = Mat4::IDENTITY;
    for raw_op in &order {
        if raw_op == "!resetXformStack!" {
            acc = Mat4::IDENTITY;
            continue;
        }
        let (inv, op_name) = match raw_op.strip_prefix("!invert!") {
            Some(rest) => (true, rest),
            None => (false, raw_op.as_str()),
        };
        let Some(val) = find_prop(prim, op_name) else { continue };
        let m = single_xform_op(op_name, val);
        let m = if inv { m.inverse().unwrap_or(Mat4::IDENTITY) } else { m };
        acc = acc * m;
    }
    ([0.0; 3], [0.0, 0.0, 0.0, 1.0], [1.0; 3], Some(acc))
}

fn is_simple_trs(prim: &UsdPrim, order: &[String]) -> bool {
    const ALLOWED: [&str; 4] = ["xformOp:translate", "xformOp:orient", "xformOp:rotateXYZ", "xformOp:scale"];
    if order.iter().any(|o| !ALLOWED.contains(&o.as_str())) {
        return false;
    }
    let rank = |s: &str| match s {
        "xformOp:translate" => 0,
        "xformOp:orient" | "xformOp:rotateXYZ" => 1,
        "xformOp:scale" => 2,
        _ => 3,
    };
    let mut prev = 0;
    let mut rot_count = 0;
    for o in order {
        let r = rank(o);
        if r < prev {
            return false;
        }
        if r == 1 {
            rot_count += 1;
        }
        prev = r;
    }
    rot_count <= 1 && find_prop(prim, "xformOp:transform").is_none()
}

fn single_xform_op(op_name: &str, val: &UsdVal) -> Mat4 {
    let base = op_name.strip_prefix("xformOp:").unwrap_or(op_name);
    let kind = base.split(':').next().unwrap_or(base);
    match kind {
        "translate" => {
            let [x, y, z] = val.as_vec3().unwrap_or([0.0; 3]);
            Mat4::translate(vec3(x, y, z))
        }
        "scale" => {
            let [x, y, z] = val.as_vec3().unwrap_or([1.0; 3]);
            Mat4::scale(vec3(x, y, z))
        }
        "orient" => quat_matrix(val.as_quat_xyzw().unwrap_or([0.0, 0.0, 0.0, 1.0])),
        "rotateX" => axis_rot_deg([1.0, 0.0, 0.0], val.as_f64().unwrap_or(0.0)),
        "rotateY" => axis_rot_deg([0.0, 1.0, 0.0], val.as_f64().unwrap_or(0.0)),
        "rotateZ" => axis_rot_deg([0.0, 0.0, 1.0], val.as_f64().unwrap_or(0.0)),
        "rotateXYZ" => euler_rot_mat(val.as_vec3().unwrap_or([0.0; 3]), [0, 1, 2]),
        "rotateXZY" => euler_rot_mat(val.as_vec3().unwrap_or([0.0; 3]), [0, 2, 1]),
        "rotateYXZ" => euler_rot_mat(val.as_vec3().unwrap_or([0.0; 3]), [1, 0, 2]),
        "rotateYZX" => euler_rot_mat(val.as_vec3().unwrap_or([0.0; 3]), [1, 2, 0]),
        "rotateZXY" => euler_rot_mat(val.as_vec3().unwrap_or([0.0; 3]), [2, 0, 1]),
        "rotateZYX" => euler_rot_mat(val.as_vec3().unwrap_or([0.0; 3]), [2, 1, 0]),
        "transform" => val.as_mat4_col().unwrap_or(Mat4::IDENTITY),
        _ => Mat4::IDENTITY,
    }
}

fn axis_quat_deg(axis: [f64; 3], deg: f64) -> [f64; 4] {
    let half = deg.to_radians() * 0.5;
    let (s, c) = half.sin_cos();
    [axis[0] * s, axis[1] * s, axis[2] * s, c]
}

fn axis_rot_deg(axis: [f64; 3], deg: f64) -> Mat4 {
    quat_matrix(axis_quat_deg(axis, deg))
}

fn quat_mul(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

fn euler_xyz_quat(deg: [f64; 3]) -> [f64; 4] {
    // USD `rotateXYZ`: X applied first to point, then Y, then Z -> column-vector `Rz * Ry * Rx`.
    let qx = axis_quat_deg([1.0, 0.0, 0.0], deg[0]);
    let qy = axis_quat_deg([0.0, 1.0, 0.0], deg[1]);
    let qz = axis_quat_deg([0.0, 0.0, 1.0], deg[2]);
    quat_mul(qz, quat_mul(qy, qx))
}

fn euler_rot_mat(deg: [f64; 3], order: [usize; 3]) -> Mat4 {
    let axes = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let r0 = axis_rot_deg(axes[order[0]], deg[order[0]]);
    let r1 = axis_rot_deg(axes[order[1]], deg[order[1]]);
    let r2 = axis_rot_deg(axes[order[2]], deg[order[2]]);
    r2 * r1 * r0
}

fn build_usd_cube(prim: &UsdPrim, mat: Option<usize>, ctx: &mut BuildCtx<'_>) -> usize {
    let s = find_prop(prim, "size").and_then(UsdVal::as_f64).unwrap_or(2.0) as f32;
    let params = crate::prim::PrimParams { size: [s, s, s], ..crate::prim::PrimParams::default() };
    let mut p = crate::prim::mesh(crate::prim::PrimitiveKind::Cube, &params);
    p.material = mat;
    let idx = ctx.meshes.len();
    ctx.meshes.push(Mesh { name: prim.name.clone(), primitives: vec![p] });
    idx
}

fn build_usd_sphere(prim: &UsdPrim, mat: Option<usize>, ctx: &mut BuildCtx<'_>) -> usize {
    let r = find_prop(prim, "radius").and_then(UsdVal::as_f64).unwrap_or(1.0) as f32;
    let params = crate::prim::PrimParams { radius: r, segments: 32, rings: 16, ..crate::prim::PrimParams::default() };
    let mut p = crate::prim::mesh(crate::prim::PrimitiveKind::Sphere, &params);
    p.material = mat;
    let idx = ctx.meshes.len();
    ctx.meshes.push(Mesh { name: prim.name.clone(), primitives: vec![p] });
    idx
}

fn read_int_array(val: Option<&UsdVal>) -> Vec<i64> {
    match val {
        Some(UsdVal::ArrayNum(v)) => v.iter().map(|&x| x as i64).collect(),
        _ => Vec::new(),
    }
}

fn read_vec3_array(val: Option<&UsdVal>) -> Vec<[f32; 3]> {
    match val {
        Some(UsdVal::ArrayTuple(v)) => v
            .iter()
            .filter(|t| t.len() >= 3)
            .map(|t| [t[0] as f32, t[1] as f32, t[2] as f32])
            .collect(),
        _ => Vec::new(),
    }
}

fn read_uv_array(val: Option<&UsdVal>) -> Vec<[f32; 2]> {
    match val {
        Some(UsdVal::ArrayTuple(v)) => v
            .iter()
            .filter(|t| t.len() >= 2)
            // USD texture coordinates have `(0, 0)` at bottom-left; EffectCraft's top-left flips V.
            .map(|t| [t[0] as f32, 1.0 - t[1] as f32])
            .collect(),
        _ => Vec::new(),
    }
}

fn ensure_double_sided_material(mat_idx: Option<usize>, ctx: &mut BuildCtx<'_>) -> Option<usize> {
    match mat_idx {
        Some(i) => {
            if ctx.materials.get(i).is_some_and(|m| m.double_sided) {
                return Some(i);
            }
            let mut m = ctx.materials.get(i)?.clone();
            m.double_sided = true;
            let next = ctx.materials.len();
            ctx.materials.push(m);
            Some(next)
        }
        None => {
            let m = Material {
                base_color: [0.8, 0.8, 0.8, 1.0],
                metallic: 0.0,
                roughness: 0.5,
                double_sided: true,
                ..Material::default()
            };
            let next = ctx.materials.len();
            ctx.materials.push(m);
            Some(next)
        }
    }
}

fn build_usd_mesh(
    prim: &UsdPrim,
    default_mat: Option<usize>,
    ctx: &mut BuildCtx<'_>,
) -> Result<Option<usize>> {
    let points = read_vec3_array(find_prop(prim, "points"));
    let counts = read_int_array(find_prop(prim, "faceVertexCounts"));
    let indices = read_int_array(find_prop(prim, "faceVertexIndices"));
    if points.is_empty() || counts.is_empty() || indices.is_empty() {
        return Ok(None);
    }

    let double_sided = find_prop(prim, "doubleSided").and_then(UsdVal::as_bool).unwrap_or(false);
    let left_handed = find_prop(prim, "orientation")
        .and_then(UsdVal::as_str)
        .is_some_and(|s| s == "leftHanded");

    let (normals, norm_indices) = read_mesh_normals(prim);
    let (uvs, uv_indices) = read_mesh_uvs(prim);

    let face_mats = resolve_face_materials(prim, counts.len(), default_mat, double_sided, ctx);
    let mut builders: HashMap<Option<usize>, PrimBuilder> = HashMap::new();
    let mut corner_cursor = 0usize;

    for (face_idx, &raw_cnt) in counts.iter().enumerate() {
        let cnt = usize::try_from(raw_cnt).unwrap_or(0);
        let end = corner_cursor.saturating_add(cnt);
        if end > indices.len() {
            break;
        }
        if cnt >= 3 {
            let mat = face_mats.get(face_idx).copied().unwrap_or(default_mat);
            let pb = builders.entry(mat).or_insert_with(|| PrimBuilder::new(mat));
            for k in 1..(cnt - 1) {
                let tri = if left_handed {
                    [corner_cursor, corner_cursor + k + 1, corner_cursor + k]
                } else {
                    [corner_cursor, corner_cursor + k, corner_cursor + k + 1]
                };
                if !emit_usd_tri(
                    pb,
                    tri,
                    face_idx,
                    &points,
                    &indices,
                    (&normals, &norm_indices),
                    (&uvs, &uv_indices),
                ) {
                    continue;
                }
            }
        }
        corner_cursor = end;
    }

    let mut primitives = Vec::new();
    let mut mat_keys: Vec<Option<usize>> = builders.keys().copied().collect();
    mat_keys.sort_unstable();
    for k in mat_keys {
        let Some(pb) = builders.remove(&k) else { continue };
        let mut p = pb.finish();
        if p.indices.is_empty() {
            continue;
        }
        ctx.total_vertices = ctx.total_vertices.saturating_add(p.positions.len());
        if ctx.total_vertices > MAX_USD_VERTICES {
            return Err(perr("usda: total expanded vertices exceed import limit"));
        }
        if p.normals.is_empty() {
            p.compute_normals();
        }
        if !p.uvs.is_empty() {
            p.compute_tangents();
        }
        primitives.push(p);
    }
    if primitives.is_empty() {
        return Ok(None);
    }
    let idx = ctx.meshes.len();
    ctx.meshes.push(Mesh { name: prim.name.clone(), primitives });
    Ok(Some(idx))
}

fn read_mesh_normals(prim: &UsdPrim) -> (Vec<[f32; 3]>, Vec<i64>) {
    if let Some(v) = find_prop(prim, "primvars:normals") {
        let idx = read_int_array(find_prop(prim, "primvars:normals:indices"));
        return (read_vec3_array(Some(v)), idx);
    }
    (read_vec3_array(find_prop(prim, "normals")), Vec::new())
}

fn read_mesh_uvs(prim: &UsdPrim) -> (Vec<[f32; 2]>, Vec<i64>) {
    for name in ["primvars:st", "primvars:st0", "primvars:UVMap", "primvars:uv", "texCoords"] {
        if let Some(v) = find_prop(prim, name) {
            let idx = read_int_array(find_prop(prim, &format!("{name}:indices")));
            return (read_uv_array(Some(v)), idx);
        }
    }
    (Vec::new(), Vec::new())
}

fn resolve_face_materials(
    prim: &UsdPrim,
    face_count: usize,
    default_mat: Option<usize>,
    double_sided: bool,
    ctx: &mut BuildCtx<'_>,
) -> Vec<Option<usize>> {
    let base_mat = if double_sided { ensure_double_sided_material(default_mat, ctx) } else { default_mat };
    let mut face_mats = vec![base_mat; face_count];
    for ch in &prim.children {
        if ch.prim_type != "GeomSubset" {
            continue;
        }
        let Some(mut sub_mat) = bound_material(ch, ctx) else { continue };
        if double_sided {
            sub_mat = ensure_double_sided_material(Some(sub_mat), ctx).unwrap_or(sub_mat);
        }
        for fi in read_int_array(find_prop(ch, "indices")) {
            if let Ok(u) = usize::try_from(fi)
                && let Some(slot) = face_mats.get_mut(u)
            {
                *slot = Some(sub_mat);
            }
        }
    }
    face_mats
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct VertKey {
    pos: u32,
    norm: [u32; 3],
    uv: [u32; 2],
}

struct PrimBuilder {
    prim: Primitive,
    has_normals: bool,
    has_uvs: bool,
    dedup: HashMap<VertKey, u32>,
}

impl PrimBuilder {
    fn new(material: Option<usize>) -> Self {
        Self {
            prim: Primitive { material, ..Primitive::default() },
            has_normals: false,
            has_uvs: false,
            dedup: HashMap::new(),
        }
    }

    fn push_vertex(&mut self, pos_idx: u32, pos: [f32; 3], norm: Option<[f32; 3]>, uv: Option<[f32; 2]>) -> u32 {
        let key = VertKey {
            pos: pos_idx,
            norm: norm.map_or([u32::MAX; 3], |n| n.map(f32::to_bits)),
            uv: uv.map_or([u32::MAX; 2], |u| u.map(f32::to_bits)),
        };
        if let Some(&id) = self.dedup.get(&key) {
            return id;
        }
        let id = self.prim.positions.len() as u32;
        self.prim.positions.push(pos);
        if let Some(n) = norm {
            self.has_normals = true;
            self.prim.normals.push(crate::normalize(n).unwrap_or([0.0, 0.0, 1.0]));
        } else if self.has_normals {
            self.prim.normals.push([0.0, 0.0, 1.0]);
        }
        if let Some(u) = uv {
            self.has_uvs = true;
            self.prim.uvs.push(u);
        } else if self.has_uvs {
            self.prim.uvs.push([0.0, 0.0]);
        }
        self.dedup.insert(key, id);
        id
    }

    fn finish(mut self) -> Primitive {
        if !self.has_normals {
            self.prim.normals.clear();
        } else {
            self.prim.normals.resize(self.prim.positions.len(), [0.0, 0.0, 1.0]);
        }
        if !self.has_uvs {
            self.prim.uvs.clear();
        } else {
            self.prim.uvs.resize(self.prim.positions.len(), [0.0, 0.0]);
        }
        self.prim
    }
}

fn sample_attr<T: Copy>(
    values: &[T],
    attr_indices: &[i64],
    corner_idx: usize,
    vert_idx: usize,
    face_idx: usize,
    total_corners: usize,
    total_verts: usize,
) -> Option<T> {
    if values.is_empty() {
        return None;
    }
    if !attr_indices.is_empty() {
        let raw = if attr_indices.len() == total_corners {
            *attr_indices.get(corner_idx)?
        } else if attr_indices.len() == total_verts {
            *attr_indices.get(vert_idx)?
        } else {
            *attr_indices.get(face_idx)?
        };
        return values.get(usize::try_from(raw).ok()?).copied();
    }
    if values.len() == total_corners {
        values.get(corner_idx).copied()
    } else if values.len() == total_verts {
        values.get(vert_idx).copied()
    } else {
        values.get(face_idx).or_else(|| values.first()).copied()
    }
}

fn emit_usd_tri(
    pb: &mut PrimBuilder,
    tri_corners: [usize; 3],
    face_idx: usize,
    points: &[[f32; 3]],
    face_indices: &[i64],
    normals: (&[[f32; 3]], &[i64]),
    uvs: (&[[f32; 2]], &[i64]),
) -> bool {
    let mut v_indices = [0usize; 3];
    for (slot, &ci) in v_indices.iter_mut().zip(&tri_corners) {
        let Some(&raw) = face_indices.get(ci) else { return false };
        let Ok(vi) = usize::try_from(raw) else { return false };
        if vi >= points.len() {
            return false;
        }
        *slot = vi;
    }
    let total_corners = face_indices.len();
    let total_verts = points.len();
    let mut out_ids = [0u32; 3];
    for i in 0..3 {
        let ci = tri_corners[i];
        let vi = v_indices[i];
        let norm = sample_attr(normals.0, normals.1, ci, vi, face_idx, total_corners, total_verts);
        let uv = sample_attr(uvs.0, uvs.1, ci, vi, face_idx, total_corners, total_verts);
        out_ids[i] = pb.push_vertex(vi as u32, points[vi], norm, uv);
    }
    pb.prim.indices.extend_from_slice(&out_ids);
    true
}
