//! Sign ▸ Content Credentials (C2PA / ISO 19566-5 JUMBF provenance inspection).

use egui::{Align, Layout};
use pdfcraft_engine::{DocId, sign::ContentCredentialsReport};

use crate::theme::{self, Tokens};
use crate::{PdfCraftApp, widgets};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct C2paDraft {
    pub last_doc: Option<(DocId, u64)>,
    pub report: Option<ContentCredentialsReport>,
}

/// Renders the Content Credentials modal body; returns `true` when closed.
pub(crate) fn body(ui: &mut egui::Ui, app: &mut PdfCraftApp, t: &Tokens) -> bool {
    ui.label(egui::RichText::new(tl!("Content Credentials")).font(theme::semibold(18.0)));
    ui.add_space(6.0);
    let Some((_, id)) = app.active_ids() else { return true };
    let Some(doc) = app.session.get(id) else { return true };
    let doc_gen = doc.edit_generation();
    if app.c2pa_draft.last_doc != Some((id, doc_gen)) || app.c2pa_draft.report.is_none() {
        app.c2pa_draft.last_doc = Some((id, doc_gen));
        app.c2pa_draft.report = Some(doc.content_credentials());
    }
    ui.label(
        egui::RichText::new(tl!(
            "Inspects C2PA JUMBF manifests in Associated Files, XMP metadata, and embedded images."
        ))
        .color(t.text_muted),
    );
    ui.add_space(6.0);

    if let Some(report) = &app.c2pa_draft.report {
        egui::Frame::new()
            .fill(t.hover)
            .corner_radius(egui::CornerRadius::same(6))
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                    ui.set_min_height(120.0);
                    render_report_contents(ui, report, t);
                });
            });
    }

    ui.add_space(10.0);
    let mut close = false;
    let mut copy_text = false;
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::pill_button(ui, tl!("Close"), true).clicked() {
                close = true;
            }
            if app.c2pa_draft.report.as_ref().is_some_and(ContentCredentialsReport::has_credentials)
                && widgets::pill_button(ui, tl!("Copy summary"), false).clicked()
            {
                copy_text = true;
            }
        });
    });
    if copy_text && let Some(report) = &app.c2pa_draft.report {
        ui.ctx().copy_text(report.summary_text());
    }
    close
}

fn render_report_contents(ui: &mut egui::Ui, report: &ContentCredentialsReport, t: &Tokens) {
    if !report.has_credentials() {
        ui.label(tl!("No Content Credentials were found in this document."));
        return;
    }
    if let Some(uri) = &report.xmp_provenance {
        ui.label(crate::i18n::fmt(tl!("XMP provenance: {uri}"), &[("uri", uri)]));
        ui.add_space(4.0);
    }
    ui.strong(crate::i18n::fmt(
        tl!("C2PA manifests found: {n}"),
        &[("n", &report.manifests.len().to_string())],
    ));
    for manifest in &report.manifests {
        ui.separator();
        ui.strong(&manifest.source);
        if !manifest.label.is_empty() {
            ui.label(egui::RichText::new(&manifest.label).small().color(t.text_muted));
        }
        if let Some(generator) = &manifest.claim_generator {
            ui.label(crate::i18n::fmt(tl!("Produced with: {tool}"), &[("tool", generator)]));
        }
        if let Some(title) = &manifest.title {
            ui.label(crate::i18n::fmt(tl!("Asset title: {title}"), &[("title", title)]));
        }
        if !manifest.assertions.is_empty() {
            ui.label(crate::i18n::fmt(
                tl!("Assertions: {list}"),
                &[("list", &manifest.assertions.join(", "))],
            ));
        }
        for signer in &manifest.signers {
            ui.label(crate::i18n::fmt(
                tl!("Signed by: {subject} (Issuer: {issuer})"),
                &[("subject", &signer.subject), ("issuer", &signer.issuer)],
            ));
        }
    }
}
