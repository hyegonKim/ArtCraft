//! PDF `/Font` resource resolution, PostScript name normalization, `/Encoding` + `/Differences`,
//! `/ToUnicode` CMap (`bfchar` / `bfrange`), and glyph advance widths.

use std::collections::HashMap;
use std::sync::Arc;

use super::parser::{Cursor, PdfDict, PdfDoc, PdfVal, decode_utf16be_bytes};

/// Maximum entries expanded from a single `bfrange` operator.
const MAX_BFRANGE_SPAN: u32 = 65_536;

#[derive(Clone, Debug)]
pub(super) struct PdfFont {
    pub family: Option<Arc<str>>,
    pub bold: bool,
    pub italic: bool,
    pub mono: bool,
    pub is_type0: bool,
    code_lengths: Vec<u8>,
    cmap: HashMap<(u8, u32), String>,
    single_byte: [Option<CompactStr>; 256],
    widths: HashMap<u32, f64>,
    default_width: f64,
    width_scale: f64,
}

#[derive(Clone, Debug)]
enum CompactStr {
    Char(char),
    Str(Arc<str>),
}

impl CompactStr {
    fn push_to(&self, out: &mut String) {
        match self {
            CompactStr::Char(c) => out.push(*c),
            CompactStr::Str(s) => out.push_str(s),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BaseEnc {
    WinAnsi,
    MacRoman,
    Standard,
}

impl PdfFont {
    pub(super) fn fallback() -> Self {
        let mut single_byte: [Option<CompactStr>; 256] = std::array::from_fn(|_| None);
        fill_base_encoding(&mut single_byte, BaseEnc::WinAnsi);
        Self {
            family: None,
            bold: false,
            italic: false,
            mono: false,
            is_type0: false,
            code_lengths: vec![1],
            cmap: HashMap::new(),
            single_byte,
            widths: HashMap::new(),
            default_width: 0.52,
            width_scale: 0.001,
        }
    }

    pub(super) fn from_dict(doc: &PdfDoc<'_>, font_dict: &PdfDict) -> Self {
        let subtype = font_dict.get(b"Subtype".as_slice()).and_then(PdfVal::as_name).unwrap_or(b"");
        let is_type0 = subtype == b"Type0";

        let descendant = if is_type0 {
            doc.resolve_dict_key(font_dict, b"DescendantFonts")
                .and_then(|v| v.as_array().and_then(|a| a.first().cloned()))
                .and_then(|first| doc.resolve(&first, 0))
                .and_then(|v| v.as_dict().cloned())
        } else {
            None
        };

        let descriptor = descendant
            .as_ref()
            .and_then(|d| doc.resolve_dict_key(d, b"FontDescriptor"))
            .or_else(|| doc.resolve_dict_key(font_dict, b"FontDescriptor"))
            .and_then(|v| v.as_dict().cloned());

        let raw_name = font_dict
            .get(b"BaseFont".as_slice())
            .and_then(PdfVal::as_name)
            .or_else(|| descendant.as_ref()?.get(b"BaseFont".as_slice())?.as_name())
            .or_else(|| descriptor.as_ref()?.get(b"FontName".as_slice())?.as_name())
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .unwrap_or_default();

        let (mut family_str, mut bold, mut italic, mut mono) = normalize_postscript_font(&raw_name);

        if let Some(desc) = &descriptor {
            if let Some(flags) = desc.get(b"Flags".as_slice()).and_then(PdfVal::as_int) {
                if (flags & 1) != 0 {
                    mono = true;
                }
                if (flags & 64) != 0 {
                    italic = true;
                }
                if (flags & (1 << 18)) != 0 {
                    bold = true;
                }
            }
            if let Some(weight) = desc.get(b"FontWeight".as_slice()).and_then(PdfVal::as_f64)
                && weight >= 600.0
            {
                bold = true;
            }
            if let Some(angle) = desc.get(b"ItalicAngle".as_slice()).and_then(PdfVal::as_f64)
                && angle.abs() >= 4.0
            {
                italic = true;
            }
            if family_str.is_empty()
                && let Some(fam_bytes) = desc.get(b"FontFamily".as_slice()).and_then(PdfVal::as_bytes)
            {
                family_str = super::parser::decode_pdf_text_string(fam_bytes);
            }
        }

        let family = crate::model::kept_font(&family_str);

        // Encoding & Differences for 1-byte fonts.
        let mut single_byte: [Option<CompactStr>; 256] = std::array::from_fn(|_| None);
        let mut base_enc = BaseEnc::WinAnsi;
        if let Some(enc_val) = doc.resolve_dict_key(font_dict, b"Encoding") {
            match &enc_val {
                PdfVal::Name(n) => {
                    base_enc = match n.as_slice() {
                        b"MacRomanEncoding" => BaseEnc::MacRoman,
                        b"StandardEncoding" => BaseEnc::Standard,
                        _ => BaseEnc::WinAnsi,
                    };
                    fill_base_encoding(&mut single_byte, base_enc);
                }
                PdfVal::Dict(enc_dict) => {
                    if let Some(bn) = enc_dict.get(b"BaseEncoding".as_slice()).and_then(PdfVal::as_name) {
                        base_enc = match bn {
                            b"MacRomanEncoding" => BaseEnc::MacRoman,
                            b"StandardEncoding" => BaseEnc::Standard,
                            _ => BaseEnc::WinAnsi,
                        };
                    }
                    fill_base_encoding(&mut single_byte, base_enc);
                    if let Some(diff_val) = doc.resolve_dict_key(enc_dict, b"Differences")
                        && let Some(diffs) = diff_val.as_array()
                    {
                        apply_differences(&mut single_byte, diffs);
                    }
                }
                _ => fill_base_encoding(&mut single_byte, base_enc),
            }
        } else {
            fill_base_encoding(&mut single_byte, base_enc);
        }

        // `/ToUnicode` CMap stream.
        let mut code_lengths = Vec::new();
        let mut cmap = HashMap::new();
        if let Some(tu_ref) = font_dict.get(b"ToUnicode".as_slice())
            && let Ok(tu_bytes) = doc.decode_stream(tu_ref)
        {
            parse_tounicode_cmap(&tu_bytes, &mut code_lengths, &mut cmap);
        }
        if code_lengths.is_empty() {
            if is_type0 {
                code_lengths.push(2);
            } else {
                code_lengths.push(1);
            }
        }
        code_lengths.sort_unstable();
        code_lengths.dedup();

        // Widths table.
        let mut width_scale = 0.001;
        if subtype == b"Type3"
            && let Some(fm_val) = doc.resolve_dict_key(font_dict, b"FontMatrix")
            && let Some(fm) = fm_val.as_array()
            && let Some(sx) = fm.first().and_then(PdfVal::as_f64)
            && sx.is_finite()
            && sx.abs() > 1e-9
        {
            width_scale = sx.abs();
        }
        let mut widths = HashMap::new();
        let mut default_width = if is_type0 { 1.0 } else if mono { 0.60 } else { 0.52 };
        if let Some(desc_cid) = &descendant {
            if let Some(dw) = desc_cid.get(b"DW".as_slice()).and_then(PdfVal::as_f64)
                && dw >= 0.0
            {
                default_width = dw * 0.001;
            }
            if let Some(w_val) = doc.resolve_dict_key(desc_cid, b"W")
                && let Some(w_arr) = w_val.as_array()
            {
                parse_cid_widths(doc, w_arr, &mut widths);
            }
        } else {
            let first_char = font_dict.get(b"FirstChar".as_slice()).and_then(PdfVal::as_int).unwrap_or(0).max(0) as u32;
            if let Some(w_val) = doc.resolve_dict_key(font_dict, b"Widths")
                && let Some(w_arr) = w_val.as_array()
            {
                for (idx, item) in w_arr.iter().enumerate() {
                    if let Some(w) = doc.resolve(item, 0).and_then(|v| v.as_f64())
                        && w >= 0.0
                    {
                        widths.insert(first_char.saturating_add(idx as u32), w * width_scale);
                    }
                }
            }
            if let Some(desc) = &descriptor
                && let Some(mw) = desc.get(b"MissingWidth".as_slice()).and_then(PdfVal::as_f64)
                && mw > 0.0
            {
                default_width = mw * width_scale;
            }
        }

        Self {
            family,
            bold,
            italic,
            mono,
            is_type0,
            code_lengths,
            cmap,
            single_byte,
            widths,
            default_width,
            width_scale,
        }
    }

    /// Decode a raw PDF content string into `(decoded_text, total_advance_em, space_char_count)`.
    pub(super) fn decode_bytes(&self, bytes: &[u8]) -> (String, f64, usize) {
        let mut out = String::with_capacity(bytes.len());
        let mut adv_em = 0.0;
        let mut spaces = 0usize;
        let mut i = 0usize;

        while i < bytes.len() {
            let mut matched: Option<(usize, u32, Option<&String>)> = None;
            // Try declared code lengths (preferring exact ToUnicode hits first).
            for &clen in &self.code_lengths {
                let len = clen as usize;
                if let Some(slice) = bytes.get(i..i + len) {
                    let code = pack_code(slice);
                    if let Some(mapped) = self.cmap.get(&(clen, code)) {
                        matched = Some((len, code, Some(mapped)));
                        break;
                    }
                }
            }
            let (step, code, cmap_hit) = if let Some(m) = matched {
                m
            } else if self.is_type0 && i + 2 <= bytes.len() {
                let code = u32::from(u16::from_be_bytes([bytes[i], bytes[i + 1]]));
                (2, code, self.cmap.get(&(2, code)))
            } else {
                let code = u32::from(bytes[i]);
                (1, code, self.cmap.get(&(1, code)))
            };

            let before_len = out.len();
            if let Some(s) = cmap_hit {
                out.push_str(s);
            } else if step == 2 && self.cmap.is_empty() {
                // Identity UTF-16BE / UCS-2 fallback when Type0 lacks ToUnicode.
                if let Some(ch) = char::from_u32(code)
                    && !ch.is_control()
                {
                    out.push(ch);
                }
            } else if step == 1
                && let Some(Some(entry)) = self.single_byte.get(code as usize)
            {
                entry.push_to(&mut out);
            } else if let Some(ch) = char::from_u32(code)
                && !ch.is_control()
            {
                out.push(ch);
            }

            let added = &out[before_len..];
            if step == 1 && code == 0x20 || added == " " {
                spaces += 1;
            }
            let glyph_w = if let Some(&w) = self.widths.get(&code) {
                w
            } else {
                estimate_em_width(added, self.default_width, self.mono)
            };
            let _ = self.width_scale;
            adv_em += glyph_w;
            i += step;
        }

        (out, adv_em, spaces)
    }
}

fn pack_code(slice: &[u8]) -> u32 {
    let mut c = 0u32;
    for &b in slice.iter().take(4) {
        c = (c << 8) | u32::from(b);
    }
    c
}

fn estimate_em_width(text: &str, default_w: f64, mono: bool) -> f64 {
    if text.is_empty() {
        return default_w;
    }
    if mono {
        return 0.60 * (text.chars().count().max(1) as f64);
    }
    let mut sum = 0.0;
    for ch in text.chars() {
        sum += match ch {
            ' ' | '\u{00A0}' => 0.28,
            'i' | 'l' | 'I' | '!' | '|' | '.' | ',' | ':' | ';' | '\'' => 0.28,
            'f' | 'j' | 'r' | 't' | '(' | ')' | '[' | ']' | '-' => 0.35,
            'm' | 'w' | 'M' | 'W' => 0.82,
            c if c.is_ascii_uppercase() => 0.66,
            c if is_wide_cjk(c) => 1.0,
            _ => default_w.clamp(0.42, 0.65),
        };
    }
    sum
}

pub(super) fn is_wide_cjk(c: char) -> bool {
    matches!(
        c as u32,
        0x1100..=0x11FF
            | 0x2E80..=0x9FFF
            | 0xAC00..=0xD7AF
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF01..=0xFF60
            | 0x20000..=0x2FA1F
    )
}

fn parse_cid_widths(doc: &PdfDoc<'_>, arr: &[PdfVal], out: &mut HashMap<u32, f64>) {
    let mut i = 0usize;
    while i < arr.len() && out.len() < 100_000 {
        let Some(c_first) = arr.get(i).and_then(|v| doc.resolve(v, 0)).and_then(|v| v.as_int()) else {
            i += 1;
            continue;
        };
        let Some(next) = arr.get(i + 1).and_then(|v| doc.resolve(v, 0)) else { break };
        if let Some(list) = next.as_array() {
            for (idx, wv) in list.iter().enumerate() {
                if let Some(w) = doc.resolve(wv, 0).and_then(|v| v.as_f64())
                    && c_first >= 0
                    && w >= 0.0
                {
                    out.insert((c_first as u32).saturating_add(idx as u32), w * 0.001);
                }
            }
            i += 2;
        } else if let Some(c_last) = next.as_int() {
            if let Some(w) = arr.get(i + 2).and_then(|v| doc.resolve(v, 0)).and_then(|v| v.as_f64())
                && c_first >= 0
                && c_last >= c_first
                && w >= 0.0
            {
                let span = ((c_last - c_first) as u32).min(MAX_BFRANGE_SPAN);
                for off in 0..=span {
                    out.insert((c_first as u32).saturating_add(off), w * 0.001);
                }
            }
            i += 3;
        } else {
            i += 1;
        }
    }
}

/// Parse a `/ToUnicode` CMap stream (`begincodespacerange`, `beginbfchar`, `beginbfrange`).
pub(super) fn parse_tounicode_cmap(
    bytes: &[u8],
    code_lengths: &mut Vec<u8>,
    cmap: &mut HashMap<(u8, u32), String>,
) {
    let mut cur = Cursor::new(bytes, 0);
    while cur.pos < bytes.len() {
        cur.skip_ws_and_comments();
        let Some(b) = cur.peek() else { break };
        if b == b'<' || b == b'(' || b == b'[' || b == b'/' {
            let _ = cur.parse_val(0);
            continue;
        }
        let tok = cur.read_regular_token();
        if tok.is_empty() {
            cur.pos += 1;
            continue;
        }
        match tok.as_slice() {
            b"begincodespacerange" => {
                loop {
                    cur.skip_ws_and_comments();
                    if cur.peek().is_none() || cur.eat_keyword(b"endcodespacerange") {
                        break;
                    }
                    if cur.peek() == Some(b'<') && cur.peek_at(1) != Some(b'<') {
                        let lo = cur.read_hex_string();
                        cur.skip_ws_and_comments();
                        if cur.peek() == Some(b'<') {
                            let _hi = cur.read_hex_string();
                        }
                        if (1..=4).contains(&lo.len()) {
                            code_lengths.push(lo.len() as u8);
                        }
                    } else {
                        let _ = cur.read_regular_token();
                    }
                }
            }
            b"beginbfchar" => {
                loop {
                    cur.skip_ws_and_comments();
                    if cur.peek().is_none() || cur.eat_keyword(b"endbfchar") {
                        break;
                    }
                    if cur.peek() != Some(b'<') {
                        let _ = cur.read_regular_token();
                        continue;
                    }
                    let src = cur.read_hex_string();
                    cur.skip_ws_and_comments();
                    let dst_str = match cur.peek() {
                        Some(b'<') => decode_cmap_target(&cur.read_hex_string()),
                        Some(b'(') => decode_cmap_target(&cur.read_literal_string()),
                        Some(b'/') => {
                            let name = cur.read_name();
                            glyph_name_to_string(&name).unwrap_or_default()
                        }
                        _ => String::new(),
                    };
                    if (1..=4).contains(&src.len()) && !dst_str.is_empty() && cmap.len() < 200_000 {
                        let clen = src.len() as u8;
                        if !code_lengths.contains(&clen) {
                            code_lengths.push(clen);
                        }
                        cmap.insert((clen, pack_code(&src)), dst_str);
                    }
                }
            }
            b"beginbfrange" => {
                loop {
                    cur.skip_ws_and_comments();
                    if cur.peek().is_none() || cur.eat_keyword(b"endbfrange") {
                        break;
                    }
                    if cur.peek() != Some(b'<') {
                        let _ = cur.read_regular_token();
                        continue;
                    }
                    let lo_bytes = cur.read_hex_string();
                    cur.skip_ws_and_comments();
                    if cur.peek() != Some(b'<') {
                        continue;
                    }
                    let hi_bytes = cur.read_hex_string();
                    if lo_bytes.len() != hi_bytes.len() || !(1..=4).contains(&lo_bytes.len()) {
                        continue;
                    }
                    let clen = lo_bytes.len() as u8;
                    if !code_lengths.contains(&clen) {
                        code_lengths.push(clen);
                    }
                    let lo = pack_code(&lo_bytes);
                    let hi = pack_code(&hi_bytes);
                    if hi < lo {
                        continue;
                    }
                    let span = (hi - lo).min(MAX_BFRANGE_SPAN);
                    cur.skip_ws_and_comments();
                    match cur.peek() {
                        Some(b'[') => {
                            cur.pos += 1;
                            let mut offset = 0u32;
                            loop {
                                cur.skip_ws_and_comments();
                                match cur.peek() {
                                    None => break,
                                    Some(b']') => {
                                        cur.pos += 1;
                                        break;
                                    }
                                    Some(b'<') => {
                                        let target = decode_cmap_target(&cur.read_hex_string());
                                        if offset <= span && !target.is_empty() && cmap.len() < 200_000 {
                                            cmap.insert((clen, lo + offset), target);
                                        }
                                        offset += 1;
                                    }
                                    Some(b'/') => {
                                        let name = cur.read_name();
                                        if let Some(target) = glyph_name_to_string(&name)
                                            && offset <= span
                                            && cmap.len() < 200_000
                                        {
                                            cmap.insert((clen, lo + offset), target);
                                        }
                                        offset += 1;
                                    }
                                    _ => {
                                        let _ = cur.parse_val(0);
                                        offset += 1;
                                    }
                                }
                            }
                        }
                        Some(b'<') => {
                            let base_dst = cur.read_hex_string();
                            for offset in 0..=span {
                                if cmap.len() >= 200_000 {
                                    break;
                                }
                                if let Some(s) = increment_cmap_target(&base_dst, offset)
                                    && !s.is_empty()
                                {
                                    cmap.insert((clen, lo + offset), s);
                                }
                            }
                        }
                        _ => {
                            let _ = cur.parse_val(0);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn decode_cmap_target(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    if bytes.len() == 1 {
        return char::from_u32(u32::from(bytes[0])).into_iter().collect();
    }
    if bytes.len().is_multiple_of(2) {
        return decode_utf16be_bytes(bytes);
    }
    String::from_utf8_lossy(bytes).into_owned()
}

fn increment_cmap_target(base: &[u8], offset: u32) -> Option<String> {
    if base.is_empty() {
        return None;
    }
    if base.len() == 1 {
        let cp = u32::from(base[0]).checked_add(offset)?;
        return char::from_u32(cp).map(|c| c.to_string());
    }
    if base.len() == 2 {
        let cp = u32::from(u16::from_be_bytes([base[0], base[1]])).checked_add(offset)?;
        return char::from_u32(cp).map(|c| c.to_string());
    }
    if base.len() == 4 {
        let w0 = u16::from_be_bytes([base[0], base[1]]);
        let w1 = u16::from_be_bytes([base[2], base[3]]);
        if (0xD800..=0xDBFF).contains(&w0) && (0xDC00..=0xDFFF).contains(&w1) {
            let scalar = 0x10000 + ((u32::from(w0 - 0xD800)) << 10) + u32::from(w1 - 0xDC00);
            return char::from_u32(scalar.checked_add(offset)?).map(|c| c.to_string());
        }
    }
    if base.len().is_multiple_of(2) {
        let mut copy = base.to_vec();
        let n = copy.len();
        let last = u32::from(u16::from_be_bytes([copy[n - 2], copy[n - 1]])).checked_add(offset)?;
        let last_u16 = u16::try_from(last).ok()?;
        let be = last_u16.to_be_bytes();
        copy[n - 2] = be[0];
        copy[n - 1] = be[1];
        return Some(decode_utf16be_bytes(&copy));
    }
    None
}

/// Strip subset tags (`ABCDEF+`), extract `(family, bold, italic, mono)` from PDF `/BaseFont`.
pub(super) fn normalize_postscript_font(raw: &str) -> (String, bool, bool, bool) {
    let trimmed = raw.trim().trim_start_matches('/');
    if trimmed.is_empty() {
        return (String::new(), false, false, false);
    }
    let without_subset = if trimmed.len() > 7
        && trimmed.as_bytes().get(6) == Some(&b'+')
        && trimmed[..6].bytes().all(|b| b.is_ascii_uppercase())
    {
        &trimmed[7..]
    } else {
        trimmed
    };

    let lower = without_subset.to_ascii_lowercase();
    let bold = lower.contains("bold")
        || lower.contains("semibold")
        || lower.contains("demibold")
        || lower.contains("extrabold")
        || lower.contains("ultrabold")
        || lower.contains("black")
        || lower.contains("heavy")
        || lower.ends_with("-bd")
        || lower.ends_with(",bd")
        || lower.contains("-bdmt");
    let italic = lower.contains("italic")
        || lower.contains("oblique")
        || lower.contains("inclined")
        || lower.contains("slanted")
        || lower.ends_with("-it")
        || lower.ends_with(",it")
        || lower.contains("-itmt")
        || lower.contains("boldit");

    // Extract base family part before `-` or `,` style suffixes.
    let base_part = without_subset
        .split([',', ';'])
        .next()
        .unwrap_or(without_subset);
    let family_stem = strip_style_suffixes(base_part);
    let stem_key: String = family_stem.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_lowercase()).collect();

    let known = match stem_key.as_str() {
        "times" | "timesroman" | "timesnewroman" | "timesnewromanps" | "timesnewromanpsmt" | "nimbusromno9l" => {
            Some(("Times New Roman", false))
        }
        "helvetica" | "helv" | "nimbussanl" => Some(("Helvetica", false)),
        "helveticaneue" => Some(("Helvetica Neue", false)),
        "arial" | "arialmt" | "arialunicodems" => Some(("Arial", false)),
        "courier" | "couriernew" | "couriernewps" | "couriernewpsmt" | "nimbusmonl" => {
            Some(("Courier New", true))
        }
        "calibri" => Some(("Calibri", false)),
        "cambria" => Some(("Cambria", false)),
        "candara" => Some(("Candara", false)),
        "consolas" => Some(("Consolas", true)),
        "georgia" => Some(("Georgia", false)),
        "verdana" => Some(("Verdana", false)),
        "tahoma" => Some(("Tahoma", false)),
        "trebuchetms" => Some(("Trebuchet MS", false)),
        "segoeui" => Some(("Segoe UI", false)),
        "garamond" => Some(("Garamond", false)),
        "palatino" | "palatinolinotype" => Some(("Palatino Linotype", false)),
        "centurygothic" => Some(("Century Gothic", false)),
        "aptos" => Some(("Aptos", false)),
        "carlito" => Some(("Carlito", false)),
        "caladea" => Some(("Caladea", false)),
        "liberationsans" => Some(("Liberation Sans", false)),
        "liberationserif" => Some(("Liberation Serif", false)),
        "liberationmono" => Some(("Liberation Mono", true)),
        "dejavusans" => Some(("DejaVu Sans", false)),
        "dejavuserif" => Some(("DejaVu Serif", false)),
        "dejavusansmono" => Some(("DejaVu Sans Mono", true)),
        "notosans" => Some(("Noto Sans", false)),
        "notoserif" => Some(("Noto Serif", false)),
        "notosanskr" | "notosanscjkkr" => Some(("Noto Sans KR", false)),
        "notoserifkr" | "notoserifcjkkr" => Some(("Noto Serif KR", false)),
        "malgungothic" | "malgungothicsemilight" => Some(("Malgun Gothic", false)),
        "nanumgothic" => Some(("NanumGothic", false)),
        "nanummyeongjo" => Some(("NanumMyeongjo", false)),
        "batang" => Some(("Batang", false)),
        "batangche" => Some(("BatangChe", true)),
        "dotum" => Some(("Dotum", false)),
        "dotumche" => Some(("DotumChe", true)),
        "gulim" => Some(("Gulim", false)),
        "gulimche" => Some(("GulimChe", true)),
        "gungsuh" => Some(("Gungsuh", false)),
        _ => None,
    };

    if let Some((clean, is_mono)) = known {
        return (clean.to_string(), bold, italic, is_mono);
    }

    let mono = lower.contains("mono")
        || lower.contains("courier")
        || lower.contains("console")
        || lower.contains("typewriter")
        || lower.contains("fixed")
        || lower.contains("firacode")
        || lower.contains("menlo")
        || lower.contains("monaco")
        || lower.contains("inconsolata");

    let cleaned = split_camel_or_ps_family(&family_stem);
    (cleaned, bold, italic, mono)
}

fn strip_style_suffixes(name: &str) -> String {
    let suffixes = [
        "PS-BoldItalicMT",
        "PS-BoldMT",
        "PS-ItalicMT",
        "PSMT",
        "-BoldOblique",
        "-BoldItalicMT",
        "-BoldItalic",
        "-BoldMT",
        "-Bold",
        "-Oblique",
        "-ItalicMT",
        "-Italic",
        "-Roman",
        "-Regular",
        "-Medium",
        "-Light",
        "-Thin",
        "-Semibold",
        "-SemiBold",
        "-DemiBold",
        "-Demi",
        "-ExtraBold",
        "-Black",
        "-Heavy",
        "-Book",
        "-BdIt",
        "-Bd",
        "-It",
        "BoldOblique",
        "BoldItalic",
        "Semibold",
        "SemiBold",
        "Regular",
        "Oblique",
        "Italic",
        "Bold",
        "MT",
    ];
    let mut cur = name.trim();
    for _ in 0..3 {
        let mut changed = false;
        for suf in suffixes {
            if cur.len() > suf.len() && cur.ends_with(suf) {
                let candidate = cur[..cur.len() - suf.len()].trim_end_matches(['-', '_']);
                if !candidate.is_empty() {
                    cur = candidate;
                    changed = true;
                    break;
                }
            }
        }
        if !changed {
            break;
        }
    }
    cur.replace('_', " ")
}

fn split_camel_or_ps_family(s: &str) -> String {
    let s = s.trim();
    if s.contains(' ') || s.contains('-') {
        return s.to_string();
    }
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len() + 4);
    for (idx, &ch) in chars.iter().enumerate() {
        if idx > 0 && ch.is_ascii_uppercase() {
            let prev = chars[idx - 1];
            let next_lower = chars.get(idx + 1).is_some_and(|n| n.is_ascii_lowercase());
            if prev.is_ascii_lowercase() || (prev.is_ascii_uppercase() && next_lower) {
                out.push(' ');
            }
        }
        out.push(ch);
    }
    out
}

fn fill_base_encoding(table: &mut [Option<CompactStr>; 256], enc: BaseEnc) {
    for code in 0x20u8..=0x7Eu8 {
        let ch = if enc == BaseEnc::Standard {
            match code {
                0x27 => '\u{2019}',
                0x60 => '\u{2018}',
                _ => char::from(code),
            }
        } else {
            char::from(code)
        };
        table[code as usize] = Some(CompactStr::Char(ch));
    }
    match enc {
        BaseEnc::WinAnsi => {
            const CP1252_80_9F: [char; 32] = [
                '\u{20AC}', '\u{FFFD}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
                '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{FFFD}', '\u{017D}', '\u{FFFD}',
                '\u{FFFD}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}',
                '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\u{FFFD}', '\u{017E}', '\u{0178}',
            ];
            for (i, &ch) in CP1252_80_9F.iter().enumerate() {
                if ch != '\u{FFFD}' {
                    table[0x80 + i] = Some(CompactStr::Char(ch));
                }
            }
            for code in 0xA0u8..=0xFFu8 {
                table[code as usize] = Some(CompactStr::Char(char::from(code)));
            }
        }
        BaseEnc::MacRoman => {
            const MAC_ROMAN_80_FF: [char; 128] = [
                '\u{00C4}', '\u{00C5}', '\u{00C7}', '\u{00C9}', '\u{00D1}', '\u{00D6}', '\u{00DC}', '\u{00E1}',
                '\u{00E0}', '\u{00E2}', '\u{00E4}', '\u{00E3}', '\u{00E5}', '\u{00E7}', '\u{00E9}', '\u{00E8}',
                '\u{00EA}', '\u{00EB}', '\u{00ED}', '\u{00EC}', '\u{00EE}', '\u{00EF}', '\u{00F1}', '\u{00F3}',
                '\u{00F2}', '\u{00F4}', '\u{00F6}', '\u{00F5}', '\u{00FA}', '\u{00F9}', '\u{00FB}', '\u{00FC}',
                '\u{2020}', '\u{00B0}', '\u{00A2}', '\u{00A3}', '\u{00A7}', '\u{2022}', '\u{00B6}', '\u{00DF}',
                '\u{00AE}', '\u{00A9}', '\u{2122}', '\u{00B4}', '\u{00A8}', '\u{2260}', '\u{00C6}', '\u{00D8}',
                '\u{221E}', '\u{00B1}', '\u{2264}', '\u{2265}', '\u{00A5}', '\u{00B5}', '\u{2202}', '\u{2211}',
                '\u{220F}', '\u{03C0}', '\u{222B}', '\u{00AA}', '\u{00BA}', '\u{03A9}', '\u{00E6}', '\u{00F8}',
                '\u{00BF}', '\u{00A1}', '\u{00AC}', '\u{221A}', '\u{0192}', '\u{2248}', '\u{2206}', '\u{00AB}',
                '\u{00BB}', '\u{2026}', '\u{00A0}', '\u{00C0}', '\u{00C3}', '\u{00D5}', '\u{0152}', '\u{0153}',
                '\u{2013}', '\u{2014}', '\u{201C}', '\u{201D}', '\u{2018}', '\u{2019}', '\u{00F7}', '\u{25CA}',
                '\u{00FF}', '\u{0178}', '\u{2044}', '\u{20AC}', '\u{2039}', '\u{203A}', '\u{FB01}', '\u{FB02}',
                '\u{2021}', '\u{00B7}', '\u{201A}', '\u{201E}', '\u{2030}', '\u{00C2}', '\u{00CA}', '\u{00C1}',
                '\u{00CB}', '\u{00C8}', '\u{00CD}', '\u{00CE}', '\u{00CF}', '\u{00CC}', '\u{00D3}', '\u{00D4}',
                '\u{F8FF}', '\u{00D2}', '\u{00DA}', '\u{00DB}', '\u{00D9}', '\u{0131}', '\u{02C6}', '\u{02DC}',
                '\u{00AF}', '\u{02D8}', '\u{02D9}', '\u{02DA}', '\u{00B8}', '\u{02DD}', '\u{02DB}', '\u{02C7}',
            ];
            for (i, &ch) in MAC_ROMAN_80_FF.iter().enumerate() {
                table[0x80 + i] = Some(CompactStr::Char(ch));
            }
            table[0xDE] = Some(CompactStr::Str(Arc::from("fi")));
            table[0xDF] = Some(CompactStr::Str(Arc::from("fl")));
        }
        BaseEnc::Standard => {
            for code in 0xA0u8..=0xFFu8 {
                table[code as usize] = Some(CompactStr::Char(char::from(code)));
            }
            table[0xA9] = Some(CompactStr::Char('\u{2018}'));
            table[0xAA] = Some(CompactStr::Char('\u{201C}'));
            table[0xAE] = Some(CompactStr::Str(Arc::from("fi")));
            table[0xAF] = Some(CompactStr::Str(Arc::from("fl")));
            table[0xB1] = Some(CompactStr::Char('\u{2013}'));
            table[0xB7] = Some(CompactStr::Char('\u{2022}'));
            table[0xBA] = Some(CompactStr::Char('\u{201D}'));
            table[0xBC] = Some(CompactStr::Char('\u{2026}'));
            table[0xD0] = Some(CompactStr::Char('\u{2014}'));
        }
    }
}

fn apply_differences(table: &mut [Option<CompactStr>; 256], diffs: &[PdfVal]) {
    let mut cur_code: Option<u32> = None;
    for item in diffs {
        match item {
            PdfVal::Int(n) if (0..=255).contains(n) => {
                cur_code = Some(*n as u32);
            }
            PdfVal::Name(name) => {
                if let Some(code) = cur_code
                    && code <= 255
                {
                    if let Some(mapped) = glyph_name_to_string(name) {
                        let mut chars = mapped.chars();
                        let first = chars.next();
                        let entry = match (first, chars.next()) {
                            (Some(ch), None) => Some(CompactStr::Char(ch)),
                            (Some(_), Some(_)) => Some(CompactStr::Str(Arc::from(mapped.as_str()))),
                            _ => None,
                        };
                        table[code as usize] = entry;
                    }
                    cur_code = Some(code + 1);
                }
            }
            _ => {}
        }
    }
}

fn glyph_name_to_string(raw_name: &[u8]) -> Option<String> {
    let s = std::str::from_utf8(raw_name).ok()?;
    let base = s.split('.').next().unwrap_or(s);
    if base.is_empty() || base == ".notdef" {
        return None;
    }
    if base.len() == 1 {
        return Some(base.to_string());
    }
    if let Some(hex) = base.strip_prefix("uni")
        && hex.len() >= 4
        && hex.len().is_multiple_of(4)
        && hex.bytes().all(|b| b.is_ascii_hexdigit())
    {
        let mut u16s = Vec::with_capacity(hex.len() / 4);
        for k in (0..hex.len()).step_by(4) {
            if let Ok(v) = u16::from_str_radix(&hex[k..k + 4], 16) {
                u16s.push(v);
            }
        }
        return Some(char::decode_utf16(u16s).map(|r| r.unwrap_or('\u{FFFD}')).collect());
    }
    if let Some(hex) = base.strip_prefix('u')
        && (4..=6).contains(&hex.len())
        && hex.bytes().all(|b| b.is_ascii_hexdigit())
        && let Ok(cp) = u32::from_str_radix(hex, 16)
        && let Some(ch) = char::from_u32(cp)
    {
        return Some(ch.to_string());
    }
    let mapped = match base {
        "space" | "nbspace" => " ",
        "exclam" => "!",
        "quotedbl" => "\"",
        "numbersign" => "#",
        "dollar" => "$",
        "percent" => "%",
        "ampersand" => "&",
        "quotesingle" => "'",
        "parenleft" => "(",
        "parenright" => ")",
        "asterisk" => "*",
        "plus" => "+",
        "comma" => ",",
        "hyphen" | "sfthyphen" => "-",
        "minus" => "\u{2212}",
        "period" => ".",
        "slash" => "/",
        "zero" => "0",
        "one" => "1",
        "two" => "2",
        "three" => "3",
        "four" => "4",
        "five" => "5",
        "six" => "6",
        "seven" => "7",
        "eight" => "8",
        "nine" => "9",
        "colon" => ":",
        "semicolon" => ";",
        "less" => "<",
        "equal" => "=",
        "greater" => ">",
        "question" => "?",
        "at" => "@",
        "bracketleft" => "[",
        "backslash" => "\\",
        "bracketright" => "]",
        "asciicircum" => "^",
        "underscore" => "_",
        "grave" => "`",
        "braceleft" => "{",
        "bar" => "|",
        "braceright" => "}",
        "asciitilde" => "~",
        "endash" => "\u{2013}",
        "emdash" => "\u{2014}",
        "quoteleft" => "\u{2018}",
        "quoteright" => "\u{2019}",
        "quotesinglbase" => "\u{201A}",
        "quotedblleft" => "\u{201C}",
        "quotedblright" => "\u{201D}",
        "quotedblbase" => "\u{201E}",
        "bullet" => "\u{2022}",
        "periodcentered" | "middot" => "\u{00B7}",
        "ellipsis" => "\u{2026}",
        "fi" => "fi",
        "fl" => "fl",
        "ff" => "ff",
        "ffi" => "ffi",
        "ffl" => "ffl",
        "euro" | "Euro" => "\u{20AC}",
        "copyright" => "\u{00A9}",
        "registered" => "\u{00AE}",
        "trademark" => "\u{2122}",
        "degree" => "\u{00B0}",
        "plusminus" => "\u{00B1}",
        "multiply" => "\u{00D7}",
        "divide" => "\u{00F7}",
        "section" => "\u{00A7}",
        "paragraph" => "\u{00B6}",
        "dagger" => "\u{2020}",
        "daggerdbl" => "\u{2021}",
        "guillemotleft" => "\u{00AB}",
        "guillemotright" => "\u{00BB}",
        "agrave" => "\u{00E0}",
        "aacute" => "\u{00E1}",
        "acircumflex" => "\u{00E2}",
        "atilde" => "\u{00E3}",
        "adieresis" => "\u{00E4}",
        "aring" => "\u{00E5}",
        "ae" => "\u{00E6}",
        "ccedilla" => "\u{00E7}",
        "egrave" => "\u{00E8}",
        "eacute" => "\u{00E9}",
        "ecircumflex" => "\u{00EA}",
        "edieresis" => "\u{00EB}",
        "igrave" => "\u{00EC}",
        "iacute" => "\u{00ED}",
        "icircumflex" => "\u{00EE}",
        "idieresis" => "\u{00EF}",
        "ntilde" => "\u{00F1}",
        "ograve" => "\u{00F2}",
        "oacute" => "\u{00F3}",
        "ocircumflex" => "\u{00F4}",
        "otilde" => "\u{00F5}",
        "odieresis" => "\u{00F6}",
        "oslash" => "\u{00F8}",
        "ugrave" => "\u{00F9}",
        "uacute" => "\u{00FA}",
        "ucircumflex" => "\u{00FB}",
        "udieresis" => "\u{00FC}",
        "yacute" => "\u{00FD}",
        "ydieresis" => "\u{00FF}",
        "germandbls" => "\u{00DF}",
        "Agrave" => "\u{00C0}",
        "Aacute" => "\u{00C1}",
        "Acircumflex" => "\u{00C2}",
        "Atilde" => "\u{00C3}",
        "Adieresis" => "\u{00C4}",
        "Aring" => "\u{00C5}",
        "AE" => "\u{00C6}",
        "Ccedilla" => "\u{00C7}",
        "Egrave" => "\u{00C8}",
        "Eacute" => "\u{00C9}",
        "Ecircumflex" => "\u{00CA}",
        "Edieresis" => "\u{00CB}",
        "Igrave" => "\u{00CC}",
        "Iacute" => "\u{00CD}",
        "Icircumflex" => "\u{00CE}",
        "Idieresis" => "\u{00CF}",
        "Ntilde" => "\u{00D1}",
        "Ograve" => "\u{00D2}",
        "Oacute" => "\u{00D3}",
        "Ocircumflex" => "\u{00D4}",
        "Otilde" => "\u{00D5}",
        "Odieresis" => "\u{00D6}",
        "Oslash" => "\u{00D8}",
        "Ugrave" => "\u{00D9}",
        "Uacute" => "\u{00DA}",
        "Ucircumflex" => "\u{00DB}",
        "Udieresis" => "\u{00DC}",
        "OE" => "\u{0152}",
        "oe" => "\u{0153}",
        _ => return None,
    };
    Some(mapped.to_string())
}
