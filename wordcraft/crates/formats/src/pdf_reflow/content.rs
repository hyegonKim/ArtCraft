//! PDF content stream tokenizer and text/graphics state interpreter (`BT`/`ET`, `q`/`Q`, `cm`,
//! `Tf`, `Tc`, `Tw`, `Tz`, `TL`, `Ts`, `Tr`, `Td`, `TD`, `Tm`, `T*`, `Tj`, `TJ`, `'`, `"`, fill
//! colours `rg`/`g`/`k`/`sc`/`scn`, and Form `/XObject` recursion).

use std::collections::HashMap;
use std::sync::Arc;

use wordcraft_doc::Rgb;

use super::cmap::PdfFont;
use super::parser::{Cursor, PageInfo, PdfDict, PdfDoc, PdfVal, is_pdf_ws, is_ws_or_delim, merge_resources};

/// Maximum graphics state stack depth (`q`/`Q`).
const MAX_GS_STACK: usize = 64;
/// Maximum Form XObject nesting depth (`Do`).
const MAX_FORM_DEPTH: usize = 8;
/// Maximum positioned spans collected per page.
const MAX_PAGE_SPANS: usize = 100_000;

#[derive(Clone, Debug)]
pub(super) struct TextSpan {
    /// Reading-order left x in normalized page space (0 = left edge, increasing rightward).
    pub x: f64,
    /// Estimated right x in normalized page space.
    pub end_x: f64,
    /// Baseline y in normalized page space (increasing upward, so larger y is higher on the page).
    pub y: f64,
    /// Raw user-space bounding box `[x0, y0, x1, y1]` for `/Annots` link matching.
    pub raw_bbox: [f64; 4],
    /// Effective font size in points.
    pub size: f32,
    pub font: Option<Arc<str>>,
    pub bold: bool,
    pub italic: bool,
    pub color: Rgb,
    pub invisible: bool,
    pub text: String,
}

#[derive(Clone)]
struct GraphicsState {
    ctm: [f64; 6],
    fill_rgb: Rgb,
    char_space: f64,
    word_space: f64,
    horiz_scale: f64,
    leading: f64,
    rise: f64,
    render_mode: i64,
    font_key: Option<Vec<u8>>,
    font_size: f64,
}

impl Default for GraphicsState {
    fn default() -> Self {
        Self {
            ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            fill_rgb: Rgb::BLACK,
            char_space: 0.0,
            word_space: 0.0,
            horiz_scale: 100.0,
            leading: 0.0,
            rise: 0.0,
            render_mode: 0,
            font_key: None,
            font_size: 12.0,
        }
    }
}

pub(super) fn extract_page_spans(doc: &PdfDoc<'_>, page: &PageInfo) -> Vec<TextSpan> {
    let Some(contents) = &page.contents else { return Vec::new() };
    let stream_bytes = collect_content_bytes(doc, contents);
    if stream_bytes.is_empty() {
        return Vec::new();
    }
    let mut all_spans = Vec::new();
    let initial_gs = GraphicsState::default();
    interpret_stream(doc, &stream_bytes, &page.resources, initial_gs, page.media_box, page.rotate, 0, &mut all_spans);

    // Prefer visible text (`Tr != 3`); fall back to invisible OCR layer only if the page has no
    // visible text at all.
    let has_visible = all_spans.iter().any(|s| !s.invisible && !s.text.trim().is_empty());
    if has_visible {
        all_spans.retain(|s| !s.invisible);
    }
    all_spans
}

fn collect_content_bytes(doc: &PdfDoc<'_>, contents: &PdfVal) -> Vec<u8> {
    let Some(resolved) = doc.resolve(contents, 0) else { return Vec::new() };
    match resolved {
        PdfVal::Stream { dict, raw } => doc.decode_stream_bytes(&dict, &raw).unwrap_or_default(),
        PdfVal::Array(arr) => {
            let mut combined = Vec::new();
            for item in &arr {
                if let Ok(chunk) = doc.decode_stream(item) {
                    if !combined.is_empty() {
                        combined.push(b'\n');
                    }
                    combined.extend_from_slice(&chunk);
                }
            }
            combined
        }
        _ => Vec::new(),
    }
}

fn build_font_map(doc: &PdfDoc<'_>, resources: &PdfDict) -> HashMap<Vec<u8>, PdfFont> {
    let mut map = HashMap::new();
    let Some(fonts_val) = doc.resolve_dict_key(resources, b"Font") else { return map };
    let Some(fonts_dict) = fonts_val.as_dict() else { return map };
    for (name, val) in fonts_dict {
        if let Some(resolved) = doc.resolve(val, 0)
            && let Some(f_dict) = resolved.as_dict()
        {
            map.insert(name.clone(), PdfFont::from_dict(doc, f_dict));
        }
    }
    map
}

#[allow(clippy::too_many_arguments)]
fn interpret_stream(
    doc: &PdfDoc<'_>,
    bytes: &[u8],
    resources: &PdfDict,
    mut gs: GraphicsState,
    media_box: [f64; 4],
    rotate: i32,
    form_depth: usize,
    out: &mut Vec<TextSpan>,
) {
    if form_depth > MAX_FORM_DEPTH || out.len() >= MAX_PAGE_SPANS {
        return;
    }
    let fonts = build_font_map(doc, resources);
    let fallback_font = PdfFont::fallback();
    let mut gs_stack: Vec<GraphicsState> = Vec::new();
    let mut tm = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut tlm = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut operands: Vec<PdfVal> = Vec::with_capacity(16);

    let mut cur = Cursor::new(bytes, 0);
    while cur.pos < bytes.len() && out.len() < MAX_PAGE_SPANS {
        cur.skip_ws_and_comments();
        let Some(b) = cur.peek() else { break };

        // Values vs operators:
        match b {
            b'/' | b'(' | b'[' => {
                if let Some(v) = cur.parse_val(0) {
                    if operands.len() < 256 {
                        operands.push(v);
                    }
                } else {
                    cur.pos += 1;
                }
                continue;
            }
            b'<' => {
                if let Some(v) = cur.parse_val(0) {
                    if operands.len() < 256 {
                        operands.push(v);
                    }
                } else {
                    cur.pos += 1;
                }
                continue;
            }
            b'+' | b'-' | b'.' | b'0'..=b'9' => {
                let tok = cur.read_regular_token();
                if let Ok(s) = std::str::from_utf8(&tok) {
                    if let Ok(i) = s.parse::<i64>() {
                        if operands.len() < 256 {
                            operands.push(PdfVal::Int(i));
                        }
                        continue;
                    }
                    if let Ok(f) = s.parse::<f64>()
                        && f.is_finite()
                    {
                        if operands.len() < 256 {
                            operands.push(PdfVal::Real(f));
                        }
                        continue;
                    }
                }
                operands.clear();
                continue;
            }
            b')' | b'>' | b']' | b'{' | b'}' => {
                cur.pos += 1;
                continue;
            }
            _ => {}
        }

        let op = cur.read_regular_token();
        if op.is_empty() {
            cur.pos += 1;
            continue;
        }
        match op.as_slice() {
            b"true" | b"false" => {
                operands.push(PdfVal::Bool);
                continue;
            }
            b"null" => {
                operands.push(PdfVal::Null);
                continue;
            }
            b"BI" => {
                skip_inline_image(&mut cur);
                operands.clear();
                continue;
            }
            b"q" => {
                if gs_stack.len() < MAX_GS_STACK {
                    gs_stack.push(gs.clone());
                }
            }
            b"Q" => {
                if let Some(prev) = gs_stack.pop() {
                    gs = prev;
                }
            }
            b"cm" => {
                if let Some(m) = pop_matrix6(&operands) {
                    gs.ctm = mul_matrix(m, gs.ctm);
                }
            }
            b"BT" => {
                tm = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
                tlm = tm;
            }
            b"ET" => {}
            b"Tf" => {
                if operands.len() >= 2 {
                    if let Some(size) = operands.get(operands.len() - 1).and_then(PdfVal::as_f64) {
                        gs.font_size = size;
                    }
                    if let Some(name) = operands.get(operands.len() - 2).and_then(PdfVal::as_name) {
                        gs.font_key = Some(name.to_vec());
                    }
                }
            }
            b"Tc" => {
                if let Some(v) = last_f64(&operands) {
                    gs.char_space = v;
                }
            }
            b"Tw" => {
                if let Some(v) = last_f64(&operands) {
                    gs.word_space = v;
                }
            }
            b"Tz" => {
                if let Some(v) = last_f64(&operands).filter(|v| v.abs() > 0.01) {
                    gs.horiz_scale = v;
                }
            }
            b"TL" => {
                if let Some(v) = last_f64(&operands) {
                    gs.leading = v;
                }
            }
            b"Ts" => {
                if let Some(v) = last_f64(&operands) {
                    gs.rise = v;
                }
            }
            b"Tr" => {
                if let Some(v) = operands.last().and_then(PdfVal::as_int) {
                    gs.render_mode = v;
                }
            }
            b"Td" => {
                if let Some((tx, ty)) = last_two_f64(&operands) {
                    tlm = mul_matrix([1.0, 0.0, 0.0, 1.0, tx, ty], tlm);
                    tm = tlm;
                }
            }
            b"TD" => {
                if let Some((tx, ty)) = last_two_f64(&operands) {
                    gs.leading = -ty;
                    tlm = mul_matrix([1.0, 0.0, 0.0, 1.0, tx, ty], tlm);
                    tm = tlm;
                }
            }
            b"Tm" => {
                if let Some(m) = pop_matrix6(&operands) {
                    tm = m;
                    tlm = m;
                }
            }
            b"T*" => {
                tlm = mul_matrix([1.0, 0.0, 0.0, 1.0, 0.0, -gs.leading], tlm);
                tm = tlm;
            }
            b"Tj" => {
                if let Some(raw_str) = operands.last().and_then(PdfVal::as_bytes) {
                    let font = gs.font_key.as_ref().and_then(|k| fonts.get(k)).unwrap_or(&fallback_font);
                    emit_pdf_string(raw_str, font, &gs, &mut tm, media_box, rotate, out);
                }
            }
            b"'" => {
                tlm = mul_matrix([1.0, 0.0, 0.0, 1.0, 0.0, -gs.leading], tlm);
                tm = tlm;
                if let Some(raw_str) = operands.last().and_then(PdfVal::as_bytes) {
                    let font = gs.font_key.as_ref().and_then(|k| fonts.get(k)).unwrap_or(&fallback_font);
                    emit_pdf_string(raw_str, font, &gs, &mut tm, media_box, rotate, out);
                }
            }
            b"\"" => {
                if operands.len() >= 3 {
                    if let Some(aw) = operands.get(operands.len() - 3).and_then(PdfVal::as_f64) {
                        gs.word_space = aw;
                    }
                    if let Some(ac) = operands.get(operands.len() - 2).and_then(PdfVal::as_f64) {
                        gs.char_space = ac;
                    }
                    tlm = mul_matrix([1.0, 0.0, 0.0, 1.0, 0.0, -gs.leading], tlm);
                    tm = tlm;
                    if let Some(raw_str) = operands.last().and_then(PdfVal::as_bytes) {
                        let font = gs.font_key.as_ref().and_then(|k| fonts.get(k)).unwrap_or(&fallback_font);
                        emit_pdf_string(raw_str, font, &gs, &mut tm, media_box, rotate, out);
                    }
                }
            }
            b"TJ" => {
                if let Some(arr) = operands.last().and_then(PdfVal::as_array) {
                    let font = gs.font_key.as_ref().and_then(|k| fonts.get(k)).unwrap_or(&fallback_font);
                    let hscale = gs.horiz_scale / 100.0;
                    for item in arr {
                        match item {
                            PdfVal::Str(bytes_elem) => {
                                emit_pdf_string(bytes_elem, font, &gs, &mut tm, media_box, rotate, out);
                            }
                            PdfVal::Int(n) => {
                                let dx = -(*n as f64) * 0.001 * gs.font_size * hscale;
                                tm = mul_matrix([1.0, 0.0, 0.0, 1.0, dx, 0.0], tm);
                            }
                            PdfVal::Real(r) if r.is_finite() => {
                                let dx = -(*r) * 0.001 * gs.font_size * hscale;
                                tm = mul_matrix([1.0, 0.0, 0.0, 1.0, dx, 0.0], tm);
                            }
                            _ => {}
                        }
                    }
                }
            }
            b"g" => {
                if let Some(gray) = last_f64(&operands) {
                    let v = unit_to_u8(gray);
                    gs.fill_rgb = Rgb(v, v, v);
                }
            }
            b"rg" => {
                if let Some((r, g, b)) = last_three_f64(&operands) {
                    gs.fill_rgb = Rgb(unit_to_u8(r), unit_to_u8(g), unit_to_u8(b));
                }
            }
            b"k" => {
                if let Some((c, m, y, k)) = last_four_f64(&operands) {
                    gs.fill_rgb = cmyk_to_rgb(c, m, y, k);
                }
            }
            b"sc" | b"scn" => {
                let nums: Vec<f64> = operands.iter().filter_map(PdfVal::as_f64).collect();
                match nums.as_slice() {
                    [gray] => {
                        let v = unit_to_u8(*gray);
                        gs.fill_rgb = Rgb(v, v, v);
                    }
                    [r, g, b] => {
                        gs.fill_rgb = Rgb(unit_to_u8(*r), unit_to_u8(*g), unit_to_u8(*b));
                    }
                    [c, m, y, k] => {
                        gs.fill_rgb = cmyk_to_rgb(*c, *m, *y, *k);
                    }
                    _ => {}
                }
            }
            b"Do" => {
                if let Some(xobj_name) = operands.last().and_then(PdfVal::as_name)
                    && let Some(xobjects_val) = doc.resolve_dict_key(resources, b"XObject")
                    && let Some(xobjects) = xobjects_val.as_dict()
                    && let Some(target_ref) = xobjects.get(xobj_name)
                    && let Some(PdfVal::Stream { dict: xdict, raw: xraw }) = doc.resolve(target_ref, 0)
                    && xdict.get(b"Subtype".as_slice()).and_then(PdfVal::as_name) == Some(b"Form")
                    && let Ok(form_bytes) = doc.decode_stream_bytes(&xdict, &xraw)
                {
                    let mut child_gs = gs.clone();
                    if let Some(mat_val) = doc.resolve_dict_key(&xdict, b"Matrix")
                        && let Some(mat_arr) = mat_val.as_array()
                        && let Some(fm) = pop_matrix6(mat_arr)
                    {
                        child_gs.ctm = mul_matrix(fm, child_gs.ctm);
                    }
                    let mut child_res = resources.clone();
                    if let Some(fres_val) = doc.resolve_dict_key(&xdict, b"Resources")
                        && let Some(fres) = fres_val.as_dict()
                    {
                        merge_resources(&mut child_res, fres);
                    }
                    interpret_stream(doc, &form_bytes, &child_res, child_gs, media_box, rotate, form_depth + 1, out);
                }
            }
            _ => {}
        }
        operands.clear();
    }
}

fn emit_pdf_string(
    raw_bytes: &[u8],
    font: &PdfFont,
    gs: &GraphicsState,
    tm: &mut [f64; 6],
    media_box: [f64; 4],
    rotate: i32,
    out: &mut Vec<TextSpan>,
) {
    if raw_bytes.is_empty() || out.len() >= MAX_PAGE_SPANS {
        return;
    }
    let (decoded, adv_em, spaces) = font.decode_bytes(raw_bytes);
    let hscale = gs.horiz_scale / 100.0;
    let char_count = decoded.chars().count().max(1) as f64;
    let tx = (adv_em * gs.font_size + gs.char_space * char_count + gs.word_space * (spaces as f64)) * hscale;

    let start_trm = mul_matrix([1.0, 0.0, 0.0, 1.0, 0.0, gs.rise], mul_matrix(*tm, gs.ctm));
    *tm = mul_matrix([1.0, 0.0, 0.0, 1.0, tx, 0.0], *tm);
    let end_trm = mul_matrix([1.0, 0.0, 0.0, 1.0, 0.0, gs.rise], mul_matrix(*tm, gs.ctm));

    let cleaned = crate::model::clean_text(&decoded);
    if cleaned.is_empty() {
        return;
    }

    let x_vec_len = start_trm[0].hypot(start_trm[1]);
    let y_vec_len = start_trm[2].hypot(start_trm[3]);
    let scale_for_size = if x_vec_len > 1e-6 { x_vec_len } else { y_vec_len.max(1.0) };
    let eff_size = ((gs.font_size.abs() * scale_for_size) as f32).clamp(1.0, 400.0);

    // Detect synthetic oblique shear in `Tm * ctm` (e.g. `[1 0 -0.21 1 ...]`).
    let synthetic_italic = if x_vec_len > 1e-4 && y_vec_len > 1e-4 {
        let cos_angle = (start_trm[0] * start_trm[2] + start_trm[1] * start_trm[3]).abs() / (x_vec_len * y_vec_len);
        cos_angle >= 0.12 && cos_angle <= 0.85
    } else {
        false
    };
    let synthetic_bold = gs.render_mode == 2 || gs.render_mode == 6;

    let (sx, sy) = normalize_page_point(start_trm[4], start_trm[5], media_box, rotate);
    let (ex, _ey) = normalize_page_point(end_trm[4], end_trm[5], media_box, rotate);
    let left_x = sx.min(ex);
    let right_x = sx.max(ex).max(left_x + f64::from(eff_size) * 0.1);

    let raw_x0 = start_trm[4].min(end_trm[4]);
    let raw_x1 = start_trm[4].max(end_trm[4]);
    let raw_y0 = start_trm[5].min(end_trm[5]);
    let raw_y1 = start_trm[5].max(end_trm[5]) + f64::from(eff_size);

    out.push(TextSpan {
        x: left_x,
        end_x: right_x,
        y: sy,
        raw_bbox: [raw_x0, raw_y0, raw_x1, raw_y1],
        size: eff_size,
        font: font.family.clone(),
        bold: font.bold || synthetic_bold,
        italic: font.italic || synthetic_italic,
        color: gs.fill_rgb,
        invisible: gs.render_mode == 3 || gs.render_mode == 7,
        text: cleaned,
    });
}

fn normalize_page_point(x: f64, y: f64, mb: [f64; 4], rotate: i32) -> (f64, f64) {
    let x0 = mb[0].min(mb[2]);
    let y0 = mb[1].min(mb[3]);
    let w = (mb[2] - mb[0]).abs().max(1.0);
    let h = (mb[3] - mb[1]).abs().max(1.0);
    let rx = x - x0;
    let ry = y - y0;
    match rotate.rem_euclid(360) {
        90 => (h - ry, rx),
        180 => (w - rx, h - ry),
        270 => (ry, w - rx),
        _ => (rx, ry),
    }
}

fn skip_inline_image(cur: &mut Cursor<'_>) {
    // Advance past `ID` keyword + single whitespace byte, then scan until `EI` surrounded by whitespace/delims.
    while cur.pos < cur.bytes.len() {
        cur.skip_ws_and_comments();
        if cur.eat_keyword(b"ID") {
            if cur.peek().is_some_and(is_pdf_ws) {
                cur.pos += 1;
            }
            break;
        }
        if cur.parse_val(0).is_none() {
            cur.pos += 1;
        }
    }
    while cur.pos + 2 <= cur.bytes.len() {
        if cur.bytes.get(cur.pos..cur.pos + 2) == Some(b"EI")
            && (cur.pos == 0 || is_pdf_ws(cur.bytes[cur.pos - 1]))
            && cur.bytes.get(cur.pos + 2).is_none_or(|&b| is_ws_or_delim(b))
        {
            cur.pos += 2;
            return;
        }
        cur.pos += 1;
    }
    cur.pos = cur.bytes.len();
}

fn mul_matrix(a: [f64; 6], b: [f64; 6]) -> [f64; 6] {
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}

fn pop_matrix6(ops: &[PdfVal]) -> Option<[f64; 6]> {
    if ops.len() < 6 {
        return None;
    }
    let base = ops.len() - 6;
    Some([
        ops.get(base)?.as_f64()?,
        ops.get(base + 1)?.as_f64()?,
        ops.get(base + 2)?.as_f64()?,
        ops.get(base + 3)?.as_f64()?,
        ops.get(base + 4)?.as_f64()?,
        ops.get(base + 5)?.as_f64()?,
    ])
}

fn last_f64(ops: &[PdfVal]) -> Option<f64> {
    ops.last()?.as_f64()
}

fn last_two_f64(ops: &[PdfVal]) -> Option<(f64, f64)> {
    if ops.len() < 2 {
        return None;
    }
    Some((ops.get(ops.len() - 2)?.as_f64()?, ops.get(ops.len() - 1)?.as_f64()?))
}

fn last_three_f64(ops: &[PdfVal]) -> Option<(f64, f64, f64)> {
    if ops.len() < 3 {
        return None;
    }
    let b = ops.len() - 3;
    Some((ops.get(b)?.as_f64()?, ops.get(b + 1)?.as_f64()?, ops.get(b + 2)?.as_f64()?))
}

fn last_four_f64(ops: &[PdfVal]) -> Option<(f64, f64, f64, f64)> {
    if ops.len() < 4 {
        return None;
    }
    let b = ops.len() - 4;
    Some((ops.get(b)?.as_f64()?, ops.get(b + 1)?.as_f64()?, ops.get(b + 2)?.as_f64()?, ops.get(b + 3)?.as_f64()?))
}

fn unit_to_u8(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn cmyk_to_rgb(c: f64, m: f64, y: f64, k: f64) -> Rgb {
    let c = c.clamp(0.0, 1.0);
    let m = m.clamp(0.0, 1.0);
    let y = y.clamp(0.0, 1.0);
    let k = k.clamp(0.0, 1.0);
    Rgb(unit_to_u8((1.0 - c) * (1.0 - k)), unit_to_u8((1.0 - m) * (1.0 - k)), unit_to_u8((1.0 - y) * (1.0 - k)))
}
