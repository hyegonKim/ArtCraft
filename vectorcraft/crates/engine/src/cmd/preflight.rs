//! Pre-export analyzer (`document.preflight` — Illustrator 2027 `31.0` `Review Before Export`),
//! decomposed white-on-black text mask exporter (`document.exportTextWhiteOnBlackPng`), and plain
//! text extractor (`document.extractAiText`).

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint, rgb_to_hsb};
use vectorcraft_doc::{ColorMode, Document, Node, NodeKind};
use vectorcraft_geom::Rect;
use vectorcraft_render::encode::{RasterExportOptions, RasterFormat};
use vectorcraft_text::FontDb;

use super::fileio::DecomposedMode;
use super::{CommandSpec, Result, Session, always, bad, cmd, has_doc, number_or, str_param, text_param};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "document.preflight",
            "Review Before Export…",
            ["File", "Export"],
            None,
            "{target?: \"print\"|\"web\"|\"svg\"|\"png\"|\"pdf\", minPpi?: number=150} analyze the active document before export for out-of-gamut colors, missing or unoutlined fonts, stray points, low-DPI raster images, and artboard or bleed bounds → {passed, errorCount, warningCount, issues, summary, recommendedFormat, recommendedSettings}",
            has_doc,
            run_preflight
        ),
        cmd!(
            query "document.exportTextWhiteOnBlackPng",
            "Export Text Mask (White on Black)",
            [],
            None,
            "{path?: string, ppi?: number=72, artboard?: int=0} export a decomposed white-text-on-black-background PNG mask for the active document → {path?, dataBase64?, width, height, textObjects}",
            has_doc,
            run_export_text_mask
        ),
        cmd!(
            query "document.extractAiText",
            "Extract Text from AI/Document",
            [],
            None,
            "{path?: string} extract plain text stories and metadata from an .ai, PDF, SVG, or VectorCraft file or the active document → {text, textObjects, stories, title}",
            always,
            run_extract_ai_text
        ),
    ]
}

#[derive(Default)]
struct ScanStats {
    issues: Vec<Value>,
    errors: usize,
    warnings: usize,
    text_objects: usize,
    raster_objects: usize,
    has_transparency: bool,
}

impl ScanStats {
    fn push(&mut self, severity: &str, code: &str, message: String, node_id: Option<u64>) {
        match severity {
            "error" => self.errors += 1,
            "warning" => self.warnings += 1,
            _ => {}
        }
        let mut item = json!({
            "severity": severity,
            "code": code,
            "message": message,
        });
        if let Some(id) = node_id {
            item["nodeId"] = json!(id);
        }
        self.issues.push(item);
    }
}

fn is_rgb_out_of_cmyk_gamut(c: &Color) -> bool {
    let Color::Rgb { r, g, b } = *c else { return false };
    let [h, sat, bri] = rgb_to_hsb([r, g, b]);
    // Vivid neon greens, electric blues/cyans, and bright magentas exceed coated CMYK gamut.
    sat > 0.82 && bri > 0.78 && ((95.0..=165.0).contains(&h) || (195.0..=255.0).contains(&h) || (295.0..=335.0).contains(&h))
}

fn check_paint_gamut(paint: &Paint, nid: u64, stats: &mut ScanStats, flagged: &mut usize) {
    let mut hit = false;
    match paint {
        Paint::Solid { color: c, .. } => hit = is_rgb_out_of_cmyk_gamut(c),
        Paint::Gradient(g) => hit = g.gradient.stops.iter().any(|st| is_rgb_out_of_cmyk_gamut(&st.color)),
        Paint::None | Paint::Pattern { .. } => {}
    }
    if hit && *flagged < 10 {
        *flagged += 1;
        stats.push(
            "warning",
            "outOfGamutColor",
            format!("Object #{nid} uses high-chroma RGB colors outside standard CMYK print gamut"),
            Some(nid),
        );
    }
}

fn check_node(
    node: &Node,
    doc: &Document,
    target_print: bool,
    min_ppi: f64,
    stats: &mut ScanStats,
    gamut_flagged: &mut usize,
) {
    if !node.visible {
        return;
    }
    if node.opacity < 0.999 || node.blend != vectorcraft_color::BlendMode::Normal {
        stats.has_transparency = true;
    }
    if target_print {
        for item in &node.appearance.items {
            if item.visible() {
                check_paint_gamut(item.paint(), node.id.0, stats, gamut_flagged);
            }
        }
    }
    match &node.kind {
        NodeKind::Path { path, guide: false, clipping: false, .. } => {
            let stray = !path.subpaths.is_empty() && path.subpaths.iter().all(|sp| sp.anchors.len() <= 1);
            if stray {
                stats.push(
                    "warning",
                    "strayPoint",
                    format!("Path #{} is an isolated single-anchor stray point", node.id.0),
                    Some(node.id.0),
                );
            }
        }
        NodeKind::Text(t) => {
            stats.text_objects += 1;
            let db = FontDb::global();
            for run in &t.runs {
                let fam = run.style.font_family.trim();
                if fam.is_empty() {
                    continue;
                }
                let missing = match db.resolve(fam, &run.style.font_style) {
                    None => true,
                    Some((face, _)) => !face.family.eq_ignore_ascii_case(fam),
                };
                if missing {
                    stats.push(
                        "error",
                        "missingFont",
                        format!("Font family `{fam}` ({}) on text #{} is missing and will substitute", run.style.font_style, node.id.0),
                        Some(node.id.0),
                    );
                    break;
                }
            }
        }
        NodeKind::Image(img) => {
            stats.raster_objects += 1;
            if !doc.images.contains_key(&img.key) && img.link.is_none() {
                stats.push(
                    "error",
                    "missingLink",
                    format!("Placed image #{} has missing or empty pixel data", node.id.0),
                    Some(node.id.0),
                );
            } else if img.width > 0 && img.height > 0 && let Some(b) = node.geometric_bounds() {
                let eff_x = if b.width() > 1e-6 { f64::from(img.width) / (b.width() / 72.0) } else { f64::INFINITY };
                let eff_y = if b.height() > 1e-6 { f64::from(img.height) / (b.height() / 72.0) } else { f64::INFINITY };
                let eff = eff_x.min(eff_y);
                if eff.is_finite() && eff < min_ppi {
                    stats.push(
                        "warning",
                        "lowResolutionImage",
                        format!("Image #{} effective resolution is {:.0} PPI (below {:.0} PPI threshold)", node.id.0, eff, min_ppi),
                        Some(node.id.0),
                    );
                }
            }
        }
        _ => {}
    }
    if let Some(children) = node.children() {
        for ch in children {
            if let NodeKind::Layer { template: true, .. } = ch.kind {
                continue;
            }
            check_node(ch, doc, target_print, min_ppi, stats, gamut_flagged);
        }
    }
}

fn check_artboard_and_bleed(doc: &Document, target_print: bool, stats: &mut ScanStats) {
    if target_print {
        let b = doc.setup.bleed;
        if b.iter().all(|&v| v <= 0.0) {
            stats.push(
                "warning",
                "missingBleed",
                "Document bleed is 0 pt on all sides; 9 pt (0.125 in) is recommended for print".into(),
                None,
            );
        }
    }
    if doc.artboards.is_empty() {
        return;
    }
    for layer in &doc.layers {
        let NodeKind::Layer { template: false, children, .. } = &layer.kind else { continue };
        if !layer.visible {
            continue;
        }
        for obj in children {
            if !obj.visible {
                continue;
            }
            if let Some(gb) = obj.geometric_bounds() {
                let touches_any = doc
                    .artboards
                    .iter()
                    .any(|ab| gb.x0 <= ab.rect.x1 && ab.rect.x0 <= gb.x1 && gb.y0 <= ab.rect.y1 && ab.rect.y0 <= gb.y1);
                if !touches_any {
                    stats.push(
                        "warning",
                        "outsideArtboard",
                        format!("Object #{} lies completely outside all artboards and will be clipped on artboard export", obj.id.0),
                        Some(obj.id.0),
                    );
                }
            }
        }
    }
}

fn recommend_export(target: &str, doc: &Document, stats: &ScanStats) -> (&'static str, Value) {
    match target {
        "print" | "pdf" => (
            "pdf",
            json!({
                "preset": "Press Quality",
                "includeBleed": doc.setup.bleed.iter().any(|&v| v > 0.0),
                "colorMode": if doc.color_mode == ColorMode::Cmyk { "cmyk" } else { "rgb" },
                "outlineFontsRecommended": stats.text_objects > 0 && stats.errors > 0,
            }),
        ),
        "svg" => (
            "svg",
            json!({
                "outlineText": stats.errors > 0,
                "responsive": true,
                "decimals": 2,
            }),
        ),
        "png" => (
            "png",
            json!({
                "ppi": 144,
                "background": if stats.has_transparency { "transparent" } else { "white" },
                "antiAlias": if stats.text_objects > 0 { "type" } else { "art" },
            }),
        ),
        _ => {
            if stats.raster_objects == 0 {
                ("svg", json!({"responsive": true, "decimals": 2}))
            } else {
                ("png", json!({"ppi": 144, "antiAlias": "art"}))
            }
        }
    }
}

fn run_preflight(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "document.preflight";
    let target = match text_param(p, "target", C)?.unwrap_or("web") {
        t @ ("print" | "web" | "svg" | "png" | "pdf") => t,
        other => return Err(bad(C, format!("unknown target `{other}` (print|web|svg|png|pdf)"))),
    };
    let default_ppi = if matches!(target, "print" | "pdf") { 300.0 } else { 150.0 };
    let min_ppi = number_or(p, "minPpi", default_ppi, C)?;
    if !min_ppi.is_finite() || !(10.0..=2400.0).contains(&min_ppi) {
        return Err(bad(C, "`minPpi` must be between 10 and 2400"));
    }

    let doc = &s.doc()?.doc;
    let target_print = matches!(target, "print" | "pdf") || doc.color_mode == ColorMode::Cmyk;
    let mut stats = ScanStats::default();
    let mut gamut_flagged = 0usize;
    for layer in &doc.layers {
        if let NodeKind::Layer { template: true, .. } = layer.kind {
            continue;
        }
        check_node(layer, doc, target_print, min_ppi, &mut stats, &mut gamut_flagged);
    }
    if target == "print" && stats.text_objects > 0 {
        stats.push(
            "info",
            "liveTextPresent",
            format!("{} live text object(s) detected; embed fonts or outline type for press-ready delivery", stats.text_objects),
            None,
        );
    }
    check_artboard_and_bleed(doc, matches!(target, "print" | "pdf"), &mut stats);

    let (rec_fmt, rec_settings) = recommend_export(target, doc, &stats);
    let passed = stats.errors == 0;
    let summary = if stats.errors == 0 && stats.warnings == 0 {
        format!("Ready for {target} export: no errors or warnings found.")
    } else {
        format!("Preflight found {} error(s) and {} warning(s) for {target} export.", stats.errors, stats.warnings)
    };
    Ok(json!({
        "passed": passed,
        "errorCount": stats.errors,
        "warningCount": stats.warnings,
        "issues": stats.issues,
        "summary": summary,
        "recommendedFormat": rec_fmt,
        "recommendedSettings": rec_settings,
    }))
}

fn count_text_nodes(doc: &Document) -> usize {
    let mut count = 0usize;
    for layer in &doc.layers {
        if !layer.visible {
            continue;
        }
        layer.walk(&mut |n| {
            if n.visible && matches!(n.kind, NodeKind::Text(_)) {
                count += 1;
            }
        });
    }
    count
}

fn run_export_text_mask(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "document.exportTextWhiteOnBlackPng";
    let ppi = number_or(p, "ppi", 72.0, C)?;
    if !ppi.is_finite() || !(1.0..=2400.0).contains(&ppi) {
        return Err(bad(C, "`ppi` must be between 1 and 2400"));
    }
    let ab_f = number_or(p, "artboard", 0.0, C)?;
    if !ab_f.is_finite() || ab_f < 0.0 || ab_f.fract() != 0.0 {
        return Err(bad(C, "`artboard` must be a 0-based integer"));
    }
    let ab_idx = ab_f as usize;
    let doc = &s.doc()?.doc;
    let region: Rect = doc
        .artboards
        .get(ab_idx)
        .ok_or_else(|| bad(C, format!("no artboard {ab_idx} (the document has {})", doc.artboards.len())))?
        .rect;

    let text_count = count_text_nodes(doc);
    let masked_doc = super::fileio::apply_decomposed_mode(doc, DecomposedMode::TextWhiteOnBlack);
    let opts = RasterExportOptions {
        ppi,
        background: Some([0, 0, 0]),
        ..RasterExportOptions::default()
    };
    let (w, h) = vectorcraft_render::raster_size(region, opts.scale()).map_err(|e| bad(C, e))?;
    let bytes = vectorcraft_render::Renderer::new()
        .export_region(&masked_doc, region, RasterFormat::Png, &opts)
        .map_err(crate::EngineError::Other)?;

    if let Some(path) = str_param(p, "path").map(str::trim).filter(|s| !s.is_empty()) {
        super::fileio::write_file(path, &bytes)?;
        Ok(json!({"path": path, "width": w, "height": h, "textObjects": text_count}))
    } else {
        Ok(json!({
            "dataBase64": vectorcraft_format::base64_encode(&bytes),
            "width": w,
            "height": h,
            "textObjects": text_count,
        }))
    }
}

fn collect_stories(doc: &Document) -> Vec<Value> {
    let mut stories = Vec::new();
    for layer in &doc.layers {
        let layer_name = layer.display_name().to_string();
        layer.walk(&mut |n| {
            if let NodeKind::Text(t) = &n.kind {
                let text = t.plain_text();
                let font = t.runs.first().map_or("", |r| r.style.font_family.as_str());
                let size = t.runs.first().map_or(12.0, |r| r.style.size);
                stories.push(json!({
                    "id": n.id.0,
                    "layer": layer_name,
                    "text": text,
                    "font": font,
                    "size": size,
                }));
            }
        });
    }
    stories
}

fn run_extract_ai_text(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "document.extractAiText";
    if let Some(path) = text_param(p, "path", C)?.map(str::trim).filter(|s| !s.is_empty()) {
        let bytes = super::fileio::read_file(path)?;
        let loaded = super::fileio::load(path, &bytes)?;
        let stories = collect_stories(&loaded.doc);
        let joined: Vec<&str> = stories.iter().filter_map(|st| st["text"].as_str()).collect();
        return Ok(json!({
            "title": loaded.doc.info_title(),
            "textObjects": stories.len(),
            "stories": stories,
            "text": joined.join("\n"),
        }));
    }
    // Enabled without a document, since a `path` needs none: with neither, say what to give.
    let Some(state) = s.active() else {
        return Err(bad(C, "give a `path`, or open a document to read the active one"));
    };
    let doc = &state.doc;
    let stories = collect_stories(doc);
    let joined: Vec<&str> = stories.iter().filter_map(|st| st["text"].as_str()).collect();
    Ok(json!({
        "title": doc.info_title(),
        "textObjects": stories.len(),
        "stories": stories,
        "text": joined.join("\n"),
    }))
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use crate::Session;

    #[test]
    fn preflight_detects_issues_and_decomposed_text_mask_and_extraction_work() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 200, "height": 120})).unwrap();
        // Add a vivid neon green shape (out of CMYK gamut) and a stray point + live text.
        let r = s.execute("shape.rectangle", &json!({"x": 20, "y": 20, "width": 80, "height": 40})).unwrap();
        s.execute("paint.setFill", &json!({"ids": [r["id"]], "color": "#00ff22"})).unwrap();
        s.execute("path.create", &json!({"anchors": [{"x": 250, "y": 250}], "closed": false})).unwrap();
        s.execute("text.create", &json!({"x": 30, "y": 60, "text": "Illustrator 2027", "size": 24, "color": "#ff3300"})).unwrap();

        let pf = s.execute("document.preflight", &json!({"target": "print"})).unwrap();
        assert!(pf["warningCount"].as_u64().unwrap() >= 3, "expected gamut + stray + bleed/outside warnings: {pf}");
        assert_eq!(pf["recommendedFormat"], "pdf");

        let mask = s.execute("document.exportTextWhiteOnBlackPng", &json!({"ppi": 72})).unwrap();
        assert_eq!(mask["width"], 200);
        assert_eq!(mask["height"], 120);
        assert_eq!(mask["textObjects"], 1);
        let bytes = vectorcraft_format::base64_decode(mask["dataBase64"].as_str().unwrap()).unwrap();
        let img = image::load_from_memory(&bytes).unwrap().to_rgba8();
        // Corner pixel (2, 2) must be pure black background; some interior text pixel must be white (> 200).
        assert_eq!(img.get_pixel(2, 2).0, [0, 0, 0, 255]);
        assert!(img.pixels().any(|px| px.0[0] > 200 && px.0[1] > 200 && px.0[2] > 200));

        // Verify `document.export` decomposed modes ("noText" vs "textOnly" vs "textWhiteOnBlack").
        let no_txt = s.execute("document.export", &json!({"format": "png", "decomposedMode": "noText", "background": "black"})).unwrap();
        let no_txt_img = image::load_from_memory(&vectorcraft_format::base64_decode(no_txt["dataBase64"].as_str().unwrap()).unwrap()).unwrap().to_rgba8();
        assert_eq!(no_txt_img.get_pixel(40, 40).0, [0, 255, 34, 255], "neon green rectangle stays in noText mode");

        let txt_only = s.execute("document.export", &json!({"format": "png", "decomposedMode": "textOnly"})).unwrap();
        let txt_only_img = image::load_from_memory(&vectorcraft_format::base64_decode(txt_only["dataBase64"].as_str().unwrap()).unwrap()).unwrap().to_rgba8();
        assert_eq!(txt_only_img.get_pixel(25, 25).0[3], 0, "non-text rectangle is omitted (transparent) in textOnly mode");

        let extracted = s.execute("document.extractAiText", &json!({})).unwrap();
        assert_eq!(extracted["text"], "Illustrator 2027");
        assert_eq!(extracted["textObjects"], 1);
    }
}
