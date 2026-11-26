//! Book module commands (`book.*`): configure trim format, cover, margins, page layouts, auto-layout,
//! and PDF export.

use lightcraft_catalog::PhotoId;
use serde_json::{Value, json};

use super::{CommandSpec, always, bad, cmd, index_req, photo_ids, str_param};
use crate::book::{BookFormat, BookPage, PageLayout, render_pdf};
use crate::{EngineError, Result, Session};

fn parse_format(s: &str, c: &str) -> Result<BookFormat> {
    serde_json::from_value(Value::String(s.to_string())).map_err(|_| bad(c, format!("unknown book format `{s}`")))
}

fn parse_layout(s: &str, c: &str) -> Result<PageLayout> {
    serde_json::from_value(Value::String(s.to_string())).map_err(|_| bad(c, format!("unknown page layout `{s}`")))
}

fn book_json(s: &Session) -> Value {
    let (w_pt, h_pt) = s.book.format.size_pt();
    let mut v = serde_json::to_value(&s.book).unwrap_or_else(|_| json!({}));
    v["widthPt"] = json!(w_pt);
    v["heightPt"] = json!(h_pt);
    v["pageCount"] = json!(s.book.pages.len());
    v["formats"] = json!(BookFormat::ALL.iter().map(|f| json!({"id": f.id(), "label": f.label()})).collect::<Vec<_>>());
    v["layouts"] = json!(PageLayout::ALL.iter().map(|l| json!({"id": l.id(), "label": l.label(), "slots": l.slots()})).collect::<Vec<_>>());
    v
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "book.get", "Book Settings", [], None, "{} — current Book module layout, cover and pages", always, |s, _| {
            Ok(book_json(s))
        }),
        cmd!(
            "book.set",
            "Set Book Settings",
            [],
            None,
            "{format?, title?, subtitle?, author?, includeCover?, background?: [r,g,b], paddingPt?, cellGapPt?, pageNumbers?, zoomToFill?, dpi?}",
            always,
            |s, p| {
                const C: &str = "book.set";
                if let Some(f) = str_param(p, "format") {
                    s.book.format = parse_format(f, C)?;
                }
                if let Some(t) = str_param(p, "title") {
                    s.book.title = t.to_string();
                }
                if let Some(sub) = str_param(p, "subtitle") {
                    s.book.subtitle = sub.to_string();
                }
                if let Some(a) = str_param(p, "author") {
                    s.book.author = a.to_string();
                }
                if let Some(v) = p.get("includeCover").and_then(Value::as_bool) {
                    s.book.include_cover = v;
                }
                if let Some(bg) = p.get("background").and_then(Value::as_array)
                    && bg.len() == 3
                {
                    let r = bg.first().and_then(Value::as_u64).unwrap_or(250).min(255) as u8;
                    let g = bg.get(1).and_then(Value::as_u64).unwrap_or(249).min(255) as u8;
                    let b = bg.get(2).and_then(Value::as_u64).unwrap_or(246).min(255) as u8;
                    s.book.background = [r, g, b];
                }
                if let Some(pad) = p.get("paddingPt").and_then(Value::as_f64) {
                    s.book.padding_pt = pad;
                }
                if let Some(gap) = p.get("cellGapPt").and_then(Value::as_f64) {
                    s.book.cell_gap_pt = gap;
                }
                if let Some(pn) = p.get("pageNumbers").and_then(Value::as_bool) {
                    s.book.page_numbers = pn;
                }
                if let Some(zf) = p.get("zoomToFill").and_then(Value::as_bool) {
                    s.book.zoom_to_fill = zf;
                }
                if let Some(dpi) = p.get("dpi").and_then(Value::as_u64) {
                    s.book.dpi = dpi.clamp(72, 300) as u32;
                }
                s.book = s.book.clone().sanitized();
                s.save_prefs()?;
                Ok(book_json(s))
            }
        ),
        CommandSpec {
            explicit_targets: true,
            ..cmd!(
                "book.autoLayout",
                "Auto-Layout Book",
                [],
                None,
                "{ids?, layout?: single|fullBleed|twoUp|grid2x2|captionBottom, clearExisting?: bool (default true)} — populate book pages from selected or visible photos",
                always,
                |s, p| {
                    const C: &str = "book.autoLayout";
                    let layout = match str_param(p, "layout") {
                        Some(l) => parse_layout(l, C)?,
                        None => PageLayout::Single,
                    };
                    let clear = p.get("clearExisting").and_then(Value::as_bool).unwrap_or(true);
                    let ids: Vec<PhotoId> = match photo_ids(p, C)? {
                        Some(list) => list,
                        None => {
                            let sel = s.selected();
                            if sel.len() > 1 { sel } else { s.visible_cloned() }
                        }
                    }
                    .into_iter()
                    .filter(|id| s.catalog.photo(*id).is_some())
                    .collect();
                    s.book.auto_layout(&ids, layout, clear);
                    s.book = s.book.clone().sanitized();
                    s.save_prefs()?;
                    Ok(book_json(s))
                }
            )
        },
        cmd!(
            "book.addPage",
            "Add Book Page",
            [],
            None,
            "{layout?: single|fullBleed|twoUp|grid2x2|captionBottom, photos?: [id,…], caption?: str, index?: usize}",
            always,
            |s, p| {
                const C: &str = "book.addPage";
                let layout = match str_param(p, "layout") {
                    Some(l) => parse_layout(l, C)?,
                    None => PageLayout::Single,
                };
                let mut photos = match p.get("photos") {
                    None | Some(Value::Null) => Vec::new(),
                    Some(Value::Array(arr)) => arr.iter().filter_map(|v| v.as_u64().map(PhotoId)).collect(),
                    Some(_) => return Err(bad(C, "`photos` must be a list of photo ids")),
                };
                photos.retain(|id| s.catalog.photo(*id).is_some());
                photos.truncate(layout.slots());
                let caption = str_param(p, "caption").unwrap_or("").to_string();
                let page = BookPage { layout, photos, caption };
                let idx = p.get("index").and_then(Value::as_u64).map(|n| n as usize).unwrap_or(s.book.pages.len()).min(s.book.pages.len());
                s.book.pages.insert(idx, page);
                s.book = s.book.clone().sanitized();
                s.save_prefs()?;
                Ok(book_json(s))
            }
        ),
        cmd!(
            "book.setPage",
            "Edit Book Page",
            [],
            None,
            "{index: usize, layout?, photos?: [id,…], caption?}",
            always,
            |s, p| {
                const C: &str = "book.setPage";
                let idx = index_req(p, "index", C)?;
                if idx >= s.book.pages.len() {
                    return Err(bad(C, "no such book page"));
                }
                let new_layout = match str_param(p, "layout") {
                    Some(l) => Some(parse_layout(l, C)?),
                    None => None,
                };
                let new_photos = match p.get("photos") {
                    None => None,
                    Some(Value::Array(arr)) => {
                        let valid: Vec<PhotoId> = arr
                            .iter()
                            .filter_map(|v| v.as_u64().map(PhotoId))
                            .filter(|id| s.catalog.photo(*id).is_some())
                            .collect();
                        Some(valid)
                    }
                    Some(_) => return Err(bad(C, "`photos` must be a list of photo ids")),
                };
                if let Some(page) = s.book.pages.get_mut(idx) {
                    if let Some(l) = new_layout {
                        page.layout = l;
                    }
                    if let Some(mut ph) = new_photos {
                        ph.truncate(page.layout.slots());
                        page.photos = ph;
                    } else {
                        page.photos.truncate(page.layout.slots());
                    }
                    if let Some(cap) = str_param(p, "caption") {
                        page.caption = cap.to_string();
                    }
                }
                s.save_prefs()?;
                Ok(book_json(s))
            }
        ),
        cmd!("book.removePage", "Remove Book Page", [], None, "{index: usize}", always, |s, p| {
            const C: &str = "book.removePage";
            let idx = index_req(p, "index", C)?;
            if idx >= s.book.pages.len() {
                return Err(bad(C, "no such book page"));
            }
            s.book.pages.remove(idx);
            s.save_prefs()?;
            Ok(book_json(s))
        }),
        cmd!("book.clear", "Clear Book", [], None, "{} — remove all pages from the book", always, |s, _| {
            s.book.pages.clear();
            s.save_prefs()?;
            Ok(book_json(s))
        }),
        cmd!(
            "book.exportPdf",
            "Export Book to PDF…",
            [],
            None,
            "{path: str, dpi?: 72..300} — render every book page (and cover) to a PDF 1.6 file",
            always,
            |s, p| {
                const C: &str = "book.exportPdf";
                let raw_path = str_param(p, "path").ok_or_else(|| bad(C, "missing `path`"))?;
                let mut path = std::path::PathBuf::from(raw_path);
                if path.extension().is_none() {
                    path.set_extension("pdf");
                }
                if let Some(parent) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
                    std::fs::create_dir_all(parent).map_err(|e| EngineError::Other(format!("{}: {e}", parent.display())))?;
                }
                let dpi = p.get("dpi").and_then(Value::as_u64).map(|d| d.clamp(72, 300) as u32);
                let (bytes, pages) = render_pdf(s, dpi)?;
                std::fs::write(&path, &bytes).map_err(|e| EngineError::Other(format!("{}: {e}", path.display())))?;
                Ok(json!({
                    "path": path.to_string_lossy(),
                    "pages": pages,
                    "pageCount": pages,
                    "bytes": bytes.len(),
                }))
            }
        ),
    ]
}
