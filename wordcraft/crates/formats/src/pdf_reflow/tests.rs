//! Unit and integration tests for PDF Reflow import.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use miniz_oxide::deflate::compress_to_vec_zlib;
use wordcraft_doc::props::TextColor;
use wordcraft_doc::{Document, Rgb};

use super::cmap::normalize_postscript_font;
use super::{import, is_pdf, parse_flow};
use crate::model::{FBlock, Kind};

/// Helper to assemble a valid classic-xref PDF from numbered objects (`(obj_num, body_bytes)`).
/// Object `1` must be the `/Type /Catalog`.
fn build_pdf(objects: &[(u32, Vec<u8>)], info_obj: Option<u32>) -> Vec<u8> {
    let mut out = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let max_id = objects.iter().map(|(id, _)| *id).max().unwrap_or(0);
    let mut offsets = vec![0usize; (max_id + 1) as usize];

    for (id, body) in objects {
        offsets[*id as usize] = out.len();
        out.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
        out.extend_from_slice(body);
        if !body.ends_with(b"\n") {
            out.push(b'\n');
        }
        out.extend_from_slice(b"endobj\n");
    }

    let xref_pos = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", max_id + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for off in offsets.iter().skip(1) {
        if *off > 0 {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        } else {
            out.extend_from_slice(b"0000000000 65535 f \n");
        }
    }
    out.extend_from_slice(b"trailer\n<< /Size ");
    out.extend_from_slice((max_id + 1).to_string().as_bytes());
    out.extend_from_slice(b" /Root 1 0 R");
    if let Some(info) = info_obj {
        out.extend_from_slice(format!(" /Info {info} 0 R").as_bytes());
    }
    out.extend_from_slice(b" >>\nstartxref\n");
    out.extend_from_slice(xref_pos.to_string().as_bytes());
    out.extend_from_slice(b"\n%%EOF\n");
    out
}

fn stream_obj(extra_dict: &str, data: &[u8], flate: bool) -> Vec<u8> {
    let payload = if flate { compress_to_vec_zlib(data, 6) } else { data.to_vec() };
    let filter = if flate { " /Filter /FlateDecode" } else { "" };
    let extra = if extra_dict.is_empty() { String::new() } else { format!(" {extra_dict}") };
    let mut v = format!("<< /Length {}{}{} >>\nstream\n", payload.len(), filter, extra).into_bytes();
    v.extend_from_slice(&payload);
    v.extend_from_slice(b"\nendstream");
    v
}

fn body_paras(doc: &Document) -> Vec<&wordcraft_doc::Paragraph> {
    doc.body.iter().filter_map(|b| b.as_para()).collect()
}

fn para_runs(p: &wordcraft_doc::Paragraph) -> Vec<(&str, &wordcraft_doc::CharProps)> {
    let mut pos = 0;
    let mut out = Vec::with_capacity(p.runs.len());
    for r in &p.runs {
        let slice = &p.text[pos..pos + r.len];
        pos += r.len;
        out.push((slice, &r.props));
    }
    out
}

#[test]
fn postscript_subset_font_names_normalize_cleanly() {
    assert_eq!(
        normalize_postscript_font("ABCDEF+Helvetica-BoldOblique"),
        ("Helvetica".to_string(), true, true, false)
    );
    assert_eq!(
        normalize_postscript_font("Times-Bold"),
        ("Times New Roman".to_string(), true, false, false)
    );
    assert_eq!(
        normalize_postscript_font("Courier-Oblique"),
        ("Courier New".to_string(), false, true, true)
    );
    assert_eq!(
        normalize_postscript_font("XYZQWE+Calibri-Bold"),
        ("Calibri".to_string(), true, false, false)
    );
    assert_eq!(
        normalize_postscript_font("GHIJKL+MalgunGothicBold"),
        ("Malgun Gothic".to_string(), true, false, false)
    );
}

#[test]
fn uncompressed_pdf_headings_styled_runs_and_colors() {
    let content = b"\
BT
/F2 22 Tf
0.1 0.2 0.6 rg
72 720 Td
(Document Main Title) Tj
ET
BT
/F2 16 Tf
0 0 0 rg
72 688 Td
(1. Architecture Overview) Tj
ET
BT
/F2 13.5 Tf
72 662 Td
(1.1 Core Pipeline) Tj
ET
BT
/F1 11 Tf
72 638 Td
(Plain words with ) Tj
/F2 11 Tf
0.8 0.1 0.1 rg
(bold crimson) Tj
/F3 11 Tf
0 0.5 0.2 rg
( and italic green) Tj
/F4 11 Tf
0 0 0 rg
( in Courier.) Tj
ET
";
    let pdf = build_pdf(
        &[
            (1, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()),
            (2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec()),
            (
                3,
                b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R /F2 5 0 R /F3 6 0 R /F4 7 0 R >> >> /Contents 8 0 R >>"
                    .to_vec(),
            ),
            (4, b"<< /Type /Font /Subtype /Type1 /BaseFont /Times-Roman >>".to_vec()),
            (5, b"<< /Type /Font /Subtype /Type1 /BaseFont /ABCDEF+Helvetica-Bold >>".to_vec()),
            (6, b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-BoldOblique >>".to_vec()),
            (7, b"<< /Type /Font /Subtype /Type1 /BaseFont /Courier-Oblique >>".to_vec()),
            (8, stream_obj("", content, false)),
        ],
        None,
    );

    assert!(is_pdf(&pdf));
    let doc = import(&pdf).unwrap();
    let paras = body_paras(&doc);
    assert_eq!(paras.len(), 4);

    assert_eq!(paras[0].props.style.as_deref(), Some("Heading1"));
    assert_eq!(paras[0].plain_text(), "Document Main Title");

    assert_eq!(paras[1].props.style.as_deref(), Some("Heading2"));
    assert_eq!(paras[1].plain_text(), "1. Architecture Overview");

    assert_eq!(paras[2].props.style.as_deref(), Some("Heading3"));
    assert_eq!(paras[2].plain_text(), "1.1 Core Pipeline");

    assert_eq!(paras[3].plain_text(), "Plain words with bold crimson and italic green in Courier.");
    let runs = para_runs(paras[3]);
    let (_, plain_props) = runs.iter().find(|(t, _)| t.contains("Plain words")).unwrap();
    assert_eq!(plain_props.font.as_deref(), Some("Times New Roman"));
    assert_eq!(plain_props.color, None);

    let (_, bold_props) = runs.iter().find(|(t, _)| t.contains("bold crimson")).unwrap();
    assert_eq!(bold_props.font.as_deref(), Some("Helvetica"));
    assert_eq!(bold_props.bold, Some(true));
    assert_eq!(bold_props.color, Some(TextColor::Rgb(Rgb(204, 26, 26))));

    let (_, bi_props) = runs.iter().find(|(t, _)| t.contains("italic green")).unwrap();
    assert_eq!(bi_props.bold, Some(true));
    assert_eq!(bi_props.italic, Some(true));
    assert_eq!(bi_props.color, Some(TextColor::Rgb(Rgb(0, 128, 51))));

    let (_, mono_props) = runs.iter().find(|(t, _)| t.contains("Courier.")).unwrap();
    assert_eq!(mono_props.font.as_deref(), Some("Courier New"));
    assert_eq!(mono_props.italic, Some(true));
}

#[test]
fn flate_compressed_multiline_reflow_dehyphenation_and_breaks() {
    // Paragraph 1: 3 wrapped lines at 14pt leading, with a hyphenated word `reflow-` + `able`.
    // Paragraph 2: separated by a 28pt vertical gap (`y = 664`), 2 wrapped lines.
    // Bullet list: two bullet items (`\225` in WinAnsi = U+2022 bullet).
    let content = b"\
BT
/F1 11 Tf
72 720 Td
(WordCraft imports PDF pages into clean editable paragraphs with reflow-) Tj
0 -14 Td
(able lines that merge naturally across column wraps without hard) Tj
0 -14 Td
(line breaks in the middle of a sentence.) Tj
0 -28 Td
(A larger vertical gap starts a brand new second paragraph that also wraps) Tj
0 -14 Td
(smoothly onto its second line.) Tj
0 -26 Td
(\\225 First bullet item) Tj
0 -15 Td
(\\225 Second bullet item) Tj
ET
";
    let pdf = build_pdf(
        &[
            (1, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()),
            (2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec()),
            (
                3,
                b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".to_vec(),
            ),
            (4, b"<< /Type /Font /Subtype /Type1 /BaseFont /Calibri /Encoding /WinAnsiEncoding >>".to_vec()),
            (5, stream_obj("", content, true)),
        ],
        None,
    );

    let (flow, _) = parse_flow(&pdf).unwrap();
    assert_eq!(flow.blocks.len(), 4);

    let FBlock::Para(p0) = &flow.blocks[0] else { panic!("expected para 0") };
    assert_eq!(p0.kind, Kind::Normal);
    assert_eq!(
        p0.text(),
        "WordCraft imports PDF pages into clean editable paragraphs with reflowable lines that merge naturally across column wraps without hard line breaks in the middle of a sentence."
    );

    let FBlock::Para(p1) = &flow.blocks[1] else { panic!("expected para 1") };
    assert_eq!(
        p1.text(),
        "A larger vertical gap starts a brand new second paragraph that also wraps smoothly onto its second line."
    );

    let FBlock::Para(p2) = &flow.blocks[2] else { panic!("expected bullet 1") };
    assert!(p2.list.is_some_and(|l| !l.ordered));
    assert_eq!(p2.text(), "First bullet item");

    let FBlock::Para(p3) = &flow.blocks[3] else { panic!("expected bullet 2") };
    assert!(p3.list.is_some_and(|l| !l.ordered));
    assert_eq!(p3.text(), "Second bullet item");
}

#[test]
fn tj_kerning_pairs_join_while_word_adjustments_insert_spaces() {
    let content = b"\
BT
/F1 12 Tf
72 700 Td
[(W) 80 (ord) -10 (Craft) -320 (PDF) -300 (Re) 25 (flow)] TJ
ET
";
    let pdf = build_pdf(
        &[
            (1, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()),
            (2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec()),
            (
                3,
                b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".to_vec(),
            ),
            (4, b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec()),
            (5, stream_obj("", content, false)),
        ],
        None,
    );

    let doc = import(&pdf).unwrap();
    let paras = body_paras(&doc);
    assert_eq!(paras.len(), 1);
    assert_eq!(paras[0].plain_text(), "WordCraft PDF Reflow");
}

#[test]
fn tounicode_cmap_bfchar_and_bfrange_decode_korean_and_unicode() {
    // Glyph codes:
    // 0001..0003 -> array bfrange 'P', 'D', 'F' ([<0050> <0044> <0046>])
    // 0021..0023 -> sequential bfrange '1', '2', '3' (<0031>)
    // 0004 -> ' ' (0020)
    // 0010..0017 -> bfchar for "리플로우 가져오기"
    //   리=B9AC, 플=D50C, 로=B85C, 우=C6B0, 가=AC00, 져=C838, 오=C624, 기=AE30
    let cmap = b"\
/CIDInit /ProcSet findresource begin
12 dict begin
begincmap
1 begincodespacerange
<0000> <FFFF>
endcodespacerange
2 beginbfrange
<0001> <0003> [<0050> <0044> <0046>]
<0021> <0023> <0031>
endbfrange
9 beginbfchar
<0004> <0020>
<0010> <B9AC>
<0011> <D50C>
<0012> <B85C>
<0013> <C6B0>
<0014> <AC00>
<0015> <C838>
<0016> <C624>
<0017> <AE30>
endbfchar
endcmap
";
    let content = b"\
BT
/F1 14 Tf
72 720 Td
<00010002000300040010001100120013000400140015001600170004002100220023> Tj
ET
";
    let pdf = build_pdf(
        &[
            (1, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()),
            (2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec()),
            (
                3,
                b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 4 0 R >> >> /Contents 6 0 R >>".to_vec(),
            ),
            (
                4,
                b"<< /Type /Font /Subtype /Type0 /BaseFont /GHIJKL+MalgunGothicBold /Encoding /Identity-H /ToUnicode 5 0 R >>".to_vec(),
            ),
            (5, stream_obj("", cmap, true)),
            (6, stream_obj("", content, true)),
        ],
        None,
    );

    let doc = import(&pdf).unwrap();
    assert_eq!((doc.last_section.page_w.round(), doc.last_section.page_h.round()), (595.0, 842.0));
    let paras = body_paras(&doc);
    assert_eq!(paras.len(), 1);
    assert_eq!(paras[0].plain_text(), "PDF 리플로우 가져오기 123");
    assert_eq!(paras[0].runs[0].props.font.as_deref(), Some("Malgun Gothic"));
    assert_eq!(paras[0].runs[0].props.bold, Some(true));
}

#[test]
fn multipage_pdf_preserves_page_breaks_metadata_and_links() {
    let p1 = b"BT /F1 12 Tf 72 720 Td (Visit ArtCraft online) Tj ET";
    let p2 = b"BT /F1 12 Tf 72 720 Td (Second page content) Tj ET";
    let pdf = build_pdf(
        &[
            (1, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()),
            (
                2,
                b"<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 /MediaBox [0 0 612 792] /Resources << /Font << /F1 5 0 R >> >> >>".to_vec(),
            ),
            (
                3,
                b"<< /Type /Page /Parent 2 0 R /Contents 6 0 R /Annots [8 0 R] >>".to_vec(),
            ),
            (4, b"<< /Type /Page /Parent 2 0 R /Contents 7 0 R >>".to_vec()),
            (5, b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec()),
            (6, stream_obj("", p1, false)),
            (7, stream_obj("", p2, false)),
            (
                8,
                b"<< /Type /Annot /Subtype /Link /Rect [70 715 240 738] /A << /S /URI /URI (https://example.com/docs) >> >>".to_vec(),
            ),
            (
                9,
                b"<< /Title (Quarterly Report) /Author (Kim Writer) /CreationDate (D:20261114093000Z) >>".to_vec(),
            ),
        ],
        Some(9),
    );

    let (flow, _) = parse_flow(&pdf).unwrap();
    assert_eq!(flow.meta.title, "Quarterly Report");
    assert_eq!(flow.meta.author, "Kim Writer");
    assert_eq!(flow.meta.created, "2026-11-14T09:30:00Z");

    let doc = import(&pdf).unwrap();
    assert_eq!(doc.core.title, "Quarterly Report");
    assert_eq!(doc.core.creator, "Kim Writer");
    let paras = body_paras(&doc);
    assert_eq!(paras.len(), 2);
    assert_eq!(paras[0].plain_text(), "Visit ArtCraft online");
    assert_eq!(paras[0].runs[0].props.link.as_deref(), Some("https://example.com/docs"));
    assert_eq!(paras[1].plain_text(), "Second page content");
    assert_eq!(paras[1].props.page_break_before, Some(true));
}

#[test]
fn pdf15_xref_stream_and_compressed_object_stream_parse() {
    // Build a PDF 1.5 file where Catalog (obj 1), Pages (obj 2), Page (obj 3), and Font (obj 4)
    // live inside a compressed `/Type /ObjStm` (obj 5), content stream is obj 6, and `/Type /XRef`
    // is obj 7!
    let o1 = b"<< /Type /Catalog /Pages 2 0 R >>";
    let o2 = b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>";
    let o3 = b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 6 0 R >>";
    let o4 = b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>";
    let bodies: [&[u8]; 4] = [o1, o2, o3, o4];
    let mut header = String::new();
    let mut body_blob = Vec::new();
    for (idx, b) in bodies.iter().enumerate() {
        let id = idx + 1;
        let rel = body_blob.len();
        header.push_str(&format!("{id} {rel} "));
        body_blob.extend_from_slice(b);
        body_blob.push(b'\n');
    }
    let first = header.len();
    let mut obj_stm_raw = header.into_bytes();
    obj_stm_raw.extend_from_slice(&body_blob);

    let mut out = b"%PDF-1.6\n".to_vec();
    let off5 = out.len();
    out.extend_from_slice(b"5 0 obj\n");
    out.extend_from_slice(&stream_obj(&format!("/Type /ObjStm /N 4 /First {first}"), &obj_stm_raw, true));
    out.extend_from_slice(b"\nendobj\n");

    let off6 = out.len();
    out.extend_from_slice(b"6 0 obj\n");
    out.extend_from_slice(&stream_obj("", b"BT /F1 12 Tf 72 700 Td (Compressed Object Stream Text) Tj ET", true));
    out.extend_from_slice(b"\nendobj\n");

    let off7 = out.len();
    // `/W [1 4 1]` -> 6 bytes per entry, objects 0..=7.
    let mut xref_bytes = Vec::new();
    let push_entry = |buf: &mut Vec<u8>, t: u8, f1: u32, f2: u8| {
        buf.push(t);
        buf.extend_from_slice(&f1.to_be_bytes());
        buf.push(f2);
    };
    push_entry(&mut xref_bytes, 0, 0, 0);
    for idx in 0..4u8 {
        push_entry(&mut xref_bytes, 2, 5, idx);
    }
    push_entry(&mut xref_bytes, 1, off5 as u32, 0);
    push_entry(&mut xref_bytes, 1, off6 as u32, 0);
    push_entry(&mut xref_bytes, 1, off7 as u32, 0);

    out.extend_from_slice(b"7 0 obj\n");
    out.extend_from_slice(&stream_obj("/Type /XRef /Size 8 /Root 1 0 R /W [1 4 1]", &xref_bytes, true));
    out.extend_from_slice(b"\nendobj\nstartxref\n");
    out.extend_from_slice(off7.to_string().as_bytes());
    out.extend_from_slice(b"\n%%EOF\n");

    let doc = import(&out).unwrap();
    let paras = body_paras(&doc);
    assert_eq!(paras.len(), 1);
    assert_eq!(paras[0].plain_text(), "Compressed Object Stream Text");
}

#[test]
fn truncated_encrypted_and_malformed_pdfs_fail_without_panicking() {
    assert!(import(b"").is_err());
    assert!(import(b"not a pdf at all").is_err());
    assert!(import(b"%PDF-1.7\n1 0 obj\n").is_err());

    let encrypted = b"%PDF-1.7\n1 0 obj\n<< /Type /Catalog >>\nendobj\ntrailer\n<< /Root 1 0 R /Encrypt << /V 4 >> >>\n%%EOF\n";
    let err = import(encrypted).unwrap_err();
    assert!(err.contains("password-protected"), "{err}");
}
