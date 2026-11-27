//! Book module: multi-page photo book layout, automatic photo placement, cover/title pages,
//! captions, page numbering, and deterministic PDF 1.6 export.

use lightcraft_catalog::PhotoId;
use serde::{Deserialize, Serialize};

use crate::print_pdf::{ImageData, PdfImage, PdfInfo, PdfPage, Placement};
use crate::text::TextLayout;
use crate::{CRAFT_FONTS, EngineError, Result, Session};

/// Standard photo-book trim sizes (points: 72 pt = 1 inch).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BookFormat {
    /// 8 × 10 in portrait (576 × 720 pt).
    StandardPortrait,
    /// 10 × 8 in landscape (720 × 576 pt).
    #[default]
    StandardLandscape,
    /// 13 × 11 in large landscape (936 × 792 pt).
    LargeLandscape,
    /// 12 × 12 in square (864 × 864 pt).
    LargeSquare,
    /// 7 × 7 in small square (504 × 504 pt).
    SmallSquare,
}

impl BookFormat {
    pub const ALL: [BookFormat; 5] =
        [BookFormat::StandardPortrait, BookFormat::StandardLandscape, BookFormat::LargeLandscape, BookFormat::LargeSquare, BookFormat::SmallSquare];

    /// `(width_pt, height_pt)` at 72 points per inch.
    pub fn size_pt(self) -> (f64, f64) {
        match self {
            BookFormat::StandardPortrait => (576.0, 720.0),
            BookFormat::StandardLandscape => (720.0, 576.0),
            BookFormat::LargeLandscape => (936.0, 792.0),
            BookFormat::LargeSquare => (864.0, 864.0),
            BookFormat::SmallSquare => (504.0, 504.0),
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            BookFormat::StandardPortrait => "standardPortrait",
            BookFormat::StandardLandscape => "standardLandscape",
            BookFormat::LargeLandscape => "largeLandscape",
            BookFormat::LargeSquare => "largeSquare",
            BookFormat::SmallSquare => "smallSquare",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            BookFormat::StandardPortrait => "Standard Portrait (8×10 in)",
            BookFormat::StandardLandscape => "Standard Landscape (10×8 in)",
            BookFormat::LargeLandscape => "Large Landscape (13×11 in)",
            BookFormat::LargeSquare => "Large Square (12×12 in)",
            BookFormat::SmallSquare => "Small Square (7×7 in)",
        }
    }
}

/// How photos are arranged on one book page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PageLayout {
    /// Centered single photo inside page margins.
    #[default]
    Single,
    /// Edge-to-edge full-bleed single photo.
    FullBleed,
    /// Side-by-side two-photo layout.
    TwoUp,
    /// 2 × 2 four-photo grid.
    Grid2x2,
    /// Single photo with dedicated bottom caption band.
    CaptionBottom,
}

impl PageLayout {
    pub const ALL: [PageLayout; 5] = [PageLayout::Single, PageLayout::FullBleed, PageLayout::TwoUp, PageLayout::Grid2x2, PageLayout::CaptionBottom];

    /// Maximum photo slots on one page of this layout.
    pub fn slots(self) -> usize {
        match self {
            PageLayout::Single | PageLayout::FullBleed | PageLayout::CaptionBottom => 1,
            PageLayout::TwoUp => 2,
            PageLayout::Grid2x2 => 4,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            PageLayout::Single => "single",
            PageLayout::FullBleed => "fullBleed",
            PageLayout::TwoUp => "twoUp",
            PageLayout::Grid2x2 => "grid2x2",
            PageLayout::CaptionBottom => "captionBottom",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PageLayout::Single => "1 Photo",
            PageLayout::FullBleed => "Full Bleed",
            PageLayout::TwoUp => "2 Photos",
            PageLayout::Grid2x2 => "4-Photo Grid",
            PageLayout::CaptionBottom => "Photo + Caption",
        }
    }

    /// Normalized `[x, y, w, h]` cell rectangles in top-down `(0..1, 0..1)` page coordinates.
    pub fn cells(self, page_w: f64, page_h: f64, pad_pt: f64, gap_pt: f64) -> Vec<[f64; 4]> {
        let pw = page_w.max(72.0);
        let ph = page_h.max(72.0);
        let mx = (pad_pt.clamp(0.0, pw * 0.35)) / pw;
        let my = (pad_pt.clamp(0.0, ph * 0.35)) / ph;
        let gx = (gap_pt.clamp(0.0, pw * 0.15)) / pw;
        let gy = (gap_pt.clamp(0.0, ph * 0.15)) / ph;
        let iw = (1.0 - 2.0 * mx).max(0.1);
        let ih = (1.0 - 2.0 * my).max(0.1);
        match self {
            PageLayout::FullBleed => vec![[0.0, 0.0, 1.0, 1.0]],
            PageLayout::Single => vec![[mx, my, iw, ih]],
            PageLayout::CaptionBottom => {
                let cap_h = (ih * 0.16).max(0.06);
                vec![[mx, my, iw, (ih - cap_h).max(0.1)]]
            }
            PageLayout::TwoUp => {
                let cw = ((iw - gx) * 0.5).max(0.05);
                vec![[mx, my, cw, ih], [mx + cw + gx, my, cw, ih]]
            }
            PageLayout::Grid2x2 => {
                let cw = ((iw - gx) * 0.5).max(0.05);
                let ch = ((ih - gy) * 0.5).max(0.05);
                vec![[mx, my, cw, ch], [mx + cw + gx, my, cw, ch], [mx, my + ch + gy, cw, ch], [mx + cw + gx, my + ch + gy, cw, ch]]
            }
        }
    }
}

/// One page in the book.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BookPage {
    pub layout: PageLayout,
    pub photos: Vec<PhotoId>,
    pub caption: String,
}

/// Complete Book module settings persisted in `prefs.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BookSettings {
    pub format: BookFormat,
    pub title: String,
    pub subtitle: String,
    pub author: String,
    pub include_cover: bool,
    pub background: [u8; 3],
    pub padding_pt: f64,
    pub cell_gap_pt: f64,
    pub page_numbers: bool,
    pub zoom_to_fill: bool,
    pub dpi: u32,
    pub pages: Vec<BookPage>,
}

impl Default for BookSettings {
    fn default() -> Self {
        Self {
            format: BookFormat::StandardLandscape,
            title: String::new(),
            subtitle: String::new(),
            author: String::new(),
            include_cover: true,
            background: [250, 249, 246],
            padding_pt: 36.0,
            cell_gap_pt: 12.0,
            page_numbers: true,
            zoom_to_fill: false,
            dpi: 180,
            pages: Vec::new(),
        }
    }
}

impl BookSettings {
    pub fn sanitized(mut self) -> Self {
        self.padding_pt = if self.padding_pt.is_finite() { self.padding_pt.clamp(0.0, 144.0) } else { 36.0 };
        self.cell_gap_pt = if self.cell_gap_pt.is_finite() { self.cell_gap_pt.clamp(0.0, 72.0) } else { 12.0 };
        self.dpi = self.dpi.clamp(72, 300);
        if self.pages.len() > 500 {
            self.pages.truncate(500);
        }
        for p in &mut self.pages {
            p.photos.truncate(p.layout.slots());
        }
        self
    }

    /// Populate pages automatically from `photos` using `layout`.
    pub fn auto_layout(&mut self, photos: &[PhotoId], layout: PageLayout, clear_existing: bool) {
        if clear_existing {
            self.pages.clear();
        }
        let slots = layout.slots().max(1);
        for chunk in photos.chunks(slots) {
            if self.pages.len() >= 500 {
                break;
            }
            self.pages.push(BookPage { layout, photos: chunk.to_vec(), caption: String::new() });
        }
    }
}

fn text_ink(bg: [u8; 3]) -> [u8; 3] {
    let luma = 0.2126 * bg[0] as f32 + 0.7152 * bg[1] as f32 + 0.0722 * bg[2] as f32;
    if luma > 132.0 { [32, 32, 35] } else { [238, 238, 240] }
}

fn blend_text(rgb: &mut [u8], (w, h): (usize, usize), text: &str, px: f32, (cx, top_y): (f32, f32), ink: [u8; 3]) {
    let Some(layout) = TextLayout::new(text, px, false, CRAFT_FONTS) else { return };
    let ox = (cx - layout.width * 0.5).max(12.0);
    layout.draw_in(ox, top_y, w as f32, h as f32, |x, y, cov| {
        if x < 0 || y < 0 {
            return;
        }
        let (ux, uy) = (x as usize, y as usize);
        if ux >= w || uy >= h {
            return;
        }
        let a = cov.clamp(0.0, 1.0);
        let base = (uy * w + ux) * 3;
        if let Some(dst) = rgb.get_mut(base..base + 3) {
            dst[0] = (dst[0] as f32 * (1.0 - a) + ink[0] as f32 * a).round() as u8;
            dst[1] = (dst[1] as f32 * (1.0 - a) + ink[1] as f32 * a).round() as u8;
            dst[2] = (dst[2] as f32 * (1.0 - a) + ink[2] as f32 * a).round() as u8;
        }
    });
}

fn blit_rgba(rgb: &mut [u8], pw: usize, ph: usize, src: &lightcraft_raster::Rgba8, cell_px: [usize; 4], zoom_to_fill: bool) {
    let [cx, cy, cw, ch] = cell_px;
    if cw == 0 || ch == 0 || src.width == 0 || src.height == 0 {
        return;
    }
    let sx_ratio = cw as f64 / src.width as f64;
    let sy_ratio = ch as f64 / src.height as f64;
    if zoom_to_fill {
        let scale = sx_ratio.max(sy_ratio).max(1e-6);
        let vis_w = (cw as f64 / scale).min(src.width as f64);
        let vis_h = (ch as f64 / scale).min(src.height as f64);
        let off_x = (src.width as f64 - vis_w) * 0.5;
        let off_y = (src.height as f64 - vis_h) * 0.5;
        for dy in 0..ch {
            let py = cy + dy;
            if py >= ph {
                break;
            }
            let sy = ((off_y + (dy as f64 + 0.5) * vis_h / ch as f64) as usize).min(src.height - 1);
            for dx in 0..cw {
                let px = cx + dx;
                if px >= pw {
                    break;
                }
                let sx = ((off_x + (dx as f64 + 0.5) * vis_w / cw as f64) as usize).min(src.width - 1);
                if let Some(p) = src.data.get(sy * src.width + sx) {
                    let idx = (py * pw + px) * 3;
                    if let Some(dst) = rgb.get_mut(idx..idx + 3) {
                        dst[0] = p[0];
                        dst[1] = p[1];
                        dst[2] = p[2];
                    }
                }
            }
        }
    } else {
        let scale = sx_ratio.min(sy_ratio).max(1e-6);
        let dw = ((src.width as f64 * scale).round() as usize).clamp(1, cw);
        let dh = ((src.height as f64 * scale).round() as usize).clamp(1, ch);
        let ox = cx + (cw - dw) / 2;
        let oy = cy + (ch - dh) / 2;
        for dy in 0..dh {
            let py = oy + dy;
            if py >= ph {
                break;
            }
            let sy = (((dy as f64 + 0.5) / dh as f64 * src.height as f64) as usize).min(src.height - 1);
            for dx in 0..dw {
                let px = ox + dx;
                if px >= pw {
                    break;
                }
                let sx = (((dx as f64 + 0.5) / dw as f64 * src.width as f64) as usize).min(src.width - 1);
                if let Some(p) = src.data.get(sy * src.width + sx) {
                    let idx = (py * pw + px) * 3;
                    if let Some(dst) = rgb.get_mut(idx..idx + 3) {
                        dst[0] = p[0];
                        dst[1] = p[1];
                        dst[2] = p[2];
                    }
                }
            }
        }
    }
}

/// Render the current book to PDF 1.6 bytes.
pub fn render_pdf(s: &mut Session, dpi_override: Option<u32>) -> Result<(Vec<u8>, usize)> {
    let book = s.book.clone().sanitized();
    let has_cover = book.include_cover && (!book.title.trim().is_empty() || !book.subtitle.trim().is_empty() || !book.author.trim().is_empty());
    if book.pages.is_empty() && !has_cover {
        return Err(EngineError::Other("the book has no pages to export; run Auto-Layout or add a page first".into()));
    }
    let (w_pt, h_pt) = book.format.size_pt();
    let dpi = dpi_override.unwrap_or(book.dpi).clamp(72, 300) as f64;
    let pw = ((w_pt / 72.0 * dpi).round() as usize).clamp(64, 4096);
    let ph = ((h_pt / 72.0 * dpi).round() as usize).clamp(64, 4096);
    let scale_px = (dpi / 72.0) as f32;
    let ink = text_ink(book.background);
    let mut pdf_pages = Vec::new();

    if has_cover {
        let mut rgb = vec![0u8; pw * ph * 3];
        for px in rgb.as_chunks_mut::<3>().0 {
            px.copy_from_slice(&book.background);
        }
        // Optional cover hero photo from the first page if present
        if let Some(&hero_id) = book.pages.first().and_then(|p| p.photos.first()) {
            let edge = (pw.max(ph) / 2).max(256);
            if let Ok(r) = s.render_now(hero_id, edge, edge) {
                let cell = [pw / 5, ph / 8, pw * 3 / 5, ph * 2 / 5];
                blit_rgba(&mut rgb, pw, ph, &r.image, cell, book.zoom_to_fill);
            }
        }
        let mut ty = ph as f32 * 0.60;
        if !book.title.trim().is_empty() {
            blend_text(&mut rgb, (pw, ph), book.title.trim(), 28.0 * scale_px, (pw as f32 * 0.5, ty), ink);
            ty += 38.0 * scale_px;
        }
        if !book.subtitle.trim().is_empty() {
            blend_text(&mut rgb, (pw, ph), book.subtitle.trim(), 16.0 * scale_px, (pw as f32 * 0.5, ty), ink);
            ty += 26.0 * scale_px;
        }
        if !book.author.trim().is_empty() {
            blend_text(&mut rgb, (pw, ph), book.author.trim(), 13.0 * scale_px, (pw as f32 * 0.5, ty + 8.0 * scale_px), ink);
        }
        let img = PdfImage { width: pw as u32, height: ph as u32, channels: 3, data: ImageData::Raw { bits: 8, samples: rgb }, icc: None }
            .deflated()
            .map_err(EngineError::Other)?;
        pdf_pages.push(PdfPage { width: w_pt, height: h_pt, images: vec![(img, Placement { x: 0.0, y: 0.0, width: w_pt, height: h_pt })] });
    }

    for (idx, page) in book.pages.iter().enumerate() {
        let mut rgb = vec![0u8; pw * ph * 3];
        for px in rgb.as_chunks_mut::<3>().0 {
            px.copy_from_slice(&book.background);
        }
        let cells = page.layout.cells(w_pt, h_pt, book.padding_pt, book.cell_gap_pt);
        for (slot, rect) in cells.iter().enumerate() {
            let Some(&photo_id) = page.photos.get(slot) else { continue };
            let cx = (rect[0] * pw as f64).round() as usize;
            let cy = (rect[1] * ph as f64).round() as usize;
            let cw = (rect[2] * pw as f64).round() as usize;
            let ch = (rect[3] * ph as f64).round() as usize;
            let edge = cw.max(ch).clamp(128, 2400);
            if let Ok(r) = s.render_now(photo_id, edge, edge) {
                blit_rgba(&mut rgb, pw, ph, &r.image, [cx, cy, cw, ch], book.zoom_to_fill);
            }
        }
        if !page.caption.trim().is_empty() {
            let cap_y = (ph as f32 - (book.padding_pt as f32 * scale_px).max(28.0 * scale_px) - 14.0 * scale_px).max(8.0);
            blend_text(&mut rgb, (pw, ph), page.caption.trim(), 12.0 * scale_px, (pw as f32 * 0.5, cap_y), ink);
        }
        if book.page_numbers && page.layout != PageLayout::FullBleed {
            let num_text = format!("{}", idx + 1);
            let num_y = (ph as f32 - 20.0 * scale_px).max(4.0);
            blend_text(&mut rgb, (pw, ph), &num_text, 10.0 * scale_px, (pw as f32 * 0.5, num_y), ink);
        }
        let img = PdfImage { width: pw as u32, height: ph as u32, channels: 3, data: ImageData::Raw { bits: 8, samples: rgb }, icc: None }
            .deflated()
            .map_err(EngineError::Other)?;
        pdf_pages.push(PdfPage { width: w_pt, height: h_pt, images: vec![(img, Placement { x: 0.0, y: 0.0, width: w_pt, height: h_pt })] });
    }

    let count = pdf_pages.len();
    let info = PdfInfo { title: (!book.title.trim().is_empty()).then(|| book.title.trim().to_string()), output_intent: None };
    let bytes = crate::print_pdf::write(&pdf_pages, &info).map_err(EngineError::Other)?;
    Ok((bytes, count))
}
