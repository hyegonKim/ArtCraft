//! PDF Reflow import (`.pdf`, PDF 1.0–1.7 / ISO 32000-1).
//!
//! Parses classic cross-reference tables (`xref` + `trailer`) and PDF 1.5+ cross-reference and
//! object streams (`/Type /XRef`, `/Type /ObjStm`), walks the `/Pages` tree inheriting
//! `/Resources` and page boxes, decodes `/FlateDecode` / `/ASCIIHexDecode` / `/ASCII85Decode`
//! streams with PNG/TIFF predictor unfiltering, resolves `/Font` dictionaries (`/BaseFont`,
//! `/Encoding`, `/Differences`, `/ToUnicode` CMaps with `bfchar` and `bfrange`, `/Widths` and
//! CID `/DW` + `/W`), interprets page and Form XObject content streams (`BT`/`ET`, graphics state,
//! text positioning, kerning `TJ`, RGB/gray/CMYK fill colours), and reflows positioned text spans
//! into editable WordCraft headings, lists, styled runs, hyperlinks and paragraphs.

mod cmap;
mod content;
mod parser;
mod reflow;
#[cfg(test)]
mod tests;

use wordcraft_doc::{Document, SectionProps};

use crate::model::{self, Flow};

/// Does `bytes` begin with a `%PDF-` header within the first 1024 bytes (ISO 32000-1 §7.5.2)?
pub fn is_pdf(bytes: &[u8]) -> bool {
    parser::find_pdf_header(bytes).is_some()
}

/// Parse `bytes` as a PDF and reflow its pages into a [`Flow`] plus the first page's size (when
/// valid). Returns `Err` for non-PDF, truncated/zero-page, or password-protected PDFs.
pub fn parse_flow(bytes: &[u8]) -> Result<(Flow, Option<SectionProps>), String> {
    model::begin_import();
    let pdf = parser::PdfDoc::parse(bytes)?;
    let pages = pdf.collect_pages()?;
    if pages.is_empty() {
        return Err("PDF contains no pages".into());
    }
    let meta = pdf.extract_meta();
    let mut extracted_pages = Vec::with_capacity(pages.len());
    let mut first_section: Option<SectionProps> = None;

    for page_info in &pages {
        if first_section.is_none()
            && let Some(sec) = page_info.section_props()
        {
            first_section = Some(sec);
        }
        let spans = content::extract_page_spans(&pdf, page_info);
        extracted_pages.push(reflow::PageSpans { width: page_info.width(), height: page_info.height(), spans, links: page_info.links.clone() });
    }

    let blocks = reflow::reflow_document(&extracted_pages);
    Ok((Flow { blocks, meta }, first_section))
}

/// Import a PDF from `bytes` into an editable [`Document`].
pub fn import(bytes: &[u8]) -> Result<Document, String> {
    import_on(bytes, &SectionProps::default())
}

/// Import a PDF from `bytes`, using the PDF's own first-page dimensions when valid and falling
/// back to `fallback_page` otherwise.
pub fn import_on(bytes: &[u8], fallback_page: &SectionProps) -> Result<Document, String> {
    let (flow, pdf_page) = parse_flow(bytes)?;
    let section = pdf_page.unwrap_or_else(|| fallback_page.clone());
    Ok(model::to_doc_in(&flow, section))
}
