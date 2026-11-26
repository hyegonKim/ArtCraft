//! Book module (`ViewMode::Book`): multi-page photo book layout workspace, cover & page inspector,
//! live page-spread preview with rendered thumbnails, and PDF 1.6 export (`book.*`).

use egui::{Align2, Color32, Rect, Stroke, pos2, vec2};
use lightcraft_engine::book::{BookFormat, PageLayout};
use serde_json::json;

use crate::LightcraftApp;
use crate::theme::Tokens;
use crate::widgets::{dropdown, register, text_button};

pub fn show(app: &mut LightcraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let book = app.session.book.clone();
    let total_views = usize::from(book.include_cover) + book.pages.len();
    if total_views > 0 && app.ui.book_page >= total_views {
        app.ui.book_page = total_views - 1;
    }

    // Top toolbar
    egui::Panel::top("book_toolbar")
        .exact_size(38.0)
        .frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin { left: 14, right: 14, top: 4, bottom: 4 }))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(egui::RichText::new(crate::i18n::tr("Book")).font(t.semibold(14.0)).color(t.text));
                ui.add_space(8.0);
                let fmt_btn = dropdown(ui, "bookFormat", crate::i18n::tr(book.format.label()), t.font(12.0), t.text_label);
                egui::Popup::menu(&fmt_btn).show(|ui| {
                    for f in BookFormat::ALL {
                        if ui.selectable_label(f == book.format, crate::i18n::tr(f.label())).clicked() {
                            let _ = app.run("book.set", json!({"format": f.id()}));
                            ui.close();
                        }
                    }
                });
                if text_button(ui, "bookAutoLayout", crate::i18n::tr("Auto-Layout"), false).clicked() {
                    let _ = app.run("book.autoLayout", json!({"layout": "twoUp"}));
                }
                if text_button(ui, "bookAddPage", crate::i18n::tr("+ Add Page"), false).clicked() {
                    let sel_ids: Vec<u64> = app.session.selected().iter().map(|p| p.0).collect();
                    let _ = app.run("book.addPage", json!({"layout": "single", "photos": sel_ids}));
                    app.ui.book_page = usize::from(app.session.book.include_cover) + app.session.book.pages.len().saturating_sub(1);
                }
                if !book.pages.is_empty() && text_button(ui, "bookClear", crate::i18n::tr("Clear Book"), false).clicked() {
                    let _ = app.run("book.clear", json!({}));
                    app.ui.book_page = 0;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can_export = book.include_cover || !book.pages.is_empty();
                    if ui.add_enabled(can_export, egui::Button::new(crate::i18n::tr("Export Book to PDF…"))).clicked() {
                        let base_dir = std::env::temp_dir().join("LightCraft-Book.pdf");
                        let default_path = base_dir.to_string_lossy().into_owned();
                        crate::panels::dialogs::prompt(
                            app,
                            "Export Book to PDF",
                            "PDF output path",
                            &default_path,
                            "book.exportPdf",
                            json!({}),
                            "path",
                        );
                    }
                });
            });
        });

    // Right-side Book Settings & Page Inspector
    egui::Panel::right("book_inspector")
        .exact_size(240.0)
        .frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin { left: 12, right: 12, top: 10, bottom: 10 }))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(crate::i18n::tr("Book Settings")).font(t.semibold(13.0)).color(t.text));
            ui.add_space(6.0);

            let mut inc_cover = book.include_cover;
            if ui.checkbox(&mut inc_cover, crate::i18n::tr("Include Cover Page")).changed() {
                let _ = app.run("book.set", json!({"includeCover": inc_cover}));
            }
            let mut page_nums = book.page_numbers;
            if ui.checkbox(&mut page_nums, crate::i18n::tr("Show Page Numbers")).changed() {
                let _ = app.run("book.set", json!({"pageNumbers": page_nums}));
            }

            ui.add_space(6.0);
            ui.label(egui::RichText::new(crate::i18n::tr("Cover Title")).color(t.text_dim).size(11.5));
            let mut title = book.title.clone();
            if ui.text_edit_singleline(&mut title).changed() {
                let _ = app.run("book.set", json!({"title": title}));
            }
            ui.label(egui::RichText::new(crate::i18n::tr("Subtitle")).color(t.text_dim).size(11.5));
            let mut subtitle = book.subtitle.clone();
            if ui.text_edit_singleline(&mut subtitle).changed() {
                let _ = app.run("book.set", json!({"subtitle": subtitle}));
            }
            ui.label(egui::RichText::new(crate::i18n::tr("Author")).color(t.text_dim).size(11.5));
            let mut author = book.author.clone();
            if ui.text_edit_singleline(&mut author).changed() {
                let _ = app.run("book.set", json!({"author": author}));
            }

            let is_cover = book.include_cover && app.ui.book_page == 0;
            if !is_cover {
                let page_idx = app.ui.book_page.saturating_sub(usize::from(book.include_cover));
                if let Some(page) = book.pages.get(page_idx) {
                    ui.separator();
                    ui.label(
                        egui::RichText::new(crate::i18n::tr_format!("Page {}", page_idx + 1))
                            .font(t.semibold(13.0))
                            .color(t.text),
                    );
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(crate::i18n::tr("Page Layout")).color(t.text_dim).size(11.5));
                    let lay_btn = dropdown(ui, "bookPageLayout", crate::i18n::tr(page.layout.label()), t.font(12.0), t.text_label);
                    egui::Popup::menu(&lay_btn).show(|ui| {
                        for l in PageLayout::ALL {
                            if ui.selectable_label(l == page.layout, crate::i18n::tr(l.label())).clicked() {
                                let _ = app.run("book.setPage", json!({"index": page_idx, "layout": l.id()}));
                                ui.close();
                            }
                        }
                    });
                    ui.add_space(4.0);
                    if !app.session.selected().is_empty()
                        && ui.button(crate::i18n::tr("Use Selected Photos")).clicked()
                    {
                        let ids: Vec<u64> = app.session.selected().iter().map(|p| p.0).collect();
                        let _ = app.run("book.setPage", json!({"index": page_idx, "photos": ids}));
                    }
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(crate::i18n::tr("Caption")).color(t.text_dim).size(11.5));
                    let mut cap = page.caption.clone();
                    if ui.text_edit_singleline(&mut cap).changed() {
                        let _ = app.run("book.setPage", json!({"index": page_idx, "caption": cap}));
                    }
                    ui.add_space(8.0);
                    if ui.button(crate::i18n::tr("Remove Page")).clicked() {
                        let _ = app.run("book.removePage", json!({"index": page_idx}));
                        app.ui.book_page = app.ui.book_page.saturating_sub(1);
                    }
                }
            }
        });

    // Main page spread preview + page strip
    let area = ui.available_rect_before_wrap();
    register(ui, "book:canvas", area);
    let p = ui.painter_at(area);
    p.rect_filled(area, 0.0, t.canvas);

    if total_views == 0 {
        p.text(
            area.center(),
            Align2::CENTER_CENTER,
            crate::i18n::tr("Click Auto-Layout or + Add Page to build your photo book"),
            t.font(13.5),
            t.text_dim,
        );
        return;
    }

    // Page selector strip at bottom of book canvas
    let strip_h = 44.0;
    let stage = Rect::from_min_max(area.min, pos2(area.right(), (area.bottom() - strip_h).max(area.top() + 40.0)));
    let strip = Rect::from_min_max(pos2(area.left(), stage.bottom()), area.max);

    let (pw_pt, ph_pt) = book.format.size_pt();
    let aspect = (pw_pt / ph_pt.max(1.0)) as f32;
    let avail = stage.shrink(24.0);
    let (draw_w, draw_h) = if avail.width() / avail.height().max(1.0) > aspect {
        (avail.height() * aspect, avail.height())
    } else {
        (avail.width(), avail.width() / aspect)
    };
    let page_rect = Rect::from_center_size(stage.center(), vec2(draw_w.max(60.0), draw_h.max(60.0)));
    let paper = Color32::from_rgb(book.background[0], book.background[1], book.background[2]);
    let fg = if (u16::from(book.background[0]) + u16::from(book.background[1]) + u16::from(book.background[2])) > 380 {
        Color32::from_gray(32)
    } else {
        Color32::from_gray(232)
    };
    p.rect_filled(page_rect, 3.0, paper);
    p.rect_stroke(page_rect, 3.0, Stroke::new(1.0, Color32::from_black_alpha(90)), egui::StrokeKind::Outside);

    let is_cover = book.include_cover && app.ui.book_page == 0;
    if is_cover {
        if let Some(cid) = book.pages.first().and_then(|pg| pg.photos.first().copied()) {
            let slot_r = Rect::from_min_max(
                pos2(page_rect.left() + page_rect.width() * 0.15, page_rect.top() + page_rect.height() * 0.10),
                pos2(page_rect.right() - page_rect.width() * 0.15, page_rect.top() + page_rect.height() * 0.65),
            );
            draw_photo_slot(app, ui, &p, slot_r, cid);
        }
        p.text(
            pos2(page_rect.center().x, page_rect.top() + page_rect.height() * 0.74),
            Align2::CENTER_CENTER,
            if book.title.is_empty() { crate::i18n::tr("Untitled Book") } else { &book.title },
            t.semibold(18.0),
            fg,
        );
        if !book.subtitle.is_empty() {
            p.text(
                pos2(page_rect.center().x, page_rect.top() + page_rect.height() * 0.82),
                Align2::CENTER_CENTER,
                &book.subtitle,
                t.font(13.0),
                fg,
            );
        }
        if !book.author.is_empty() {
            p.text(
                pos2(page_rect.center().x, page_rect.top() + page_rect.height() * 0.90),
                Align2::CENTER_CENTER,
                &book.author,
                t.font(11.5),
                fg,
            );
        }
    } else {
        let page_idx = app.ui.book_page.saturating_sub(usize::from(book.include_cover));
        if let Some(page) = book.pages.get(page_idx) {
            let slots = preview_slots(page.layout, page_rect);
            for (si, sr) in slots.iter().enumerate() {
                if let Some(&pid) = page.photos.get(si) {
                    draw_photo_slot(app, ui, &p, *sr, pid);
                } else {
                    p.rect_stroke(*sr, 2.0, Stroke::new(1.0, Color32::from_gray(150)), egui::StrokeKind::Inside);
                }
            }
            if !page.caption.is_empty() {
                p.text(
                    pos2(page_rect.center().x, page_rect.bottom() - 18.0),
                    Align2::CENTER_CENTER,
                    &page.caption,
                    t.font(11.5),
                    fg,
                );
            }
            if book.page_numbers {
                p.text(
                    pos2(page_rect.center().x, page_rect.bottom() - 6.0),
                    Align2::CENTER_BOTTOM,
                    format!("{}", page_idx + 1),
                    t.font(10.0),
                    fg,
                );
            }
        }
    }

    // Bottom page navigation pills
    let mut strip_ui = ui.new_child(egui::UiBuilder::new().max_rect(strip));
    strip_ui.horizontal_centered(|ui| {
        ui.add_space(12.0);
        egui::ScrollArea::horizontal().id_salt("book_page_strip").show(ui, |ui| {
            for vi in 0..total_views {
                let lbl = if book.include_cover && vi == 0 {
                    crate::i18n::tr("Cover").to_string()
                } else {
                    let n = vi + 1 - usize::from(book.include_cover);
                    format!("{n}")
                };
                if text_button(ui, &format!("bookPageTab-{vi}"), &lbl, app.ui.book_page == vi).clicked() {
                    app.ui.book_page = vi;
                }
            }
        });
    });
}

fn preview_slots(layout: PageLayout, r: Rect) -> Vec<Rect> {
    let mx = r.width() * 0.08;
    let my = r.height() * 0.08;
    let inner = Rect::from_min_max(pos2(r.left() + mx, r.top() + my), pos2(r.right() - mx, r.bottom() - my));
    match layout {
        PageLayout::FullBleed => vec![r],
        PageLayout::Single => vec![inner],
        PageLayout::CaptionBottom => vec![Rect::from_min_max(inner.min, pos2(inner.right(), inner.bottom() - r.height() * 0.10))],
        PageLayout::TwoUp => {
            let g = r.width() * 0.02;
            let hw = (inner.width() - g) * 0.5;
            vec![
                Rect::from_min_size(inner.min, vec2(hw, inner.height())),
                Rect::from_min_size(pos2(inner.left() + hw + g, inner.top()), vec2(hw, inner.height())),
            ]
        }
        PageLayout::Grid2x2 => {
            let gx = r.width() * 0.02;
            let gy = r.height() * 0.02;
            let cw = (inner.width() - gx) * 0.5;
            let ch = (inner.height() - gy) * 0.5;
            vec![
                Rect::from_min_size(inner.min, vec2(cw, ch)),
                Rect::from_min_size(pos2(inner.left() + cw + gx, inner.top()), vec2(cw, ch)),
                Rect::from_min_size(pos2(inner.left(), inner.top() + ch + gy), vec2(cw, ch)),
                Rect::from_min_size(pos2(inner.left() + cw + gx, inner.top() + ch + gy), vec2(cw, ch)),
            ]
        }
    }
}

fn draw_photo_slot(app: &mut LightcraftApp, ui: &egui::Ui, p: &egui::Painter, slot: Rect, id: lightcraft_catalog::PhotoId) {
    let edge = (slot.width().max(slot.height()) * ui.ctx().pixels_per_point()).round().clamp(64.0, 512.0) as usize;
    super::grid::request_thumb(app, id, edge, 8);
    if let Some(tex) = app.renderer.thumb(id) {
        let [tw, th] = tex.size;
        let ta = tw as f32 / (th.max(1) as f32);
        let sa = slot.width() / slot.height().max(1.0);
        let draw_r = if sa > ta {
            let w = slot.height() * ta;
            Rect::from_center_size(slot.center(), vec2(w, slot.height()))
        } else {
            let h = slot.width() / ta;
            Rect::from_center_size(slot.center(), vec2(slot.width(), h))
        };
        p.image(tex.id(), draw_r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    } else {
        p.rect_filled(slot, 2.0, Color32::from_gray(60));
    }
}
