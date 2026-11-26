//! Style-definition tracked changes (`w:pPrChange` and `w:rPrChange` inside `word/styles.xml`).
//!
//! `craft-wordmodel` does not carry `Style::para_change` or `Style::chr_change` directly yet, so
//! this module injects ECMA-376 `w:pPrChange` / `w:rPrChange` elements into `word/styles.xml` on
//! write and reconstructs them via `craft-docx`'s own style parser on read (preserving heading
//! outline-level defaults and `basedOn` list-level inheritance).

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use std::sync::Arc;

use craft_docx::DocxError;
use quick_xml::events::{BytesEnd, Event};
use quick_xml::{Reader, Writer};
use wordcraft_doc::props::PropChange;
use wordcraft_doc::{Document, Revision, RevisionKind};

const STYLES_PART: &str = "word/styles.xml";
const DOCUMENT_PART: &str = "word/document.xml";
const MINIMAL_DOCUMENT_XML: &[u8] =
    br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p/></w:body></w:document>"#;

#[derive(Clone, Debug, PartialEq, Eq)]
struct RawRev {
    id: Option<String>,
    author: String,
    date: String,
}

#[derive(Clone, Debug, Default)]
struct ExtractedChanges {
    ppr: Option<(RawRev, Vec<u8>)>,
    rpr: Option<(RawRev, Vec<u8>)>,
}

#[derive(Clone, Debug, Default)]
struct StyleChangeXml {
    ppr_change: Option<String>,
    rpr_change: Option<String>,
}

/// Injects `<w:pPrChange>` and `<w:rPrChange>` into `word/styles.xml` when any style in `doc`
/// carries `para_change` or `chr_change`.
pub(crate) fn apply_on_write(doc: &Document, docx_bytes: Vec<u8>) -> Result<Vec<u8>, DocxError> {
    if !doc.styles.styles.iter().any(|s| s.para_change.is_some() || s.chr_change.is_some()) {
        return Ok(docx_bytes);
    }
    let Some(styles_bytes) = read_zip_part(&docx_bytes, STYLES_PART) else {
        return Ok(docx_bytes);
    };
    let change_map = build_change_elements(doc)?;
    if change_map.is_empty() {
        return Ok(docx_bytes);
    }
    let updated_styles = inject_changes_into_styles(&styles_bytes, &change_map);
    replace_zip_parts(&docx_bytes, &[(STYLES_PART, &updated_styles)])
}

/// Reads `<w:pPrChange>` and `<w:rPrChange>` from `word/styles.xml` into `doc.styles` and
/// `doc.revisions`.
pub(crate) fn apply_on_read(bytes: &[u8], password: Option<&str>, doc: &mut Document) {
    let decrypted;
    let pkg_bytes: &[u8] = if crate::is_encrypted(bytes) {
        let Ok(plain) = crate::decrypt(bytes, password) else {
            return;
        };
        decrypted = plain;
        &decrypted
    } else {
        bytes
    };
    let Some(styles_bytes) = read_zip_part(pkg_bytes, STYLES_PART) else {
        return;
    };
    let Ok(styles_str) = std::str::from_utf8(&styles_bytes) else {
        return;
    };
    if !styles_str.contains("PrChange") {
        return;
    }
    let (order, extracted) = extract_changes_from_styles(&styles_bytes);
    if extracted.is_empty() {
        return;
    }
    let old_styles_xml = build_old_styles_xml(&styles_bytes, &extracted);
    let Ok(mini_docx) = replace_zip_parts(pkg_bytes, &[(STYLES_PART, &old_styles_xml), (DOCUMENT_PART, MINIMAL_DOCUMENT_XML)]) else {
        return;
    };
    let Ok(shared_old) = craft_docx::read(&mini_docx) else {
        return;
    };
    let Ok(old_doc) = crate::from_shared_model(shared_old) else {
        return;
    };
    attach_parsed_changes(doc, &old_doc, &order, &extracted);
}

fn attach_parsed_changes(doc: &mut Document, old_doc: &Document, order: &[String], extracted: &BTreeMap<String, ExtractedChanges>) {
    let sheet = Arc::make_mut(&mut doc.styles);
    for style_id in order {
        let (Some(ext), Some(old_st)) = (extracted.get(style_id), old_doc.styles.get(style_id)) else {
            continue;
        };
        let Some(target_st) = sheet.styles.iter_mut().find(|s| s.id == *style_id) else {
            continue;
        };
        let mut ppr_rev_idx = None;
        if let Some((rev, _)) = &ext.ppr {
            let idx = push_revision(&mut doc.revisions, rev);
            ppr_rev_idx = Some((rev, idx));
            target_st.para_change = PropChange::boxed(idx, old_st.para.clone());
        }
        if let Some((rev, _)) = &ext.rpr {
            let idx = match ppr_rev_idx {
                Some((prev_rev, prev_idx)) if prev_rev == rev => prev_idx,
                _ => push_revision(&mut doc.revisions, rev),
            };
            target_st.chr_change = PropChange::boxed(idx, old_st.chr.clone());
        }
    }
}

fn push_revision(revisions: &mut Vec<Revision>, raw: &RawRev) -> u32 {
    let idx = u32::try_from(revisions.len()).unwrap_or(u32::MAX);
    revisions.push(Revision { kind: RevisionKind::Format, author: raw.author.clone(), date: raw.date.clone(), move_name: None });
    idx
}

fn build_change_elements(doc: &Document) -> Result<BTreeMap<String, StyleChangeXml>, DocxError> {
    let mut helper = Document::new();
    helper.numbering = doc.numbering.clone();
    let mut old_sheet = (*doc.styles).clone();
    for st in &mut old_sheet.styles {
        if let Some(pc) = st.para_change.take() {
            st.para = pc.old;
        }
        if let Some(cc) = st.chr_change.take() {
            st.chr = cc.old;
        }
    }
    helper.styles = Arc::new(old_sheet);
    let helper_bytes = craft_docx::write(&crate::to_shared_model(&helper)?)?;
    let old_props = read_zip_part(&helper_bytes, STYLES_PART).map(|b| extract_direct_style_props(&b)).unwrap_or_default();

    let mut out = BTreeMap::new();
    for st in &doc.styles.styles {
        if st.para_change.is_none() && st.chr_change.is_none() {
            continue;
        }
        let (old_ppr, old_rpr) = old_props.get(&st.id).cloned().unwrap_or_default();
        let ppr_change = st.para_change.as_ref().map(|pc| format_change_element("w:pPrChange", "w:pPr", doc, pc.rev, &old_ppr));
        let rpr_change = st.chr_change.as_ref().map(|cc| format_change_element("w:rPrChange", "w:rPr", doc, cc.rev, &old_rpr));
        out.insert(st.id.clone(), StyleChangeXml { ppr_change, rpr_change });
    }
    Ok(out)
}

fn format_change_element(tag: &str, inner_tag: &str, doc: &Document, rev_idx: u32, inner_xml: &str) -> String {
    let rev = doc.revisions.get(rev_idx as usize);
    let id = rev_idx.saturating_add(1);
    let author = rev.map(|r| quick_xml::escape::escape(&r.author)).unwrap_or_default();
    let date_attr = match rev.filter(|r| !r.date.is_empty()) {
        Some(r) => format!(r#" w:date="{}""#, quick_xml::escape::escape(&r.date)),
        None => String::new(),
    };
    if inner_xml.is_empty() {
        format!(r#"<{tag} w:id="{id}" w:author="{author}"{date_attr}><{inner_tag}/></{tag}>"#)
    } else {
        format!(r#"<{tag} w:id="{id}" w:author="{author}"{date_attr}><{inner_tag}>{inner_xml}</{inner_tag}></{tag}>"#)
    }
}

/// Extracts `(pPr_inner_xml, rPr_inner_xml)` for each `<w:style>`'s direct `<w:pPr>` and `<w:rPr>`.
fn extract_direct_style_props(styles_bytes: &[u8]) -> BTreeMap<String, (String, String)> {
    let mut reader = Reader::from_reader(styles_bytes);
    let mut buf = Vec::new();
    let mut depth = 0usize;
    let mut cur_style: Option<String> = None;
    let mut out = BTreeMap::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let local = e.local_name();
                if depth == 1 && local.as_ref() == b"style" {
                    cur_style = attr_value(&e, b"styleId");
                    depth += 1;
                } else if depth == 2 && cur_style.is_some() && (local.as_ref() == b"pPr" || local.as_ref() == b"rPr") {
                    let is_ppr = local.as_ref() == b"pPr";
                    let inner = capture_element_children(&mut reader, local.as_ref());
                    if let Some(id) = &cur_style {
                        let entry = out.entry(id.clone()).or_insert_with(|| (String::new(), String::new()));
                        let text = String::from_utf8_lossy(&inner).into_owned();
                        if is_ppr {
                            entry.0 = text;
                        } else {
                            entry.1 = text;
                        }
                    }
                } else {
                    depth += 1;
                }
            }
            Ok(Event::End(_)) => {
                if depth == 2 {
                    cur_style = None;
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

fn capture_element_children(reader: &mut Reader<&[u8]>, end_local: &[u8]) -> Vec<u8> {
    let mut writer = Writer::new(Vec::new());
    let mut buf = Vec::new();
    let mut depth = 1usize;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                depth += 1;
                let _ = writer.write_event(Event::Start(e));
            }
            Ok(Event::End(e)) => {
                depth -= 1;
                if depth == 0 && e.local_name().as_ref() == end_local {
                    break;
                }
                let _ = writer.write_event(Event::End(e));
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(other) => {
                let _ = writer.write_event(other);
            }
        }
        buf.clear();
    }
    writer.into_inner()
}

fn inject_changes_into_styles(styles_bytes: &[u8], changes: &BTreeMap<String, StyleChangeXml>) -> Vec<u8> {
    let mut reader = Reader::from_reader(styles_bytes);
    let mut writer = Writer::new(Vec::with_capacity(styles_bytes.len() + 256));
    let mut buf = Vec::new();
    let mut depth = 0usize;
    let mut pending_ppr: Option<&str> = None;
    let mut pending_rpr: Option<&str> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let local_buf = e.local_name().as_ref().to_vec();
                handle_inject_start(&mut writer, e, &local_buf, depth, changes, &mut pending_ppr, &mut pending_rpr);
                depth += 1;
            }
            Ok(Event::Empty(e)) => {
                let local_buf = e.local_name().as_ref().to_vec();
                handle_inject_empty(&mut writer, e, &local_buf, depth, changes, &mut pending_ppr, &mut pending_rpr);
            }
            Ok(Event::End(e)) => {
                let local_buf = e.local_name().as_ref().to_vec();
                handle_inject_end(&mut writer, e, &local_buf, depth, &mut pending_ppr, &mut pending_rpr);
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(other) => {
                let _ = writer.write_event(other);
            }
        }
        buf.clear();
    }
    writer.into_inner()
}

fn handle_inject_start<'a>(
    writer: &mut Writer<Vec<u8>>,
    e: quick_xml::events::BytesStart<'_>,
    local: &[u8],
    depth: usize,
    changes: &'a BTreeMap<String, StyleChangeXml>,
    pending_ppr: &mut Option<&'a str>,
    pending_rpr: &mut Option<&'a str>,
) {
    if depth == 1 && local == b"style" {
        if let Some(ch) = attr_value(&e, b"styleId").and_then(|id| changes.get(&id)) {
            *pending_ppr = ch.ppr_change.as_deref();
            *pending_rpr = ch.rpr_change.as_deref();
        }
    } else if depth == 2 {
        if local == b"rPr" {
            flush_wrapped(writer, "w:pPr", pending_ppr);
        } else if is_post_rpr_style_child(local) {
            flush_wrapped(writer, "w:pPr", pending_ppr);
            flush_wrapped(writer, "w:rPr", pending_rpr);
        }
    }
    let _ = writer.write_event(Event::Start(e));
}

fn handle_inject_empty<'a>(
    writer: &mut Writer<Vec<u8>>,
    e: quick_xml::events::BytesStart<'_>,
    local: &[u8],
    depth: usize,
    changes: &'a BTreeMap<String, StyleChangeXml>,
    pending_ppr: &mut Option<&'a str>,
    pending_rpr: &mut Option<&'a str>,
) {
    if depth == 1 && local == b"style" {
        if let Some(ch) = attr_value(&e, b"styleId").and_then(|id| changes.get(&id)) {
            let tag_name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
            let _ = writer.write_event(Event::Start(e));
            let mut p = ch.ppr_change.as_deref();
            let mut r = ch.rpr_change.as_deref();
            flush_wrapped(writer, "w:pPr", &mut p);
            flush_wrapped(writer, "w:rPr", &mut r);
            let _ = writer.write_event(Event::End(BytesEnd::new(tag_name)));
            return;
        }
    } else if depth == 2 {
        if local == b"pPr"
            && let Some(xml) = pending_ppr.take()
        {
            expand_empty_with_raw(writer, e, xml.as_bytes());
            return;
        }
        if local == b"rPr" {
            flush_wrapped(writer, "w:pPr", pending_ppr);
            if let Some(xml) = pending_rpr.take() {
                expand_empty_with_raw(writer, e, xml.as_bytes());
                return;
            }
        } else if is_post_rpr_style_child(local) {
            flush_wrapped(writer, "w:pPr", pending_ppr);
            flush_wrapped(writer, "w:rPr", pending_rpr);
        }
    }
    let _ = writer.write_event(Event::Empty(e));
}

fn handle_inject_end(
    writer: &mut Writer<Vec<u8>>,
    e: BytesEnd<'_>,
    local: &[u8],
    depth: usize,
    pending_ppr: &mut Option<&str>,
    pending_rpr: &mut Option<&str>,
) {
    if depth == 3 {
        if local == b"pPr"
            && let Some(xml) = pending_ppr.take()
        {
            writer.get_mut().extend_from_slice(xml.as_bytes());
        } else if local == b"rPr"
            && let Some(xml) = pending_rpr.take()
        {
            writer.get_mut().extend_from_slice(xml.as_bytes());
        }
    } else if depth == 2 && local == b"style" {
        flush_wrapped(writer, "w:pPr", pending_ppr);
        flush_wrapped(writer, "w:rPr", pending_rpr);
    }
    let _ = writer.write_event(Event::End(e));
}

fn expand_empty_with_raw(writer: &mut Writer<Vec<u8>>, e: quick_xml::events::BytesStart<'_>, raw: &[u8]) {
    let tag_name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
    let _ = writer.write_event(Event::Start(e));
    writer.get_mut().extend_from_slice(raw);
    let _ = writer.write_event(Event::End(BytesEnd::new(tag_name)));
}

fn flush_wrapped(writer: &mut Writer<Vec<u8>>, wrapper: &str, pending: &mut Option<&str>) {
    if let Some(xml) = pending.take() {
        let out = writer.get_mut();
        out.extend_from_slice(b"<");
        out.extend_from_slice(wrapper.as_bytes());
        out.extend_from_slice(b">");
        out.extend_from_slice(xml.as_bytes());
        out.extend_from_slice(b"</");
        out.extend_from_slice(wrapper.as_bytes());
        out.extend_from_slice(b">");
    }
}

fn is_post_rpr_style_child(local: &[u8]) -> bool {
    matches!(local, b"tblPr" | b"trPr" | b"tcPr" | b"tblStylePr")
}

/// Pass 1 on read: scans `word/styles.xml` for `<w:pPrChange>` / `<w:rPrChange>` inside direct
/// `<w:pPr>` / `<w:rPr>` children of `<w:style>`.
fn extract_changes_from_styles(styles_bytes: &[u8]) -> (Vec<String>, BTreeMap<String, ExtractedChanges>) {
    let mut reader = Reader::from_reader(styles_bytes);
    let mut buf = Vec::new();
    let mut depth = 0usize;
    let mut cur_style: Option<String> = None;
    let mut direct_prop: Option<bool> = None; // Some(true) = pPr, Some(false) = rPr
    let mut order = Vec::new();
    let mut out: BTreeMap<String, ExtractedChanges> = BTreeMap::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let local = e.local_name();
                if depth == 1 && local.as_ref() == b"style" {
                    cur_style = attr_value(&e, b"styleId");
                    depth += 1;
                } else if depth == 2 && (local.as_ref() == b"pPr" || local.as_ref() == b"rPr") {
                    direct_prop = Some(local.as_ref() == b"pPr");
                    depth += 1;
                } else if depth == 3 && direct_prop == Some(true) && local.as_ref() == b"pPrChange" {
                    let rev = parse_raw_rev(&e);
                    let inner = read_change_inner_xml(&mut reader, b"pPrChange", b"pPr");
                    record_extracted(&mut order, &mut out, cur_style.as_deref(), true, rev, inner);
                } else if depth == 3 && direct_prop == Some(false) && local.as_ref() == b"rPrChange" {
                    let rev = parse_raw_rev(&e);
                    let inner = read_change_inner_xml(&mut reader, b"rPrChange", b"rPr");
                    record_extracted(&mut order, &mut out, cur_style.as_deref(), false, rev, inner);
                } else {
                    depth += 1;
                }
            }
            Ok(Event::Empty(e)) => {
                let local = e.local_name();
                if depth == 3 && direct_prop == Some(true) && local.as_ref() == b"pPrChange" {
                    record_extracted(&mut order, &mut out, cur_style.as_deref(), true, parse_raw_rev(&e), Vec::new());
                } else if depth == 3 && direct_prop == Some(false) && local.as_ref() == b"rPrChange" {
                    record_extracted(&mut order, &mut out, cur_style.as_deref(), false, parse_raw_rev(&e), Vec::new());
                }
            }
            Ok(Event::End(_)) => {
                if depth == 3 {
                    direct_prop = None;
                } else if depth == 2 {
                    cur_style = None;
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    (order, out)
}

fn record_extracted(
    order: &mut Vec<String>,
    out: &mut BTreeMap<String, ExtractedChanges>,
    style_id: Option<&str>,
    is_ppr: bool,
    rev: RawRev,
    inner: Vec<u8>,
) {
    let Some(id) = style_id else {
        return;
    };
    if !out.contains_key(id) {
        order.push(id.to_string());
    }
    let entry = out.entry(id.to_string()).or_default();
    if is_ppr {
        entry.ppr = Some((rev, inner));
    } else {
        entry.rpr = Some((rev, inner));
    }
}

fn read_change_inner_xml(reader: &mut Reader<&[u8]>, change_local: &[u8], inner_local: &[u8]) -> Vec<u8> {
    let mut writer = Writer::new(Vec::new());
    let mut buf = Vec::new();
    let mut change_depth = 1usize;
    let mut inner_depth = 0usize;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                change_depth += 1;
                if inner_depth == 0 && e.local_name().as_ref() == inner_local {
                    inner_depth = 1;
                } else if inner_depth >= 1 {
                    inner_depth += 1;
                    let _ = writer.write_event(Event::Start(e));
                }
            }
            Ok(Event::Empty(e)) => {
                if inner_depth >= 1 {
                    let _ = writer.write_event(Event::Empty(e));
                }
            }
            Ok(Event::End(e)) => {
                change_depth = change_depth.saturating_sub(1);
                if change_depth == 0 && e.local_name().as_ref() == change_local {
                    break;
                }
                if inner_depth >= 1 {
                    inner_depth -= 1;
                    if inner_depth >= 1 {
                        let _ = writer.write_event(Event::End(e));
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(other) => {
                if inner_depth >= 1 {
                    let _ = writer.write_event(other);
                }
            }
        }
        buf.clear();
    }
    writer.into_inner()
}

/// Pass 2 on read: produces a synthetic `word/styles.xml` where each changed style's direct
/// `<w:pPr>` / `<w:rPr>` is replaced with its `<w:pPrChange>/<w:pPr>` or `<w:rPrChange>/<w:rPr>`.
fn build_old_styles_xml(styles_bytes: &[u8], extracted: &BTreeMap<String, ExtractedChanges>) -> Vec<u8> {
    let mut reader = Reader::from_reader(styles_bytes);
    let mut writer = Writer::new(Vec::with_capacity(styles_bytes.len()));
    let mut buf = Vec::new();
    let mut depth = 0usize;
    let mut cur_ext: Option<&ExtractedChanges> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let local = e.local_name();
                if depth == 1 && local.as_ref() == b"style" {
                    cur_ext = attr_value(&e, b"styleId").and_then(|id| extracted.get(&id));
                    let _ = writer.write_event(Event::Start(e));
                    depth += 1;
                } else if depth == 2 && local.as_ref() == b"pPr" && cur_ext.and_then(|x| x.ppr.as_ref()).is_some() {
                    let _ = capture_element_children(&mut reader, b"pPr");
                    if let Some((_, old_inner)) = cur_ext.and_then(|x| x.ppr.as_ref()) {
                        write_prop_wrapper(&mut writer, b"w:pPr", old_inner);
                    }
                } else if depth == 2 && local.as_ref() == b"rPr" && cur_ext.and_then(|x| x.rpr.as_ref()).is_some() {
                    let _ = capture_element_children(&mut reader, b"rPr");
                    if let Some((_, old_inner)) = cur_ext.and_then(|x| x.rpr.as_ref()) {
                        write_prop_wrapper(&mut writer, b"w:rPr", old_inner);
                    }
                } else {
                    let _ = writer.write_event(Event::Start(e));
                    depth += 1;
                }
            }
            Ok(Event::End(e)) => {
                if depth == 2 {
                    cur_ext = None;
                }
                depth = depth.saturating_sub(1);
                let _ = writer.write_event(Event::End(e));
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(other) => {
                let _ = writer.write_event(other);
            }
        }
        buf.clear();
    }
    writer.into_inner()
}

fn write_prop_wrapper(writer: &mut Writer<Vec<u8>>, tag: &[u8], inner: &[u8]) {
    let out = writer.get_mut();
    if inner.is_empty() {
        out.extend_from_slice(b"<");
        out.extend_from_slice(tag);
        out.extend_from_slice(b"/>");
    } else {
        out.extend_from_slice(b"<");
        out.extend_from_slice(tag);
        out.extend_from_slice(b">");
        out.extend_from_slice(inner);
        out.extend_from_slice(b"</");
        out.extend_from_slice(tag);
        out.extend_from_slice(b">");
    }
}

fn parse_raw_rev(e: &quick_xml::events::BytesStart<'_>) -> RawRev {
    RawRev {
        id: attr_value(e, b"id"),
        author: attr_value(e, b"author").unwrap_or_default(),
        date: attr_value(e, b"date").unwrap_or_default(),
    }
}

fn attr_value(e: &quick_xml::events::BytesStart<'_>, key_local: &[u8]) -> Option<String> {
    for attr in e.attributes().flatten() {
        if attr.key.local_name().as_ref() == key_local {
            let raw = std::str::from_utf8(attr.value.as_ref()).ok()?;
            return quick_xml::escape::unescape(raw).ok().map(|v| v.into_owned());
        }
    }
    None
}

fn read_zip_part(zip_bytes: &[u8], target: &str) -> Option<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes)).ok()?;
    for idx in 0..archive.len() {
        let mut entry = archive.by_index(idx).ok()?;
        if entry.name().eq_ignore_ascii_case(target) {
            let mut out = Vec::new();
            entry.read_to_end(&mut out).ok()?;
            return Some(out);
        }
    }
    None
}

fn replace_zip_parts(zip_bytes: &[u8], replacements: &[(&str, &[u8])]) -> Result<Vec<u8>, DocxError> {
    let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes)).map_err(|e| DocxError::Xml(e.to_string()))?;
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::with_capacity(zip_bytes.len() + 512)));
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for idx in 0..archive.len() {
        let entry = archive.by_index_raw(idx).map_err(|e| DocxError::Xml(e.to_string()))?;
        let name = entry.name().to_string();
        if let Some((_, new_bytes)) = replacements.iter().find(|(target, _)| name.eq_ignore_ascii_case(target)) {
            drop(entry);
            writer.start_file(name, opts).map_err(|e| DocxError::Xml(e.to_string()))?;
            writer.write_all(new_bytes).map_err(|e| DocxError::Xml(e.to_string()))?;
        } else {
            writer.raw_copy_file(entry).map_err(|e| DocxError::Xml(e.to_string()))?;
        }
    }
    let cursor = writer.finish().map_err(|e| DocxError::Xml(e.to_string()))?;
    Ok(cursor.into_inner())
}
