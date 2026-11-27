//! Fill & Sign flat-form detector (`detect_flat_form_candidates`) and vector page stamper
//! (`stamp_flat_form`, `stamp_flat_form_items`) for non-interactive PDF forms.
//!
//! Detects flat horizontal underline segments (including segmented/dashed rules), rectangular
//! checkboxes, empty writing boxes, and multi-cell comb boxes on a PDF page, and stamps crisp
//! vector checkmarks (`✓`), crosses (`✗`), circles/ovals, comb-spaced characters, formatted dates,
//! and drawn or typed calligraphy signatures directly onto the page content stream.

use pdfcraft_cos::{Dict, Document, ObjRef, Object, Stream};
use pdfcraft_fonts::{helvetica_width, literal, script_outline, win_ansi};

use crate::{FormError, appearance, detect, fields, page_refs};

/// Resource name injected into a page's `/Resources /Font` when stamping flat text/comb/dates.
const FLAT_HELV_NAME: &[u8] = b"PCFlatHelv";

/// Kind of flat-form input region detected on a non-AcroForm page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlatCandidateKind {
    /// Horizontal underline blank or empty writing box.
    TextLine,
    /// Small square checkbox.
    CheckBox,
    /// Row of `cells` adjacent character boxes (postcode, IBAN, date digits, SSN…).
    Comb { cells: usize },
}

/// One flat-form region proposed by [`detect_flat_form_candidates`], in PDF user space (`[x0, y0, x1, y1]`).
#[derive(Clone, Debug, PartialEq)]
pub struct FlatFormCandidate {
    pub kind: FlatCandidateKind,
    pub rect: [f64; 4],
    pub label: String,
}

/// Vector Fill & Sign mark or text to stamp onto a PDF page in user space (`[x0, y0, x1, y1]`).
#[derive(Clone, Debug, PartialEq)]
pub enum FlatStamp {
    /// Vector checkmark (`✓`) fitted inside `rect`.
    Checkmark { rect: [f64; 4], color: [f64; 3], stroke_width: f64 },
    /// Vector cross (`✗`) fitted inside `rect`.
    Cross { rect: [f64; 4], color: [f64; 3], stroke_width: f64 },
    /// Smooth 4-segment cubic Bézier circle/oval around `rect`.
    Circle { rect: [f64; 4], color: [f64; 3], stroke_width: f64 },
    /// Comb-spaced characters centered one character per cell across `cells` equal columns of `rect`.
    CombText {
        rect: [f64; 4],
        cells: usize,
        text: String,
        color: [f64; 3],
        /// `0.0` selects automatic cell-fitted sizing.
        font_size: f64,
    },
    /// Formatted date (`YYYY-MM-DD` or comb digits when `comb_cells` is `Some(n)`).
    Date { rect: [f64; 4], year: i32, month: u8, day: u8, separator: char, comb_cells: Option<usize>, color: [f64; 3] },
    /// Freehand drawn signature strokes (`strokes` in normalized `0.0..=1.0` coordinates inside
    /// `rect`, or directly in user-space coordinates when points lie outside `0.0..=1.0`).
    DrawnSignature { rect: [f64; 4], strokes: Vec<Vec<[f64; 2]>>, color: [f64; 3], stroke_width: f64 },
    /// Typed calligraphy signature rendered as filled script outlines scaled into `rect`.
    TypedSignature { rect: [f64; 4], text: String, color: [f64; 3] },
}

/// Detect flat-form text underlines, checkboxes, and comb boxes on 0-based `page` of `doc`.
///
/// Existing interactive AcroForm widgets on `page` are excluded automatically so hybrid forms
/// never receive duplicate proposals.
pub fn detect_flat_form_candidates(doc: &Document, page: usize, words: &[detect::Word]) -> Vec<FlatFormCandidate> {
    let mut shapes = detect::page_shapes(doc, page);
    merge_collinear_rules(&mut shapes.rules);

    let all_fields = fields(doc);
    let existing: Vec<[f64; 4]> = all_fields.iter().flat_map(|f| &f.widgets).filter(|w| w.page == Some(page) && !w.hidden).map(|w| w.rect).collect();
    let taken: Vec<String> = all_fields.iter().map(|f| f.name.clone()).collect();

    detect::detect(words, &shapes, &existing, &taken)
        .into_iter()
        .map(|c| FlatFormCandidate {
            kind: match c.kind {
                detect::Kind::Text => FlatCandidateKind::TextLine,
                detect::Kind::CheckBox => FlatCandidateKind::CheckBox,
                detect::Kind::Comb { cells } => FlatCandidateKind::Comb { cells },
            },
            rect: c.rect,
            label: c.name,
        })
        .collect()
}

/// Stamp a single [`FlatStamp`] onto 0-based `page` of `doc`.
pub fn stamp_flat_form(doc: &mut Document, page: usize, stamp: &FlatStamp) -> Result<(), FormError> {
    stamp_flat_form_items(doc, page, std::slice::from_ref(stamp)).map(|_| ())
}

/// Stamp multiple [`FlatStamp`] items onto 0-based `page` of `doc` in a single appended stream.
/// Returns how many stamps emitted visible marks.
pub fn stamp_flat_form_items(doc: &mut Document, page: usize, stamps: &[FlatStamp]) -> Result<usize, FormError> {
    let pages = page_refs(doc);
    let page_ref = *pages.get(page).ok_or_else(|| FormError::Invalid(format!("page {} does not exist", page + 1)))?;

    let mut ops = String::new();
    let mut needs_font = false;
    let mut count = 0usize;

    for stamp in stamps {
        if let Some((snippet, uses_font)) = render_stamp_ops(stamp)? {
            ops.push_str(&snippet);
            needs_font |= uses_font;
            count += 1;
        }
    }
    if count == 0 {
        return Ok(0);
    }

    if needs_font {
        ensure_page_helvetica(doc, page_ref)?;
    }
    append_page_content(doc, page_ref, ops.as_bytes())?;
    Ok(count)
}

/// Coalesce collinear horizontal dash/dot segments (`y` within `0.5 pt`, gap `<= 4.5 pt`) so
/// dashed flat underlines are detected as continuous fill lines.
fn merge_collinear_rules(rules: &mut Vec<[f64; 4]>) {
    if rules.len() < 2 {
        return;
    }
    rules.sort_by(|a, b| a[1].total_cmp(&b[1]).then(a[0].total_cmp(&b[0])));
    let mut merged: Vec<[f64; 4]> = Vec::with_capacity(rules.len());
    for seg in rules.drain(..) {
        if let Some(last) = merged.last_mut()
            && (last[1] - seg[1]).abs() <= 0.5
            && seg[0] <= last[2] + 4.5
        {
            last[2] = last[2].max(seg[2]);
            continue;
        }
        merged.push(seg);
    }
    *rules = merged;
}

fn norm_rect(r: [f64; 4]) -> Result<[f64; 4], FormError> {
    if r.iter().any(|v| !v.is_finite()) {
        return Err(FormError::Invalid("stamp rectangle coordinates must be finite".into()));
    }
    let out = [r[0].min(r[2]), r[1].min(r[3]), r[0].max(r[2]), r[1].max(r[3])];
    if out[2] - out[0] < 1.0 || out[3] - out[1] < 1.0 {
        return Err(FormError::Invalid("stamp rectangle must be at least 1 × 1 pt".into()));
    }
    Ok(out)
}

fn clamp_rgb(c: [f64; 3]) -> [f64; 3] {
    c.map(|v| if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 })
}

fn n(v: f64) -> String {
    appearance::fmt(v)
}

fn render_stamp_ops(stamp: &FlatStamp) -> Result<Option<(String, bool)>, FormError> {
    match stamp {
        FlatStamp::Checkmark { rect, color, stroke_width } => {
            let [x0, y0, x1, y1] = norm_rect(*rect)?;
            let (w, h) = (x1 - x0, y1 - y0);
            let [r, g, b] = clamp_rgb(*color);
            let sw = if *stroke_width > 0.0 && stroke_width.is_finite() { stroke_width.clamp(0.5, 12.0) } else { (w.min(h) * 0.14).clamp(1.0, 3.5) };
            let p1 = [x0 + 0.18 * w, y0 + 0.48 * h];
            let p2 = [x0 + 0.42 * w, y0 + 0.20 * h];
            let p3 = [x0 + 0.84 * w, y0 + 0.80 * h];
            Ok(Some((
                format!(
                    "q\n1 J 1 j\n{} {} {} RG\n{} w\n{} {} m {} {} l {} {} l S\nQ\n",
                    n(r),
                    n(g),
                    n(b),
                    n(sw),
                    n(p1[0]),
                    n(p1[1]),
                    n(p2[0]),
                    n(p2[1]),
                    n(p3[0]),
                    n(p3[1])
                ),
                false,
            )))
        }
        FlatStamp::Cross { rect, color, stroke_width } => {
            let [x0, y0, x1, y1] = norm_rect(*rect)?;
            let (w, h) = (x1 - x0, y1 - y0);
            let [r, g, b] = clamp_rgb(*color);
            let sw = if *stroke_width > 0.0 && stroke_width.is_finite() { stroke_width.clamp(0.5, 12.0) } else { (w.min(h) * 0.13).clamp(1.0, 3.5) };
            let (ix0, iy0, ix1, iy1) = (x0 + 0.20 * w, y0 + 0.20 * h, x1 - 0.20 * w, y1 - 0.20 * h);
            Ok(Some((
                format!(
                    "q\n1 J\n{} {} {} RG\n{} w\n{} {} m {} {} l {} {} m {} {} l S\nQ\n",
                    n(r),
                    n(g),
                    n(b),
                    n(sw),
                    n(ix0),
                    n(iy0),
                    n(ix1),
                    n(iy1),
                    n(ix0),
                    n(iy1),
                    n(ix1),
                    n(iy0)
                ),
                false,
            )))
        }
        FlatStamp::Circle { rect, color, stroke_width } => {
            let [x0, y0, x1, y1] = norm_rect(*rect)?;
            let (cx, cy) = ((x0 + x1) * 0.5, (y0 + y1) * 0.5);
            let (rx, ry) = ((x1 - x0) * 0.46, (y1 - y0) * 0.46);
            let (kx, ky) = (0.55228475 * rx, 0.55228475 * ry);
            let [r, g, b] = clamp_rgb(*color);
            let sw =
                if *stroke_width > 0.0 && stroke_width.is_finite() { stroke_width.clamp(0.5, 12.0) } else { (rx.min(ry) * 0.22).clamp(1.0, 3.0) };
            Ok(Some((
                format!(
                    "q\n{} {} {} RG\n{} w\n{} {} m {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c h S\nQ\n",
                    n(r),
                    n(g),
                    n(b),
                    n(sw),
                    n(cx + rx),
                    n(cy),
                    n(cx + rx),
                    n(cy + ky),
                    n(cx + kx),
                    n(cy + ry),
                    n(cx),
                    n(cy + ry),
                    n(cx - kx),
                    n(cy + ry),
                    n(cx - rx),
                    n(cy + ky),
                    n(cx - rx),
                    n(cy),
                    n(cx - rx),
                    n(cy - ky),
                    n(cx - kx),
                    n(cy - ry),
                    n(cx),
                    n(cy - ry),
                    n(cx + kx),
                    n(cy - ry),
                    n(cx + rx),
                    n(cy - ky),
                    n(cx + rx),
                    n(cy),
                ),
                false,
            )))
        }
        FlatStamp::CombText { rect, cells, text, color, font_size } => render_comb_text(*rect, *cells, text, *color, *font_size),
        FlatStamp::Date { rect, year, month, day, separator, comb_cells, color } => {
            let y = (*year).clamp(1, 9999);
            let m = (*month).clamp(1, 12);
            let d = (*day).clamp(1, 31);
            if let Some(cells) = comb_cells {
                let digits = if *cells >= 10 { format!("{y:04}{separator}{m:02}{separator}{d:02}") } else { format!("{y:04}{m:02}{d:02}") };
                return render_comb_text(*rect, *cells, &digits, *color, 0.0);
            }
            let formatted = format!("{y:04}{separator}{m:02}{separator}{d:02}");
            render_single_line_text(*rect, &formatted, *color)
        }
        FlatStamp::DrawnSignature { rect, strokes, color, stroke_width } => render_drawn_signature(*rect, strokes, *color, *stroke_width),
        FlatStamp::TypedSignature { rect, text, color } => render_typed_signature(*rect, text, *color),
    }
}

fn render_comb_text(rect: [f64; 4], cells: usize, text: &str, color: [f64; 3], font_size: f64) -> Result<Option<(String, bool)>, FormError> {
    let [x0, y0, x1, y1] = norm_rect(rect)?;
    let cells = cells.clamp(1, 256);
    if text.is_empty() {
        return Ok(None);
    }
    let (w, h) = (x1 - x0, y1 - y0);
    let cell_w = w / cells as f64;
    let size = if font_size > 0.0 && font_size.is_finite() {
        font_size.clamp(4.0, 72.0)
    } else {
        appearance::comb_auto_text_size((h - 2.0).max(4.0)).min(cell_w * 1.1)
    };
    let baseline = y0 + (h - size * 0.72).max(1.5) * 0.5;
    let [r, g, b] = clamp_rgb(color);
    let mut s = format!("q\nBT\n/PCFlatHelv {} Tf\n{} {} {} rg\n", n(size), n(r), n(g), n(b));
    for (idx, ch) in text.chars().take(cells).enumerate() {
        let mut ch_buf = [0u8; 4];
        let ch_str = ch.encode_utf8(&mut ch_buf);
        let encoded = win_ansi(ch_str);
        let adv = helvetica_width(ch_str, size);
        let cx = x0 + (idx as f64 + 0.5) * cell_w - adv * 0.5;
        let lit = String::from_utf8_lossy(&literal(&encoded)).into_owned();
        s.push_str(&format!("1 0 0 1 {} {} Tm\n{} Tj\n", n(cx), n(baseline), lit));
    }
    s.push_str("ET\nQ\n");
    Ok(Some((s, true)))
}

fn render_drawn_signature(
    rect: [f64; 4],
    strokes: &[Vec<[f64; 2]>],
    color: [f64; 3],
    stroke_width: f64,
) -> Result<Option<(String, bool)>, FormError> {
    let [x0, y0, x1, y1] = norm_rect(rect)?;
    let (w, h) = (x1 - x0, y1 - y0);
    let [r, g, b] = clamp_rgb(color);
    let sw = if stroke_width > 0.0 && stroke_width.is_finite() { stroke_width.clamp(0.4, 10.0) } else { 1.6 };
    let normalized = strokes.iter().flatten().all(|p| (0.0..=1.0).contains(&p[0]) && (0.0..=1.0).contains(&p[1]));
    let mut s = format!("q\n1 J 1 j\n{} {} {} RG\n{} w\n", n(r), n(g), n(b), n(sw));
    let mut any_seg = false;
    for stroke in strokes.iter().take(512) {
        let mut pts = stroke.iter().take(4096).filter(|p| p[0].is_finite() && p[1].is_finite());
        let Some(first) = pts.next() else { continue };
        let map = |p: &[f64; 2]| -> [f64; 2] { if normalized { [x0 + p[0] * w, y0 + p[1] * h] } else { *p } };
        let p0 = map(first);
        s.push_str(&format!("{} {} m\n", n(p0[0]), n(p0[1])));
        for p in pts {
            let q = map(p);
            s.push_str(&format!("{} {} l\n", n(q[0]), n(q[1])));
            any_seg = true;
        }
    }
    if !any_seg {
        return Ok(None);
    }
    s.push_str("S\nQ\n");
    Ok(Some((s, false)))
}

fn render_typed_signature(rect: [f64; 4], text: &str, color: [f64; 3]) -> Result<Option<(String, bool)>, FormError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let [x0, y0, x1, y1] = norm_rect(rect)?;
    let (w, h) = (x1 - x0, y1 - y0);
    let outline = script_outline(trimmed);
    let [ox0, oy0, ox1, oy1] = outline.bounds();
    let (ow, oh) = (ox1 - ox0, oy1 - oy0);
    if outline.contours.is_empty() || ow <= 0.01 || oh <= 0.01 {
        return render_single_line_text(rect, trimmed, color);
    }
    let scale = ((w * 0.92) / ow).min((h * 0.86) / oh);
    let tx = x0 + (w - ow * scale) * 0.5 - ox0 * scale;
    let ty = y0 + (h - oh * scale) * 0.5 - oy0 * scale;
    let [r, g, b] = clamp_rgb(color);
    let mut s = format!("q\n{} {} {} rg\n", n(r), n(g), n(b));
    for contour in &outline.contours {
        let Some(first) = contour.first() else { continue };
        s.push_str(&format!("{} {} m\n", n(tx + first[0] * scale), n(ty + first[1] * scale)));
        for pt in contour.iter().skip(1) {
            s.push_str(&format!("{} {} l\n", n(tx + pt[0] * scale), n(ty + pt[1] * scale)));
        }
        s.push_str("h\n");
    }
    s.push_str("f\nQ\n");
    Ok(Some((s, false)))
}

fn render_single_line_text(rect: [f64; 4], text: &str, color: [f64; 3]) -> Result<Option<(String, bool)>, FormError> {
    if text.is_empty() {
        return Ok(None);
    }
    let [x0, y0, x1, y1] = norm_rect(rect)?;
    let (w, h) = (x1 - x0, y1 - y0);
    let size = appearance::auto_text_size(text, (w - 4.0).max(4.0), (h - 2.0).max(4.0), false, helvetica_width).min(14.0);
    let baseline = y0 + (h - size * 0.72).max(1.5) * 0.5;
    let [r, g, b] = clamp_rgb(color);
    let encoded = win_ansi(text);
    let lit = String::from_utf8_lossy(&literal(&encoded)).into_owned();
    Ok(Some((
        format!("q\nBT\n/PCFlatHelv {} Tf\n{} {} {} rg\n1 0 0 1 {} {} Tm\n{} Tj\nET\nQ\n", n(size), n(r), n(g), n(b), n(x0 + 2.0), n(baseline), lit),
        true,
    )))
}

fn ensure_page_helvetica(doc: &mut Document, page_ref: ObjRef) -> Result<(), FormError> {
    let page_dict = doc.get(page_ref).as_dict().cloned().ok_or_else(|| FormError::Invalid("page object is not a dictionary".into()))?;
    let mut res = page_dict.get(b"Resources").map(|o| doc.resolve(o)).and_then(|o| o.as_dict().cloned()).unwrap_or_default();
    let mut fonts = res.get(b"Font").map(|o| doc.resolve(o)).and_then(|o| o.as_dict().cloned()).unwrap_or_default();

    if !fonts.contains(FLAT_HELV_NAME) {
        let mut f = Dict::new();
        f.set(b"Type".to_vec(), Object::name("Font"));
        f.set(b"Subtype".to_vec(), Object::name("Type1"));
        f.set(b"BaseFont".to_vec(), Object::name("Helvetica"));
        f.set(b"Encoding".to_vec(), Object::name("WinAnsiEncoding"));
        let font_ref = doc.add(Object::Dict(f));
        fonts.set(FLAT_HELV_NAME.to_vec(), Object::Ref(font_ref));
        res.set(b"Font".to_vec(), Object::Dict(fonts));
        doc.update_dict(page_ref, |d| d.set(b"Resources".to_vec(), Object::Dict(res)))?;
    }
    Ok(())
}

fn append_page_content(doc: &mut Document, page_ref: ObjRef, data: &[u8]) -> Result<(), FormError> {
    let page_dict = doc.get(page_ref).as_dict().cloned().ok_or_else(|| FormError::Invalid("page object is not a dictionary".into()))?;

    let open_q = doc.add(Object::Stream(Stream::flate(Dict::new(), b"q\n")));
    let close_q = doc.add(Object::Stream(Stream::flate(Dict::new(), b"\nQ\n")));
    let appended = doc.add(Object::Stream(Stream::flate(Dict::new(), data)));

    let mut arr = Vec::new();
    match page_dict.get(b"Contents") {
        Some(Object::Array(existing)) if !existing.is_empty() => {
            arr.push(Object::Ref(open_q));
            arr.extend(existing.iter().cloned());
            arr.push(Object::Ref(close_q));
        }
        Some(single) if !matches!(single, Object::Null) => {
            arr.push(Object::Ref(open_q));
            arr.push(single.clone());
            arr.push(Object::Ref(close_q));
        }
        _ => {}
    }
    arr.push(Object::Ref(appended));
    doc.update_dict(page_ref, |d| d.set(b"Contents".to_vec(), Object::Array(arr)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdfcraft_cos::DecodeBudget;

    fn make_flat_form_doc() -> Document {
        let mut doc = Document::new_empty();
        // Draw:
        // 1) A flat underline at y=700 from x=120..320 (200 pt wide)
        // 2) A dashed/segmented underline at y=650 (40 segments of 4pt separated by 2pt)
        // 3) A 12x12 checkbox at (72, 600)
        // 4) A 4-cell comb row at y=540: (120..136), (136..152), (152..168), (168..184)
        let mut stream = String::from("q\n1 w\n120 700 m 320 700 l S\n");
        for i in 0..20 {
            let x = 120.0 + f64::from(i) * 6.0;
            stream.push_str(&format!("{x} 650 m {} 650 l S\n", x + 4.0));
        }
        stream.push_str("72 600 12 12 re S\n");
        for i in 0..4 {
            let x = 120.0 + f64::from(i) * 16.0;
            stream.push_str(&format!("{x} 540 16 20 re S\n"));
        }
        stream.push_str("Q\n");

        let cs = doc.add(Object::Stream(Stream::flate(Dict::new(), stream.as_bytes())));
        let pages_id = doc.add(Object::Null);
        let mut p = Dict::new();
        p.set(b"Type".to_vec(), Object::name("Page"));
        p.set(b"Parent".to_vec(), Object::Ref(pages_id));
        p.set(b"MediaBox".to_vec(), Object::Array(vec![0.into(), 0.into(), 612.into(), 792.into()]));
        p.set(b"Contents".to_vec(), Object::Ref(cs));
        let p_id = doc.add(Object::Dict(p));

        let mut pages = Dict::new();
        pages.set(b"Type".to_vec(), Object::name("Pages"));
        pages.set(b"Kids".to_vec(), Object::Array(vec![Object::Ref(p_id)]));
        pages.set(b"Count".to_vec(), Object::Int(1));
        doc.set(pages_id, Object::Dict(pages));

        let mut cat = Dict::new();
        cat.set(b"Type".to_vec(), Object::name("Catalog"));
        cat.set(b"Pages".to_vec(), Object::Ref(pages_id));
        let cat_id = doc.add(Object::Dict(cat));
        doc.trailer_mut().set(b"Root".to_vec(), Object::Ref(cat_id));
        doc
    }

    #[test]
    fn detects_flat_underlines_checkboxes_and_combs_and_stamps_all_marks() {
        let mut doc = make_flat_form_doc();
        let words = vec![
            detect::Word { text: "Full Name:".into(), rect: [40.0, 702.0, 110.0, 714.0] },
            detect::Word { text: "Email:".into(), rect: [40.0, 652.0, 90.0, 664.0] },
            detect::Word { text: "Agree to terms".into(), rect: [90.0, 601.0, 180.0, 613.0] },
            detect::Word { text: "Year:".into(), rect: [72.0, 544.0, 112.0, 556.0] },
        ];

        let cands = detect_flat_form_candidates(&doc, 0, &words);
        assert!(cands.iter().any(|c| c.kind == FlatCandidateKind::TextLine && c.label == "Full Name"));
        assert!(cands.iter().any(|c| c.kind == FlatCandidateKind::TextLine && c.label == "Email"));
        assert!(cands.iter().any(|c| c.kind == FlatCandidateKind::CheckBox && c.label == "Agree to terms"));
        assert!(cands.iter().any(|c| c.kind == FlatCandidateKind::Comb { cells: 4 } && c.label == "Year"));

        let stamps = vec![
            FlatStamp::Checkmark { rect: [72.0, 600.0, 84.0, 612.0], color: [0.0, 0.4, 0.1], stroke_width: 1.5 },
            FlatStamp::Cross { rect: [100.0, 600.0, 112.0, 612.0], color: [0.8, 0.0, 0.0], stroke_width: 1.5 },
            FlatStamp::Circle { rect: [130.0, 598.0, 154.0, 614.0], color: [0.1, 0.2, 0.8], stroke_width: 1.2 },
            FlatStamp::CombText { rect: [120.0, 540.0, 184.0, 560.0], cells: 4, text: "2027".into(), color: [0.0; 3], font_size: 0.0 },
            FlatStamp::Date { rect: [120.0, 701.0, 220.0, 717.0], year: 2026, month: 11, day: 27, separator: '-', comb_cells: None, color: [0.0; 3] },
            FlatStamp::DrawnSignature {
                rect: [120.0, 460.0, 260.0, 500.0],
                strokes: vec![vec![[0.05, 0.2], [0.35, 0.85], [0.65, 0.3], [0.95, 0.6]]],
                color: [0.05, 0.1, 0.45],
                stroke_width: 1.8,
            },
            FlatStamp::TypedSignature { rect: [280.0, 460.0, 440.0, 500.0], text: "Jane Roe".into(), color: [0.0, 0.0, 0.0] },
        ];

        let count = stamp_flat_form_items(&mut doc, 0, &stamps).unwrap();
        assert_eq!(count, 7);

        // Verify the appended content stream decodes cleanly and carries font + vector operators.
        let p = pdfcraft_model::pages(&doc).into_iter().next().unwrap();
        let arr = p.dict.get(b"Contents").unwrap().as_array().unwrap();
        let last_ref = arr.last().unwrap().as_ref().unwrap();
        let resolved = doc.get(last_ref);
        let Object::Stream(st) = &*resolved else { panic!("expected stream") };
        let decoded = DecodeBudget::page().decode(st).unwrap();
        let text = String::from_utf8_lossy(&decoded);
        assert!(text.contains("/PCFlatHelv"));
        assert!(text.contains("(2) Tj"));
        assert!(text.contains("(2026-11-27) Tj"));
        assert!(text.contains("c h S"));
    }
}
