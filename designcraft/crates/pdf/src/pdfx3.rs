//! PDF/X-3:2003 (ISO 15930-6) finalization and conformance checking.
//!
//! PDF/X-3:2003 shares PDF/X-1a:2003's PDF 1.4 substrate (no live transparency, no encryption,
//! embedded fonts, `/TrimBox` or `/ArtBox` on every page, and an ICC v2 or registered `GTS_PDFX`
//! output intent), while additionally permitting color-managed color spaces (`ICCBased`, `CalRGB`,
//! `CalGray`, `Lab`, and `DeviceRGB` remapped via `DefaultRGB`) alongside `DeviceCMYK`,
//! `DeviceGray`, `Separation`, and `DeviceN`.

use std::collections::HashSet;

use craft_pdf::syntax::Pdf;
use craft_pdf::syntax::content::UntypedIter;
use craft_pdf::syntax::object::{Dict, Name, Object, ObjectIdentifier, Stream};
use craft_pdf::syntax::page::Resources;

use crate::optional_content::{append_with, edited_with, trailer_at};
use crate::pdfx::{MAX_DEPTH, fonts, info_text, pdfx_intents};
use crate::pdfx1a::PageGraphics;
use crate::transitions::{int_after, rfind};
use crate::{PdfError, Result};

fn damaged() -> PdfError {
    PdfError::Write("the exported PDF could not be finalized as PDF/X-3:2003".into())
}

fn encrypted() -> PdfError {
    PdfError::Write("an encrypted PDF cannot be finalized as PDF/X-3:2003".into())
}

/// Finalizes a PDF/X-4 base print PDF into PDF/X-3:2003 (PDF 1.4, no live transparency,
/// `PDF/X-3:2003` identification, preserving ICC-tagged RGB color spaces when no live
/// transparency requires flattening).
pub(crate) fn finish_pdfx3(pdf: &[u8], dpi: f32, pages: &[PageGraphics], warnings: &mut Vec<String>) -> Result<Vec<u8>> {
    let base = if has_live_transparency(pdf) {
        let (flattened, findings) = craft_prepress::pdfx1a::flatten_x1a(pdf, dpi, pages).map_err(PdfError::Write)?;
        for finding in findings {
            warnings.push(finding.replace("PDF/X-1a", "PDF/X-3"));
        }
        flattened
    } else {
        craft_prepress::pdfx1a::finish_x1a(pdf).map_err(PdfError::Write)?
    };
    relabel_x1a_to_x3(&base)
}

/// Returns whether `bytes` contains live PDF 1.4+ transparency constructs (soft masks, non-unit
/// constant opacity, non-Normal blend modes, or form transparency groups). Page-level blending
/// groups alone are ignored because `finish_x1a` strips them cleanly when no object uses
/// transparency.
pub(crate) fn has_live_transparency(bytes: &[u8]) -> bool {
    let Ok(pdf) = Pdf::new(bytes.to_vec()) else { return false };
    for obj in pdf.objects() {
        match obj {
            Object::Dict(d) => {
                if dict_has_transparency_state(&d) {
                    return true;
                }
            }
            Object::Stream(s) => {
                let d = s.dict();
                if d.get::<Name<'_>>(b"Subtype").is_some_and(|n| n.as_ref() == b"Form")
                    && d.get::<Dict<'_>>(b"Group").and_then(|g| g.get::<Name<'_>>(b"S")).is_some_and(|s| s.as_ref() == b"Transparency")
                {
                    return true;
                }
                if dict_has_transparency_state(d) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn dict_has_transparency_state(d: &Dict<'_>) -> bool {
    if let Some(smask) = d.get::<Object<'_>>(b"SMask") {
        let is_none = matches!(&smask, Object::Name(n) if n.as_ref() == b"None");
        if !is_none {
            return true;
        }
    }
    if d.get::<f32>(b"ca").is_some_and(|v| v < 0.9999) {
        return true;
    }
    if d.get::<f32>(b"CA").is_some_and(|v| v < 0.9999) {
        return true;
    }
    if let Some(bm) = d.get::<Object<'_>>(b"BM") {
        let normal = match bm {
            Object::Name(n) => matches!(n.as_ref(), b"Normal" | b"Compatible"),
            Object::Array(a) => a.iter::<Name<'_>>().all(|n| matches!(n.as_ref(), b"Normal" | b"Compatible")),
            _ => false,
        };
        if !normal {
            return true;
        }
    }
    false
}

/// Rewrites `/Info` and catalog `/Metadata` from `PDF/X-1:2003` / `PDF/X-1a:2003` (or `PDF/X-4`)
/// to `PDF/X-3:2003` via an incremental update with exact xref offsets.
pub(crate) fn relabel_x1a_to_x3(bytes: &[u8]) -> Result<Vec<u8>> {
    let pdf = Pdf::new(bytes.to_vec()).map_err(|_| damaged())?;
    let xref = pdf.xref();
    let mut changes: Vec<(ObjectIdentifier, Vec<u8>)> = Vec::new();

    if let Some(info_id) = latest_trailer_info_ref(bytes)
        && let Some(info_dict) = xref.get::<Dict<'_>>(info_id)
    {
        let updated = edited_with(
            &info_dict,
            &[(b"GTS_PDFXVersion", b"(PDF/X-3:2003)"), (b"GTS_PDFXConformance", b"(PDF/X-3:2003)"), (b"Trapped", b"/False")],
            damaged,
        )?;
        changes.push((info_id, updated));
    }

    if let Some(catalog) = xref.get::<Dict<'_>>(xref.root_id()) {
        if catalog.contains_key(b"Version") {
            let updated_cat = edited_with(&catalog, &[(b"Version", b"")], damaged)?;
            changes.push((xref.root_id(), updated_cat));
        }
        if let Some(meta_ref) = catalog.get_ref(b"Metadata") {
            let meta_id: ObjectIdentifier = meta_ref.into();
            if let Some(stream) = xref.get::<Stream<'_>>(meta_id)
                && let Ok(raw_xml) = stream.decoded()
                && let Ok(xml_str) = std::str::from_utf8(&raw_xml)
            {
                let replaced =
                    xml_str.replace("PDF/X-1a:2003", "PDF/X-3:2003").replace("PDF/X-1:2003", "PDF/X-3:2003").replace("PDF/X-4", "PDF/X-3:2003");
                let len_bytes = replaced.len().to_string().into_bytes();
                let dict_bytes = edited_with(stream.dict(), &[(b"Length", len_bytes.as_slice()), (b"Filter", b"")], damaged)?;
                let mut stream_obj = dict_bytes;
                stream_obj.extend_from_slice(b"\nstream\n");
                stream_obj.extend_from_slice(replaced.as_bytes());
                stream_obj.extend_from_slice(b"\nendstream");
                changes.push((meta_id, stream_obj));
            }
        }
    }

    for page in pdf.pages().iter() {
        let page_dict = page.raw();
        if page_dict.contains_key(b"Group")
            && let Some(id) = page_dict.obj_id()
        {
            let updated_page = edited_with(page_dict, &[(b"Group", b"")], damaged)?;
            changes.push((id, updated_page));
        }
    }

    let mut out = if changes.is_empty() { bytes.to_vec() } else { append_with(bytes, &changes, None, damaged, encrypted)? };
    let head_len = 1024.min(out.len());
    if let Some(head) = out.get_mut(..head_len)
        && let Some(pos) = head.windows(7).position(|w| w == b"%PDF-1.")
        && let Some(ver_byte) = head.get_mut(pos + 7)
        && *ver_byte > b'4'
    {
        *ver_byte = b'4';
    }
    Ok(out)
}

fn latest_trailer_info_ref(bytes: &[u8]) -> Option<ObjectIdentifier> {
    let tail = bytes.get(rfind(bytes, b"startxref")?..)?;
    let mut offset = int_after(&String::from_utf8_lossy(tail), "startxref");
    let mut seen = HashSet::new();
    for _ in 0..32 {
        let at = offset.filter(|at| seen.insert(*at))?;
        let trailer = trailer_at(bytes, at)?;
        if let Some(r) = trailer.get_ref(b"Info") {
            return Some(r.into());
        }
        offset = trailer.get::<usize>(b"Prev");
    }
    None
}

/// Checks `pdf` against ISO 15930-6 (`PDF/X-3:2003`). Empty vector = conformant.
pub fn check_pdfx3(pdf: &[u8]) -> Vec<String> {
    let mut issues = Vec::new();
    let text = String::from_utf8_lossy(pdf);
    if !pdf.starts_with(b"%PDF-1.") || pdf.get(7).is_none_or(|v| *v > b'4') {
        issues.push("the PDF version must be 1.4 or lower for PDF/X-3:2003".into());
    }
    if text.contains("/Encrypt") {
        issues.push("the file is encrypted".into());
    }
    let Ok(file) = Pdf::new(pdf.to_vec()) else {
        issues.push("the file couldn't be parsed as a valid PDF".into());
        return issues;
    };

    // Output intent checks (registered condition or ICC v2 destination profile).
    let intents = pdfx_intents(&file);
    if intents.is_empty() {
        issues.push("no GTS_PDFX output intent".into());
    } else {
        for oi in &intents {
            let cond =
                oi.get::<craft_pdf::syntax::object::String<'_>>(b"OutputConditionIdentifier").map(|s| !s.as_bytes().is_empty()).unwrap_or(false);
            if !cond {
                issues.push("the output intent has an empty OutputConditionIdentifier".into());
            }
            let registered = oi.get::<craft_pdf::syntax::object::String<'_>>(b"RegistryName").map(|s| !s.as_bytes().is_empty()).unwrap_or(false);
            match oi.get::<Stream<'_>>(b"DestOutputProfile") {
                Some(profile) => {
                    if let Ok(data) = profile.decoded()
                        && data.get(8).is_some_and(|major| *major >= 4)
                    {
                        issues.push("PDF/X-3:2003 (PDF 1.4) does not allow ICC v4 output profiles".into());
                    }
                }
                None if !registered => {
                    issues.push("unregistered output condition has no destination profile".into());
                }
                None => {}
            }
        }
    }

    // Identification & Trapped checks in /Info.
    if info_text(pdf, &file, b"GTS_PDFXVersion").as_deref() != Some("PDF/X-3:2003") {
        issues.push("the document information doesn't name GTS_PDFXVersion PDF/X-3:2003".into());
    }
    if info_text(pdf, &file, b"GTS_PDFXConformance").as_deref() != Some("PDF/X-3:2003") {
        issues.push("the document information doesn't name GTS_PDFXConformance PDF/X-3:2003".into());
    }
    if !has_valid_trapped(pdf, &file) {
        issues.push("the document information must set /Trapped to /True or /False".into());
    }

    if text.contains("/Interpolate true") {
        issues.push("an image asks for interpolation".into());
    }
    if text.contains("/Subtype/NChannel") || text.contains("/Subtype /NChannel") {
        issues.push("PDF 1.6 NChannel DeviceN attributes aren't allowed in PDF/X-3:2003".into());
    }

    // Page box & page transparency group checks.
    for (idx, page) in file.pages().iter().enumerate() {
        let raw = page.raw();
        let has_trim = raw.contains_key(b"TrimBox");
        let has_art = raw.contains_key(b"ArtBox");
        if !has_trim && !has_art {
            issues.push(format!("page {} has neither TrimBox nor ArtBox", idx + 1));
        } else if has_trim && has_art {
            issues.push(format!("page {} specifies both TrimBox and ArtBox", idx + 1));
        }
        if raw.get::<Dict<'_>>(b"Group").and_then(|g| g.get::<Name<'_>>(b"S")).is_some_and(|s| s.as_ref() == b"Transparency") {
            issues.push(format!("page {} defines a transparency group", idx + 1));
        }
    }

    // Scan content streams, ExtGState, forms, shadings, images, and Type 3 fonts.
    let mut scan = X3Scan::default();
    for page in file.pages().iter() {
        let res = page.resources();
        scan.resources(res, 0);
        if let Some(content) = page.page_stream() {
            scan.content(content, res, 0);
        }
    }
    issues.extend(scan.issues);
    issues.extend(fonts::unembedded_fonts(pdf));
    issues
}

fn has_valid_trapped(pdf: &[u8], file: &Pdf) -> bool {
    let Some(info_id) = latest_trailer_info_ref(pdf) else {
        return String::from_utf8_lossy(pdf).contains("/Trapped/False")
            || String::from_utf8_lossy(pdf).contains("/Trapped /False")
            || String::from_utf8_lossy(pdf).contains("/Trapped/True")
            || String::from_utf8_lossy(pdf).contains("/Trapped /True");
    };
    let Some(info) = file.xref().get::<Dict<'_>>(info_id) else { return false };
    info.get::<Name<'_>>(b"Trapped").is_some_and(|n| matches!(n.as_ref(), b"True" | b"False"))
}

#[derive(Default)]
struct X3Scan {
    issues: Vec<String>,
    known: HashSet<String>,
    seen: HashSet<ObjectIdentifier>,
}

impl X3Scan {
    fn report(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        if self.known.insert(msg.clone()) {
            self.issues.push(msg);
        }
    }

    fn managed_rgb(res: &Resources<'_>) -> bool {
        res.get_color_space(&Name::new_unescaped(b"DefaultRGB")).is_some()
    }

    fn check_color_space(&mut self, cs: &Object<'_>, res: &Resources<'_>, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        match cs {
            Object::Name(n) => match n.as_ref() {
                b"DeviceRGB" | b"RGB" if !Self::managed_rgb(res) => {
                    self.report("untagged DeviceRGB color space is not allowed in PDF/X-3:2003");
                }
                _ => {}
            },
            Object::Array(a) => {
                let mut it = a.iter::<Object<'_>>();
                let Some(Object::Name(family)) = it.next() else { return };
                match family.as_ref() {
                    b"ICCBased" => {
                        if let Some(Object::Stream(s)) = it.next()
                            && let Ok(data) = s.decoded()
                            && data.get(8).is_some_and(|major| *major >= 4)
                        {
                            self.report("ICC v4 color space profile is not allowed in PDF/X-3:2003");
                        }
                    }
                    b"Indexed" | b"I" => {
                        if let Some(base) = it.next() {
                            self.check_color_space(&base, res, depth + 1);
                        }
                    }
                    b"Separation" | b"DeviceN" => {
                        let _ = it.next(); // ink name(s)
                        if let Some(alt) = it.next() {
                            if matches!(&alt, Object::Name(n) if matches!(n.as_ref(), b"DeviceRGB" | b"RGB")) {
                                self.report("Separation/DeviceN alternate space uses untagged DeviceRGB");
                            } else {
                                self.check_color_space(&alt, res, depth + 1);
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn resources(&mut self, res: &Resources<'_>, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        // Check Type 3 font glyph streams if any exist in resources.
        for fname in res.fonts.keys() {
            if let Some(font_dict) = res.get_font(&fname)
                && font_dict.get::<Name<'_>>(b"Subtype").is_some_and(|s| s.as_ref() == b"Type3")
                && let Some(procs) = font_dict.get::<Dict<'_>>(b"CharProcs")
            {
                let glyph_res = Resources::from_parent(font_dict.get::<Dict<'_>>(b"Resources").unwrap_or_default(), res.clone());
                for key in procs.keys() {
                    let kbytes: &[u8] = key.as_ref();
                    if let Some(stream) = procs.get::<Stream<'_>>(kbytes)
                        && let Ok(data) = stream.decoded()
                    {
                        self.content(&data, &glyph_res, depth + 1);
                    }
                }
            }
        }
    }

    fn ext_g_state(&mut self, gs: &Dict<'_>) {
        if let Some(smask) = gs.get::<Object<'_>>(b"SMask") {
            let is_none = matches!(&smask, Object::Name(n) if n.as_ref() == b"None");
            if !is_none {
                self.report("soft mask (/SMask) transparency is not allowed in PDF/X-3:2003");
            }
        }
        if gs.get::<f32>(b"ca").is_some_and(|v| v < 0.9999) || gs.get::<f32>(b"CA").is_some_and(|v| v < 0.9999) {
            self.report("non-unit constant opacity (/ca or /CA) is not allowed in PDF/X-3:2003");
        }
        if let Some(bm) = gs.get::<Object<'_>>(b"BM") {
            let normal = match &bm {
                Object::Name(n) => matches!(n.as_ref(), b"Normal" | b"Compatible"),
                Object::Array(a) => a.iter::<Name<'_>>().all(|n| matches!(n.as_ref(), b"Normal" | b"Compatible")),
                _ => false,
            };
            if !normal {
                self.report(format!("blend mode {bm:?} is not allowed in PDF/X-3:2003"));
            }
        }
    }

    fn content(&mut self, data: &[u8], res: &Resources<'_>, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        let managed = Self::managed_rgb(res);
        let mut ops = UntypedIter::new(data);
        while let Some(op) = ops.next() {
            let operator: &[u8] = op.operator;
            let last = op.operands().last().cloned();
            let name = || last.clone().and_then(Object::into_name);
            match operator {
                b"rg" | b"RG" if !managed => {
                    self.report("untagged DeviceRGB fills or strokes are not allowed in PDF/X-3:2003");
                }
                b"cs" | b"CS" => {
                    if let Some(n) = name() {
                        if matches!(n.as_ref(), b"DeviceRGB" | b"RGB") && !managed {
                            self.report("untagged DeviceRGB color space is not allowed in PDF/X-3:2003");
                        } else if let Some(cs) = res.get_color_space(&n) {
                            self.check_color_space(&cs, res, depth + 1);
                        }
                    }
                }
                b"gs" => {
                    if let Some(gs) = name().and_then(|n| res.get_ext_g_state(&n)) {
                        self.ext_g_state(&gs);
                    }
                }
                b"sh" => {
                    if let Some(sh) = name().and_then(|n| res.get_shading(&n)) {
                        let dict = match sh {
                            Object::Dict(d) => Some(d),
                            Object::Stream(s) => Some(s.dict().clone()),
                            _ => None,
                        };
                        if let Some(cs) = dict.and_then(|d| d.get::<Object<'_>>(b"ColorSpace")) {
                            self.check_color_space(&cs, res, depth + 1);
                        }
                    }
                }
                b"scn" | b"SCN" => {
                    if let Some(Object::Stream(pat)) = name().and_then(|n| res.get_pattern(&n)) {
                        self.form(&pat, res, depth + 1);
                    }
                }
                b"Do" => {
                    if let Some(xo) = name().and_then(|n| res.get_x_object(&n)) {
                        match xo.dict().get::<Name<'_>>(b"Subtype").as_deref() {
                            Some(b"Image") => {
                                if xo.dict().get::<bool>(b"ImageMask") != Some(true)
                                    && let Some(cs) = xo.dict().get::<Object<'_>>(b"ColorSpace")
                                {
                                    self.check_color_space(&cs, res, depth + 1);
                                }
                            }
                            Some(b"Form") => self.form(&xo, res, depth + 1),
                            _ => {}
                        }
                    }
                }
                b"BI" | b"EI" if !managed => {
                    if let Some(Object::Stream(img)) = last {
                        let cs = img.dict().get::<Object<'_>>(b"CS").or_else(|| img.dict().get::<Object<'_>>(b"ColorSpace"));
                        if let Some(cs) = cs {
                            self.check_color_space(&cs, res, depth + 1);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn form(&mut self, form: &Stream<'_>, parent: &Resources<'_>, depth: usize) {
        if depth > MAX_DEPTH || !self.seen.insert(form.obj_id()) {
            return;
        }
        if form.dict().get::<Dict<'_>>(b"Group").and_then(|g| g.get::<Name<'_>>(b"S")).is_some_and(|s| s.as_ref() == b"Transparency") {
            self.report("transparency group in Form XObject is not allowed in PDF/X-3:2003");
        }
        let res = Resources::from_parent(form.dict().get::<Dict<'_>>(b"Resources").unwrap_or_default(), parent.clone());
        self.resources(&res, depth);
        if let Ok(data) = form.decoded() {
            self.content(&data, &res, depth);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{PdfOptions, Standard, export_pdf_with_report};
    use designcraft_color::{Color, ColorType, Swatch, SwatchValue};
    use designcraft_compose::Cache;
    use designcraft_doc::build::NewDocument;
    use designcraft_doc::{Asset, AssetId, Content, Document, Fill, Graphic, Item, ItemId, ParaFormat, SpreadRef};
    use designcraft_geom::{Affine, Rect};

    #[test]
    fn pdfx3_parses_standard_names() {
        for s in ["x3", "pdfx3", "PDF/X-3", "PDF/X-3:2002", "PDF/X-3:2003"] {
            assert_eq!(Standard::parse(s), Some(Standard::PdfX3), "{s}");
        }
        assert!(Standard::PdfX3.is_pdfx());
    }

    #[test]
    fn pdfx3_export_produces_valid_pdf_1_4_and_preserves_icc_rgb_images() {
        let mut d = Document::new(&NewDocument::default());
        let lid = d.default_layer();
        let gold = Color::cmyk(0.0, 0.25, 0.9, 0.05);
        d.swatches.push(Swatch { value: SwatchValue::Color { color: gold, color_type: ColorType::Spot }, ..Swatch::color("Gold", gold) });
        let mut rect_item = Item::new(
            ItemId(d.alloc()),
            lid,
            designcraft_doc::Shape::Rectangle,
            designcraft_geom::shapes::rectangle(Rect::new(36.0, 36.0, 200.0, 140.0)),
        );
        rect_item.fill = Fill::swatch("Gold");
        d.insert_item(SpreadRef::Doc(0), rect_item, None).unwrap();
        d.add_text_frame(SpreadRef::Doc(0), Rect::new(36.0, 180.0, 320.0, 260.0), lid, "PDF/X-3 Live Text", ParaFormat::default()).unwrap();

        // Place an RGB PNG image — PDF/X-3 tags it with sRGB ICC v2 (`[/ICCBased ...]`), which
        // PDF/X-3 allows whereas PDF/X-1a would reject unless converted to CMYK.
        let mut img_buf = image::RgbImage::new(4, 4);
        for p in img_buf.pixels_mut() {
            *p = image::Rgb([220, 40, 90]);
        }
        let mut png_bytes = Vec::new();
        img_buf.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png).unwrap();
        let aid = AssetId(d.alloc());
        d.add_asset(Asset { id: aid, name: "rgb_sample.png".into(), mime: "image/png".into(), data: Arc::new(png_bytes), ..Default::default() });
        let mut pic_item = Item::new(
            ItemId(d.alloc()),
            lid,
            designcraft_doc::Shape::Rectangle,
            designcraft_geom::shapes::rectangle(Rect::new(220.0, 36.0, 320.0, 136.0)),
        );
        pic_item.stroke.weight = 0.0;
        pic_item.content = Content::Graphic(Graphic {
            layer_options: None,
            asset: aid,
            size: (100.0, 100.0),
            xf: Affine::translate((220.0, 36.0)),
            clip: None,
            auto_fit: Default::default(),
            fit_align: 4,
            crop: [0.0; 4],
            wrap: Default::default(),
            fill: Default::default(),
        });
        d.insert_item(SpreadRef::Doc(0), pic_item, None).unwrap();

        let report = export_pdf_with_report(&d, &Cache::new(), &PdfOptions { standard: Standard::PdfX3, ..PdfOptions::default() }).unwrap();
        assert!(report.bytes.starts_with(b"%PDF-1.4"));
        assert_eq!(check_pdfx3(&report.bytes), Vec::<String>::new(), "warnings: {:?}", report.warnings);
        assert_eq!(crate::pdfx_version(&report.bytes).as_deref(), Some("PDF/X-3:2003"));
        let text = String::from_utf8_lossy(&report.bytes);
        assert!(text.contains("/ICCBased"), "PDF/X-3 preserves ICCBased sRGB image space");
        assert!(
            crate::check_pdfx1a(&report.bytes).iter().any(|e| e.contains("ICC-based") || e.contains("PDF/X-1")),
            "PDF/X-3 file with ICCBased RGB is distinct from PDF/X-1a"
        );
    }

    #[test]
    fn pdfx3_flattens_live_transparency_automatically() {
        let mut d = Document::new(&NewDocument::default());
        let lid = d.default_layer();
        let mut item = Item::new(
            ItemId(d.alloc()),
            lid,
            designcraft_doc::Shape::Rectangle,
            designcraft_geom::shapes::rectangle(Rect::new(50.0, 50.0, 200.0, 200.0)),
        );
        item.fill = Fill::swatch("C=100 M=0 Y=0 K=0");
        item.opacity = 0.5;
        d.insert_item(SpreadRef::Doc(0), item, None).unwrap();

        let report =
            export_pdf_with_report(&d, &Cache::new(), &PdfOptions { standard: Standard::PdfX3, flatten_dpi: 72.0, ..PdfOptions::default() }).unwrap();
        assert_eq!(check_pdfx3(&report.bytes), Vec::<String>::new());
    }
}
