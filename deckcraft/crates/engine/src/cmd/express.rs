//! Express & Firefly productivity commands for DeckCraft:
//! - `express.applyBrandKit`: apply a Brand Kit (primary/secondary/accent hex colors, heading/body
//!   fonts, optional logo image) to the active slide or the entire presentation.
//! - `express.smartResize`: smart-resize the presentation across social & deck channels
//!   (`InstagramPost` 1080×1080, `InstagramReel` 1080×1920, `YouTubeThumbnail` 1920×1080,
//!   `LinkedInBanner` 1200×627, `Presentation16x9` 1920×1080) around safe margins.
//! - `express.bulkCreateCsv`: expand a template slide containing `{{column}}` placeholders across
//!   CSV rows into N populated slides.
//! - `express.insertQrCode`: generate a pure-Rust ISO/IEC 18004 vector QR code shape directly on
//!   the active slide.

use std::collections::BTreeMap;
use std::sync::Arc;

use deckcraft_color::{Rgba, SchemeSlot};
use deckcraft_geom::Xfrm;
use deckcraft_model::express::{BrandKit, SocialChannelPreset, builtin_brand_kits, encode_qr_matrix, qr_custom_geom};
use deckcraft_model::style::PictureFill;
use deckcraft_model::text::{Action, Hyperlink, TextBody};
use deckcraft_model::{Background, ColorRef, Fill, MediaId, PhType, Shape, ShapeId, ShapeKind, Slide, SlideId};
use serde_json::{Value, json};

use super::{CommandSpec, NO_ID_SPACE, bad, base64_decode, bool_or, cmd, f64_param, has_doc, has_slide, opt_usize_param, rect_param, str_param};
use crate::{EngineError, Result, Session};

const LOGO_TAG: (&str, &str) = ("expressRole", "brandLogo");

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "express.applyBrandKit",
            "Apply Brand Kit",
            ["Design", "Variants"],
            None,
            "{preset?, primaryHex?, secondaryHex?, accentHex?, headingFont?, bodyFont?, logoData?: base64, logoPath?, all?: bool} → {name, slides}",
            has_slide,
            apply_brand_kit
        ),
        cmd!(
            "express.smartResize",
            "Smart Resize",
            ["Design", "Customize"],
            None,
            "{channel: instagramPost|instagramReel|youtubeThumbnail|linkedinBanner|presentation16x9, safeMargin?: 0..0.25} → {channel, widthPx, heightPx, w, h}",
            has_doc,
            smart_resize
        ),
        cmd!(
            "express.bulkCreateCsv",
            "Bulk Create from CSV",
            ["Insert", "Slides"],
            None,
            "{csv? | path?, templateSlide?: index, replaceTemplate?: bool} → {created, columns}",
            has_slide,
            bulk_create_csv
        ),
        cmd!(
            "express.insertQrCode",
            "Insert QR Code",
            ["Insert", "Illustrations"],
            None,
            "{text, rect?: [x,y,w,h] pt, fgColor?: #RRGGBB, bgColor?: #RRGGBB, quietZone?: 0..8} → {id, version, modules}",
            has_slide,
            insert_qr_code
        ),
    ]
}

fn resolve_brand_kit_input(doc_kit: Option<&BrandKit>, p: &Value, cmd_id: &str) -> Result<BrandKit> {
    let mut kit = if let Some(preset_name) = str_param(p, "preset").map(str::trim).filter(|s| !s.is_empty()) {
        builtin_brand_kits()
            .into_iter()
            .find(|k| k.name.eq_ignore_ascii_case(preset_name))
            .ok_or_else(|| bad(cmd_id, format!("unknown Brand Kit preset `{preset_name}`")))?
    } else {
        doc_kit.cloned().unwrap_or_default()
    };

    if let Some(v) = str_param(p, "name").map(str::trim).filter(|s| !s.is_empty()) {
        kit.name = v.into();
    }
    if let Some(v) = str_param(p, "primaryHex") {
        let (_, norm) = BrandKit::parse_hex_rgb(v, "primaryHex").map_err(|e| bad(cmd_id, e))?;
        kit.primary_hex = norm;
    }
    if let Some(v) = str_param(p, "secondaryHex") {
        let (_, norm) = BrandKit::parse_hex_rgb(v, "secondaryHex").map_err(|e| bad(cmd_id, e))?;
        kit.secondary_hex = norm;
    }
    if let Some(v) = str_param(p, "accentHex") {
        let (_, norm) = BrandKit::parse_hex_rgb(v, "accentHex").map_err(|e| bad(cmd_id, e))?;
        kit.accent_hex = norm;
    }
    if let Some(v) = str_param(p, "headingFont").map(str::trim).filter(|s| !s.is_empty()) {
        kit.heading_font = v.into();
    }
    if let Some(v) = str_param(p, "bodyFont").map(str::trim).filter(|s| !s.is_empty()) {
        kit.body_font = v.into();
    }
    kit.validated_colors().map_err(|e| bad(cmd_id, e))?;
    Ok(kit)
}

fn load_optional_logo_bytes(p: &Value, cmd_id: &str) -> Result<Option<(String, Vec<u8>)>> {
    if let Some(b64) = str_param(p, "logoData").map(str::trim).filter(|s| !s.is_empty()) {
        let bytes = base64_decode(b64).ok_or_else(|| bad(cmd_id, "invalid base64 `logoData`"))?;
        return Ok(Some(("brand-logo.png".into(), bytes)));
    }
    if let Some(path) = str_param(p, "logoPath").map(str::trim).filter(|s| !s.is_empty()) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let bytes = crate::io::read_file(path).map_err(EngineError::Other)?;
            let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "brand-logo.png".into());
            return Ok(Some((name, bytes)));
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = path;
            return Err(bad(cmd_id, "use `logoData` on wasm targets"));
        }
    }
    Ok(None)
}

fn is_heading_shape(sh: &Shape) -> bool {
    if sh.ph_type().is_some_and(|k| matches!(k, PhType::Title | PhType::CtrTitle | PhType::SubTitle)) {
        return true;
    }
    sh.text.as_ref().and_then(|tb| tb.paragraphs.first()).and_then(|p| p.runs.first()).and_then(|r| r.props.size).is_some_and(|sz| sz >= 24.0)
}

fn restyle_shape_tree(shapes: &mut [Shape], primary: Rgba, accent: Rgba, heading_font: &str, body_font: &str) {
    for sh in shapes {
        if sh.tags.iter().any(|(k, v)| k == LOGO_TAG.0 && v == LOGO_TAG.1) {
            continue;
        }
        let heading = is_heading_shape(sh);
        if let Some(tb) = sh.text.as_mut() {
            let font_name = if heading { heading_font } else { body_font };
            deckcraft_model::edit::format_all(tb, &|r| {
                r.font = Some(font_name.to_string());
                r.font_ea = Some(font_name.to_string());
                r.fill = Some(Fill::solid(ColorRef::rgb(primary)));
            });
        }
        if matches!(sh.kind, ShapeKind::Shape) && !sh.text_box && sh.ph.is_none() && !sh.is_line() {
            sh.fill = Some(Fill::solid(ColorRef::rgb(accent)));
        }
        if let Some(children) = sh.children_mut() {
            restyle_shape_tree(children, primary, accent, heading_font, body_font);
        }
    }
}

fn place_or_update_logo(sl: &mut Slide, slide_size: deckcraft_geom::Size, logo: MediaId, new_id: Option<ShapeId>) {
    let logo_w = (slide_size.width * 0.09).clamp(36.0, 96.0);
    let pad = (slide_size.width * 0.035).clamp(16.0, 36.0);
    let xfrm = Xfrm::new((slide_size.width - logo_w - pad).max(8.0), pad, logo_w, logo_w);
    if let Some(existing) = sl.shapes.iter_mut().find(|s| s.tags.iter().any(|(k, v)| k == LOGO_TAG.0 && v == LOGO_TAG.1)) {
        existing.xfrm = Some(xfrm);
        existing.kind = ShapeKind::Picture { fill: PictureFill { media: logo, ..Default::default() } };
    } else if let Some(id) = new_id {
        sl.shapes.push(Shape {
            id,
            name: "Brand Logo".into(),
            xfrm: Some(xfrm),
            kind: ShapeKind::Picture { fill: PictureFill { media: logo, ..Default::default() } },
            decorative: true,
            tags: vec![(LOGO_TAG.0.into(), LOGO_TAG.1.into())],
            ..Default::default()
        });
    }
}

fn apply_brand_kit(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "express.applyBrandKit";
    let current_kit = s.active().and_then(|d| d.doc.brand_kit.as_ref()).cloned();
    let mut kit = resolve_brand_kit_input(current_kit.as_ref(), p, CMD)?;
    let (primary, secondary, accent) = kit.validated_colors().map_err(|e| bad(CMD, e))?;
    let logo_upload = load_optional_logo_bytes(p, CMD)?;
    let apply_all = bool_or(p, "all", false);

    let mut styled_slides = 0_usize;
    s.edit(|doc, sel| {
        if let Some((name, bytes)) = logo_upload {
            let ct = super::insert::content_type(&name, &bytes);
            let mid = doc.add_media(&name, ct, bytes).ok_or_else(|| bad(CMD, NO_ID_SPACE))?;
            kit.logo_asset = Some(mid);
        }
        doc.brand_kit = Some(kit.clone());

        if apply_all && let Some(master_arc) = doc.masters.first_mut() {
            let m = Arc::make_mut(master_arc);
            m.theme.colors.colors[SchemeSlot::Dk1 as usize] = primary;
            m.theme.colors.colors[SchemeSlot::Lt1 as usize] = secondary;
            m.theme.colors.colors[SchemeSlot::Accent1 as usize] = accent;
            m.theme.fonts.major.latin.clone_from(&kit.heading_font);
            m.theme.fonts.major.ea.clone_from(&kit.heading_font);
            m.theme.fonts.minor.latin.clone_from(&kit.body_font);
            m.theme.fonts.minor.ea.clone_from(&kit.body_font);
        }

        let slide_indices: Vec<usize> =
            if apply_all { (0..doc.slides.len()).collect() } else { vec![sel.slide.min(doc.slides.len().saturating_sub(1))] };
        let slide_sz = doc.slide_size;
        for idx in slide_indices {
            let logo_shape_id = if kit.logo_asset.is_some() { Some(ShapeId(doc.alloc_id().ok_or_else(|| bad(CMD, NO_ID_SPACE))?)) } else { None };
            let Some(sl_arc) = doc.slides.get_mut(idx) else { continue };
            let sl = Arc::make_mut(sl_arc);
            sl.background = Some(Background::Fill { fill: Fill::solid(ColorRef::rgb(secondary)) });
            restyle_shape_tree(&mut sl.shapes, primary, accent, &kit.heading_font, &kit.body_font);
            if let Some(logo) = kit.logo_asset {
                place_or_update_logo(sl, slide_sz, logo, logo_shape_id);
            }
            styled_slides += 1;
        }
        Ok(())
    })?;

    Ok(json!({
        "name": kit.name,
        "primaryHex": kit.primary_hex,
        "secondaryHex": kit.secondary_hex,
        "accentHex": kit.accent_hex,
        "headingFont": kit.heading_font,
        "bodyFont": kit.body_font,
        "slides": styled_slides,
    }))
}

fn reposition_around_safe_rect(shapes: &mut [Shape], old_size: deckcraft_geom::Size, safe: deckcraft_geom::Rect) {
    let ow = old_size.width.max(1.0);
    let oh = old_size.height.max(1.0);
    let sw = safe.width();
    let sh_h = safe.height();
    let sx = sw / ow;
    let sy = sh_h / oh;
    let k = sx.min(sy);
    let ox = safe.x0 + (sw - ow * k) * 0.5;
    let oy = safe.y0 + (sh_h - oh * k) * 0.5;

    for sh in shapes {
        if let Some(xf) = sh.xfrm.as_mut() {
            let nw = (xf.w * k).clamp(4.0, sw);
            let nh = (xf.h * k).clamp(4.0, sh_h);
            let nx = (xf.x * k + ox).clamp(safe.x0, (safe.x0 + sw - nw).max(safe.x0));
            let ny = (xf.y * k + oy).clamp(safe.y0, (safe.y0 + sh_h - nh).max(safe.y0));
            xf.x = nx;
            xf.y = ny;
            xf.w = nw;
            xf.h = nh;
        }
        if let Some(tb) = sh.text.as_mut() {
            deckcraft_model::edit::format_all(tb, &|r| {
                if let Some(sz) = r.size.as_mut() {
                    *sz = (*sz * k).clamp(8.0, 240.0);
                }
            });
        }
        if let Some(children) = sh.children_mut() {
            reposition_around_safe_rect(children, old_size, safe);
        }
    }
}

fn smart_resize(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "express.smartResize";
    let ch_raw = str_param(p, "channel").ok_or_else(|| bad(CMD, "missing `channel`"))?;
    let preset = SocialChannelPreset::parse(ch_raw).ok_or_else(|| {
        bad(
            CMD,
            format!(
                "unknown social channel `{ch_raw}` (expected instagramPost, instagramReel, youtubeThumbnail, linkedinBanner, or presentation16x9)"
            ),
        )
    })?;
    let margin_override = match p.get("safeMargin").filter(|v| !v.is_null()) {
        None => None,
        Some(_) => {
            let m = f64_param(p, "safeMargin").ok_or_else(|| bad(CMD, "`safeMargin` must be a finite number in 0..=0.25"))?;
            if !(0.0..=0.25).contains(&m) {
                return Err(bad(CMD, "`safeMargin` must be between 0.0 and 0.25"));
            }
            Some(m)
        }
    };

    let target_size = preset.size_points();
    let safe = preset.safe_rect(margin_override);
    let (px_w, px_h) = preset.pixels();

    s.edit(|doc, _| {
        let old = doc.slide_size;
        let msx = target_size.width / old.width.max(1.0);
        let msy = target_size.height / old.height.max(1.0);
        for m in &mut doc.masters {
            let m = Arc::make_mut(m);
            reposition_around_safe_rect(&mut m.shapes, old, safe);
            for l in &mut m.layouts {
                for sh in &mut l.shapes {
                    if let Some(x) = sh.xfrm.as_mut() {
                        x.x *= msx;
                        x.y *= msy;
                        x.w *= msx;
                        x.h *= msy;
                    }
                }
            }
        }
        for sl in &mut doc.slides {
            reposition_around_safe_rect(&mut Arc::make_mut(sl).shapes, old, safe);
        }
        doc.slide_size = target_size;
        Ok(())
    })?;

    Ok(json!({
        "channel": preset.id(),
        "widthPx": px_w,
        "heightPx": px_h,
        "w": target_size.width,
        "h": target_size.height,
        "safeRect": [safe.x0, safe.y0, safe.width(), safe.height()],
    }))
}

fn parse_csv_rows(input: &str) -> std::result::Result<Vec<Vec<String>>, String> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut cell = String::new();
    let mut in_quotes = false;
    let mut chars = input.trim_start_matches('\u{feff}').chars().peekable();

    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    let _ = chars.next();
                    cell.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                cell.push(ch);
            }
        } else {
            match ch {
                '"' if cell.is_empty() => in_quotes = true,
                ',' => row.push(std::mem::take(&mut cell)),
                '\r' => {
                    if chars.peek() == Some(&'\n') {
                        let _ = chars.next();
                    }
                    row.push(std::mem::take(&mut cell));
                    if row.iter().any(|s| !s.trim().is_empty()) {
                        rows.push(std::mem::take(&mut row));
                    } else {
                        row.clear();
                    }
                }
                '\n' => {
                    row.push(std::mem::take(&mut cell));
                    if row.iter().any(|s| !s.trim().is_empty()) {
                        rows.push(std::mem::take(&mut row));
                    } else {
                        row.clear();
                    }
                }
                other => cell.push(other),
            }
        }
    }
    if in_quotes {
        return Err("unterminated quoted field in CSV".into());
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        if row.iter().any(|s| !s.trim().is_empty()) {
            rows.push(row);
        }
    }
    Ok(rows)
}

fn replace_placeholders_str(text: &str, vars: &BTreeMap<String, String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        let after_open = &rest[open + 2..];
        if let Some(close) = after_open.find("}}") {
            let key = after_open[..close].trim().to_ascii_lowercase();
            if let Some(val) = vars.get(&key) {
                out.push_str(val);
            } else {
                out.push_str(&rest[open..open + 2 + close + 2]);
            }
            rest = &after_open[close + 2..];
        } else {
            out.push_str(&rest[open..]);
            return out;
        }
    }
    out.push_str(rest);
    out
}

fn replace_in_text_body(tb: &mut TextBody, vars: &BTreeMap<String, String>) {
    for para in &mut tb.paragraphs {
        // First replace inside individual runs
        for run in &mut para.runs {
            if run.text.contains("{{") {
                run.text = replace_placeholders_str(&run.text, vars);
            }
        }
        // If a `{{column}}` token was split across adjacent runs, coalesce paragraph text into the first run's style
        let joined: String = para.runs.iter().map(|r| r.text.as_str()).collect();
        if joined.contains("{{") {
            let replaced = replace_placeholders_str(&joined, vars);
            if replaced != joined
                && let Some(first) = para.runs.first().cloned()
            {
                para.runs = vec![deckcraft_model::Run { text: replaced, ..first }];
            }
        }
    }
}

fn populate_shape_tree(
    shapes: &mut [Shape],
    vars: &BTreeMap<String, String>,
    doc_next_id: &mut dyn FnMut() -> Option<u32>,
) -> std::result::Result<(), &'static str> {
    for sh in shapes {
        sh.id = ShapeId(doc_next_id().ok_or(NO_ID_SPACE)?);
        if sh.name.contains("{{") {
            sh.name = replace_placeholders_str(&sh.name, vars);
        }
        if let Some(tb) = sh.text.as_mut() {
            replace_in_text_body(tb, vars);
        }
        if let ShapeKind::Table(tbl) = &mut sh.kind {
            for row in &mut tbl.rows {
                for cell in &mut row.cells {
                    replace_in_text_body(&mut cell.text, vars);
                }
            }
        }
        if let Some(diag) = sh.diagram.as_mut() {
            for node in &mut diag.nodes {
                node.text = replace_placeholders_str(&node.text, vars);
                node.title = replace_placeholders_str(&node.title, vars);
            }
        }
        if let Some(children) = sh.children_mut() {
            populate_shape_tree(children, vars, doc_next_id)?;
        }
    }
    Ok(())
}

fn bulk_create_csv(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "express.bulkCreateCsv";
    let csv_text = if let Some(inline) = str_param(p, "csv") {
        inline.to_string()
    } else if let Some(path) = str_param(p, "path").map(str::trim).filter(|v| !v.is_empty()) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let bytes = crate::io::read_file(path).map_err(EngineError::Other)?;
            String::from_utf8(bytes).map_err(|e| bad(CMD, format!("{path}: invalid UTF-8: {e}")))?
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = path;
            return Err(bad(CMD, "provide inline `csv` on wasm targets"));
        }
    } else {
        return Err(bad(CMD, "provide `csv` text or `path`"));
    };

    if csv_text.len() > 8 * 1024 * 1024 {
        return Err(bad(CMD, "CSV input exceeds 8 MiB limit"));
    }
    let rows = parse_csv_rows(&csv_text).map_err(|e| bad(CMD, e))?;
    let Some((header, data_rows)) = rows.split_first() else {
        return Err(bad(CMD, "CSV must contain a header row and at least one data row"));
    };
    if data_rows.is_empty() {
        return Err(bad(CMD, "CSV has a header row but no data rows"));
    }
    if data_rows.len() > 1000 {
        return Err(bad(CMD, "CSV bulk create supports at most 1000 rows per run"));
    }

    let columns: Vec<String> = header.iter().map(|h| h.trim().to_string()).filter(|h| !h.is_empty()).collect();
    if columns.is_empty() {
        return Err(bad(CMD, "CSV header row has no column names"));
    }
    let tpl_param = opt_usize_param(p, "templateSlide", CMD)?;
    let replace_tpl = bool_or(p, "replaceTemplate", false);

    let mut created_ids = Vec::with_capacity(data_rows.len());
    s.edit(|doc, sel| {
        let tpl_idx = tpl_param.unwrap_or(sel.slide);
        let template = doc
            .slides
            .get(tpl_idx)
            .cloned()
            .ok_or_else(|| bad(CMD, format!("templateSlide index {tpl_idx} out of range ({} slides)", doc.slides.len())))?;

        let mut generated: Vec<Arc<Slide>> = Vec::with_capacity(data_rows.len());
        for row in data_rows {
            let mut vars = BTreeMap::new();
            for (ci, raw_col) in header.iter().enumerate() {
                let k = raw_col.trim().to_ascii_lowercase();
                if !k.is_empty() {
                    vars.insert(k, row.get(ci).map(|s| s.as_str()).unwrap_or_default().to_string());
                }
            }
            let mut slide_copy = (*template).clone();
            slide_copy.id = SlideId(doc.alloc_id().ok_or_else(|| bad(CMD, NO_ID_SPACE))?);
            if !slide_copy.name.is_empty() {
                slide_copy.name = replace_placeholders_str(&slide_copy.name, &vars);
            }
            replace_in_text_body(&mut slide_copy.notes, &vars);
            slide_copy.animations.clear();
            let mut alloc = || doc.alloc_id();
            populate_shape_tree(&mut slide_copy.shapes, &vars, &mut alloc).map_err(|msg| bad(CMD, msg))?;
            created_ids.push(slide_copy.id.0);
            generated.push(Arc::new(slide_copy));
        }

        let insert_pos = if replace_tpl {
            doc.slides.remove(tpl_idx);
            tpl_idx
        } else {
            (tpl_idx + 1).min(doc.slides.len())
        };
        let first_new = insert_pos;
        for (offset, sl) in generated.into_iter().enumerate() {
            doc.slides.insert(insert_pos + offset, sl);
        }
        sel.slide = first_new;
        sel.slides = created_ids.first().map(|&id| vec![SlideId(id)]).unwrap_or_default();
        sel.shapes.clear();
        sel.text = None;
        Ok(())
    })?;

    Ok(json!({
        "created": created_ids.len(),
        "slideIds": created_ids,
        "columns": columns,
    }))
}

fn insert_qr_code(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "express.insertQrCode";
    let text = str_param(p, "text").map(str::trim).filter(|t| !t.is_empty()).ok_or_else(|| bad(CMD, "need non-empty `text`"))?.to_string();
    let fg = match str_param(p, "fgColor") {
        None => Rgba::rgb(0x11, 0x18, 0x27),
        Some(hex) => BrandKit::parse_hex_rgb(hex, "fgColor").map_err(|e| bad(CMD, e))?.0,
    };
    let bg_opt = match str_param(p, "bgColor").filter(|v| !v.trim().is_empty()) {
        None => None,
        Some(hex) => Some(BrandKit::parse_hex_rgb(hex, "bgColor").map_err(|e| bad(CMD, e))?.0),
    };
    let quiet_zone = match p.get("quietZone").filter(|v| !v.is_null()) {
        None => 2_usize,
        Some(v) => v.as_u64().filter(|&q| q <= 8).ok_or_else(|| bad(CMD, "`quietZone` must be an integer 0..=8"))? as usize,
    };

    let matrix = encode_qr_matrix(&text).map_err(|e| bad(CMD, e))?;
    let geom = qr_custom_geom(&matrix, quiet_zone);
    let default_side = 144.0_f64;
    let slide_sz = s.active().map(|d| d.doc.slide_size).unwrap_or(deckcraft_model::defaults::WIDE);
    let rect = rect_param(p, "rect")
        .unwrap_or_else(|| Xfrm::new((slide_sz.width - default_side) * 0.5, (slide_sz.height - default_side) * 0.5, default_side, default_side));

    let click_action = if text.starts_with("https://") || text.starts_with("http://") {
        Some(Hyperlink {
            action: Action::Url { url: text.clone() },
            tooltip: text.clone(),
            highlight_click: false,
            sound: None,
            stop_sound: false,
            zoom_return: None,
        })
    } else {
        None
    };

    let qr_shape = if let Some(bg) = bg_opt {
        // Group a solid quiet-zone background card behind the crisp QR module path
        let card_rect = Xfrm::new(0.0, 0.0, rect.w, rect.h);
        let bg_child = Shape {
            id: ShapeId(0),
            name: "QR Background".into(),
            xfrm: Some(card_rect),
            fill: Some(Fill::solid(ColorRef::rgb(bg))),
            decorative: true,
            ..Default::default()
        };
        let fg_child = Shape {
            id: ShapeId(0),
            name: "QR Modules".into(),
            xfrm: Some(card_rect),
            geom,
            fill: Some(Fill::solid(ColorRef::rgb(fg))),
            ..Default::default()
        };
        Shape {
            name: "QR Code".into(),
            xfrm: Some(rect),
            kind: ShapeKind::Group { child: card_rect, children: vec![bg_child, fg_child] },
            descr: format!("QR Code: {text}"),
            click: click_action,
            ..Default::default()
        }
    } else {
        Shape {
            name: "QR Code".into(),
            xfrm: Some(rect),
            geom,
            fill: Some(Fill::solid(ColorRef::rgb(fg))),
            descr: format!("QR Code: {text}"),
            click: click_action,
            ..Default::default()
        }
    };

    let mut final_id = ShapeId(0);
    s.edit(|doc, sel| {
        let mut shape = qr_shape.clone();
        shape.id = ShapeId(doc.alloc_id().ok_or_else(|| bad(CMD, NO_ID_SPACE))?);
        if let ShapeKind::Group { children, .. } = &mut shape.kind {
            for child in children {
                child.id = ShapeId(doc.alloc_id().ok_or_else(|| bad(CMD, NO_ID_SPACE))?);
            }
        }
        final_id = shape.id;
        let list = crate::shapes_mut(doc, sel).ok_or_else(|| bad(CMD, "no slide"))?;
        list.push(shape);
        sel.shapes = vec![final_id];
        sel.text = None;
        Ok(())
    })?;

    Ok(json!({
        "id": final_id.0,
        "version": matrix.version,
        "modules": matrix.size,
        "text": text,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn express_commands_parse_params_and_execute_end_to_end() {
        for sp in specs() {
            assert!(crate::cmd::parse_params(sp.params).is_ok(), "invalid params grammar on `{}`: {}", sp.id, sp.params);
        }

        let mut s = Session::with_new();
        // Add a template text box with {{name}} and {{role}} placeholders
        s.execute("insert.textBox", &json!({ "rect": [120.0, 120.0, 400.0, 80.0], "text": "Speaker: {{name}} — {{role}}" })).expect("insert textBox");

        // 1. Bulk Create from CSV -> expands 3 rows into 3 populated slides
        let csv = "name,role\nAda Lovelace,Chief Architect\nGrace Hopper,Compiler Pioneer\n Margaret Hamilton ,Flight Software Lead\n";
        let bulk = s.execute("express.bulkCreateCsv", &json!({ "csv": csv, "replaceTemplate": true })).expect("bulkCreateCsv");
        assert_eq!(bulk["created"], json!(3));
        assert_eq!(s.active().expect("doc").doc.slides.len(), 3);
        let first_text: String =
            s.active().expect("doc").doc.slides[0].shapes.iter().filter_map(|sh| sh.text.as_ref().map(TextBody::text)).collect::<Vec<_>>().join("\n");
        assert!(first_text.contains("Ada Lovelace — Chief Architect"), "{first_text}");

        // 2. Apply Brand Kit across entire deck
        let bk = s
            .execute(
                "express.applyBrandKit",
                &json!({
                    "primaryHex": "#1E293B",
                    "secondaryHex": "#F8FAFC",
                    "accentHex": "#6366F1",
                    "headingFont": "Montserrat",
                    "bodyFont": "Inter",
                    "all": true
                }),
            )
            .expect("applyBrandKit");
        assert_eq!(bk["slides"], json!(3));
        assert_eq!(s.active().expect("doc").doc.brand_kit.as_ref().expect("brand kit").accent_hex, "#6366F1");

        // 3. Smart Resize to Instagram Reel (1080x1920 -> 540x960 pt)
        let resized = s.execute("express.smartResize", &json!({ "channel": "instagramReel" })).expect("smartResize");
        assert_eq!(resized["widthPx"], json!(1080));
        assert_eq!(resized["heightPx"], json!(1920));
        assert_eq!(s.active().expect("doc").doc.slide_size, deckcraft_geom::Size::new(540.0, 960.0));

        // 4. Insert Pure-Rust Vector QR Code
        let qr = s
            .execute(
                "express.insertQrCode",
                &json!({
                    "text": "https://getartcraft.com/deck",
                    "fgColor": "#0F172A",
                    "bgColor": "#FFFFFF",
                    "quietZone": 2
                }),
            )
            .expect("insertQrCode");
        assert!(qr["modules"].as_u64().expect("modules") >= 21);
    }
}
