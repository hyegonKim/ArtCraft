//! Print production & Standards ▸ Preflight: multi-profile document inspection without mutation.

use egui::{Align, Layout};
use pdfcraft_engine::{
    DocId,
    pdfa::{PreflightProfile, PreflightSeverity, PreflightSummary},
};

use crate::theme::{self, Tokens};
use crate::{PdfCraftApp, widgets};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreflightDraft {
    pub profile: PreflightProfile,
    pub last_doc: Option<(DocId, u64)>,
    pub summary: Option<PreflightSummary>,
}

fn profile_label(profile: PreflightProfile) -> &'static str {
    match profile {
        PreflightProfile::PrintProduction => tl!("Print production"),
        PreflightProfile::PdfA2b => tl!("PDF/A-2b archival"),
        PreflightProfile::PdfA3b => tl!("PDF/A-3b archival"),
        PreflightProfile::DigitalPublishing => tl!("Online & digital publishing"),
        PreflightProfile::SecurityAudit => tl!("Interactive actions & hidden content"),
    }
}

fn profile_description(profile: PreflightProfile) -> &'static str {
    match profile {
        PreflightProfile::PrintProduction => {
            tl!("Checks effective image resolution, trim and bleed boxes, output intents, and embedded fonts.")
        }
        PreflightProfile::PdfA2b => {
            tl!("Checks ISO 19005-2 structural rules for self-contained archival documents.")
        }
        PreflightProfile::PdfA3b => {
            tl!("Checks ISO 19005-3 structural rules including associated embedded files.")
        }
        PreflightProfile::DigitalPublishing => {
            tl!("Checks document title, language tag, tagged structure, embedded fonts, and encryption.")
        }
        PreflightProfile::SecurityAudit => {
            tl!("Audits scripts, launch and external actions, attachments, unapplied redactions, and XFA.")
        }
    }
}

/// Renders the Preflight modal body; returns `true` when the user closes the dialog.
pub(crate) fn body(ui: &mut egui::Ui, app: &mut PdfCraftApp, t: &Tokens) -> bool {
    ui.label(egui::RichText::new(tl!("Preflight")).font(theme::semibold(18.0)));
    ui.add_space(6.0);
    let Some((view_idx, id)) = app.active_ids() else { return true };
    let Some(doc_gen) = app.session.get(id).map(|d| d.edit_generation()) else {
        return true;
    };
    if app.preflight_draft.last_doc != Some((id, doc_gen)) {
        app.preflight_draft.last_doc = Some((id, doc_gen));
        app.preflight_draft.summary = None;
    }

    let before = app.preflight_draft.profile;
    ui.horizontal(|ui| {
        ui.label(tl!("Profile:"));
        craft_ui::ComboBox::from_id_salt("preflight-profile").selected_text(profile_label(app.preflight_draft.profile)).show_ui(ui, |ui| {
            for p in PreflightProfile::ALL {
                ui.selectable_value(&mut app.preflight_draft.profile, p, profile_label(p));
            }
        });
    });
    if app.preflight_draft.profile != before {
        app.preflight_draft.summary = None;
    }
    ui.label(egui::RichText::new(profile_description(app.preflight_draft.profile)).color(t.text_muted));
    ui.add_space(6.0);

    if app.preflight_draft.summary.is_none()
        && let Some(doc) = app.session.get(id)
    {
        app.preflight_draft.summary = Some(doc.preflight_run(app.preflight_draft.profile));
    }

    let mut jump_to_page = None;
    if let Some(summary) = &app.preflight_draft.summary {
        ui.horizontal(|ui| {
            ui.strong(crate::i18n::fmt(
                tl!("Errors: {e}  ·  Warnings: {w}  ·  Info: {i}"),
                &[("e", &summary.errors.to_string()), ("w", &summary.warnings.to_string()), ("i", &summary.infos.to_string())],
            ));
        });
        ui.add_space(4.0);
        egui::Frame::new().fill(t.hover).corner_radius(egui::CornerRadius::same(6)).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                ui.set_min_height(120.0);
                if summary.findings.is_empty() {
                    ui.label(tl!("Structural preflight checks found no problems for this profile."));
                    return;
                }
                for finding in &summary.findings {
                    render_finding_row(ui, finding, t, &mut jump_to_page);
                }
            });
        });
    }

    if let Some(page_zero) = jump_to_page
        && let Some(v) = app.views.get_mut(view_idx)
    {
        v.current = page_zero.min(v.page_count().saturating_sub(1));
    }

    ui.add_space(10.0);
    render_footer(ui, app, id)
}

fn render_finding_row(ui: &mut egui::Ui, finding: &pdfcraft_engine::pdfa::PreflightFinding, t: &Tokens, jump_to_page: &mut Option<usize>) {
    let badge = match finding.severity {
        PreflightSeverity::Error => (tl!("Error"), ui.visuals().error_fg_color),
        PreflightSeverity::Warning => (tl!("Warning"), ui.visuals().warn_fg_color),
        PreflightSeverity::Info => (tl!("Info"), t.text_muted),
    };
    ui.horizontal_wrapped(|ui| {
        ui.colored_label(badge.1, format!("[{}]", badge.0));
        ui.label(&finding.message);
        if let Some(p) = finding.page {
            let page_btn = crate::i18n::fmt(tl!("Page {p}"), &[("p", &p.to_string())]);
            if ui.small_button(page_btn).clicked() {
                *jump_to_page = Some(p.saturating_sub(1));
            }
        }
    });
}

fn render_footer(ui: &mut egui::Ui, app: &mut PdfCraftApp, id: DocId) -> bool {
    let mut close = false;
    let mut rerun = false;
    let mut copy_report = false;
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::pill_button(ui, tl!("Analyze"), true).clicked() {
                rerun = true;
            }
            if widgets::pill_button(ui, tl!("Copy report"), false).clicked() {
                copy_report = true;
            }
            if widgets::pill_button(ui, tl!("Close"), false).clicked() {
                close = true;
            }
        });
    });
    if rerun && let Some(doc) = app.session.get(id) {
        app.preflight_draft.summary = Some(doc.preflight_run(app.preflight_draft.profile));
    }
    if copy_report && let Some(summary) = &app.preflight_draft.summary {
        ui.ctx().copy_text(summary.report_text());
    }
    close
}
