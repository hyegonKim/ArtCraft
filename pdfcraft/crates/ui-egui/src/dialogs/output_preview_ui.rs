//! Print production ▸ Output preview: interactive CMYK channel separation viewer (`C`, `M`, `Y`, `K`)
//! and Total Area Coverage (`TAC %`) press ink-limit inspector & heatmap.

use egui::{Align, ColorImage, Layout, TextureHandle, TextureOptions, vec2};
use pdfcraft_engine::{
    DocId,
    output_preview::DEFAULT_PREVIEW_MAX_SIDE,
    pdfa::{CmykPlates, DEFAULT_INK_LIMIT_PERCENT, NEWSPRINT_INK_LIMIT_PERCENT, OutputPreviewResult},
};

use crate::theme::{self, Tokens};
use crate::{PdfCraftApp, widgets};

#[derive(Clone, Copy, PartialEq, Eq)]
struct CacheKey {
    doc: DocId,
    generation: u64,
    page: usize,
    plates: CmykPlates,
    limit_tenths: u32,
    heatmap: bool,
}

pub struct OutputPreviewDraft {
    pub plates: CmykPlates,
    pub ink_limit_percent: f32,
    pub show_tac_heatmap: bool,
    cache_key: Option<CacheKey>,
    pub result: Option<Result<OutputPreviewResult, String>>,
    texture: Option<TextureHandle>,
}

impl Default for OutputPreviewDraft {
    fn default() -> Self {
        Self {
            plates: CmykPlates::ALL,
            ink_limit_percent: DEFAULT_INK_LIMIT_PERCENT,
            show_tac_heatmap: true,
            cache_key: None,
            result: None,
            texture: None,
        }
    }
}

impl OutputPreviewDraft {
    pub fn invalidate(&mut self) {
        self.cache_key = None;
        self.result = None;
        self.texture = None;
    }
}

/// Renders the Output Preview modal body; returns `true` when closed.
pub(crate) fn body(ui: &mut egui::Ui, app: &mut PdfCraftApp, t: &Tokens) -> bool {
    ui.label(egui::RichText::new(tl!("Output preview")).font(theme::semibold(18.0)));
    ui.add_space(6.0);

    let Some((view_idx, id)) = app.active_ids() else {
        return true;
    };
    let page = app.views.get(view_idx).map_or(0, |v| v.current);
    let Some(generation) = app.session.get(id).map(|d| d.edit_generation()) else {
        return true;
    };

    render_controls(ui, &mut app.output_preview_draft);
    ui.add_space(6.0);

    let limit_tenths = (app.output_preview_draft.ink_limit_percent * 10.0).round() as u32;
    let wanted = CacheKey {
        doc: id,
        generation,
        page,
        plates: app.output_preview_draft.plates,
        limit_tenths,
        heatmap: app.output_preview_draft.show_tac_heatmap,
    };
    if app.output_preview_draft.cache_key != Some(wanted) {
        app.output_preview_draft.cache_key = Some(wanted);
        let computed = app
            .session
            .output_preview(id, page, wanted.plates, app.output_preview_draft.ink_limit_percent, wanted.heatmap, DEFAULT_PREVIEW_MAX_SIDE)
            .map_err(|e| e.to_string());
        app.output_preview_draft.texture = computed.as_ref().ok().map(|res| {
            let img = ColorImage::from_rgba_unmultiplied([res.summary.width as usize, res.summary.height as usize], &res.preview_rgba);
            ui.ctx().load_texture("output-preview-plate", img, TextureOptions::LINEAR)
        });
        app.output_preview_draft.result = Some(computed);
    }

    render_preview_and_stats(ui, &app.output_preview_draft, t);
    ui.add_space(10.0);

    let mut close = false;
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::pill_button(ui, tl!("Close"), true).clicked() {
                close = true;
            }
        });
    });
    close
}

fn render_controls(ui: &mut egui::Ui, draft: &mut OutputPreviewDraft) {
    ui.horizontal_wrapped(|ui| {
        ui.strong(tl!("Process plates"));
        ui.add(widgets::checkbox(&mut draft.plates.cyan, tl!("Cyan (C)")));
        ui.add(widgets::checkbox(&mut draft.plates.magenta, tl!("Magenta (M)")));
        ui.add(widgets::checkbox(&mut draft.plates.yellow, tl!("Yellow (Y)")));
        ui.add(widgets::checkbox(&mut draft.plates.black, tl!("Black (K)")));
    });
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.strong(tl!("Total Area Coverage (TAC)"));
        ui.add(widgets::checkbox(&mut draft.show_tac_heatmap, tl!("Show TAC heatmap")));
        ui.label(tl!("Ink limit:"));
        let selected_label = if (draft.ink_limit_percent - DEFAULT_INK_LIMIT_PERCENT).abs() < 0.5 {
            tl!("300% (Coated offset)").to_string()
        } else if (draft.ink_limit_percent - NEWSPRINT_INK_LIMIT_PERCENT).abs() < 0.5 {
            tl!("240% (Newsprint / uncoated)").to_string()
        } else {
            format!("{:.0}%", draft.ink_limit_percent)
        };
        craft_ui::ComboBox::from_id_salt("output-preview-ink-limit").selected_text(selected_label).show_ui(ui, |ui| {
            ui.selectable_value(&mut draft.ink_limit_percent, DEFAULT_INK_LIMIT_PERCENT, tl!("300% (Coated offset)"));
            ui.selectable_value(&mut draft.ink_limit_percent, NEWSPRINT_INK_LIMIT_PERCENT, tl!("240% (Newsprint / uncoated)"));
            ui.selectable_value(&mut draft.ink_limit_percent, 280.0, "280%");
            ui.selectable_value(&mut draft.ink_limit_percent, 320.0, "320%");
        });
    });
}

fn render_preview_and_stats(ui: &mut egui::Ui, draft: &OutputPreviewDraft, t: &Tokens) {
    match &draft.result {
        Some(Ok(res)) => {
            let s = &res.summary;
            let status_color = if s.over_limit_pixels > 0 || s.content_stream_over_limit_ops > 0 { ui.visuals().warn_fg_color } else { t.text };
            ui.colored_label(
                status_color,
                crate::i18n::fmt(
                    tl!("Max TAC: {max}%  ·  Over limit: {pct}% ({px} px)"),
                    &[
                        ("max", &format!("{:.1}", s.max_tac_percent)),
                        ("pct", &format!("{:.2}", s.over_limit_percent)),
                        ("px", &s.over_limit_pixels.to_string()),
                    ],
                ),
            );
            if s.over_limit_pixels == 0 && s.content_stream_over_limit_ops == 0 {
                ui.label(
                    egui::RichText::new(crate::i18n::fmt(
                        tl!("All pixels are within the {limit}% press ink limit."),
                        &[("limit", &format!("{:.0}", s.ink_limit_percent))],
                    ))
                    .color(t.text_muted),
                );
            }
            ui.label(
                egui::RichText::new(format!(
                    "C {:.1}% (max {:.0}%) · M {:.1}% (max {:.0}%) · Y {:.1}% (max {:.0}%) · K {:.1}% (max {:.0}%)",
                    s.plate_mean_percent[0],
                    s.plate_max_percent[0],
                    s.plate_mean_percent[1],
                    s.plate_max_percent[1],
                    s.plate_mean_percent[2],
                    s.plate_max_percent[2],
                    s.plate_mean_percent[3],
                    s.plate_max_percent[3],
                ))
                .color(t.text_muted),
            );
            ui.add_space(6.0);
            if let Some(tex) = &draft.texture {
                egui::Frame::NONE.fill(t.hover).corner_radius(egui::CornerRadius::same(6)).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                    let avail_w = ui.available_width().max(120.0);
                    let max_h = 280.0f32;
                    let tex_sz = tex.size_vec2();
                    let scale = (avail_w / tex_sz.x.max(1.0)).min(max_h / tex_sz.y.max(1.0)).min(1.0);
                    let draw_sz = vec2(tex_sz.x * scale, tex_sz.y * scale);
                    ui.vertical_centered(|ui| {
                        let resp = ui.image((tex.id(), draw_sz));
                        if let Some(pointer) = resp.hover_pos() {
                            let rel_x = ((pointer.x - resp.rect.min.x) / draw_sz.x.max(1.0)).clamp(0.0, 0.9999);
                            let rel_y = ((pointer.y - resp.rect.min.y) / draw_sz.y.max(1.0)).clamp(0.0, 0.9999);
                            let px = (rel_x * s.width as f32) as usize;
                            let py = (rel_y * s.height as f32) as usize;
                            if let Some(&tenths) = res.tac_tenths.get(py * s.width as usize + px) {
                                resp.on_hover_text(format!("({px}, {py}) — TAC {:.1}%", f32::from(tenths) / 10.0));
                            }
                        }
                    });
                });
            }
        }
        Some(Err(err)) => {
            ui.colored_label(ui.visuals().error_fg_color, err);
        }
        None => {}
    }
}
