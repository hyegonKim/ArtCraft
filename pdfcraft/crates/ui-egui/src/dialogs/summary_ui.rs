//! Local offline extractive document summary (`ai.summary`).

use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc,
};

use egui::{Align, Layout};
use pdfcraft_engine::{
    DocId,
    export::ExportSource,
    summary::{self, DocumentSummary, SummaryOptions},
};

use crate::theme::{self, Tokens};
use crate::{PdfCraftApp, widgets};

#[derive(Clone, Copy, PartialEq, Eq)]
struct Key {
    doc: DocId,
    generation: u64,
    page: Option<usize>,
    max_points: usize,
}

struct Job {
    key: Key,
    receiver: mpsc::Receiver<Result<DocumentSummary, String>>,
    cancel: Arc<AtomicBool>,
    done: Arc<AtomicUsize>,
}

impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

pub(crate) struct SummaryDraft {
    pub current_page: bool,
    pub max_points: usize,
    key: Option<Key>,
    job: Option<Job>,
    pub(crate) result: Option<Result<DocumentSummary, String>>,
}

impl Default for SummaryDraft {
    fn default() -> Self {
        Self {
            current_page: false,
            max_points: 5,
            key: None,
            job: None,
            result: None,
        }
    }
}

impl SummaryDraft {
    pub(crate) fn cancel(&mut self) {
        if let Some(job) = &self.job {
            job.cancel.store(true, Ordering::Relaxed);
        }
        self.key = None;
        self.result = None;
    }

    fn update(&mut self, key: Key, source: impl FnOnce() -> ExportSource, ctx: &egui::Context, inline: bool) {
        if self.key != Some(key) {
            self.cancel();
            self.key = Some(key);
        }
        if let Some(job) = &self.job {
            let received = match job.receiver.try_recv() {
                Ok(res) => Some(res),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(tl!("Document summarization stopped unexpectedly.").into())),
            };
            if let Some(res) = received {
                if job.key == key && !job.cancel.load(Ordering::Relaxed) {
                    self.result = Some(res);
                }
                self.job = None;
            }
        }
        if self.job.is_some() || self.result.is_some() {
            return;
        }
        let source = source();
        let pages: Vec<_> = key.page.map_or_else(|| (0..source.pages).collect(), |p| vec![p]);
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicUsize::new(0));
        self.job = Some(Job { key, receiver, cancel: cancel.clone(), done: done.clone() });
        let ctx = ctx.clone();
        let work = move || {
            let res = summary::summarize(source, &pages, SummaryOptions { max_points: key.max_points }, |n| {
                done.store(n, Ordering::Relaxed);
                ctx.request_repaint();
                !cancel.load(Ordering::Relaxed)
            });
            let _ = sender.send(res);
            ctx.request_repaint();
        };
        #[cfg(not(target_arch = "wasm32"))]
        if !inline {
            if let Err(err) = std::thread::Builder::new().name("doc-summary".into()).spawn(work) {
                self.job = None;
                self.result = Some(Err(err.to_string()));
            }
            return;
        }
        let _ = inline;
        work();
    }
}

/// Renders the Document Summary modal body; returns `true` when closed.
pub(crate) fn body(ui: &mut egui::Ui, app: &mut PdfCraftApp, t: &Tokens) -> bool {
    ui.label(egui::RichText::new(tl!("Document summary")).font(theme::semibold(18.0)));
    ui.add_space(4.0);
    let Some((view_idx, id)) = app.active_ids() else { return true };
    let Some(doc) = app.session.get(id) else { return true };
    if let Err(error) = doc.check_copying() {
        app.summary_draft.cancel();
        ui.label(error.to_string());
        return render_close_only(ui);
    }
    let doc_title = doc.info.title.clone();
    let current_view_page = app.views[view_idx].current;

    ui.horizontal(|ui| {
        ui.add(widgets::checkbox(&mut app.summary_draft.current_page, tl!("Current page only")));
        ui.separator();
        ui.label(tl!("Key points:"));
        for n in [3usize, 5, 8] {
            ui.selectable_value(&mut app.summary_draft.max_points, n, n.to_string());
        }
    });
    ui.label(egui::RichText::new(tl!("Runs locally using TF-IDF key terms and sentence centrality.")).color(t.text_muted));
    let key = Key {
        doc: id,
        generation: doc.edit_generation(),
        page: app.summary_draft.current_page.then_some(current_view_page),
        max_points: app.summary_draft.max_points,
    };
    app.summary_draft.update(key, || doc.export_source(), ui.ctx(), app.run_inline);
    ui.add_space(6.0);

    let mut jump_page = None;
    render_summary_result(ui, &app.summary_draft, t, &mut jump_page);

    if let Some(target_page) = jump_page
        && let Some(v) = app.views.get_mut(view_idx)
    {
        v.current = target_page.min(v.page_count().saturating_sub(1));
    }

    ui.add_space(10.0);
    render_summary_footer(ui, &app.summary_draft, doc_title.as_deref().unwrap_or(""))
}

fn render_summary_result(
    ui: &mut egui::Ui,
    draft: &SummaryDraft,
    t: &Tokens,
    jump_page: &mut Option<usize>,
) {
    egui::Frame::new()
        .fill(t.hover)
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                ui.set_min_height(140.0);
                match &draft.result {
                    Some(Ok(summary)) => render_summary_ok(ui, summary, t, jump_page),
                    Some(Err(err)) => {
                        ui.colored_label(ui.visuals().error_fg_color, err);
                    }
                    None => {
                        ui.spinner();
                        let done = draft.job.as_ref().map_or(0, |j| j.done.load(Ordering::Relaxed));
                        ui.label(crate::i18n::fmt(tl!("Summarizing… {n} pages read"), &[("n", &done.to_string())]));
                    }
                }
            });
        });
}

fn render_summary_ok(
    ui: &mut egui::Ui,
    summary: &DocumentSummary,
    t: &Tokens,
    jump_page: &mut Option<usize>,
) {
    ui.strong(crate::i18n::fmt(
        tl!("Words: {w}  ·  Reading time: {m} min"),
        &[
            ("w", &summary.word_count.to_string()),
            ("m", &summary.reading_minutes.to_string()),
        ],
    ));
    if !summary.key_terms.is_empty() {
        ui.label(
            egui::RichText::new(crate::i18n::fmt(
                tl!("Key terms: {terms}"),
                &[("terms", &summary.key_terms.join(", "))],
            ))
            .color(t.text_muted),
        );
    }
    ui.separator();
    if summary.points.is_empty() {
        ui.label(tl!("No extractable sentences found. Scanned images need OCR first."));
        return;
    }
    for pt in &summary.points {
        ui.horizontal_wrapped(|ui| {
            let p_str = pt.page_number().to_string();
            if ui
                .small_button(crate::i18n::fmt(tl!("Page {p}"), &[("p", &p_str)]))
                .clicked()
            {
                *jump_page = Some(pt.page);
            }
            ui.label(&pt.text);
        });
        ui.add_space(4.0);
    }
}

fn render_close_only(ui: &mut egui::Ui) -> bool {
    let mut close = false;
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::pill_button(ui, tl!("Close"), true).clicked() {
                close = true;
            }
        });
    });
    close
}

fn render_summary_footer(ui: &mut egui::Ui, draft: &SummaryDraft, title: &str) -> bool {
    let mut close = false;
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::pill_button(ui, tl!("Close"), true).clicked() {
                close = true;
            }
            if let Some(Ok(summary)) = &draft.result
                && widgets::pill_button(ui, tl!("Copy Markdown"), false).clicked()
            {
                ui.ctx().copy_text(summary.to_markdown(title));
            }
        });
    });
    close
}
