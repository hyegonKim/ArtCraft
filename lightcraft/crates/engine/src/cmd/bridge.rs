//! DAM workflow commands (`bridge.*`):
//! - `bridge.batchRename`: Token-based batch rename (`{date}`, `{seq:04}`, `{camera}`, `{iso}`,
//!   `{rating}`, `{orig}`) with collision-safe preview (`preview: true`) and undoable execution.
//! - `bridge.contactSheet`: Configurable `rows × cols` contact sheet layout with custom margins and
//!   filename + EXIF caption strips, rendering RGBA buffers, PNG/JPEG images, or paginated PDFs.
//! - `bridge.applyMetadataTemplate`: Batch IPTC/XMP metadata stamp (`creator`, `copyright`,
//!   `copyrightStatus`, `credit`, `usageTerms`, `keywords`, `rating`) in a single undo step.

use lightcraft_catalog::{CopyrightStatus, Meta, Op, Photo};
use lightcraft_pipeline::{OutputDepth, OutputSpace};
use lightcraft_raster::Rgba8;
use serde_json::{Value, json};

use super::{CommandSpec, bad, bool_or, cmd, has_selection, str_param};
use crate::export::{Anchor, ExportFormat, ExportOptions, Watermark, draw_watermark, encode_image};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec {
            explicit_targets: true,
            ..cmd!(
                "bridge.batchRename",
                "Batch Rename",
                ["Photo"],
                None,
                "{template: e.g. `{date}_{orig}_{seq:04}` (tokens: {date} {seq:04} {camera} {iso} {rating} {orig} {name} {lens} {ext}), start?: 1, preview?: false, ids?} — collision-safe preview or undoable batch rename",
                has_selection,
                batch_rename
            )
        },
        CommandSpec {
            explicit_targets: true,
            ..cmd!(
                "bridge.contactSheet",
                "Contact Sheet",
                [],
                None,
                "{rows?: 4, cols?: 3, columns?: 3, paper?: 'a4'|'letter', landscape?: false, margin?: 60, margins?: {top, right, bottom, left}, cellSpacing?: 16, captions?: true, showExif?: true, format?: 'pdf'|'png'|'jpeg'|'rgba', path?, ids?} — renders a paginated contact sheet with filename/EXIF caption strips",
                has_selection,
                contact_sheet
            )
        },
        CommandSpec {
            explicit_targets: true,
            ..cmd!(
                "bridge.applyMetadataTemplate",
                "Apply Metadata Template",
                ["Photo"],
                None,
                "{creator?, copyright?, copyrightStatus?: unknown|copyrighted|publicDomain, credit?, usageTerms?, keywords?: [..], keywordMode?: 'append'|'replace', rating?: 0..5, ids?} — batch stamps IPTC/XMP metadata and rating in one undo step",
                has_selection,
                apply_metadata_template
            )
        },
    ]
}

// ─── 1. Token-Based Batch Rename (`bridge.batchRename`) ─────────────────────────────────────────

fn batch_rename(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "bridge.batchRename";
    let template = str_param(p, "template").ok_or_else(|| bad(C, "missing `template` (e.g. `{date}_{orig}_{seq:04}`)"))?.to_string();
    let start_raw = p.get("start").and_then(Value::as_u64).unwrap_or(1);
    let ids = s.targets(p)?;
    if ids.is_empty() {
        return Err(bad(C, "no photos selected"));
    }
    let last = start_raw.checked_add((ids.len() as u64).saturating_sub(1));
    let start = last
        .and_then(|l| usize::try_from(l).ok())
        .and_then(|_| usize::try_from(start_raw).ok())
        .ok_or_else(|| bad(C, "`start` sequence number out of range"))?;

    let preview_only = bool_or(p, "preview", false) || bool_or(p, "dryRun", false);
    let plans = s.plan_rename(&ids, &template, start);
    let unknown = crate::rename::unknown_tokens(&template);

    if preview_only {
        return Ok(json!({
            "preview": true,
            "renamed": 0,
            "plans": plans,
            "unknownTokens": unknown,
        }));
    }

    let renamed = s.apply_rename(&plans)?;
    Ok(json!({
        "preview": false,
        "renamed": renamed,
        "plans": plans,
        "unknownTokens": unknown,
    }))
}

// ─── 2. Contact Sheet Layout & Export (`bridge.contactSheet`) ───────────────────────────────────

#[derive(Clone, Copy, Debug)]
struct Margins {
    top: usize,
    right: usize,
    bottom: usize,
    left: usize,
}

#[derive(Clone, Debug)]
struct SheetLayout {
    points: (f64, f64),
    width: usize,
    height: usize,
    cols: usize,
    rows: usize,
    margins: Margins,
    spacing: usize,
    cell_w: usize,
    cell_h: usize,
    captions: bool,
    show_exif: bool,
}

fn parse_margins(p: &Value, cmd_id: &str) -> Result<Margins> {
    let to_px = |v: f64| -> Result<usize> {
        if !v.is_finite() || !(0.0..=400.0).contains(&v) {
            return Err(bad(cmd_id, "margins must be between 0 and 400 px"));
        }
        Ok(v.round() as usize)
    };
    if let Some(m) = p.get("margins").and_then(Value::as_object) {
        let get_side = |k: &str| m.get(k).and_then(Value::as_f64).unwrap_or(60.0);
        return Ok(Margins {
            top: to_px(get_side("top"))?,
            right: to_px(get_side("right"))?,
            bottom: to_px(get_side("bottom"))?,
            left: to_px(get_side("left"))?,
        });
    }
    if let Some(uniform) = p.get("margin").or_else(|| p.get("margins")).and_then(Value::as_f64) {
        let px = to_px(uniform)?;
        return Ok(Margins { top: px, right: px, bottom: px, left: px });
    }
    Ok(Margins { top: 60, right: 60, bottom: 60, left: 60 })
}

fn parse_sheet_layout(p: &Value, cmd_id: &str) -> Result<SheetLayout> {
    let cols_u64 = p.get("cols").or_else(|| p.get("columns")).and_then(Value::as_u64).unwrap_or(3);
    let rows_u64 = p.get("rows").and_then(Value::as_u64).unwrap_or(4);
    let cols = usize::try_from(cols_u64).unwrap_or(0);
    let rows = usize::try_from(rows_u64).unwrap_or(0);
    if !(1..=10).contains(&cols) || !(1..=12).contains(&rows) {
        return Err(bad(cmd_id, "columns/cols must be 1–10 and rows 1–12"));
    }

    let paper = str_param(p, "paper").unwrap_or("a4").to_ascii_lowercase();
    let (mut pt_w, mut pt_h) = match paper.as_str() {
        "a4" => (595.28_f64, 841.89_f64),
        "letter" => (612.0_f64, 792.0_f64),
        _ => return Err(bad(cmd_id, "paper must be `a4` or `letter`")),
    };
    if bool_or(p, "landscape", false) {
        std::mem::swap(&mut pt_w, &mut pt_h);
    }

    let dpi = p.get("dpi").and_then(Value::as_f64).filter(|d| d.is_finite()).unwrap_or(150.0).clamp(72.0, 300.0);
    let width = (pt_w * dpi / 72.0).round() as usize;
    let height = (pt_h * dpi / 72.0).round() as usize;

    let margins = parse_margins(p, cmd_id)?;
    let spacing =
        p.get("cellSpacing").and_then(Value::as_f64).filter(|v| v.is_finite() && *v >= 0.0).map_or(16_usize, |v| (v.round() as usize).min(80));

    let inner_w = width.saturating_sub(margins.left + margins.right + spacing * cols.saturating_sub(1));
    let inner_h = height.saturating_sub(margins.top + margins.bottom + spacing * rows.saturating_sub(1));
    let cell_w = inner_w / cols;
    let cell_h = inner_h / rows;
    if cell_w < 48 || cell_h < 64 {
        return Err(bad(cmd_id, "grid cells are too small for the chosen paper and margins"));
    }

    Ok(SheetLayout {
        points: (pt_w, pt_h),
        width,
        height,
        cols,
        rows,
        margins,
        spacing,
        cell_w,
        cell_h,
        captions: bool_or(p, "captions", true),
        show_exif: bool_or(p, "showExif", true),
    })
}

fn contact_sheet(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "bridge.contactSheet";
    let layout = parse_sheet_layout(p, C)?;
    let ids = s.targets(p)?;
    if ids.is_empty() || ids.len() > 1000 {
        return Err(bad(C, "select between 1 and 1000 photos"));
    }
    let capacity = layout.cols * layout.rows;
    if ids.len().div_ceil(capacity) > 100 {
        return Err(bad(C, "contact sheets are limited to 100 pages"));
    }

    let out_path = str_param(p, "path").map(str::trim).filter(|x| !x.is_empty());
    let fmt = resolve_sheet_format(p, out_path, C)?;
    if let Some(path) = out_path {
        s.check_write_target(path).map_err(|e| bad(C, e))?;
    }

    let caption_h = if !layout.captions {
        0
    } else if layout.show_exif {
        44
    } else {
        26
    };
    let thumb_max_w = (layout.cell_w.saturating_sub(8)).max(16);
    let thumb_max_h = (layout.cell_h.saturating_sub(caption_h + 8)).max(16);

    let mut jobs = Vec::with_capacity(ids.len());
    for id in &ids {
        let photo = s.catalog.photo(*id).ok_or_else(|| bad(C, "no such photo"))?.clone();
        let job = s.export_job(*id, thumb_max_w, thumb_max_h, OutputSpace::Srgb, OutputDepth::U8).map_err(|e| bad(C, e))?;
        jobs.push((photo, job));
    }

    let pages = render_sheet_pages(&jobs, &layout, caption_h).map_err(|e| bad(C, e))?;
    write_or_encode_sheet(&pages, &layout, fmt, out_path, ids.len()).map_err(|e| bad(C, e))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SheetOutput {
    Pdf,
    Png,
    Jpeg,
    Rgba,
}

fn resolve_sheet_format(p: &Value, path: Option<&str>, cmd_id: &str) -> Result<SheetOutput> {
    if let Some(f) = str_param(p, "format") {
        return match f.trim().to_ascii_lowercase().as_str() {
            "pdf" => Ok(SheetOutput::Pdf),
            "png" => Ok(SheetOutput::Png),
            "jpg" | "jpeg" => Ok(SheetOutput::Jpeg),
            "rgba" => Ok(SheetOutput::Rgba),
            other => Err(bad(cmd_id, format!("unknown format `{other}` (pdf, png, jpeg or rgba)"))),
        };
    }
    if let Some(path) = path {
        let lower = path.to_ascii_lowercase();
        if lower.ends_with(".png") {
            return Ok(SheetOutput::Png);
        }
        if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
            return Ok(SheetOutput::Jpeg);
        }
        return Ok(SheetOutput::Pdf);
    }
    Ok(SheetOutput::Rgba)
}

fn render_sheet_pages(
    jobs: &[(std::sync::Arc<Photo>, crate::media::RenderJob)],
    layout: &SheetLayout,
    caption_h: usize,
) -> std::result::Result<Vec<Rgba8>, String> {
    let capacity = layout.cols * layout.rows;
    let total = jobs.len();
    let mut pages = Vec::with_capacity(total.div_ceil(capacity));
    let mut page = Rgba8::filled(layout.width, layout.height, [255, 255, 255, 255]);

    for (idx, (photo, job)) in jobs.iter().cloned().enumerate() {
        let img = job.run().rendered?.image;
        let slot = idx % capacity;
        let col = slot % layout.cols;
        let row = slot / layout.cols;
        let cell_x = layout.margins.left + col * (layout.cell_w + layout.spacing);
        let cell_y = layout.margins.top + row * (layout.cell_h + layout.spacing);
        let avail_h = layout.cell_h.saturating_sub(caption_h);

        let img_x = cell_x + layout.cell_w.saturating_sub(img.width) / 2;
        let img_y = cell_y + avail_h.saturating_sub(img.height) / 2;
        blit_rgba(&mut page, &img, img_x, img_y)?;

        if layout.captions && caption_h > 0 {
            paint_cell_caption(&mut page, &photo, cell_x, cell_y + avail_h, layout.cell_w, caption_h, layout.show_exif)?;
        }

        if slot + 1 == capacity || idx + 1 == total {
            pages.push(page);
            page = Rgba8::filled(layout.width, layout.height, [255, 255, 255, 255]);
        }
    }
    Ok(pages)
}

fn paint_cell_caption(page: &mut Rgba8, photo: &Photo, x: usize, y: usize, w: usize, h: usize, show_exif: bool) -> std::result::Result<(), String> {
    let strip_w = w.saturating_sub(8).max(16);
    if !show_exif {
        let mut strip = Rgba8::filled(strip_w, h.min(26), [255, 255, 255, 255]);
        draw_line(&mut strip, &truncate_caption(&photo.file_name, strip_w), [32, 32, 32]);
        return blit_rgba(page, &strip, x + 4, y);
    }

    let line1_h = (h / 2).max(18);
    let line2_h = h.saturating_sub(line1_h).max(18);
    let mut top_strip = Rgba8::filled(strip_w, line1_h, [255, 255, 255, 255]);
    let stars = if photo.rating > 0 { format!("  {}", "★".repeat(usize::from(photo.rating.min(5)))) } else { String::new() };
    draw_line(&mut top_strip, &truncate_caption(&format!("{}{stars}", photo.file_name), strip_w), [28, 28, 28]);
    blit_rgba(page, &top_strip, x + 4, y)?;

    let exif_str = format_exif_line(&photo.meta);
    if !exif_str.is_empty() && y + line1_h + line2_h <= page.height {
        let mut bot_strip = Rgba8::filled(strip_w, line2_h, [255, 255, 255, 255]);
        draw_line(&mut bot_strip, &truncate_caption(&exif_str, strip_w), [96, 96, 96]);
        blit_rgba(page, &bot_strip, x + 4, y + line1_h)?;
    }
    Ok(())
}

fn draw_line(strip: &mut Rgba8, text: &str, color: [u8; 3]) {
    draw_watermark(
        strip,
        &Watermark {
            text: text.to_string(),
            size: 0.48,
            opacity: 1.0,
            anchor: Anchor::Center,
            inset_x: 0.0,
            inset_y: 0.0,
            color,
            shadow: false,
            ..Default::default()
        },
    );
}

fn format_exif_line(m: &Meta) -> String {
    let mut parts = Vec::new();
    if !m.camera.trim().is_empty() {
        parts.push(m.camera.trim().to_string());
    }
    if let Some(fl) = m.focal_mm.filter(|v| v.is_finite() && *v > 0.0) {
        parts.push(format!("{fl:.0}mm"));
    }
    if let Some(ap) = m.aperture.filter(|v| v.is_finite() && *v > 0.0) {
        parts.push(format!("f/{ap:.1}"));
    }
    if !m.shutter.trim().is_empty() {
        parts.push(format!("{}s", m.shutter.trim().trim_end_matches('s')));
    }
    if let Some(iso) = m.iso.filter(|&v| v > 0) {
        parts.push(format!("ISO {iso}"));
    }
    parts.join(" · ")
}

fn truncate_caption(s: &str, width_px: usize) -> String {
    use unicode_segmentation::UnicodeSegmentation;
    let clean: String = s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let graphemes: Vec<_> = clean.graphemes(true).collect();
    let max_chars = (width_px / 11).max(4);
    if graphemes.len() <= max_chars {
        return clean;
    }
    let left = max_chars / 2;
    let right = max_chars.saturating_sub(left + 1);
    format!(
        "{}…{}",
        graphemes.iter().take(left).copied().collect::<String>(),
        graphemes.iter().skip(graphemes.len() - right).copied().collect::<String>()
    )
}

fn blit_rgba(dst: &mut Rgba8, src: &Rgba8, x: usize, y: usize) -> std::result::Result<(), String> {
    if x.checked_add(src.width).is_none_or(|w| w > dst.width) || y.checked_add(src.height).is_none_or(|h| h > dst.height) {
        return Err("contact sheet element exceeds page bounds".into());
    }
    for r in 0..src.height {
        let d_off = (y + r) * dst.width + x;
        let d_row = dst.data.get_mut(d_off..d_off + src.width).ok_or("invalid page slice")?;
        let s_row = src.data.get(r * src.width..(r + 1) * src.width).ok_or("invalid cell slice")?;
        for (dp, sp) in d_row.iter_mut().zip(s_row) {
            let a = u32::from(sp[3]);
            for c in 0..3 {
                dp[c] = ((u32::from(sp[c]) * a + u32::from(dp[c]) * (255 - a) + 127) / 255) as u8;
            }
            dp[3] = 255;
        }
    }
    Ok(())
}

fn write_or_encode_sheet(
    pages: &[Rgba8],
    layout: &SheetLayout,
    fmt: SheetOutput,
    out_path: Option<&str>,
    photo_count: usize,
) -> std::result::Result<Value, String> {
    match fmt {
        SheetOutput::Pdf => {
            let mut pdf = crate::pdf::Pdf::new(Vec::new())?;
            for page in pages {
                let jpg = encode_image(page, &ExportOptions { format: ExportFormat::Jpeg, quality: 92, ..Default::default() })?;
                if pdf.len().saturating_add(jpg.len()) > 256 * 1024 * 1024 {
                    return Err("contact sheet exceeds 256 MiB".into());
                }
                pdf.image_page(&jpg, layout.width, layout.height, layout.points, "")?;
            }
            let bytes = pdf.finish("")?;
            if let Some(path) = out_path {
                crate::export::write_file(path, &bytes)?;
            }
            Ok(json!({
                "format": "pdf",
                "pages": pages.len(),
                "photos": photo_count,
                "width": layout.width,
                "height": layout.height,
                "bytes": bytes.len(),
                "path": out_path,
            }))
        }
        SheetOutput::Png | SheetOutput::Jpeg => {
            let first = pages.first().ok_or("no pages rendered")?;
            let exp_fmt = if fmt == SheetOutput::Png { ExportFormat::Png } else { ExportFormat::Jpeg };
            let encoded = encode_image(first, &ExportOptions { format: exp_fmt, quality: 92, ..Default::default() })?;
            if let Some(path) = out_path {
                crate::export::write_file(path, &encoded)?;
            }
            Ok(json!({
                "format": if fmt == SheetOutput::Png { "png" } else { "jpeg" },
                "pages": pages.len(),
                "photos": photo_count,
                "width": layout.width,
                "height": layout.height,
                "bytes": encoded.len(),
                "path": out_path,
            }))
        }
        SheetOutput::Rgba => {
            let rgba_bytes = pages.first().map_or(0, |pg| pg.data.len() * 4);
            Ok(json!({
                "format": "rgba",
                "pages": pages.len(),
                "photos": photo_count,
                "width": layout.width,
                "height": layout.height,
                "bytes": rgba_bytes,
            }))
        }
    }
}

// ─── 3. IPTC/XMP Metadata Stamp Template (`bridge.applyMetadataTemplate`) ───────────────────────

fn apply_metadata_template(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "bridge.applyMetadataTemplate";
    let targets = s.targets(p)?;
    if targets.is_empty() {
        return Err(bad(C, "no photos selected"));
    }

    let status = match str_param(p, "copyrightStatus") {
        None => None,
        Some(raw) => {
            Some(CopyrightStatus::parse(raw).ok_or_else(|| bad(C, format!("copyrightStatus `{raw}` must be unknown, copyrighted or publicDomain")))?)
        }
    };
    let rating = match p.get("rating") {
        None | Some(Value::Null) => None,
        Some(v) => {
            let r = v.as_u64().ok_or_else(|| bad(C, "`rating` must be an integer 0..5"))?;
            if r > 5 {
                return Err(bad(C, "`rating` must be between 0 and 5"));
            }
            Some(r as u8)
        }
    };
    let kw_replace = str_param(p, "keywordMode").is_some_and(|m| m.eq_ignore_ascii_case("replace"));
    let keywords: Option<Vec<String>> = p
        .get("keywords")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_str).map(lightcraft_catalog::keywords::clean).filter(|k| !k.is_empty()).collect());

    let mut ops = Vec::new();
    let mut touched_photos = 0_usize;

    for id in targets {
        let Some(ph) = s.catalog.photo(id) else { continue };
        let mut photo_changed = false;
        let mut m = ph.meta.clone();

        apply_template_fields(&mut m, p, status, keywords.as_deref(), kw_replace);
        if m != ph.meta {
            ops.push(Op::SetMeta { id, meta: Box::new(m) });
            photo_changed = true;
        }
        if let Some(r) = rating
            && ph.rating != r
        {
            ops.push(Op::SetRating { id, rating: r });
            photo_changed = true;
        }
        if photo_changed {
            touched_photos += 1;
        }
    }

    let ops_count = ops.len();
    if !ops.is_empty() {
        s.commit("Apply Metadata Template", Op::Batch { ops })?;
        if let Some(kws) = &keywords
            && !kws.is_empty()
        {
            crate::cmd::keywords::note_recent(s, kws);
        }
    }

    Ok(json!({
        "changed": touched_photos,
        "ops": ops_count,
    }))
}

fn apply_template_fields(m: &mut Meta, p: &Value, status: Option<CopyrightStatus>, keywords: Option<&[String]>, kw_replace: bool) {
    if let Some(st) = status {
        m.copyright_status = st;
    }
    for (key, dst) in [
        ("creator", &mut m.creator),
        ("copyright", &mut m.copyright),
        ("credit", &mut m.credit),
        ("usageTerms", &mut m.usage_terms),
        ("copyrightUrl", &mut m.copyright_url),
        ("title", &mut m.title),
        ("caption", &mut m.caption),
    ] {
        if let Some(val) = str_param(p, key) {
            *dst = val.to_string();
        }
    }
    if let Some(kws) = keywords {
        if kw_replace {
            m.keywords.clear();
        }
        let mut seen: std::collections::HashSet<String> = m.keywords.iter().map(|x| x.to_lowercase()).collect();
        for kw in kws {
            if seen.insert(kw.to_lowercase()) {
                m.keywords.push(kw.clone());
            }
        }
    }
}
