//! Frame.io (`FFC_framemob`) Team Review & Approval modal dialog with reply threading and
//! interactive normalized `[0, 1]` Telestrator vector annotations.

use effectcraft_interchange::frameio::{FrameIoReviewComment, ReviewStatus, TelestratorAnnotation, TelestratorShape, replies_to, root_comments};
use egui::{Align2, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use serde_json::json;

use crate::i18n::{tr, tr_args};
use crate::theme::Tokens;
use crate::{Dialog, EffectcraftApp};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TelestratorTool {
    #[default]
    Freehand,
    Arrow,
    Rect,
    Ellipse,
}

impl TelestratorTool {
    pub const ALL: [TelestratorTool; 4] = [TelestratorTool::Freehand, TelestratorTool::Arrow, TelestratorTool::Rect, TelestratorTool::Ellipse];

    pub fn label(self) -> &'static str {
        match self {
            TelestratorTool::Freehand => "Freehand",
            TelestratorTool::Arrow => "Arrow",
            TelestratorTool::Rect => "Rectangle",
            TelestratorTool::Ellipse => "Ellipse",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ReviewDraft {
    pub author: String,
    pub draft_text: String,
    pub reply_text: String,
    pub import_buffer: String,
    pub show_import: bool,
    pub status_filter: Option<ReviewStatus>,
    pub selected_comment: Option<String>,
    pub tool: TelestratorTool,
    pub stroke_rgba: [f32; 4],
    pub drag_points: Vec<[f32; 2]>,
}

impl Default for ReviewDraft {
    fn default() -> Self {
        Self {
            author: "Reviewer".into(),
            draft_text: String::new(),
            reply_text: String::new(),
            import_buffer: String::new(),
            show_import: false,
            status_filter: None,
            selected_comment: None,
            tool: TelestratorTool::Freehand,
            stroke_rgba: [1.0, 0.25, 0.25, 1.0],
            drag_points: Vec::new(),
        }
    }
}

pub fn open(app: &mut EffectcraftApp) {
    app.dialog = Some(Dialog::FrameIoReview);
}

pub fn dialog(app: &mut EffectcraftApp, ctx: &egui::Context, t: &Tokens) {
    let mut state = app.dialog_state.review.clone();
    let mut close = false;
    let comments: Vec<FrameIoReviewComment> = app.session.active_comp().map(effectcraft_engine::commands::review::extract_comp_reviews).unwrap_or_default();

    if state.selected_comment.as_deref().is_some_and(|id| !comments.iter().any(|c| c.id == id)) {
        state.selected_comment = comments.first().map(|c| c.id.clone());
    } else if state.selected_comment.is_none() {
        state.selected_comment = comments.first().map(|c| c.id.clone());
    }

    super::dialogs::modal(ctx, tr("Frame.io Review"), vec2(620.0, 560.0), t, |ui| {
        if app.session.active_comp().is_none() {
            ui.label(tr("(no composition)"));
            if ui.button(tr("Close")).clicked() {
                close = true;
            }
            return;
        }
        show_toolbar(app, ui, &mut state);
        if state.show_import {
            show_import_drawer(app, ui, &mut state);
        }
        ui.separator();
        show_composer(app, ui, &mut state);
        ui.separator();
        show_telestrator_canvas(app, ui, t, &mut state, &comments);
        ui.separator();
        show_thread_list(app, ui, t, &mut state, &comments);
        ui.separator();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let r = ui.button(tr("Close"));
            app.auto.add("dialog.review.close", r.rect, "Close");
            if r.clicked() {
                close = true;
            }
        });
    });

    app.dialog_state.review = state;
    if close {
        app.dialog = None;
    }
}

fn status_label(st: ReviewStatus) -> &'static str {
    match st {
        ReviewStatus::NeedsReview => tr("Needs Review"),
        ReviewStatus::InProgress => tr("In Progress"),
        ReviewStatus::Approved => tr("Approved"),
    }
}

fn show_toolbar(app: &mut EffectcraftApp, ui: &mut egui::Ui, state: &mut ReviewDraft) {
    ui.horizontal_wrapped(|ui| {
        if ui.selectable_label(state.status_filter.is_none(), tr("All")).clicked() {
            state.status_filter = None;
        }
        for st in ReviewStatus::ALL {
            if ui.selectable_label(state.status_filter == Some(st), status_label(st)).clicked() {
                state.status_filter = Some(st);
            }
        }
        if ui.button(tr("Export JSON")).clicked() {
            export_to_clipboard(app, ui, "json");
        }
        if ui.button(tr("Export CSV")).clicked() {
            export_to_clipboard(app, ui, "csv");
        }
        if ui.selectable_label(state.show_import, tr("Import Frame.io...")).clicked() {
            state.show_import = !state.show_import;
        }
    });
}

fn export_to_clipboard(app: &mut EffectcraftApp, ui: &egui::Ui, fmt: &str) {
    match app.session.execute("review.exportFrameIo", json!({ "format": fmt })) {
        Ok(res) => {
            if let Some(data) = res.get("data").and_then(|v| v.as_str()) {
                ui.ctx().copy_text(data.to_string());
            }
            let n = res.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
            app.ui.status = tr_args("Copied {} Frame.io review comment(s)", &[&n]);
        }
        Err(e) => app.ui.status = e.to_string(),
    }
}

fn show_import_drawer(app: &mut EffectcraftApp, ui: &mut egui::Ui, state: &mut ReviewDraft) {
    ui.label(tr("Paste Frame.io JSON or CSV:"));
    ui.add(egui::TextEdit::multiline(&mut state.import_buffer).desired_rows(3).desired_width(f32::INFINITY));
    if ui.button(tr("Import Review Notes")).clicked() {
        match app.session.execute("review.importFrameIo", json!({ "data": state.import_buffer })) {
            Ok(res) => {
                let n = res.get("imported").and_then(|v| v.as_u64()).unwrap_or(0);
                app.ui.status = tr_args("Imported {} Frame.io review comment(s)", &[&n]);
                state.import_buffer.clear();
                state.show_import = false;
            }
            Err(e) => app.ui.status = e.to_string(),
        }
    }
}

fn show_composer(app: &mut EffectcraftApp, ui: &mut egui::Ui, state: &mut ReviewDraft) {
    ui.horizontal(|ui| {
        ui.label(tr("Author:"));
        ui.add(egui::TextEdit::singleline(&mut state.author).desired_width(110.0));
        ui.add(egui::TextEdit::singleline(&mut state.draft_text).hint_text(tr("Add frame-accurate review note...")).desired_width(ui.available_width() - 96.0));
        if ui.add_enabled(!state.draft_text.trim().is_empty(), egui::Button::new(tr("Add Note"))).clicked() {
            match app.session.execute("review.addComment", json!({ "author": state.author, "text": state.draft_text })) {
                Ok(res) => {
                    if let Some(id) = res.get("id").and_then(|v| v.as_str()) {
                        state.selected_comment = Some(id.to_string());
                    }
                    state.draft_text.clear();
                }
                Err(e) => app.ui.status = e.to_string(),
            }
        }
    });
}

fn to_screen_pt(canvas: Rect, norm: [f32; 2]) -> Pos2 {
    pos2(canvas.min.x + norm[0].clamp(0.0, 1.0) * canvas.width(), canvas.min.y + norm[1].clamp(0.0, 1.0) * canvas.height())
}

fn to_norm_pt(canvas: Rect, p: Pos2) -> [f32; 2] {
    let w = canvas.width().max(1.0);
    let h = canvas.height().max(1.0);
    [((p.x - canvas.min.x) / w).clamp(0.0, 1.0), ((p.y - canvas.min.y) / h).clamp(0.0, 1.0)]
}

fn rgba_color32(c: [f32; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(
        (c[0].clamp(0.0, 1.0) * 255.0).round() as u8,
        (c[1].clamp(0.0, 1.0) * 255.0).round() as u8,
        (c[2].clamp(0.0, 1.0) * 255.0).round() as u8,
        (c[3].clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

pub fn paint_telestrator_annotation(painter: &egui::Painter, canvas: Rect, ann: &TelestratorAnnotation) {
    let stroke = Stroke::new(ann.stroke_width.clamp(1.0, 12.0), rgba_color32(ann.color));
    match &ann.shape {
        TelestratorShape::Freehand { points } => {
            for seg in points.windows(2) {
                if let (Some(&a), Some(&b)) = (seg.first(), seg.get(1)) {
                    painter.line_segment([to_screen_pt(canvas, a), to_screen_pt(canvas, b)], stroke);
                }
            }
        }
        TelestratorShape::Arrow { start, end } => {
            let a = to_screen_pt(canvas, *start);
            let b = to_screen_pt(canvas, *end);
            painter.line_segment([a, b], stroke);
            let dir = b - a;
            let len = dir.length();
            if len > 2.0 {
                let u = dir / len;
                let n = vec2(-u.y, u.x);
                let head = 8.0_f32.min(len * 0.45);
                painter.line_segment([b, b - u * head + n * (head * 0.55)], stroke);
                painter.line_segment([b, b - u * head - n * (head * 0.55)], stroke);
            }
        }
        TelestratorShape::Rect { min, max } => {
            let r = Rect::from_two_pos(to_screen_pt(canvas, *min), to_screen_pt(canvas, *max));
            painter.rect_stroke(r, 2.0, stroke, StrokeKind::Inside);
        }
        TelestratorShape::Ellipse { center, radii } => {
            let c = to_screen_pt(canvas, *center);
            let rx = (radii[0] * canvas.width()).max(1.0);
            let ry = (radii[1] * canvas.height()).max(1.0);
            let mut pts = Vec::with_capacity(33);
            for i in 0..=32 {
                let ang = (i as f32 / 32.0) * std::f32::consts::TAU;
                pts.push(pos2(c.x + rx * ang.cos(), c.y + ry * ang.sin()));
            }
            for seg in pts.windows(2) {
                if let (Some(&p0), Some(&p1)) = (seg.first(), seg.get(1)) {
                    painter.line_segment([p0, p1], stroke);
                }
            }
        }
    }
}

fn show_telestrator_canvas(app: &mut EffectcraftApp, ui: &mut egui::Ui, t: &Tokens, state: &mut ReviewDraft, comments: &[FrameIoReviewComment]) {
    ui.horizontal_wrapped(|ui| {
        ui.label(tr("Telestrator:"));
        for tool in TelestratorTool::ALL {
            if ui.selectable_label(state.tool == tool, tr(tool.label())).clicked() {
                state.tool = tool;
            }
        }
        for rgba in [[1.0, 0.25, 0.25, 1.0], [1.0, 0.85, 0.15, 1.0], [0.20, 0.88, 0.45, 1.0], [0.20, 0.78, 1.0, 1.0]] {
            let active = state.stroke_rgba == rgba;
            let (sw, resp) = ui.allocate_exact_size(vec2(15.0, 15.0), Sense::click());
            ui.painter().rect_filled(sw.shrink(1.5), 3.0, rgba_color32(rgba));
            if active {
                ui.painter().rect_stroke(sw, 3.0, Stroke::new(1.5, Color32::WHITE), StrokeKind::Inside);
            }
            if resp.clicked() {
                state.stroke_rgba = rgba;
            }
        }
    });

    let avail_w = ui.available_width().clamp(200.0, 560.0);
    let canvas_h = (avail_w * 9.0 / 16.0).clamp(95.0, 145.0);
    let (canvas, resp) = ui.allocate_exact_size(vec2(avail_w, canvas_h), Sense::drag());
    ui.painter().rect_filled(canvas, 4.0, Color32::from_rgb(18, 20, 24));
    ui.painter().rect_stroke(canvas, 4.0, Stroke::new(1.0, t.separator), StrokeKind::Inside);

    let active_comment = state.selected_comment.as_deref().and_then(|id| comments.iter().find(|c| c.id == id));
    if let Some(c) = active_comment {
        for ann in &c.annotations {
            paint_telestrator_annotation(ui.painter(), canvas, ann);
        }
    } else {
        let hint = tr("Add or select a comment to draw Telestrator markup");
        ui.painter().text(canvas.center(), Align2::CENTER_CENTER, hint, Tokens::ui(11.5), t.text_dim);
    }

    if let Some(cid) = state.selected_comment.clone() {
        handle_canvas_drag(app, &resp, canvas, state, &cid);
    }
}

fn handle_canvas_drag(app: &mut EffectcraftApp, resp: &egui::Response, canvas: Rect, state: &mut ReviewDraft, cid: &str) {
    if resp.drag_started() {
        state.drag_points.clear();
        if let Some(pos) = resp.interact_pointer_pos() {
            state.drag_points.push(to_norm_pt(canvas, pos));
        }
    }
    if resp.dragged()
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let pt = to_norm_pt(canvas, pos);
        if state.drag_points.last().is_none_or(|last| (last[0] - pt[0]).hypot(last[1] - pt[1]) > 0.006) && state.drag_points.len() < 512 {
            state.drag_points.push(pt);
        }
    }
    if resp.drag_stopped() {
        if let (Some(&first), Some(&last)) = (state.drag_points.first(), state.drag_points.last()) {
            let payload = match state.tool {
                TelestratorTool::Freehand => {
                    let pts = if state.drag_points.len() >= 2 { state.drag_points.clone() } else { vec![first, [(first[0] + 0.02).min(1.0), first[1]]] };
                    json!({ "id": cid, "kind": "freehand", "points": pts, "color": state.stroke_rgba, "strokeWidth": 3.0 })
                }
                TelestratorTool::Arrow => json!({ "id": cid, "kind": "arrow", "start": first, "end": last, "color": state.stroke_rgba, "strokeWidth": 3.0 }),
                TelestratorTool::Rect => json!({
                    "id": cid,
                    "kind": "rect",
                    "min": [first[0].min(last[0]), first[1].min(last[1])],
                    "max": [first[0].max(last[0]).max(first[0].min(last[0]) + 0.02).min(1.0), first[1].max(last[1]).max(first[1].min(last[1]) + 0.02).min(1.0)],
                    "color": state.stroke_rgba,
                    "strokeWidth": 3.0,
                }),
                TelestratorTool::Ellipse => {
                    let cx = (first[0] + last[0]) * 0.5;
                    let cy = (first[1] + last[1]) * 0.5;
                    let rx = ((last[0] - first[0]).abs() * 0.5).max(0.02);
                    let ry = ((last[1] - first[1]).abs() * 0.5).max(0.02);
                    json!({ "id": cid, "kind": "ellipse", "center": [cx, cy], "radii": [rx, ry], "color": state.stroke_rgba, "strokeWidth": 3.0 })
                }
            };
            if let Err(e) = app.session.execute("review.addAnnotation", payload) {
                app.ui.status = e.to_string();
            }
        }
        state.drag_points.clear();
    }
}

fn show_thread_list(app: &mut EffectcraftApp, ui: &mut egui::Ui, t: &Tokens, state: &mut ReviewDraft, comments: &[FrameIoReviewComment]) {
    let roots: Vec<&FrameIoReviewComment> = root_comments(comments).into_iter().filter(|c| state.status_filter.is_none_or(|st| c.status == st)).collect();

    egui::ScrollArea::vertical().max_height(180.0).auto_shrink([false, false]).show(ui, |ui| {
        if roots.is_empty() {
            ui.label(tr("No Frame.io review comments"));
            return;
        }
        for root in roots {
            show_comment_card(app, ui, t, state, root, false);
            for reply in replies_to(comments, &root.id) {
                show_comment_card(app, ui, t, state, reply, true);
            }
            if state.selected_comment.as_deref() == Some(root.id.as_str()) {
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.add(egui::TextEdit::singleline(&mut state.reply_text).hint_text(tr("Reply to thread...")).desired_width(ui.available_width() - 78.0));
                    if ui.add_enabled(!state.reply_text.trim().is_empty(), egui::Button::new(tr("Reply"))).clicked() {
                        if let Err(e) = app.session.execute(
                            "review.addComment",
                            json!({
                                "author": state.author,
                                "parentId": root.id,
                                "text": state.reply_text,
                            }),
                        ) {
                            app.ui.status = e.to_string();
                        } else {
                            state.reply_text.clear();
                        }
                    }
                });
            }
            ui.add_space(4.0);
        }
    });
}

fn show_comment_card(app: &mut EffectcraftApp, ui: &mut egui::Ui, t: &Tokens, state: &mut ReviewDraft, c: &FrameIoReviewComment, is_reply: bool) {
    let selected = state.selected_comment.as_deref() == Some(c.id.as_str());
    ui.horizontal(|ui| {
        if is_reply {
            ui.add_space(16.0);
        }
        egui::Frame::group(ui.style()).fill(if selected { t.row_selected } else { t.panel_bg }).show(ui, |ui| {
            ui.set_min_width(ui.available_width() - 8.0);
            ui.horizontal_wrapped(|ui| {
                if ui.link(format!("{} · {}", c.timecode, c.author)).clicked() {
                    state.selected_comment = Some(c.id.clone());
                    let _ = app.session.execute("time.set", json!({ "frame": c.frame.max(0) }));
                }
                if !c.annotations.is_empty() {
                    let count = c.annotations.len();
                    ui.small(tr_args("{} drawing(s)", &[&count]));
                }
                for st in ReviewStatus::ALL {
                    if ui.selectable_label(c.status == st, status_label(st)).clicked()
                        && let Err(e) = app.session.execute("review.setStatus", json!({ "id": c.id, "status": st.as_str() }))
                    {
                        app.ui.status = e.to_string();
                    }
                }
            });
            ui.label(&c.text);
        });
    });
}
