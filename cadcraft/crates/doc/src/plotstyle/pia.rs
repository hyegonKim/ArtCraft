//! AutoCAD PIA 2.0 plot style table (`.ctb` / `.stb`) compressed (`pmzlibcodec`) and uncompressed
//! text reader and writer.
//!
//! Binary `.ctb` and `.stb` files start with `PIAFILEVERSION_2.0,{CTB|STB}VER1,compress\r\n`,
//! followed by `pmzlibcodec`, a 12-byte little-endian `[adler32, uncompressed_size, compressed_size]`
//! header (or a raw zlib stream in simplified writers), and a zlib-compressed PIA key/value and
//! nested-block body. Uncompressed PIA files use `uncompress` and store the same structured body
//! directly as text.

use std::path::Path;

use cadcraft_color::{Rgb, aci_rgb};
use miniz_oxide::deflate::compress_to_vec_zlib;
use miniz_oxide::inflate::decompress_to_vec_zlib_with_limit;

use super::{EndStyle, FillStyle, JoinStyle, MAX_TABLE_FILE, PlotStyle, PlotStyleKind, PlotStyleTable};

/// Magic prefix of every AutoCAD PIA file.
const PIA_PREFIX: &str = "PIAFILEVERSION_";
/// Codec tag placed before the compressed payload in binary PIA files.
const ZLIB_CODEC: &[u8] = b"pmzlibcodec";
/// PIA colour value meaning "Use object color" (`0xFA000000u32 as i32`).
const PIA_OBJECT_COLOR: i32 = -100_663_296;
/// PIA linetype index meaning "Use object linetype".
const PIA_OBJECT_LINETYPE: i32 = 31;

/// Standard 27 ISO/ANSI lineweight table entries in millimetres (`0`..=`26`).
const DEFAULT_LINEWEIGHTS_MM: [f64; 27] = [
    0.0, 0.05, 0.09, 0.1, 0.13, 0.15, 0.18, 0.2, 0.25, 0.3, 0.35, 0.4, 0.45, 0.5, 0.53, 0.6, 0.65, 0.7, 0.8, 0.9, 1.0, 1.06, 1.2, 1.4, 1.58,
    2.0, 2.11,
];

/// Standard 31 PIA plot style linetype names (`0`..=`30`; `31` is "Use object linetype").
const PIA_LINETYPES: [&str; 31] = [
    "Continuous",
    "DASHED",
    "DOTTED",
    "DASHDOT",
    "Medium Dash",
    "Long Dash",
    "Short Dash",
    "Short Dash X2",
    "Medium Dash X2",
    "Long Dash X2",
    "Medium Long Dash",
    "Medium Dash Short Dash Short Dash",
    "Long Dash Short Dash",
    "Long Dash Dot Dot",
    "Long Dash Dot",
    "Medium Dash Dot Short Dash Dot",
    "Sparse Dot",
    "ISO Dash",
    "ISO Dash Space",
    "ISO Long Dash Dot",
    "ISO Long Dash Double Dot",
    "ISO Long Dash Triple Dot",
    "ISO Dot",
    "ISO Long Dash Short Dash",
    "ISO Long Dash Double Short Dash",
    "ISO Dash Dot",
    "ISO Double Dash Dot",
    "ISO Dash Double Dot",
    "ISO Double Dash Double Dot",
    "ISO Dash Triple Dot",
    "ISO Double Dash Triple Dot",
];

/// Which PIA plot style table variant a file carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PiaVariant {
    /// Colour-dependent plot style table (`CTBVER1`).
    Ctb,
    /// Named plot style table (`STBVER1`).
    Stb,
}

impl PiaVariant {
    fn tag(self) -> &'static str {
        match self {
            PiaVariant::Ctb => "CTBVER1",
            PiaVariant::Stb => "STBVER1",
        }
    }

    fn kind(self) -> PlotStyleKind {
        match self {
            PiaVariant::Ctb => PlotStyleKind::ColorDependent,
            PiaVariant::Stb => PlotStyleKind::Named,
        }
    }
}

/// Parsed first line of a PIA plot style table file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PiaHeader {
    pub version: String,
    pub variant: PiaVariant,
    pub compressed: bool,
}

/// One node in a structured PIA document: either `key=value` or `name{ ... }`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PiaNode {
    Pair { key: String, value: String },
    Block { name: String, children: Vec<PiaNode> },
}

/// Structured representation of one `plot_style{ i{ ... } }` entry in a PIA table.
#[derive(Clone, Debug, PartialEq)]
pub struct PiaPlotStyle {
    pub name: String,
    pub localized_name: String,
    pub description: String,
    pub color: i32,
    pub mode_color: Option<i32>,
    pub color_policy: i32,
    pub physical_pen_number: i32,
    pub virtual_pen_number: i32,
    pub screen: i32,
    pub linepattern_size: f64,
    pub linetype: i32,
    pub custom_linetype: Option<String>,
    pub adaptive_linetype: bool,
    pub lineweight: i32,
    pub fill_style: i32,
    pub end_style: i32,
    pub join_style: i32,
    pub dither: bool,
    pub grayscale: bool,
}

impl Default for PiaPlotStyle {
    fn default() -> Self {
        Self {
            name: String::new(),
            localized_name: String::new(),
            description: String::new(),
            color: PIA_OBJECT_COLOR,
            mode_color: None,
            color_policy: 1,
            physical_pen_number: 0,
            virtual_pen_number: 0,
            screen: 100,
            linepattern_size: 0.5,
            linetype: PIA_OBJECT_LINETYPE,
            custom_linetype: None,
            adaptive_linetype: true,
            lineweight: 0,
            fill_style: 73,
            end_style: 4,
            join_style: 5,
            dither: true,
            grayscale: false,
        }
    }
}

/// Structured representation of a parsed PIA `.ctb` or `.stb` file.
#[derive(Clone, Debug, PartialEq)]
pub struct PiaTable {
    pub header: PiaHeader,
    pub description: String,
    pub aci_table_available: bool,
    pub scale_factor: f64,
    pub apply_factor: bool,
    pub custom_lineweight_display_units: i32,
    pub lineweights_mm: Vec<f64>,
    pub styles: Vec<PiaPlotStyle>,
}

/// Whether `bytes` begins with an AutoCAD PIA header (`PIAFILEVERSION_...`).
pub fn looks_like_pia(bytes: &[u8]) -> bool {
    let trimmed = strip_bom_and_ws(bytes);
    trimmed.len() >= PIA_PREFIX.len() && trimmed[..PIA_PREFIX.len()].eq_ignore_ascii_case(PIA_PREFIX.as_bytes())
}

fn strip_bom_and_ws(mut b: &[u8]) -> &[u8] {
    if let Some(rest) = b.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        b = rest;
    }
    while let Some((&first, rest)) = b.split_first() {
        if first == b' ' || first == b'\t' || first == b'\r' || first == b'\n' {
            b = rest;
        } else {
            break;
        }
    }
    b
}

/// Compute RFC 1950 Adler-32 checksum.
pub fn adler32(bytes: &[u8]) -> u32 {
    const MOD_ADLER: u32 = 65_521;
    let mut s1: u32 = 1;
    let mut s2: u32 = 0;
    for chunk in bytes.chunks(5552) {
        for &b in chunk {
            s1 += u32::from(b);
            s2 += s1;
        }
        s1 %= MOD_ADLER;
        s2 %= MOD_ADLER;
    }
    (s2 << 16) | s1
}

/// Parse the first header line `PIAFILEVERSION_2.0,{CTB|STB}VER1,{compress|uncompress}`.
fn parse_header_line(bytes: &[u8]) -> Result<(PiaHeader, &[u8]), String> {
    let input = strip_bom_and_ws(bytes);
    let nl = input.iter().position(|&b| b == b'\n').ok_or_else(|| "missing PIA header newline".to_string())?;
    let mut line = &input[..nl];
    if let Some(stripped) = line.strip_suffix(b"\r") {
        line = stripped;
    }
    let header_str = std::str::from_utf8(line).map_err(|_| "invalid UTF-8 in PIA header".to_string())?;
    let mut parts = header_str.split(',').map(str::trim);
    let ver_part = parts.next().unwrap_or_default();
    if ver_part.len() <= PIA_PREFIX.len() || !ver_part[..PIA_PREFIX.len()].eq_ignore_ascii_case(PIA_PREFIX) {
        return Err("not an AutoCAD PIA plot style table".into());
    }
    let version = ver_part[PIA_PREFIX.len()..].to_string();
    let kind_part = parts.next().ok_or_else(|| "missing PIA table type (CTBVER1/STBVER1)".to_string())?;
    let variant = if kind_part.eq_ignore_ascii_case("CTBVER1") {
        PiaVariant::Ctb
    } else if kind_part.eq_ignore_ascii_case("STBVER1") {
        PiaVariant::Stb
    } else {
        return Err(format!("unsupported PIA table type `{kind_part}`"));
    };
    let mode_part = parts.next().ok_or_else(|| "missing PIA compression mode".to_string())?;
    let compressed = if mode_part.eq_ignore_ascii_case("compress") || mode_part.eq_ignore_ascii_case("pmzlibcodec") {
        true
    } else if mode_part.eq_ignore_ascii_case("uncompress") || mode_part.eq_ignore_ascii_case("none") {
        false
    } else {
        return Err(format!("unsupported PIA compression mode `{mode_part}`"));
    };
    Ok((PiaHeader { version, variant, compressed }, &input[nl + 1..]))
}

/// Decompress a `pmzlibcodec` payload, accepting both the standard AutoCAD 12-byte
/// `[adler32: u32 LE, uncompressed_size: u32 LE, compressed_size: u32 LE]` header and raw zlib streams.
fn decode_compressed_body(after_header: &[u8]) -> Result<Vec<u8>, String> {
    let mut rest = after_header;
    if rest.len() >= ZLIB_CODEC.len() && rest[..ZLIB_CODEC.len()].eq_ignore_ascii_case(ZLIB_CODEC) {
        rest = &rest[ZLIB_CODEC.len()..];
    }
    if let Some(decoded) = try_12byte_header_zlib(rest)? {
        return Ok(decoded);
    }
    if let Some(trimmed) = rest.strip_prefix(b"\r\n").or_else(|| rest.strip_prefix(b"\n")) {
        if let Some(decoded) = try_12byte_header_zlib(trimmed)? {
            return Ok(decoded);
        }
        if let Ok(decoded) = decompress_to_vec_zlib_with_limit(trimmed, MAX_TABLE_FILE) {
            return Ok(decoded);
        }
    }
    decompress_to_vec_zlib_with_limit(rest, MAX_TABLE_FILE).map_err(|e| format!("corrupt PIA zlib stream: {e}"))
}

fn try_12byte_header_zlib(buf: &[u8]) -> Result<Option<Vec<u8>>, String> {
    if buf.len() < 14 {
        return Ok(None);
    }
    let uncomp_len = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]) as usize;
    let comp_len = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]) as usize;
    if uncomp_len > MAX_TABLE_FILE {
        // Distinguish a raw zlib stream (starts with 0x78) from an oversized 12-byte header.
        if buf[0] == 0x78 {
            return Ok(None);
        }
        return Err("the decompressed plot style table exceeds the size limit".into());
    }
    if comp_len == 0 || 12 + comp_len > buf.len() {
        return Ok(None);
    }
    let zslice = &buf[12..12 + comp_len];
    match decompress_to_vec_zlib_with_limit(zslice, MAX_TABLE_FILE) {
        Ok(out) => Ok(Some(out)),
        Err(_) if buf[0] == 0x78 => Ok(None),
        Err(e) => Err(format!("corrupt PIA zlib payload: {e}")),
    }
}

/// Decode a PIA byte slice (UTF-8 or Latin-1 fallback for legacy localized names/descriptions).
fn decode_pia_text(bytes: &[u8]) -> String {
    let bytes = bytes.strip_suffix(&[0]).unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| char::from(b)).collect(),
    }
}

/// Parse the structured PIA body into a tree of [`PiaNode`]s.
pub fn parse_pia_nodes(body: &str) -> Result<Vec<PiaNode>, String> {
    let mut stack: Vec<(String, Vec<PiaNode>)> = vec![(String::new(), Vec::new())];
    for (line_no, raw_line) in body.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "}" {
            if stack.len() <= 1 {
                return Err(format!("unexpected `}}` on PIA line {}", line_no + 1));
            }
            if let Some((name, children)) = stack.pop()
                && let Some(parent) = stack.last_mut()
            {
                parent.1.push(PiaNode::Block { name, children });
            }
            continue;
        }
        if let Some(block_name) = line.strip_suffix('{') {
            if stack.len() >= 16 {
                return Err("PIA block nesting is too deep".into());
            }
            stack.push((block_name.trim().to_string(), Vec::new()));
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if let Some(cur) = stack.last_mut() {
                cur.1.push(PiaNode::Pair { key: k.trim().to_string(), value: v.to_string() });
            }
            continue;
        }
        return Err(format!("invalid PIA syntax on line {}: `{line}`", line_no + 1));
    }
    if stack.len() != 1 {
        return Err("unclosed `{` block in PIA plot style table".into());
    }
    Ok(stack.pop().map(|(_, nodes)| nodes).unwrap_or_default())
}

fn pia_unquote(raw: &str) -> String {
    let trimmed = raw.trim_end_matches('\r');
    match trimmed.strip_prefix('"') {
        Some(rest) => rest.strip_suffix('"').unwrap_or(rest).to_string(),
        None => trimmed.trim().to_string(),
    }
}

fn pia_bool(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    }
}

fn pia_i32(raw: &str) -> Option<i32> {
    raw.trim().parse::<i32>().ok()
}

fn pia_f64(raw: &str) -> Option<f64> {
    raw.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

fn parse_lineweight_table(children: &[PiaNode]) -> Vec<f64> {
    let mut indexed: Vec<(usize, f64)> = Vec::new();
    for node in children {
        if let PiaNode::Pair { key, value } = node
            && let (Ok(idx), Some(mm)) = (key.parse::<usize>(), pia_f64(value))
            && idx < 256
        {
            indexed.push((idx, mm.max(0.0)));
        }
    }
    if indexed.is_empty() {
        return DEFAULT_LINEWEIGHTS_MM.to_vec();
    }
    indexed.sort_by_key(|(i, _)| *i);
    let len = indexed.last().map_or(0, |(i, _)| i + 1);
    let mut out = vec![0.0; len];
    for (i, mm) in indexed {
        out[i] = mm;
    }
    out
}

fn parse_pia_style(children: &[PiaNode]) -> PiaPlotStyle {
    let mut st = PiaPlotStyle::default();
    let mut explicit_dither: Option<bool> = None;
    let mut explicit_gray: Option<bool> = None;
    let mut has_policy = false;
    for node in children {
        let PiaNode::Pair { key, value } = node else { continue };
        apply_style_pair(&mut st, key, value, &mut explicit_dither, &mut explicit_gray, &mut has_policy);
    }
    if has_policy {
        st.dither = (st.color_policy & 1) != 0;
        st.grayscale = (st.color_policy & 2) != 0;
    }
    if let Some(d) = explicit_dither {
        st.dither = d;
    }
    if let Some(g) = explicit_gray {
        st.grayscale = g;
    }
    if st.localized_name.is_empty() {
        st.localized_name = st.name.clone();
    }
    st
}

fn apply_style_pair(
    st: &mut PiaPlotStyle,
    key: &str,
    value: &str,
    explicit_dither: &mut Option<bool>,
    explicit_gray: &mut Option<bool>,
    has_policy: &mut bool,
) {
    match key.to_ascii_lowercase().as_str() {
        "name" => st.name = pia_unquote(value),
        "localized_name" => st.localized_name = pia_unquote(value),
        "description" => st.description = pia_unquote(value),
        "color" => st.color = parse_pia_color_field(value),
        "mode_color" => st.mode_color = pia_i32(value),
        "color_policy" => {
            if let Some(v) = pia_i32(value) {
                st.color_policy = v;
                *has_policy = true;
            }
        }
        "physical_pen_number" => st.physical_pen_number = pia_i32(value).unwrap_or(0),
        "virtual_pen_number" => st.virtual_pen_number = pia_i32(value).unwrap_or(0),
        "screen" => st.screen = pia_i32(value).unwrap_or(100),
        "linepattern_size" => st.linepattern_size = pia_f64(value).unwrap_or(0.5),
        "linetype" => match pia_i32(value) {
            Some(v) => {
                st.linetype = v;
                st.custom_linetype = None;
            }
            None => {
                let s = pia_unquote(value);
                if s.is_empty() || s.eq_ignore_ascii_case("object") {
                    st.linetype = PIA_OBJECT_LINETYPE;
                    st.custom_linetype = None;
                } else {
                    st.custom_linetype = Some(s);
                }
            }
        },
        "adaptive_linetype" => st.adaptive_linetype = pia_bool(value).unwrap_or(true),
        "lineweight" => st.lineweight = pia_i32(value).unwrap_or(0),
        "fill_style" => st.fill_style = pia_i32(value).unwrap_or(73),
        "end_style" => st.end_style = pia_i32(value).unwrap_or(4),
        "join_style" => st.join_style = pia_i32(value).unwrap_or(5),
        "dither" => *explicit_dither = pia_bool(value),
        "grayscale" => *explicit_gray = pia_bool(value),
        _ => {}
    }
}

fn parse_pia_color_field(value: &str) -> i32 {
    if let Some(n) = pia_i32(value) {
        return n;
    }
    let unquoted = pia_unquote(value);
    if unquoted.eq_ignore_ascii_case("object") || unquoted.is_empty() {
        return PIA_OBJECT_COLOR;
    }
    if let Some(rgb) = Rgb::parse_hex(&unquoted) {
        return encode_pia_rgb(rgb);
    }
    PIA_OBJECT_COLOR
}

fn encode_pia_rgb(rgb: Rgb) -> i32 {
    ((0xC2u32 << 24) | rgb.to_u32()) as i32
}

fn decode_pia_color(color: i32, policy: i32) -> Option<Rgb> {
    if color == PIA_OBJECT_COLOR || color == -1 {
        return None;
    }
    let u = color as u32;
    let tag = (u >> 24) & 0xFF;
    match tag {
        0xFA | 0xFF => None,
        0xC3 => {
            let aci = (u & 0xFF) as u8;
            if aci == 0 { None } else { Some(aci_rgb(aci)) }
        }
        0xC2 | 0xC0 | 0xC1 => Some(Rgb::from_u32(u)),
        0x00 => {
            // Plain 0x00RRGGBB integer when explicit colour bit or non-object value is set.
            if color == 0 && (policy & 4) == 0 && policy != 0 {
                Some(Rgb(0, 0, 0))
            } else {
                Some(Rgb::from_u32(u))
            }
        }
        _ => Some(Rgb::from_u32(u)),
    }
}

fn decode_linetype(code: i32, custom: Option<&str>) -> Option<String> {
    if let Some(name) = custom {
        return Some(name.to_string());
    }
    if code == PIA_OBJECT_LINETYPE || code < 0 {
        return None;
    }
    PIA_LINETYPES.get(code as usize).map(|s| (*s).to_string())
}

fn encode_linetype(lt: Option<&str>) -> String {
    let Some(name) = lt.map(str::trim).filter(|s| !s.is_empty()) else {
        return PIA_OBJECT_LINETYPE.to_string();
    };
    if name.eq_ignore_ascii_case("solid") {
        return "0".to_string();
    }
    if let Some(idx) = PIA_LINETYPES.iter().position(|x| x.eq_ignore_ascii_case(name)) {
        return idx.to_string();
    }
    format!("\"{}", sanitize_pia_str(name))
}

fn decode_end_style(v: i32) -> EndStyle {
    match v {
        0 => EndStyle::Butt,
        1 => EndStyle::Square,
        2 => EndStyle::Round,
        3 => EndStyle::Diamond,
        _ => EndStyle::Object,
    }
}

fn encode_end_style(e: EndStyle) -> i32 {
    match e {
        EndStyle::Butt => 0,
        EndStyle::Square => 1,
        EndStyle::Round => 2,
        EndStyle::Diamond => 3,
        EndStyle::Object => 4,
    }
}

fn decode_join_style(v: i32) -> JoinStyle {
    match v {
        0 => JoinStyle::Miter,
        1 => JoinStyle::Bevel,
        2 => JoinStyle::Round,
        3 => JoinStyle::Diamond,
        _ => JoinStyle::Object,
    }
}

fn encode_join_style(j: JoinStyle) -> i32 {
    match j {
        JoinStyle::Miter => 0,
        JoinStyle::Bevel => 1,
        JoinStyle::Round => 2,
        JoinStyle::Diamond => 3,
        JoinStyle::Object => 5,
    }
}

fn decode_fill_style(v: i32) -> FillStyle {
    match v {
        64 => FillStyle::Solid,
        65 => FillStyle::Checkerboard,
        66 => FillStyle::Crosshatch,
        67 => FillStyle::Diamonds,
        68 => FillStyle::HorizontalBars,
        69 => FillStyle::SlantLeft,
        70 => FillStyle::SlantRight,
        71 => FillStyle::SquareDots,
        72 => FillStyle::VerticalBars,
        _ => FillStyle::Object,
    }
}

fn encode_fill_style(f: FillStyle) -> i32 {
    match f {
        FillStyle::Solid => 64,
        FillStyle::Checkerboard => 65,
        FillStyle::Crosshatch => 66,
        FillStyle::Diamonds => 67,
        FillStyle::HorizontalBars => 68,
        FillStyle::SlantLeft => 69,
        FillStyle::SlantRight => 70,
        FillStyle::SquareDots => 71,
        FillStyle::VerticalBars => 72,
        FillStyle::Object => 73,
    }
}

/// Parse a binary or uncompressed PIA `.ctb` / `.stb` payload into its structured [`PiaTable`].
pub fn parse_pia_table(bytes: &[u8]) -> Result<PiaTable, String> {
    if bytes.len() > MAX_TABLE_FILE {
        return Err("the plot style table file is too large".into());
    }
    let (header, rest) = parse_header_line(bytes)?;
    let body_bytes;
    let raw_body = if header.compressed {
        body_bytes = decode_compressed_body(rest)?;
        &body_bytes[..]
    } else {
        rest
    };
    let text = decode_pia_text(raw_body);
    let nodes = parse_pia_nodes(&text)?;

    let mut table = PiaTable {
        aci_table_available: header.variant == PiaVariant::Ctb,
        header,
        description: String::new(),
        scale_factor: 1.0,
        apply_factor: false,
        custom_lineweight_display_units: 0,
        lineweights_mm: DEFAULT_LINEWEIGHTS_MM.to_vec(),
        styles: Vec::new(),
    };

    for node in &nodes {
        match node {
            PiaNode::Pair { key, value } => match key.to_ascii_lowercase().as_str() {
                "description" => table.description = pia_unquote(value),
                "aci_table_available" => table.aci_table_available = pia_bool(value).unwrap_or(table.aci_table_available),
                "scale_factor" => table.scale_factor = pia_f64(value).unwrap_or(1.0),
                "apply_factor" => table.apply_factor = pia_bool(value).unwrap_or(false),
                "custom_lineweight_display_units" => table.custom_lineweight_display_units = pia_i32(value).unwrap_or(0),
                _ => {}
            },
            PiaNode::Block { name, children } if name.eq_ignore_ascii_case("custom_lineweight_table") => {
                table.lineweights_mm = parse_lineweight_table(children);
            }
            PiaNode::Block { name, children } if name.eq_ignore_ascii_case("plot_style") => {
                let mut indexed: Vec<(usize, PiaPlotStyle)> = Vec::new();
                for (pos, child) in children.iter().enumerate() {
                    if let PiaNode::Block { name: idx_str, children: style_nodes } = child {
                        let idx = idx_str.parse::<usize>().unwrap_or(pos);
                        indexed.push((idx, parse_pia_style(style_nodes)));
                    }
                }
                indexed.sort_by_key(|(idx, _)| *idx);
                table.styles = indexed.into_iter().map(|(_, s)| s).collect();
            }
            PiaNode::Block { .. } => {}
        }
    }
    Ok(table)
}

/// Parse a binary or uncompressed PIA `.ctb` / `.stb` file into a sanitized [`PlotStyleTable`].
pub fn parse_pia_bytes(bytes: &[u8], file_name: Option<&str>) -> Result<PlotStyleTable, String> {
    let pia = parse_pia_table(bytes)?;
    let kind = pia.header.variant.kind();
    let styles: Vec<PlotStyle> = pia
        .styles
        .iter()
        .enumerate()
        .map(|(idx, ps)| {
            let fallback = if kind == PlotStyleKind::ColorDependent { format!("Color_{}", idx + 1) } else { format!("Style_{}", idx + 1) };
            let name = if !ps.name.trim().is_empty() {
                ps.name.clone()
            } else if !ps.localized_name.trim().is_empty() {
                ps.localized_name.clone()
            } else {
                fallback
            };
            let lineweight = if ps.lineweight <= 0 {
                None
            } else {
                pia.lineweights_mm.get((ps.lineweight - 1) as usize).map(|mm| (mm * 100.0).round().clamp(0.0, 211.0) as u16)
            };
            PlotStyle {
                name,
                description: ps.description.clone(),
                color: decode_pia_color(ps.color, ps.color_policy),
                grayscale: ps.grayscale,
                screening: ps.screen.clamp(0, 100) as u8,
                linetype: decode_linetype(ps.linetype, ps.custom_linetype.as_deref()),
                lineweight,
                end: decode_end_style(ps.end_style),
                join: decode_join_style(ps.join_style),
                fill: decode_fill_style(ps.fill_style),
            }
        })
        .collect();

    let inferred_name = file_name
        .and_then(|f| Path::new(f).file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    Ok(PlotStyleTable { name: inferred_name, description: pia.description, kind, styles }.sanitized())
}

fn sanitize_pia_str(s: &str) -> String {
    s.chars().filter(|c| *c != '\r' && *c != '\n').collect()
}

fn build_lineweight_table_for_write(styles: &[PlotStyle]) -> (Vec<f64>, Vec<i32>) {
    let mut table: Vec<u16> = DEFAULT_LINEWEIGHTS_MM.iter().map(|mm| (mm * 100.0).round() as u16).collect();
    let mut indices = Vec::with_capacity(styles.len());
    for st in styles {
        match st.lineweight {
            None => indices.push(0),
            Some(w) => {
                let clamped = w.min(211);
                let pos = match table.iter().position(|&x| x == clamped) {
                    Some(i) => i,
                    None => {
                        table.push(clamped);
                        table.len() - 1
                    }
                };
                indices.push((pos + 1) as i32);
            }
        }
    }
    let floats = table
        .iter()
        .enumerate()
        .map(|(i, &w)| if i < DEFAULT_LINEWEIGHTS_MM.len() { DEFAULT_LINEWEIGHTS_MM[i] } else { f64::from(w) / 100.0 })
        .collect();
    (floats, indices)
}

fn format_pia_body(table: &PlotStyleTable, variant: PiaVariant) -> String {
    let clean = {
        let mut copy = table.clone();
        copy.kind = variant.kind();
        copy.sanitized()
    };
    let (lw_table, lw_indices) = build_lineweight_table_for_write(&clean.styles);
    let mut out = String::with_capacity(if variant == PiaVariant::Ctb { 96 * 1024 } else { 4096 });
    out.push_str(&format!("description=\"{}\r\n", sanitize_pia_str(&clean.description)));
    out.push_str(&format!("aci_table_available={}\r\n", if variant == PiaVariant::Ctb { "TRUE" } else { "FALSE" }));
    out.push_str("scale_factor=1.0\r\n");
    out.push_str("apply_factor=FALSE\r\n");
    out.push_str("custom_lineweight_display_units=0\r\n");

    if variant == PiaVariant::Ctb {
        out.push_str("aci_table{\r\n");
        for i in 0..clean.styles.len() {
            out.push_str(&format!(" {i}=\"Color_{}\r\n", i + 1));
        }
        out.push_str("}\r\n");
    }

    out.push_str("plot_style{\r\n");
    for (i, st) in clean.styles.iter().enumerate() {
        let localized = if variant == PiaVariant::Ctb { format!("Color {}", i + 1) } else { st.name.clone() };
        let (color_val, mode_color, explicit_bit) = match st.color {
            Some(rgb) => {
                let encoded = encode_pia_rgb(rgb);
                (encoded, encoded, 4)
            }
            None => (PIA_OBJECT_COLOR, PIA_OBJECT_COLOR, 0),
        };
        let policy = 1 | if st.grayscale { 2 } else { 0 } | explicit_bit;
        let lw_idx = lw_indices.get(i).copied().unwrap_or(0);
        out.push_str(&format!(" {i}{{\r\n"));
        out.push_str(&format!("  name=\"{}\r\n", sanitize_pia_str(&st.name)));
        out.push_str(&format!("  localized_name=\"{}\r\n", sanitize_pia_str(&localized)));
        out.push_str(&format!("  description=\"{}\r\n", sanitize_pia_str(&st.description)));
        out.push_str(&format!("  color={color_val}\r\n"));
        if variant == PiaVariant::Ctb || st.color.is_some() {
            out.push_str(&format!("  mode_color={mode_color}\r\n"));
        }
        out.push_str(&format!("  color_policy={policy}\r\n"));
        out.push_str("  physical_pen_number=0\r\n");
        out.push_str("  virtual_pen_number=0\r\n");
        out.push_str(&format!("  screen={}\r\n", st.screening.min(100)));
        out.push_str("  linepattern_size=0.5\r\n");
        out.push_str(&format!("  linetype={}\r\n", encode_linetype(st.linetype.as_deref())));
        out.push_str("  adaptive_linetype=TRUE\r\n");
        out.push_str(&format!("  lineweight={lw_idx}\r\n"));
        out.push_str(&format!("  fill_style={}\r\n", encode_fill_style(st.fill)));
        out.push_str(&format!("  end_style={}\r\n", encode_end_style(st.end)));
        out.push_str(&format!("  join_style={}\r\n", encode_join_style(st.join)));
        out.push_str("  dither=TRUE\r\n");
        out.push_str(&format!("  grayscale={}\r\n", if st.grayscale { "TRUE" } else { "FALSE" }));
        out.push_str(" }\r\n");
    }
    out.push_str("}\r\n");

    out.push_str("custom_lineweight_table{\r\n");
    for (i, mm) in lw_table.iter().enumerate() {
        out.push_str(&format!(" {i}={}\r\n", format_mm(*mm)));
    }
    out.push_str("}\r\n");
    out
}

fn format_mm(mm: f64) -> String {
    if mm == 0.0 {
        "0.0".to_string()
    } else {
        let s = format!("{mm:.2}");
        let trimmed = s.trim_end_matches('0');
        if trimmed.ends_with('.') { format!("{trimmed}0") } else { trimmed.to_string() }
    }
}

/// Serialize `table` as an uncompressed AutoCAD PIA 2.0 text file (`CTBVER1` or `STBVER1`).
pub fn write_pia_uncompressed(table: &PlotStyleTable) -> String {
    let variant = if table.is_color_dependent() { PiaVariant::Ctb } else { PiaVariant::Stb };
    let body = format_pia_body(table, variant);
    format!("PIAFILEVERSION_2.0,{},uncompress\r\n{body}", variant.tag())
}

fn write_pia_binary_variant(table: &PlotStyleTable, variant: PiaVariant) -> Vec<u8> {
    let body = format_pia_body(table, variant);
    let uncompressed = body.as_bytes();
    let compressed = compress_to_vec_zlib(uncompressed, 6);
    let checksum = adler32(&compressed);

    let header = format!("PIAFILEVERSION_2.0,{},compress\r\n", variant.tag());
    let mut out = Vec::with_capacity(header.len() + ZLIB_CODEC.len() + 12 + compressed.len());
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(ZLIB_CODEC);
    out.extend_from_slice(&checksum.to_le_bytes());
    out.extend_from_slice(&(uncompressed.len() as u32).to_le_bytes());
    out.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
    out.extend_from_slice(&compressed);
    out
}

/// Serialize `table` as a binary compressed AutoCAD PIA 2.0 `.ctb` or `.stb` matching its `kind`.
pub fn write_pia_compressed(table: &PlotStyleTable) -> Vec<u8> {
    let variant = if table.is_color_dependent() { PiaVariant::Ctb } else { PiaVariant::Stb };
    write_pia_binary_variant(table, variant)
}

/// Write a colour-dependent plot style table as a binary AutoCAD `.ctb` (`PIAFILEVERSION_2.0,CTBVER1,compress\r\npmzlibcodec`).
pub fn write_binary_ctb(table: &PlotStyleTable) -> Result<Vec<u8>, String> {
    if !table.is_color_dependent() {
        return Err("named (.stb) plot style tables cannot be written as colour-dependent (.ctb)".into());
    }
    Ok(write_pia_binary_variant(table, PiaVariant::Ctb))
}

/// Write a named plot style table as a binary AutoCAD `.stb` (`PIAFILEVERSION_2.0,STBVER1,compress\r\npmzlibcodec`).
pub fn write_binary_stb(table: &PlotStyleTable) -> Result<Vec<u8>, String> {
    if table.is_color_dependent() {
        return Err("colour-dependent (.ctb) plot style tables cannot be written as named (.stb)".into());
    }
    Ok(write_pia_binary_variant(table, PiaVariant::Stb))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::{EndStyle, FillStyle, JoinStyle, PlotStyle, PlotStyleKind, PlotStyleTable};
    use cadcraft_color::Rgb;

    #[test]
    fn binary_ctb_zlib_roundtrip_preserves_255_aci_styles_and_pen_attributes() {
        let mut table = PlotStyleTable::builtin("default.ctb").unwrap();
        table.name = "arch.ctb".into();
        table.description = "Architectural CTB".into();
        table.styles[0].color = Some(Rgb(0, 0, 0));
        table.styles[0].screening = 85;
        table.styles[0].lineweight = Some(18);
        table.styles[0].linetype = Some("DASHED".into());
        table.styles[0].end = EndStyle::Butt;
        table.styles[0].join = JoinStyle::Miter;
        table.styles[0].fill = FillStyle::Crosshatch;

        table.styles[6].color = None;
        table.styles[6].grayscale = true;
        table.styles[6].lineweight = Some(35);
        table.styles[6].end = EndStyle::Square;
        table.styles[6].join = JoinStyle::Bevel;
        table.styles[6].fill = FillStyle::HorizontalBars;

        let bytes = write_binary_ctb(&table).unwrap();
        assert!(bytes.starts_with(b"PIAFILEVERSION_2.0,CTBVER1,compress\r\npmzlibcodec"));
        assert!(write_binary_stb(&table).is_err());

        let parsed = parse_pia_bytes(&bytes, Some("arch.ctb")).unwrap();
        assert_eq!(parsed.name, "arch.ctb");
        assert_eq!(parsed.description, "Architectural CTB");
        assert_eq!(parsed.kind, PlotStyleKind::ColorDependent);
        assert_eq!(parsed.styles.len(), 255);

        let c1 = &parsed.styles[0];
        assert_eq!(c1.name, "Color_1");
        assert_eq!(c1.color, Some(Rgb(0, 0, 0)));
        assert_eq!(c1.screening, 85);
        assert_eq!(c1.lineweight, Some(18));
        assert_eq!(c1.linetype.as_deref(), Some("DASHED"));
        assert_eq!(c1.end, EndStyle::Butt);
        assert_eq!(c1.join, JoinStyle::Miter);
        assert_eq!(c1.fill, FillStyle::Crosshatch);

        let c7 = &parsed.styles[6];
        assert_eq!(c7.color, None);
        assert!(c7.grayscale);
        assert_eq!(c7.lineweight, Some(35));
        assert_eq!(c7.end, EndStyle::Square);
        assert_eq!(c7.join, JoinStyle::Bevel);
        assert_eq!(c7.fill, FillStyle::HorizontalBars);
    }

    #[test]
    fn binary_stb_and_uncompressed_pia_and_raw_zlib_stream_variants_roundtrip() {
        let table = PlotStyleTable {
            name: "civil.stb".into(),
            description: "Named Civil Pens".into(),
            kind: PlotStyleKind::Named,
            styles: vec![
                PlotStyle::object("Normal"),
                PlotStyle {
                    name: "Heavy".into(),
                    description: "Boundary line".into(),
                    color: Some(Rgb(18, 52, 86)),
                    grayscale: false,
                    screening: 60,
                    linetype: Some("DASHDOT".into()),
                    lineweight: Some(70),
                    end: EndStyle::Diamond,
                    join: JoinStyle::Round,
                    fill: FillStyle::Checkerboard,
                },
            ],
        }
        .sanitized();

        let bin = write_binary_stb(&table).unwrap();
        assert!(bin.starts_with(b"PIAFILEVERSION_2.0,STBVER1,compress\r\npmzlibcodec"));
        assert!(write_binary_ctb(&table).is_err());

        let from_bin = parse_pia_bytes(&bin, Some("civil.stb")).unwrap();
        assert_eq!(from_bin.kind, PlotStyleKind::Named);
        assert_eq!(from_bin.description, "Named Civil Pens");
        assert_eq!(from_bin.styles.len(), 2);
        assert_eq!(from_bin.styles[0].name, "Normal");
        assert_eq!(from_bin.styles[1].name, "Heavy");
        assert_eq!(from_bin.styles[1].color, Some(Rgb(18, 52, 86)));
        assert_eq!(from_bin.styles[1].screening, 60);
        assert_eq!(from_bin.styles[1].lineweight, Some(70));
        assert_eq!(from_bin.styles[1].linetype.as_deref(), Some("DASHDOT"));
        assert_eq!(from_bin.styles[1].end, EndStyle::Diamond);
        assert_eq!(from_bin.styles[1].join, JoinStyle::Round);
        assert_eq!(from_bin.styles[1].fill, FillStyle::Checkerboard);

        // Uncompressed PIA text format round-trip.
        let pia_text = write_pia_uncompressed(&table);
        assert!(pia_text.starts_with("PIAFILEVERSION_2.0,STBVER1,uncompress\r\n"));
        let from_text = PlotStyleTable::from_file_text_named(&pia_text, Some("civil.stb")).unwrap();
        assert_eq!(from_text, from_bin);

        // Raw zlib stream right after `pmzlibcodec` (without 12-byte Adler32/len header) also parses.
        let raw_zlib = compress_to_vec_zlib(pia_text.split_once("\r\n").unwrap().1.as_bytes(), 6);
        let mut raw_stream = b"PIAFILEVERSION_2.0,STBVER1,compress\r\npmzlibcodec".to_vec();
        raw_stream.extend_from_slice(&raw_zlib);
        let from_raw = parse_pia_bytes(&raw_stream, Some("civil.stb")).unwrap();
        assert_eq!(from_raw, from_bin);
    }
}
