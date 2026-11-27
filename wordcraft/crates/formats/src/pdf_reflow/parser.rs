//! PDF 1.x / 1.5+ container, cross-reference, stream filter, page tree, and metadata parser.

use std::collections::{HashMap, HashSet};

use crate::model::Meta;
use wordcraft_doc::SectionProps;

/// Maximum decompressed bytes for a single stream (64 MiB).
const MAX_STREAM_BYTES: usize = 64 * 1024 * 1024;
/// Maximum total decompressed stream bytes per document (256 MiB).
const MAX_TOTAL_STREAM_BYTES: usize = 256 * 1024 * 1024;
/// Maximum indirect objects indexed.
const MAX_OBJECTS: usize = 200_000;
/// Maximum pages walked in `/Pages`.
const MAX_PAGES: usize = 5_000;
/// Maximum object / syntax recursion depth.
const MAX_PARSE_DEPTH: usize = 32;
/// Maximum `/Prev` trailer chain hops.
const MAX_XREF_CHAIN: usize = 64;

pub(super) type ObjId = (u32, u16);
pub(super) type PdfDict = HashMap<Vec<u8>, PdfVal>;

#[derive(Clone, Debug)]
pub(super) enum PdfVal {
    Null,
    Bool,
    Int(i64),
    Real(f64),
    Name(Vec<u8>),
    Str(Vec<u8>),
    Array(Vec<PdfVal>),
    Dict(PdfDict),
    Stream { dict: PdfDict, raw: Vec<u8> },
    Ref(ObjId),
}

impl PdfVal {
    pub(super) fn as_int(&self) -> Option<i64> {
        match self {
            PdfVal::Int(n) => Some(*n),
            PdfVal::Real(r) if r.is_finite() => Some(*r as i64),
            _ => None,
        }
    }

    pub(super) fn as_f64(&self) -> Option<f64> {
        match self {
            PdfVal::Int(n) => Some(*n as f64),
            PdfVal::Real(r) if r.is_finite() => Some(*r),
            _ => None,
        }
    }

    pub(super) fn as_name(&self) -> Option<&[u8]> {
        match self {
            PdfVal::Name(n) => Some(n.as_slice()),
            _ => None,
        }
    }

    pub(super) fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            PdfVal::Str(b) => Some(b.as_slice()),
            _ => None,
        }
    }

    pub(super) fn as_array(&self) -> Option<&[PdfVal]> {
        match self {
            PdfVal::Array(a) => Some(a.as_slice()),
            _ => None,
        }
    }

    pub(super) fn as_dict(&self) -> Option<&PdfDict> {
        match self {
            PdfVal::Dict(d) | PdfVal::Stream { dict: d, .. } => Some(d),
            _ => None,
        }
    }

    pub(super) fn as_ref(&self) -> Option<ObjId> {
        match self {
            PdfVal::Ref(id) => Some(*id),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum XrefEntry {
    Offset(usize),
    Compressed { obj_stm: u32, index: usize },
}

#[derive(Clone, Debug)]
pub(super) struct LinkAnnot {
    pub rect: [f64; 4],
    pub uri: String,
}

#[derive(Clone, Debug)]
pub(super) struct PageInfo {
    pub media_box: [f64; 4],
    pub rotate: i32,
    pub resources: PdfDict,
    pub contents: Option<PdfVal>,
    pub links: Vec<LinkAnnot>,
}

impl PageInfo {
    pub(super) fn width(&self) -> f64 {
        let w = (self.media_box[2] - self.media_box[0]).abs();
        let h = (self.media_box[3] - self.media_box[1]).abs();
        let raw = if matches!(self.rotate.rem_euclid(360), 90 | 270) { h } else { w };
        if raw.is_finite() && raw >= 36.0 && raw <= 14_400.0 { raw } else { 612.0 }
    }

    pub(super) fn height(&self) -> f64 {
        let w = (self.media_box[2] - self.media_box[0]).abs();
        let h = (self.media_box[3] - self.media_box[1]).abs();
        let raw = if matches!(self.rotate.rem_euclid(360), 90 | 270) { w } else { h };
        if raw.is_finite() && raw >= 36.0 && raw <= 14_400.0 { raw } else { 792.0 }
    }

    pub(super) fn section_props(&self) -> Option<SectionProps> {
        let w = self.width() as f32;
        let h = self.height() as f32;
        if w >= 72.0 && h >= 72.0 && w <= 3600.0 && h <= 3600.0 { Some(SectionProps { page_w: w, page_h: h, ..Default::default() }) } else { None }
    }
}

pub(super) struct PdfDoc<'a> {
    bytes: &'a [u8],
    header_offset: usize,
    xref: HashMap<u32, XrefEntry>,
    trailer: PdfDict,
    cache: std::cell::RefCell<HashMap<u32, PdfVal>>,
    obj_stm_cache: std::cell::RefCell<HashMap<u32, HashMap<u32, PdfVal>>>,
    inflated_total: std::cell::Cell<usize>,
}

/// Find `%PDF-` within the first 1024 bytes.
pub(super) fn find_pdf_header(bytes: &[u8]) -> Option<usize> {
    let head = bytes.get(..bytes.len().min(1024)).unwrap_or(bytes);
    head.windows(5).position(|w| w == b"%PDF-")
}

impl<'a> PdfDoc<'a> {
    pub(super) fn parse(bytes: &'a [u8]) -> Result<Self, String> {
        let header_offset = find_pdf_header(bytes).ok_or_else(|| "not a PDF file (missing `%PDF-` header)".to_string())?;
        let mut doc = Self {
            bytes,
            header_offset,
            xref: HashMap::new(),
            trailer: HashMap::new(),
            cache: std::cell::RefCell::new(HashMap::new()),
            obj_stm_cache: std::cell::RefCell::new(HashMap::new()),
            inflated_total: std::cell::Cell::new(0),
        };
        doc.load_xref_and_trailer();
        if doc.trailer.contains_key(b"Encrypt".as_slice()) {
            return Err("PDF is password-protected (encrypted PDFs cannot be imported)".into());
        }
        if !doc.trailer.contains_key(b"Root".as_slice()) {
            doc.scan_objects_linear();
        } else if doc.resolve_dict_key(&doc.trailer.clone(), b"Root").is_none() {
            // Re-index by scanning if xref offsets were shifted or damaged.
            doc.scan_objects_linear();
        }
        if doc.trailer.contains_key(b"Encrypt".as_slice()) {
            return Err("PDF is password-protected (encrypted PDFs cannot be imported)".into());
        }
        if !doc.trailer.contains_key(b"Root".as_slice()) {
            return Err("invalid or truncated PDF (missing `/Root` catalog)".into());
        }
        Ok(doc)
    }

    fn load_xref_and_trailer(&mut self) {
        let Some(first_xref) = find_startxref(self.bytes) else {
            self.scan_objects_linear();
            return;
        };
        let mut queue = vec![first_xref];
        let mut visited = HashSet::new();
        let mut steps = 0usize;

        while let Some(rel_pos) = queue.pop() {
            if steps >= MAX_XREF_CHAIN {
                break;
            }
            steps += 1;
            let pos = self.adjust_offset(rel_pos);
            if !visited.insert(pos) || pos >= self.bytes.len() {
                continue;
            }
            let mut cur = Cursor::new(self.bytes, pos);
            cur.skip_ws_and_comments();
            if cur.eat_keyword(b"xref") {
                if let Some(tr) = parse_classic_xref(&mut cur, &mut self.xref) {
                    if let Some(prev) = tr.get(b"Prev".as_slice()).and_then(PdfVal::as_int).filter(|&p| p >= 0) {
                        queue.push(prev as usize);
                    }
                    if let Some(xref_stm) = tr.get(b"XRefStm".as_slice()).and_then(PdfVal::as_int).filter(|&p| p >= 0) {
                        queue.push(xref_stm as usize);
                    }
                    merge_dict_newer_wins(&mut self.trailer, tr);
                }
            } else if let Some((_id, val)) = cur.parse_indirect_obj(0)
                && let PdfVal::Stream { dict, raw } = val
            {
                if let Some(prev) = dict.get(b"Prev".as_slice()).and_then(PdfVal::as_int).filter(|&p| p >= 0) {
                    queue.push(prev as usize);
                }
                if let Ok(decoded) = self.decode_stream_bytes(&dict, &raw) {
                    parse_xref_stream(&dict, &decoded, &mut self.xref);
                }
                merge_dict_newer_wins(&mut self.trailer, dict);
            }
        }
    }

    fn adjust_offset(&self, off: usize) -> usize {
        if self.header_offset > 0
            && let Some(shifted) = off.checked_add(self.header_offset)
            && shifted < self.bytes.len()
        {
            // Check whether `off` already points at `xref` or an `obj` header; otherwise use shifted.
            let direct_ok = looks_like_xref_or_obj(self.bytes, off);
            if !direct_ok && looks_like_xref_or_obj(self.bytes, shifted) {
                return shifted;
            }
        }
        off
    }

    /// Fallback linear scan for `N G obj` headers and `trailer << ... >>` dictionaries when
    /// `startxref` is missing or shifted.
    fn scan_objects_linear(&mut self) {
        let b = self.bytes;
        let len = b.len();
        let mut i = self.header_offset;
        while i < len && self.xref.len() < MAX_OBJECTS {
            // Skip past line start or delimiter before matching `N G obj` or `trailer`.
            let at_boundary = i == 0 || is_ws_or_delim(b[i - 1]);
            if at_boundary {
                if b.get(i..i + 7) == Some(b"trailer") && b.get(i + 7).is_none_or(|&c| is_ws_or_delim(c)) {
                    let mut cur = Cursor::new(b, i + 7);
                    cur.skip_ws_and_comments();
                    if let Some(PdfVal::Dict(tr)) = cur.parse_val(0) {
                        // Later trailers in file order override earlier incremental trailers.
                        for (k, v) in tr {
                            self.trailer.insert(k, v);
                        }
                    }
                } else if b[i].is_ascii_digit()
                    && let Some((obj_num, _gen, after_hdr)) = match_obj_header(b, i)
                {
                    self.xref.insert(obj_num, XrefEntry::Offset(i));
                    // Also inspect if this object is `/Type /Catalog` or `/Type /XRef`.
                    let mut cur = Cursor::new(b, after_hdr);
                    cur.skip_ws_and_comments();
                    if matches!(cur.peek(), Some(b'<'))
                        && cur.peek_at(1) == Some(b'<')
                        && let Some(val) = cur.parse_val_or_stream(0)
                    {
                        if let Some(dict) = val.as_dict() {
                            match dict.get(b"Type".as_slice()).and_then(PdfVal::as_name) {
                                Some(b"Catalog") => {
                                    self.trailer.entry(b"Root".to_vec()).or_insert(PdfVal::Ref((obj_num, 0)));
                                }
                                Some(b"XRef") => {
                                    if let PdfVal::Stream { dict: d, raw } = &val
                                        && let Ok(decoded) = self.decode_stream_bytes(d, raw)
                                    {
                                        parse_xref_stream(d, &decoded, &mut self.xref);
                                    }
                                    for (k, v) in dict {
                                        if matches!(k.as_slice(), b"Root" | b"Info" | b"Encrypt") {
                                            self.trailer.insert(k.clone(), v.clone());
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                        i = cur.pos.max(i + 1);
                        continue;
                    }
                }
            }
            i += 1;
        }
    }

    pub(super) fn resolve<'b>(&self, val: &'b PdfVal, depth: usize) -> Option<PdfVal> {
        if depth > MAX_PARSE_DEPTH {
            return None;
        }
        match val {
            PdfVal::Ref((num, _)) => self.get_object(*num, depth + 1),
            other => Some(other.clone()),
        }
    }

    pub(super) fn resolve_dict_key(&self, dict: &PdfDict, key: &[u8]) -> Option<PdfVal> {
        let v = dict.get(key)?;
        self.resolve(v, 0)
    }

    pub(super) fn get_object(&self, obj_num: u32, depth: usize) -> Option<PdfVal> {
        if depth > MAX_PARSE_DEPTH {
            return None;
        }
        if let Some(cached) = self.cache.borrow().get(&obj_num).cloned() {
            return Some(cached);
        }
        let entry = *self.xref.get(&obj_num)?;
        let val = match entry {
            XrefEntry::Offset(raw_off) => {
                let off = self.adjust_offset(raw_off);
                let mut cur = Cursor::new(self.bytes, off);
                let (_id, v) = cur.parse_indirect_obj(depth + 1)?;
                v
            }
            XrefEntry::Compressed { obj_stm, index } => {
                if obj_stm == obj_num {
                    return None;
                }
                self.load_from_obj_stm(obj_stm, obj_num, index, depth + 1)?
            }
        };
        if self.cache.borrow().len() < MAX_OBJECTS {
            self.cache.borrow_mut().insert(obj_num, val.clone());
        }
        Some(val)
    }

    fn load_from_obj_stm(&self, stm_id: u32, target_obj: u32, fallback_idx: usize, depth: usize) -> Option<PdfVal> {
        if let Some(stm_map) = self.obj_stm_cache.borrow().get(&stm_id) {
            return stm_map.get(&target_obj).cloned();
        }
        let stm_val = self.get_object(stm_id, depth + 1)?;
        let PdfVal::Stream { dict, raw } = stm_val else { return None };
        let n = dict.get(b"N".as_slice()).and_then(PdfVal::as_int).filter(|&v| v > 0 && v <= 50_000)? as usize;
        let first = dict.get(b"First".as_slice()).and_then(PdfVal::as_int).filter(|&v| v >= 0)? as usize;
        let decoded = self.decode_stream_bytes(&dict, &raw).ok()?;
        if first > decoded.len() {
            return None;
        }
        let mut hdr_cur = Cursor::new(&decoded[..first], 0);
        let mut pairs = Vec::with_capacity(n.min(4096));
        for _ in 0..n {
            let Some(PdfVal::Int(id)) = hdr_cur.parse_val(depth + 1) else { break };
            let Some(PdfVal::Int(rel_off)) = hdr_cur.parse_val(depth + 1) else { break };
            if id >= 0 && rel_off >= 0 {
                pairs.push((id as u32, first.saturating_add(rel_off as usize)));
            }
        }
        let mut parsed_map = HashMap::with_capacity(pairs.len());
        let mut by_idx_result = None;
        for (idx, &(id, abs_off)) in pairs.iter().enumerate() {
            if abs_off > decoded.len() {
                continue;
            }
            let mut body_cur = Cursor::new(&decoded, abs_off);
            if let Some(v) = body_cur.parse_val(depth + 1) {
                if idx == fallback_idx && id == target_obj {
                    by_idx_result = Some(v.clone());
                }
                parsed_map.insert(id, v);
            }
        }
        let found = by_idx_result.or_else(|| parsed_map.get(&target_obj).cloned());
        if self.obj_stm_cache.borrow().len() < 4_096 {
            self.obj_stm_cache.borrow_mut().insert(stm_id, parsed_map);
        }
        found
    }

    pub(super) fn decode_stream(&self, val: &PdfVal) -> Result<Vec<u8>, String> {
        let resolved = self.resolve(val, 0).ok_or_else(|| "unresolved PDF stream reference".to_string())?;
        let PdfVal::Stream { dict, raw } = resolved else {
            return Err("expected PDF stream object".into());
        };
        self.decode_stream_bytes(&dict, &raw)
    }

    pub(super) fn decode_stream_bytes(&self, dict: &PdfDict, raw: &[u8]) -> Result<Vec<u8>, String> {
        let filters = stream_filters(dict, self);
        let parms = stream_decode_parms(dict, self);
        if filters.is_empty() {
            return self.charge_stream_bytes(raw.to_vec());
        }
        let mut data = raw.to_vec();
        for (idx, filter) in filters.iter().enumerate() {
            data = match filter.as_slice() {
                b"FlateDecode" | b"Fl" => inflate_pdf_stream(&data)?,
                b"ASCIIHexDecode" | b"AHx" => decode_ascii_hex(&data),
                b"ASCII85Decode" | b"A85" => decode_ascii85(&data)?,
                b"Identity" | b"Crypt" => data,
                other => {
                    return Err(format!("unsupported PDF stream filter `{}`", String::from_utf8_lossy(other)));
                }
            };
            if let Some(Some(p_dict)) = parms.get(idx) {
                data = apply_predictor(&data, p_dict);
            } else if idx == 0
                && parms.len() == 1
                && let Some(Some(p_dict)) = parms.first()
            {
                data = apply_predictor(&data, p_dict);
            }
        }
        self.charge_stream_bytes(data)
    }

    fn charge_stream_bytes(&self, data: Vec<u8>) -> Result<Vec<u8>, String> {
        if data.len() > MAX_STREAM_BYTES {
            return Err("PDF stream exceeds 64 MiB limit".into());
        }
        let next = self.inflated_total.get().saturating_add(data.len());
        if next > MAX_TOTAL_STREAM_BYTES {
            return Err("PDF total decompressed streams exceed 256 MiB limit".into());
        }
        self.inflated_total.set(next);
        Ok(data)
    }

    pub(super) fn collect_pages(&self) -> Result<Vec<PageInfo>, String> {
        let root_val = self.resolve_dict_key(&self.trailer, b"Root").ok_or_else(|| "missing PDF `/Root` catalog".to_string())?;
        let root_dict = root_val.as_dict().ok_or_else(|| "invalid PDF `/Root` catalog".to_string())?;
        let pages_val = self.resolve_dict_key(root_dict, b"Pages").ok_or_else(|| "missing PDF `/Pages` root".to_string())?;
        let mut out = Vec::new();
        let mut visited = HashSet::new();
        if let Some(root_pages_ref) = root_dict.get(b"Pages".as_slice()).and_then(PdfVal::as_ref) {
            visited.insert(root_pages_ref.0);
        }
        let inherited = InheritedPageAttrs { media_box: [0.0, 0.0, 612.0, 792.0], crop_box: None, rotate: 0, resources: HashMap::new() };
        self.walk_page_node(&pages_val, &inherited, &mut visited, &mut out, 0);
        Ok(out)
    }

    fn walk_page_node(&self, node: &PdfVal, parent: &InheritedPageAttrs, visited: &mut HashSet<u32>, out: &mut Vec<PageInfo>, depth: usize) {
        if depth > MAX_PARSE_DEPTH || out.len() >= MAX_PAGES {
            return;
        }
        let Some(dict) = node.as_dict() else { return };
        let mut cur = parent.clone();
        if let Some(mb) = self.resolve_rect(dict, b"MediaBox") {
            cur.media_box = mb;
        }
        if let Some(cb) = self.resolve_rect(dict, b"CropBox") {
            cur.crop_box = Some(cb);
        }
        if let Some(rot) = self.resolve_dict_key(dict, b"Rotate").and_then(|v| v.as_int()) {
            cur.rotate = rot as i32;
        }
        if let Some(res_val) = self.resolve_dict_key(dict, b"Resources")
            && let Some(res_dict) = res_val.as_dict()
        {
            merge_resources(&mut cur.resources, res_dict);
        }

        let node_type = dict.get(b"Type".as_slice()).and_then(PdfVal::as_name);
        let has_kids = dict.contains_key(b"Kids".as_slice());
        if node_type == Some(b"Pages") || (node_type != Some(b"Page") && has_kids) {
            let Some(kids_val) = self.resolve_dict_key(dict, b"Kids") else { return };
            let Some(kids) = kids_val.as_array() else { return };
            for kid in kids {
                if out.len() >= MAX_PAGES {
                    break;
                }
                if let Some((obj_num, _)) = kid.as_ref()
                    && !visited.insert(obj_num)
                {
                    continue;
                }
                if let Some(resolved_kid) = self.resolve(kid, 0) {
                    self.walk_page_node(&resolved_kid, &cur, visited, out, depth + 1);
                }
            }
        } else if node_type == Some(b"Page") || dict.contains_key(b"Contents".as_slice()) || dict.contains_key(b"MediaBox".as_slice()) {
            let eff_box = cur.crop_box.unwrap_or(cur.media_box);
            let contents = dict.get(b"Contents".as_slice()).cloned();
            let links = self.extract_page_links(dict);
            out.push(PageInfo { media_box: eff_box, rotate: cur.rotate, resources: cur.resources, contents, links });
        }
    }

    fn extract_page_links(&self, page_dict: &PdfDict) -> Vec<LinkAnnot> {
        let mut links = Vec::new();
        let Some(annots_val) = self.resolve_dict_key(page_dict, b"Annots") else { return links };
        let Some(annots) = annots_val.as_array() else { return links };
        for item in annots.iter().take(512) {
            let Some(annot_val) = self.resolve(item, 0) else { continue };
            let Some(annot) = annot_val.as_dict() else { continue };
            if annot.get(b"Subtype".as_slice()).and_then(PdfVal::as_name) != Some(b"Link") {
                continue;
            }
            let Some(rect) = self.resolve_rect(annot, b"Rect") else { continue };
            let Some(action_val) = self.resolve_dict_key(annot, b"A") else { continue };
            let Some(action) = action_val.as_dict() else { continue };
            if let Some(uri_val) = self.resolve_dict_key(action, b"URI")
                && let Some(uri_bytes) = uri_val.as_bytes()
            {
                let uri = decode_pdf_text_string(uri_bytes);
                if let Some(kept) = crate::model::kept_link(&uri) {
                    let x0 = rect[0].min(rect[2]);
                    let y0 = rect[1].min(rect[3]);
                    let x1 = rect[0].max(rect[2]);
                    let y1 = rect[1].max(rect[3]);
                    links.push(LinkAnnot { rect: [x0, y0, x1, y1], uri: kept.to_string() });
                }
            }
        }
        links
    }

    fn resolve_rect(&self, dict: &PdfDict, key: &[u8]) -> Option<[f64; 4]> {
        let val = self.resolve_dict_key(dict, key)?;
        let arr = val.as_array()?;
        if arr.len() < 4 {
            return None;
        }
        let a = self.resolve(arr.first()?, 0)?.as_f64()?;
        let b = self.resolve(arr.get(1)?, 0)?.as_f64()?;
        let c = self.resolve(arr.get(2)?, 0)?.as_f64()?;
        let d = self.resolve(arr.get(3)?, 0)?.as_f64()?;
        Some([a, b, c, d])
    }

    pub(super) fn extract_meta(&self) -> Meta {
        let mut meta = Meta::default();
        let Some(info_val) = self.resolve_dict_key(&self.trailer, b"Info") else { return meta };
        let Some(info) = info_val.as_dict() else { return meta };
        let get_str = |key: &[u8]| -> String {
            self.resolve_dict_key(info, key)
                .and_then(|v| v.as_bytes().map(decode_pdf_text_string))
                .map(|s| crate::model::clean_text(s.trim()))
                .unwrap_or_default()
        };
        meta.title = get_str(b"Title");
        meta.author = get_str(b"Author");
        meta.subject = get_str(b"Subject");
        meta.keywords = get_str(b"Keywords");
        let raw_date = get_str(b"CreationDate");
        if !raw_date.is_empty() {
            meta.created = parse_pdf_date(&raw_date).unwrap_or(raw_date);
        }
        meta
    }
}

#[derive(Clone)]
struct InheritedPageAttrs {
    media_box: [f64; 4],
    crop_box: Option<[f64; 4]>,
    rotate: i32,
    resources: PdfDict,
}

pub(super) fn merge_resources(dst: &mut PdfDict, child: &PdfDict) {
    for (k, v) in child {
        if matches!(k.as_slice(), b"Font" | b"XObject" | b"ExtGState" | b"ColorSpace")
            && let (Some(PdfVal::Dict(existing)), PdfVal::Dict(sub)) = (dst.get_mut(k.as_slice()), v)
        {
            for (sk, sv) in sub {
                existing.insert(sk.clone(), sv.clone());
            }
            continue;
        }
        dst.insert(k.clone(), v.clone());
    }
}

fn merge_dict_newer_wins(newer: &mut PdfDict, older: PdfDict) {
    for (k, v) in older {
        newer.entry(k).or_insert(v);
    }
}

/// Decode a PDF text string (UTF-16BE BOM, UTF-8 BOM, or PDFDocEncoding).
pub(super) fn decode_pdf_text_string(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return decode_utf16be_bytes(rest);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if let Ok(utf8) = std::str::from_utf8(bytes)
        && !bytes.iter().any(|&b| (0x80..=0x9F).contains(&b))
    {
        return utf8.to_string();
    }
    bytes.iter().map(|&b| pdf_doc_encoding_char(b)).collect()
}

pub(super) fn decode_utf16be_bytes(bytes: &[u8]) -> String {
    let mut u16s = Vec::with_capacity(bytes.len() / 2);
    let mut chunks = bytes.chunks_exact(2);
    for pair in &mut chunks {
        u16s.push(u16::from_be_bytes([pair[0], pair[1]]));
    }
    char::decode_utf16(u16s).map(|r| r.unwrap_or('\u{FFFD}')).collect()
}

fn pdf_doc_encoding_char(b: u8) -> char {
    match b {
        0x18 => '\u{02D8}',
        0x19 => '\u{02C7}',
        0x1A => '\u{02C6}',
        0x1B => '\u{02D9}',
        0x1C => '\u{02DD}',
        0x1D => '\u{02DB}',
        0x1E => '\u{02DA}',
        0x1F => '\u{02DC}',
        0x80 => '\u{2022}',
        0x81 => '\u{2020}',
        0x82 => '\u{2021}',
        0x83 => '\u{2026}',
        0x84 => '\u{2014}',
        0x85 => '\u{2013}',
        0x86 => '\u{0192}',
        0x87 => '\u{2044}',
        0x88 => '\u{2039}',
        0x89 => '\u{203A}',
        0x8A => '\u{2212}',
        0x8B => '\u{2030}',
        0x8C => '\u{201E}',
        0x8D => '\u{201C}',
        0x8E => '\u{201D}',
        0x8F => '\u{2018}',
        0x90 => '\u{2019}',
        0x91 => '\u{201A}',
        0x92 => '\u{2122}',
        0x93 => '\u{FB01}',
        0x94 => '\u{FB02}',
        0x95 => '\u{0141}',
        0x96 => '\u{0152}',
        0x97 => '\u{0160}',
        0x98 => '\u{0178}',
        0x99 => '\u{017D}',
        0x9A => '\u{0131}',
        0x9B => '\u{0142}',
        0x9C => '\u{0153}',
        0x9D => '\u{0161}',
        0x9E => '\u{017E}',
        0xA0 => '\u{20AC}',
        other => char::from(other),
    }
}

/// Convert PDF `D:YYYYMMDDHHmmSSOHH'mm'` date strings to ISO 8601.
fn parse_pdf_date(s: &str) -> Option<String> {
    let s = s.trim();
    let body = s.strip_prefix("D:").unwrap_or(s);
    if body.len() < 4 || !body.get(..4)?.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let yr = body.get(0..4)?;
    let mo = body.get(4..6).filter(|x| x.bytes().all(|b| b.is_ascii_digit())).unwrap_or("01");
    let dy = body.get(6..8).filter(|x| x.bytes().all(|b| b.is_ascii_digit())).unwrap_or("01");
    if body.len() < 10 || !body.get(8..10).is_some_and(|x| x.bytes().all(|b| b.is_ascii_digit())) {
        return Some(format!("{yr}-{mo}-{dy}"));
    }
    let hh = body.get(8..10).unwrap_or("00");
    let mm = body.get(10..12).filter(|x| x.bytes().all(|b| b.is_ascii_digit())).unwrap_or("00");
    let ss = body.get(12..14).filter(|x| x.bytes().all(|b| b.is_ascii_digit())).unwrap_or("00");
    let tz_raw = body.get(14..).unwrap_or("Z").trim();
    let tz = if tz_raw.is_empty() || tz_raw.starts_with('Z') || tz_raw.starts_with('z') {
        "Z".to_string()
    } else if (tz_raw.starts_with('+') || tz_raw.starts_with('-')) && tz_raw.len() >= 3 {
        let sign = &tz_raw[..1];
        let clean: String = tz_raw[1..].chars().filter(char::is_ascii_digit).collect();
        let tzh = clean.get(0..2).unwrap_or("00");
        let tzm = clean.get(2..4).unwrap_or("00");
        format!("{sign}{tzh}:{tzm}")
    } else {
        "Z".to_string()
    };
    Some(format!("{yr}-{mo}-{dy}T{hh}:{mm}:{ss}{tz}"))
}

fn stream_filters(dict: &PdfDict, doc: &PdfDoc<'_>) -> Vec<Vec<u8>> {
    let Some(f_val) = doc.resolve_dict_key(dict, b"Filter").or_else(|| doc.resolve_dict_key(dict, b"F")) else {
        return Vec::new();
    };
    match f_val {
        PdfVal::Name(n) => vec![n],
        PdfVal::Array(arr) => arr
            .iter()
            .filter_map(|item| doc.resolve(item, 0))
            .filter_map(|v| match v {
                PdfVal::Name(n) => Some(n),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn stream_decode_parms(dict: &PdfDict, doc: &PdfDoc<'_>) -> Vec<Option<PdfDict>> {
    let Some(p_val) = doc.resolve_dict_key(dict, b"DecodeParms").or_else(|| doc.resolve_dict_key(dict, b"DP")) else {
        return Vec::new();
    };
    match p_val {
        PdfVal::Dict(d) => vec![Some(d)],
        PdfVal::Array(arr) => arr.iter().map(|item| doc.resolve(item, 0).and_then(|v| v.as_dict().cloned())).collect(),
        _ => Vec::new(),
    }
}

fn inflate_pdf_stream(raw: &[u8]) -> Result<Vec<u8>, String> {
    let trimmed = trim_trailing_pdf_ws(raw);
    for candidate in [raw, trimmed] {
        if let Ok(out) = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(candidate, MAX_STREAM_BYTES) {
            return Ok(out);
        }
        if let Ok(out) = miniz_oxide::inflate::decompress_to_vec_with_limit(candidate, MAX_STREAM_BYTES) {
            return Ok(out);
        }
    }
    Err("corrupt FlateDecode PDF stream".into())
}

fn trim_trailing_pdf_ws(mut b: &[u8]) -> &[u8] {
    while let Some((&last, rest)) = b.split_last() {
        if matches!(last, b' ' | b'\t' | b'\r' | b'\n' | 0x0C) {
            b = rest;
        } else {
            break;
        }
    }
    b
}

fn decode_ascii_hex(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len() / 2);
    let mut hi: Option<u8> = None;
    for &b in raw {
        if b == b'>' {
            break;
        }
        if is_pdf_ws(b) {
            continue;
        }
        let Some(nib) = hex_val(b) else { continue };
        match hi.take() {
            None => hi = Some(nib),
            Some(h) => out.push((h << 4) | nib),
        }
    }
    if let Some(h) = hi {
        out.push(h << 4);
    }
    out
}

fn decode_ascii85(raw: &[u8]) -> Result<Vec<u8>, String> {
    let mut s = raw;
    if let Some(rest) = s.strip_prefix(b"<~") {
        s = rest;
    }
    let mut out = Vec::with_capacity(s.len());
    let mut tuple = [0u8; 5];
    let mut count = 0usize;
    let mut i = 0usize;
    while i < s.len() {
        let b = s[i];
        i += 1;
        if b == b'~' && s.get(i) == Some(&b'>') {
            break;
        }
        if is_pdf_ws(b) {
            continue;
        }
        if b == b'z' && count == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
            if out.len() > MAX_STREAM_BYTES {
                return Err("ASCII85 stream exceeds size limit".into());
            }
            continue;
        }
        if !(b'!'..=b'u').contains(&b) {
            continue;
        }
        tuple[count] = b - b'!';
        count += 1;
        if count == 5 {
            let mut val: u64 = 0;
            for &d in &tuple {
                val = val * 85 + u64::from(d);
            }
            let bytes = (val as u32).to_be_bytes();
            out.extend_from_slice(&bytes);
            count = 0;
            if out.len() > MAX_STREAM_BYTES {
                return Err("ASCII85 stream exceeds size limit".into());
            }
        }
    }
    if count >= 2 {
        for slot in tuple.iter_mut().skip(count) {
            *slot = 84;
        }
        let mut val: u64 = 0;
        for &d in &tuple {
            val = val * 85 + u64::from(d);
        }
        let bytes = (val as u32).to_be_bytes();
        out.extend_from_slice(&bytes[..count - 1]);
    }
    Ok(out)
}

fn apply_predictor(data: &[u8], parms: &PdfDict) -> Vec<u8> {
    let predictor = parms.get(b"Predictor".as_slice()).and_then(PdfVal::as_int).unwrap_or(1);
    if predictor <= 1 {
        return data.to_vec();
    }
    let columns = parms.get(b"Columns".as_slice()).and_then(PdfVal::as_int).unwrap_or(1).clamp(1, 65_536) as usize;
    let colors = parms.get(b"Colors".as_slice()).and_then(PdfVal::as_int).unwrap_or(1).clamp(1, 32) as usize;
    let bpc = parms.get(b"BitsPerComponent".as_slice()).and_then(PdfVal::as_int).unwrap_or(8).clamp(1, 16) as usize;
    let bpp = (colors * bpc).div_ceil(8).max(1);
    let row_bytes = (columns * colors * bpc).div_ceil(8).max(1);

    if predictor == 2 {
        // TIFF Predictor 2 (horizontal differencing, 8-bit components common case).
        if bpc != 8 {
            return data.to_vec();
        }
        let mut out = data.to_vec();
        for row in out.chunks_mut(row_bytes) {
            for i in bpp..row.len() {
                row[i] = row[i].wrapping_add(row[i - bpp]);
            }
        }
        return out;
    }

    if (10..=15).contains(&predictor) {
        let stride = row_bytes + 1;
        let mut out = Vec::with_capacity(data.len());
        let mut prev_row = vec![0u8; row_bytes];
        for chunk in data.chunks(stride) {
            let Some((&filter_byte, src)) = chunk.split_first() else { break };
            let mut cur_row = vec![0u8; row_bytes];
            let n = src.len().min(row_bytes);
            for i in 0..n {
                let raw = src[i];
                let left = if i >= bpp { cur_row[i - bpp] } else { 0 };
                let up = prev_row[i];
                let up_left = if i >= bpp { prev_row[i - bpp] } else { 0 };
                cur_row[i] = match filter_byte {
                    0 => raw,
                    1 => raw.wrapping_add(left),
                    2 => raw.wrapping_add(up),
                    3 => raw.wrapping_add(((u16::from(left) + u16::from(up)) / 2) as u8),
                    4 => raw.wrapping_add(paeth(left, up, up_left)),
                    _ => raw,
                };
            }
            out.extend_from_slice(&cur_row[..n]);
            prev_row = cur_row;
        }
        return out;
    }

    data.to_vec()
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = i32::from(a) + i32::from(b) - i32::from(c);
    let pa = (p - i32::from(a)).abs();
    let pb = (p - i32::from(b)).abs();
    let pc = (p - i32::from(c)).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

fn find_startxref(bytes: &[u8]) -> Option<usize> {
    let tail_start = bytes.len().saturating_sub(4096);
    let tail = &bytes[tail_start..];
    let rel = tail.windows(9).rposition(|w| w == b"startxref")?;
    let mut cur = Cursor::new(bytes, tail_start + rel + 9);
    cur.skip_ws_and_comments();
    let PdfVal::Int(off) = cur.parse_val(0)? else { return None };
    (off >= 0).then_some(off as usize)
}

fn looks_like_xref_or_obj(bytes: &[u8], off: usize) -> bool {
    let mut cur = Cursor::new(bytes, off);
    cur.skip_ws_and_comments();
    if cur.eat_keyword(b"xref") {
        return true;
    }
    match_obj_header(bytes, cur.pos).is_some()
}

fn match_obj_header(bytes: &[u8], start: usize) -> Option<(u32, u16, usize)> {
    let mut cur = Cursor::new(bytes, start);
    let obj_num = cur.read_u32()?;
    if !cur.skip_ws_and_comments_any() {
        return None;
    }
    let gen_num = cur.read_u16()?;
    if !cur.skip_ws_and_comments_any() {
        return None;
    }
    if !cur.eat_keyword(b"obj") {
        return None;
    }
    Some((obj_num, gen_num, cur.pos))
}

fn parse_classic_xref(cur: &mut Cursor<'_>, xref: &mut HashMap<u32, XrefEntry>) -> Option<PdfDict> {
    loop {
        cur.skip_ws_and_comments();
        if cur.eat_keyword(b"trailer") {
            cur.skip_ws_and_comments();
            return match cur.parse_val(0)? {
                PdfVal::Dict(d) => Some(d),
                _ => None,
            };
        }
        let Some(first_id) = cur.read_u32() else { break };
        cur.skip_ws_and_comments();
        let Some(count) = cur.read_u32() else { break };
        let capped = (count as usize).min(MAX_OBJECTS);
        for idx in 0..capped {
            cur.skip_ws_and_comments();
            let Some(off) = cur.read_u64() else { break };
            cur.skip_ws_and_comments();
            let Some(_gen) = cur.read_u32() else { break };
            cur.skip_ws_and_comments();
            let Some(flag) = cur.next_byte() else { break };
            let obj_id = first_id.saturating_add(idx as u32);
            if flag == b'n' && off > 0 && xref.len() < MAX_OBJECTS {
                xref.entry(obj_id).or_insert(XrefEntry::Offset(off as usize));
            }
        }
    }
    None
}

fn parse_xref_stream(dict: &PdfDict, data: &[u8], xref: &mut HashMap<u32, XrefEntry>) {
    let Some(w_arr) = dict.get(b"W".as_slice()).and_then(PdfVal::as_array) else { return };
    if w_arr.len() < 3 {
        return;
    }
    let w0 = w_arr[0].as_int().unwrap_or(0).clamp(0, 8) as usize;
    let w1 = w_arr[1].as_int().unwrap_or(0).clamp(0, 8) as usize;
    let w2 = w_arr[2].as_int().unwrap_or(0).clamp(0, 8) as usize;
    let entry_len = w0 + w1 + w2;
    if entry_len == 0 {
        return;
    }
    let size = dict.get(b"Size".as_slice()).and_then(PdfVal::as_int).unwrap_or(0).max(0) as u32;
    let subsections: Vec<(u32, u32)> = if let Some(idx_arr) = dict.get(b"Index".as_slice()).and_then(PdfVal::as_array) {
        let mut v = Vec::new();
        let mut it = idx_arr.chunks_exact(2);
        for pair in &mut it {
            if let (Some(start), Some(cnt)) = (pair[0].as_int(), pair[1].as_int())
                && start >= 0
                && cnt >= 0
            {
                v.push((start as u32, cnt as u32));
            }
        }
        v
    } else {
        vec![(0, size)]
    };

    let mut pos = 0usize;
    for (start_id, count) in subsections {
        let capped = (count as usize).min(MAX_OBJECTS);
        for i in 0..capped {
            let Some(rec) = data.get(pos..pos + entry_len) else { return };
            pos += entry_len;
            let f_type = if w0 == 0 { 1 } else { read_be_uint(&rec[..w0]) };
            let f1 = read_be_uint(&rec[w0..w0 + w1]);
            let f2 = read_be_uint(&rec[w0 + w1..entry_len]);
            let obj_id = start_id.saturating_add(i as u32);
            if xref.len() >= MAX_OBJECTS {
                return;
            }
            match f_type {
                1 if f1 > 0 => {
                    xref.entry(obj_id).or_insert(XrefEntry::Offset(f1 as usize));
                }
                2 if f1 > 0 => {
                    xref.entry(obj_id).or_insert(XrefEntry::Compressed { obj_stm: f1 as u32, index: f2 as usize });
                }
                _ => {}
            }
        }
    }
}

fn read_be_uint(bytes: &[u8]) -> u64 {
    let mut val = 0u64;
    for &b in bytes {
        val = (val << 8) | u64::from(b);
    }
    val
}

pub(super) struct Cursor<'a> {
    pub bytes: &'a [u8],
    pub pos: usize,
}

impl<'a> Cursor<'a> {
    pub(super) fn new(bytes: &'a [u8], pos: usize) -> Self {
        Self { bytes, pos: pos.min(bytes.len()) }
    }

    pub(super) fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    pub(super) fn peek_at(&self, rel: usize) -> Option<u8> {
        self.bytes.get(self.pos.checked_add(rel)?).copied()
    }

    pub(super) fn next_byte(&mut self) -> Option<u8> {
        let b = self.bytes.get(self.pos).copied()?;
        self.pos += 1;
        Some(b)
    }

    pub(super) fn skip_ws_and_comments(&mut self) {
        let _ = self.skip_ws_and_comments_any();
    }

    pub(super) fn skip_ws_and_comments_any(&mut self) -> bool {
        let start = self.pos;
        while let Some(b) = self.peek() {
            if is_pdf_ws(b) {
                self.pos += 1;
            } else if b == b'%' {
                self.pos += 1;
                while let Some(c) = self.next_byte() {
                    if c == b'\n' || c == b'\r' {
                        break;
                    }
                }
            } else {
                break;
            }
        }
        self.pos > start
    }

    pub(super) fn eat_keyword(&mut self, kw: &[u8]) -> bool {
        if self.bytes.get(self.pos..self.pos + kw.len()) == Some(kw) && self.bytes.get(self.pos + kw.len()).is_none_or(|&b| is_ws_or_delim(b)) {
            self.pos += kw.len();
            true
        } else {
            false
        }
    }

    pub(super) fn read_u64(&mut self) -> Option<u64> {
        let start = self.pos;
        let mut val: u64 = 0;
        while let Some(b) = self.peek() {
            if b.is_ascii_digit() {
                val = val.checked_mul(10)?.checked_add(u64::from(b - b'0'))?;
                self.pos += 1;
            } else {
                break;
            }
        }
        (self.pos > start && self.peek().is_none_or(is_ws_or_delim)).then_some(val)
    }

    pub(super) fn read_u32(&mut self) -> Option<u32> {
        u32::try_from(self.read_u64()?).ok()
    }

    pub(super) fn read_u16(&mut self) -> Option<u16> {
        u16::try_from(self.read_u64()?).ok()
    }

    pub(super) fn parse_indirect_obj(&mut self, depth: usize) -> Option<(ObjId, PdfVal)> {
        if depth > MAX_PARSE_DEPTH {
            return None;
        }
        self.skip_ws_and_comments();
        let num = self.read_u32()?;
        self.skip_ws_and_comments();
        let gen_num = self.read_u16()?;
        self.skip_ws_and_comments();
        if !self.eat_keyword(b"obj") {
            return None;
        }
        let val = self.parse_val_or_stream(depth + 1)?;
        Some(((num, gen_num), val))
    }

    pub(super) fn parse_val_or_stream(&mut self, depth: usize) -> Option<PdfVal> {
        let val = self.parse_val(depth)?;
        if let PdfVal::Dict(dict) = val {
            let saved = self.pos;
            self.skip_ws_and_comments();
            if self.eat_keyword(b"stream") {
                // ISO 32000-1 §7.3.8.1: `stream` is followed by CRLF or LF (not bare CR, though
                // some generators emit bare CR or space+LF; handle cleanly).
                if self.peek() == Some(b' ') {
                    self.pos += 1;
                }
                if self.peek() == Some(b'\r') {
                    self.pos += 1;
                    if self.peek() == Some(b'\n') {
                        self.pos += 1;
                    }
                } else if self.peek() == Some(b'\n') {
                    self.pos += 1;
                }
                let data_start = self.pos;
                let raw = extract_stream_slice(self.bytes, data_start, &dict, &mut self.pos);
                return Some(PdfVal::Stream { dict, raw });
            }
            self.pos = saved;
            return Some(PdfVal::Dict(dict));
        }
        Some(val)
    }

    pub(super) fn parse_val(&mut self, depth: usize) -> Option<PdfVal> {
        if depth > MAX_PARSE_DEPTH {
            return None;
        }
        self.skip_ws_and_comments();
        let b = self.peek()?;
        match b {
            b'/' => Some(PdfVal::Name(self.read_name())),
            b'(' => Some(PdfVal::Str(self.read_literal_string())),
            b'<' => {
                if self.peek_at(1) == Some(b'<') {
                    self.pos += 2;
                    let mut dict = HashMap::new();
                    loop {
                        self.skip_ws_and_comments();
                        if self.peek().is_none() {
                            break;
                        }
                        if self.peek() == Some(b'>') && self.peek_at(1) == Some(b'>') {
                            self.pos += 2;
                            break;
                        }
                        if self.peek() != Some(b'/') {
                            // Recover from stray token inside malformed dictionary.
                            let _ = self.parse_val(depth + 1);
                            continue;
                        }
                        let key = self.read_name();
                        let Some(v) = self.parse_val(depth + 1) else { break };
                        dict.insert(key, v);
                    }
                    Some(PdfVal::Dict(dict))
                } else {
                    Some(PdfVal::Str(self.read_hex_string()))
                }
            }
            b'[' => {
                self.pos += 1;
                let mut arr = Vec::new();
                loop {
                    self.skip_ws_and_comments();
                    match self.peek() {
                        None => break,
                        Some(b']') => {
                            self.pos += 1;
                            break;
                        }
                        _ => {
                            let Some(item) = self.parse_val(depth + 1) else { break };
                            arr.push(item);
                        }
                    }
                }
                Some(PdfVal::Array(arr))
            }
            b'+' | b'-' | b'.' | b'0'..=b'9' => self.read_number_or_ref(),
            _ => {
                let tok = self.read_regular_token();
                match tok.as_slice() {
                    b"true" | b"false" => Some(PdfVal::Bool),
                    b"null" => Some(PdfVal::Null),
                    _ => None,
                }
            }
        }
    }

    pub(super) fn read_name(&mut self) -> Vec<u8> {
        if self.peek() == Some(b'/') {
            self.pos += 1;
        }
        let mut out = Vec::new();
        while let Some(b) = self.peek() {
            if is_ws_or_delim(b) {
                break;
            }
            self.pos += 1;
            if b == b'#'
                && let (Some(h1), Some(h2)) = (self.peek().and_then(hex_val), self.peek_at(1).and_then(hex_val))
            {
                self.pos += 2;
                out.push((h1 << 4) | h2);
            } else {
                out.push(b);
            }
        }
        out
    }

    pub(super) fn read_literal_string(&mut self) -> Vec<u8> {
        if self.peek() == Some(b'(') {
            self.pos += 1;
        }
        let mut out = Vec::new();
        let mut paren_depth = 1u32;
        while let Some(b) = self.next_byte() {
            match b {
                b'(' => {
                    paren_depth += 1;
                    out.push(b'(');
                }
                b')' => {
                    paren_depth -= 1;
                    if paren_depth == 0 {
                        break;
                    }
                    out.push(b')');
                }
                b'\\' => {
                    let Some(esc) = self.next_byte() else { break };
                    match esc {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'b' => out.push(0x08),
                        b'f' => out.push(0x0C),
                        b'(' => out.push(b'('),
                        b')' => out.push(b')'),
                        b'\\' => out.push(b'\\'),
                        b'\r' => {
                            if self.peek() == Some(b'\n') {
                                self.pos += 1;
                            }
                        }
                        b'\n' => {}
                        b'0'..=b'7' => {
                            let mut oct = u16::from(esc - b'0');
                            for _ in 0..2 {
                                if let Some(d @ b'0'..=b'7') = self.peek() {
                                    self.pos += 1;
                                    oct = oct * 8 + u16::from(d - b'0');
                                } else {
                                    break;
                                }
                            }
                            out.push((oct & 0xFF) as u8);
                        }
                        other => out.push(other),
                    }
                }
                b'\r' => {
                    if self.peek() == Some(b'\n') {
                        self.pos += 1;
                    }
                    out.push(b'\n');
                }
                other => out.push(other),
            }
        }
        out
    }

    pub(super) fn read_hex_string(&mut self) -> Vec<u8> {
        if self.peek() == Some(b'<') {
            self.pos += 1;
        }
        let mut out = Vec::new();
        let mut hi: Option<u8> = None;
        while let Some(b) = self.next_byte() {
            if b == b'>' {
                break;
            }
            if is_pdf_ws(b) {
                continue;
            }
            if let Some(nib) = hex_val(b) {
                match hi.take() {
                    None => hi = Some(nib),
                    Some(h) => out.push((h << 4) | nib),
                }
            }
        }
        if let Some(h) = hi {
            out.push(h << 4);
        }
        out
    }

    fn read_number_or_ref(&mut self) -> Option<PdfVal> {
        let tok = self.read_regular_token();
        let s = std::str::from_utf8(&tok).ok()?;
        if !s.contains('.')
            && let Ok(n) = s.parse::<i64>()
        {
            if n >= 0
                && let Ok(obj_num) = u32::try_from(n)
            {
                let saved = self.pos;
                self.skip_ws_and_comments();
                if let Some(gen_num) = self.read_u16() {
                    self.skip_ws_and_comments();
                    if self.eat_keyword(b"R") {
                        return Some(PdfVal::Ref((obj_num, gen_num)));
                    }
                }
                self.pos = saved;
            }
            return Some(PdfVal::Int(n));
        }
        let r = s.parse::<f64>().ok()?;
        r.is_finite().then_some(PdfVal::Real(r))
    }

    pub(super) fn read_regular_token(&mut self) -> Vec<u8> {
        let start = self.pos;
        while let Some(b) = self.peek() {
            if is_ws_or_delim(b) {
                break;
            }
            self.pos += 1;
        }
        self.bytes.get(start..self.pos).unwrap_or_default().to_vec()
    }
}

fn extract_stream_slice(bytes: &[u8], data_start: usize, dict: &PdfDict, out_pos: &mut usize) -> Vec<u8> {
    if let Some(len) = dict.get(b"Length".as_slice()).and_then(PdfVal::as_int).filter(|&l| l >= 0) {
        let declared_end = data_start.saturating_add(len as usize);
        if let Some(slice) = bytes.get(data_start..declared_end) {
            let mut probe = Cursor::new(bytes, declared_end);
            probe.skip_ws_and_comments();
            if probe.eat_keyword(b"endstream") {
                *out_pos = probe.pos;
                return slice.to_vec();
            }
        }
    }
    // Fallback when `/Length` is an indirect reference or inaccurate: scan for `endstream`.
    if let Some(rel) = find_endstream(&bytes[data_start..]) {
        let mut end = data_start + rel;
        if end > data_start && bytes.get(end - 1) == Some(&b'\n') {
            end -= 1;
            if end > data_start && bytes.get(end - 1) == Some(&b'\r') {
                end -= 1;
            }
        } else if end > data_start && bytes.get(end - 1) == Some(&b'\r') {
            end -= 1;
        }
        *out_pos = (data_start + rel + 9).min(bytes.len());
        return bytes.get(data_start..end).unwrap_or_default().to_vec();
    }
    *out_pos = bytes.len();
    bytes.get(data_start..).unwrap_or_default().to_vec()
}

fn find_endstream(haystack: &[u8]) -> Option<usize> {
    haystack.windows(9).position(|w| w == b"endstream")
}

pub(super) fn is_pdf_ws(b: u8) -> bool {
    matches!(b, 0x00 | 0x09 | 0x0A | 0x0C | 0x0D | 0x20)
}

pub(super) fn is_pdf_delim(b: u8) -> bool {
    matches!(b, b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%')
}

pub(super) fn is_ws_or_delim(b: u8) -> bool {
    is_pdf_ws(b) || is_pdf_delim(b)
}

pub(super) fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
