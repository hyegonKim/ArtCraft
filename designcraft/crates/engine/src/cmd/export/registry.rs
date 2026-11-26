use super::formats::{epub_split_param, export_epub, export_html, export_pptx, export_text, has_story_target, print_booklet, write_file};
use super::pdf::{export_pdf, flatten_ppi, options_with_session};
use super::{CommandSpec, Result, Session, Value, bad, cmd, has_doc, json, str_param};
use crate::EngineError;
use crate::cmd::always;

fn preset_json(p: &designcraft_pdf::PdfPreset, builtin: bool) -> Value {
    json!({
        "name": p.name,
        "description": p.description,
        "builtin": builtin,
        "compatibilityLevel": p.compatibility_level,
        "standard": p.standard.code(),
        "spreads": p.spreads,
        "bleed": p.use_document_bleed,
        "marks": {
            "crop": p.marks.crop,
            "bleed": p.marks.bleed,
            "pageInfo": p.marks.page_info,
            "weight": p.marks.weight,
            "offset": p.marks.offset,
        },
        "compressImages": p.compress_images,
        "compressPages": p.compress_pages,
        "colorImageDpi": p.color_image_dpi,
        "tagged": p.tagged,
        "bookmarks": p.bookmarks,
        "hyperlinks": p.hyperlinks,
        "media": p.interactive_elements,
        "flattenDpi": p.flatten_dpi,
        "outputIntentProfile": p.output_intent_profile,
    })
}

fn build_preset_from_params(id: &str, s: &Session, p: &Value, default_name: &str) -> Result<designcraft_pdf::PdfPreset> {
    let name = str_param(p, "name").filter(|n| !n.trim().is_empty()).unwrap_or(default_name).to_string();
    let opts = options_with_session(Some(s), p, 1)?;
    let mut preset = designcraft_pdf::PdfPreset::from_options(name, &opts);
    if let Some(ppi) = flatten_ppi(id, p)? {
        preset.flatten_dpi = ppi as f32;
    }
    if let Some(desc) = str_param(p, "description") {
        preset.description = desc.to_string();
    }
    Ok(preset)
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(noundo "file.exportPdf", "Export PDF…", ["File"], None,
        "{path?, preset?: str, joboptionsPath?: path, pages?: \"1-3,5\" | [1,3] (1-based positions: \"1\" is the first page, which ui.render and app.export take as page 0; default all), flatten?: high|medium|low|ppi (Transparency Flattener: spreads with transparency are rasterised; with standard x1a or x3, the resolution complex transparent regions are rasterized at: high 300, medium 150, low 72, default 300), spreads?: bool, fullScreen?: bool, bookmarksPanel?: bool, pageLayout?: single|continuous|twoUp|twoUpCover|twoUpContinuous, view?: fitPage|fitWidth|actual, advanceSeconds?: number (interactive PDF), bleed?: bool (document bleed), marks?: bool | {crop?, bleed?, pageInfo?, weight?, offset?}, standard?: \"none\"|\"x4\"|\"x3\"|\"x1a\"|\"a2b\" (x3: PDF/X-3:2003, PDF 1.4 with flattened transparency and ICC-tagged RGB allowed; x1a: PDF/X-1a:2003, CMYK/gray/spot only), compressImages?: bool, tagged?: bool (structure tree: stories as paragraphs, figures with alt text), media?: bool (interactive: embed placed video and sound), title?, author?, created?: int (creation date in Unix seconds; default now, for reproducible files)} → {path, bytes, pages, warnings} (no path: {base64, …})",
        has_doc, export_pdf),
        cmd!(query "pdfPreset.list", "Adobe PDF Presets", ["File", "Adobe PDF Presets"], None,
        "{} → {presets: [{name, description, builtin, compatibilityLevel, standard, spreads, bleed, marks, compressImages, tagged, media, flattenDpi}]}",
        always, |s, _| {
            let mut out: Vec<Value> = designcraft_pdf::PdfPreset::builtins()
                .iter()
                .map(|pr| preset_json(pr, true))
                .collect();
            for pr in &s.pdf_presets {
                out.push(preset_json(pr, false));
            }
            Ok(json!({"presets": out}))
        }),
        cmd!(noundo "pdfPreset.save", "Define PDF Preset…", [], None,
        "{name, description?, preset?, standard?, spreads?, bleed?, marks?, compressImages?, tagged?, media?, flatten?} → {preset}",
        always, |s, p| {
            let name = str_param(p, "name").filter(|n| !n.trim().is_empty()).ok_or_else(|| bad("pdfPreset.save", "`name` required"))?;
            if designcraft_pdf::PdfPreset::builtins().iter().any(|b| b.name == name) {
                return Err(bad("pdfPreset.save", format!("cannot overwrite built-in preset `{name}`")));
            }
            let preset = build_preset_from_params("pdfPreset.save", s, p, name)?;
            let js = preset_json(&preset, false);
            if let Some(existing) = s.pdf_presets.iter_mut().find(|pr| pr.name == preset.name) {
                *existing = preset;
            } else {
                s.pdf_presets.push(preset);
            }
            Ok(json!({"preset": js}))
        }),
        cmd!(noundo "pdfPreset.delete", "Delete PDF Preset", [], None,
        "{name} → {deleted}",
        always, |s, p| {
            let name = str_param(p, "name").ok_or_else(|| bad("pdfPreset.delete", "`name` required"))?;
            let before = s.pdf_presets.len();
            s.pdf_presets.retain(|pr| pr.name != name);
            Ok(json!({"deleted": s.pdf_presets.len() < before}))
        }),
        cmd!(noundo "pdfPreset.importJoboptions", "Load .joboptions Preset…", ["File", "Adobe PDF Presets"], None,
        "{path?, content?: str, name?: str} → {preset}",
        always, |s, p| {
            let fallback = str_param(p, "name")
                .or_else(|| str_param(p, "path").and_then(|pt| std::path::Path::new(pt).file_stem().and_then(|st| st.to_str())))
                .unwrap_or("Imported Preset");
            let mut preset = if let Some(content) = str_param(p, "content") {
                designcraft_pdf::parse_joboptions(content, fallback).map_err(|e| bad("pdfPreset.importJoboptions", e))?
            } else if let Some(path) = str_param(p, "path") {
                #[cfg(not(target_arch = "wasm32"))]
                let bytes = crate::io::read_file(path).map_err(EngineError::Other)?;
                #[cfg(target_arch = "wasm32")]
                let bytes: Vec<u8> = {
                    let _ = path;
                    return Err(EngineError::Other("importing .joboptions from disk requires a file system".into()));
                };
                designcraft_pdf::parse_joboptions_bytes(&bytes, fallback).map_err(|e| bad("pdfPreset.importJoboptions", e))?
            } else {
                return Err(bad("pdfPreset.importJoboptions", "`path` or `content` required"));
            };
            if let Some(explicit) = str_param(p, "name").filter(|n| !n.trim().is_empty()) {
                preset.name = explicit.to_string();
            }
            let js = preset_json(&preset, false);
            if let Some(existing) = s.pdf_presets.iter_mut().find(|pr| pr.name == preset.name) {
                *existing = preset;
            } else {
                s.pdf_presets.push(preset);
            }
            Ok(json!({"preset": js}))
        }),
        cmd!(noundo "pdfPreset.exportJoboptions", "Save .joboptions Preset…", ["File", "Adobe PDF Presets"], None,
        "{path?, name?, description?, preset?, standard?, spreads?, bleed?, marks?, compressImages?, tagged?, media?, flatten?} → {path?, joboptions, bytes}",
        always, |s, p| {
            let preset = if let Some(lookup) = str_param(p, "name").and_then(|n| {
                s.pdf_presets
                    .iter()
                    .find(|pr| pr.name == n)
                    .cloned()
                    .or_else(|| designcraft_pdf::PdfPreset::builtins().into_iter().find(|pr| pr.name == n))
            }) && p.get("standard").is_none() && p.get("marks").is_none() && p.get("bleed").is_none() {
                lookup
            } else {
                let fallback = str_param(p, "name")
                    .or_else(|| str_param(p, "path").and_then(|pt| std::path::Path::new(pt).file_stem().and_then(|st| st.to_str())))
                    .unwrap_or("DesignCraft Preset");
                build_preset_from_params("pdfPreset.exportJoboptions", s, p, fallback)?
            };
            let text = designcraft_pdf::write_joboptions(&preset);
            let bytes = text.as_bytes();
            if let Some(path) = str_param(p, "path") {
                write_file(path, bytes)?;
                Ok(json!({"path": path, "joboptions": text, "bytes": bytes.len()}))
            } else {
                Ok(json!({"joboptions": text, "bytes": bytes.len()}))
            }
        }),
        cmd!(noundo "file.exportEpub", "Export EPUB (Reflowable)…", ["File"], None,
        "{path?, title?, author?, language?: \"en\", identifier?: str (dc:identifier; a stable urn:uuid by default), coverImage?: path (a PNG or JPEG as the cover), tocTitle?: str (the table of contents' title; from the language by default), cover?: bool (the first page as the cover image), embedFonts?: bool (package the stylesheet's fonts whose licence allows it, whole files), split?: \"off\"|\"exportTags\"|{style: name} (Split Document for this export, not stored; the document's `document.epubSplit` by default), fixedLayout?: bool (pre-paginated: true XHTML5 + CSS3 fixed-layout pages)} → {path, bytes, warnings} (no path: {base64, bytes, warnings}); warnings name images without alt text, exported as decorative, and fonts that could not be embedded",
        has_doc, export_epub),
        cmd!(
            "document.epubSplit",
            "EPUB Split Document",
            [],
            None,
            "{split?: \"off\"|\"exportTags\"|{style: name}} — EPUB Export Options › Split Document, stored with the document: one content file (off, the default), a file before each paragraph whose style has Export Tagging › Split Document on (exportTags), or before each paragraph of one style → {split}",
            has_doc,
            |s, p| {
                const ID: &str = "document.epubSplit";
                let cur = s.doc()?.doc.settings.epub_split.clone();
                let Some(v) = p.get("split") else { return Ok(json!({"split": cur})) };
                let split = epub_split_param(ID, &s.doc()?.doc, v)?;
                if split != cur {
                    let stored = split.clone();
                    s.edit(move |d, _| {
                        d.settings.epub_split = stored;
                        Ok(Value::Null)
                    })?;
                }
                Ok(json!({"split": split}))
            }
        ),
        cmd!(noundo "file.exportFixedEpub", "Export EPUB (Fixed Layout)…", ["File"], None,
        "{path?, title?, author?, language?, embedFonts?: bool} — pre-paginated HTML5+CSS3 EPUB 3.0 → {path, bytes, pages, warnings} (no path: {base64, bytes, pages, warnings})",
        has_doc, |s, p| {
            export_epub(s, &crate::cmd::with_param(p, "fixedLayout", serde_json::json!(true)))
        }),
        cmd!(noundo "file.printBooklet", "Print Booklet…", ["File"], None,
        "{path?, type?: saddleStitch|twoUpConsecutive, spaceBetween? (pt)} — printer spreads as PDF (pages imposed in booklet order) → {path, bytes, sheets} (no path: {base64, …})",
        has_doc, print_booklet),
        cmd!(noundo "file.exportHtml", "Export HTML…", ["File"], None,
        "{path?, title?, language?} — one self-contained page (styles inline, images embedded), stories and graphics in reading order → {path, bytes, warnings} (no path: {text, bytes, warnings})",
        has_doc, export_html),
        cmd!(noundo "file.exportPptx", "Export PowerPoint…", ["File"], None,
        "{path?, pages?: \"1-3,5\" | [1,3] (1-based positions; default all), lineBreaks?: keep|reflow (keep: DesignCraft's lines, spacing and leading; reflow: one box per frame, PowerPoint sets the lines), rasterize?: bool (default true: objects PowerPoint can't show — placed PDF/EPS, type on a path, some effects — go in as 300 ppi images), embedFonts?: bool (default true: the TrueType fonts whose licences allow it), title?, author?} — every page as a slide of editable shapes, pictures, text boxes and tables → {path, bytes, slides, warnings} (no path: {base64, …})",
        has_doc, export_pptx),
        cmd!(noundo "file.exportText", "Export Text…", ["File"], None,
        "{path?, format?: \"txt\"|\"rtf\"|\"tagged\" (Tagged Text; default from the path, else txt), story?, frame?} — the story being edited or of the selected frame → {path, bytes} (no path: {text, bytes})",
        has_story_target, export_text),
    ]
}
