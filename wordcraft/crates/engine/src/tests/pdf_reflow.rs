use serde_json::json;
use wordcraft_doc::props::{CharProps, TextColor};
use wordcraft_doc::{Document, Paragraph, Rgb, StoryRef, para_block};

use crate::Session;

fn b64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = if chunk.len() > 1 { u32::from(chunk[1]) } else { 0 };
        let b2 = if chunk.len() > 2 { u32::from(chunk[2]) } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((n >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((n >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(n & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[test]
fn file_import_pdf_without_args_requests_picker() {
    let mut s = Session::new(Document::new());
    let res = s.run("file.importPdf", &json!({})).unwrap();
    assert_eq!(res["imported"], false);
    assert_eq!(s.ui_requests, vec![json!({"open": "importPdf"})]);
}

#[test]
fn pdf_export_to_reflow_import_and_open_round_trip() {
    let mut src = Document::new();
    src.core.title = "WordCraft Reflow Spec".into();
    src.core.creator = "ArtCraft QA".into();
    src.body.clear();
    src.body.push(para_block(
        Paragraph::with_text(
            "Quarterly Strategy",
            CharProps { bold: Some(true), size: Some(22.0), ..Default::default() },
        )
        .styled("Heading1"),
    ));
    src.body.push(para_block(
        Paragraph::with_text(
            "1. Product Roadmap",
            CharProps { bold: Some(true), size: Some(15.0), ..Default::default() },
        )
        .styled("Heading2"),
    ));

    let mut body_para = Paragraph::with_text(
        "Reflow preserves ",
        CharProps { size: Some(11.0), ..Default::default() },
    );
    let end = body_para.len();
    body_para
        .insert_text(
            end,
            "bold crimson",
            &CharProps {
                bold: Some(true),
                size: Some(11.0),
                color: Some(TextColor::Rgb(Rgb(200, 30, 30))),
                ..Default::default()
            },
        )
        .unwrap();
    let end = body_para.len();
    body_para
        .insert_text(
            end,
            " and PDF 리플로우 가져오기.",
            &CharProps { italic: Some(true), size: Some(11.0), ..Default::default() },
        )
        .unwrap();
    src.body.push(para_block(body_para));

    let pdf_bytes = crate::io::save_bytes("quarterly-report.pdf", &src).unwrap();
    assert!(pdf_bytes.starts_with(b"%PDF-"));

    // 1. `file.importPdf` via base64 `data` + `path` stem on an untouched blank session.
    let mut s = Session::new(Document::new());
    let b64 = b64_encode(&pdf_bytes);
    let imported = s
        .run("file.importPdf", &json!({"path": "quarterly-report.pdf", "data": b64}))
        .unwrap();
    assert_eq!(imported["imported"], true);
    assert!(s.path.is_none(), "imported PDF starts untitled so Save prompts for .docx");
    assert_eq!(s.file_name.as_deref(), Some("quarterly-report"));
    assert!(s.dirty);

    let plain = s.doc.plain_text(StoryRef::Body);
    assert!(plain.contains("Quarterly Strategy"), "{plain}");
    assert!(plain.contains("1. Product Roadmap"), "{plain}");
    assert!(plain.contains("bold crimson"), "{plain}");
    assert!(plain.contains("PDF 리플로우 가져오기"), "{plain}");

    let paras: Vec<&Paragraph> = s.doc.body.iter().filter_map(|b| b.as_para()).collect();
    assert_eq!(paras[0].props.style.as_deref(), Some("Heading1"));
    assert_eq!(paras[1].props.style.as_deref(), Some("Heading2"));

    // 2. Disk `file.importPdf` and `file.open` on `.pdf`, plus extensionless `%PDF-` sniffing.
    let dir = std::env::temp_dir().join(format!("wordcraft-pdf-reflow-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let pdf_path = dir.join("roadmap.pdf");
    std::fs::write(&pdf_path, &pdf_bytes).unwrap();

    // Importing into a session that already has content opens a second document beside it.
    let before_id = s.document_id();
    let res2 = s
        .run("file.importPdf", &json!({"path": pdf_path.to_string_lossy()}))
        .unwrap();
    assert_eq!(res2["imported"], true);
    assert_ne!(s.document_id(), before_id);
    assert_eq!(s.file_name.as_deref(), Some("roadmap"));

    // Opening `.pdf` directly via `file.open`.
    let mut s_open = Session::new(Document::new());
    s_open
        .run("file.open", &json!({"path": pdf_path.to_string_lossy()}))
        .unwrap();
    assert!(s_open.doc.plain_text(StoryRef::Body).contains("PDF 리플로우 가져오기"));

    // Content sniffing when filename has no extension.
    let sniffed = crate::io::open_bytes("uploaded_blob", &pdf_bytes).unwrap();
    assert!(sniffed.plain_text(StoryRef::Body).contains("Quarterly Strategy"));

    let _ = std::fs::remove_dir_all(&dir);
}
