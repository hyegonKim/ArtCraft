//! Photoshop 2027 (`28.0.0.20`) `Window › Timelapse` (`타임랩스`) floating panel and
//! `File › Export › Export Timelapse…` (`타임랩스 내보내기…`) dialog.
//!
//! All state mutations go strictly through `Session::execute` (`app.run`) and `menus::invoke`.

use egui::{CornerRadius, RichText, Sense, Stroke, pos2, vec2};
use photocraft_engine::timelapse_cmds::TimelapseState;
use photocraft_io::{TimelapseQuality, TimelapseResolution};
use serde_json::{Map, Value, json};

use crate::file_dialog::{FileDialogAnswer, FileDialogRequest};
use crate::state::DialogKind;
use crate::theme::{Tokens, mono, semibold};
use crate::{PhotocraftApp, icons, widgets};

/// Route marker for the `File › Export › Export Timelapse…` command dialog.
pub const ROUTE_MARKER: &str = "__timelapseExport";

pub fn handles(id: &str) -> bool {
    matches!(id, "window.toggle.timelapse" | "window.panel.timelapse")
}

pub fn owns(fields: &Map<String, Value>) -> bool {
    fields.contains_key(ROUTE_MARKER)
}

/// Intercepts `window.toggle.timelapse` and parameterless `file.export.timelapse`.
pub fn invoke(app: &mut PhotocraftApp, id: &str, params: &Value) -> Option<Result<Value, String>> {
    match id {
        "window.toggle.timelapse" | "window.panel.timelapse" => {
            let on = params
                .get("on")
                .and_then(Value::as_bool)
                .unwrap_or(!app.ui.panels.timelapse);
            app.ui.panels.timelapse = on;
            Some(Ok(json!({ "open": on })))
        }
        "file.export.timelapse" if params.get("path").is_none() => Some(open_export_dialog(app, params)),
        _ => None,
    }
}

pub fn open_export_dialog(app: &mut PhotocraftApp, params: &Value) -> Result<Value, String> {
    if app.session.active().is_none() {
        return Err("no document open".into());
    }
    let fmt = params
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("mp4")
        .to_string();
    let fps = params
        .get("fps")
        .and_then(Value::as_u64)
        .unwrap_or(24)
        .clamp(1, 60);
    let mut fields = Map::new();
    fields.insert(ROUTE_MARKER.into(), Value::Bool(true));
    fields.insert("__command".into(), json!("file.export.timelapse"));
    fields.insert("__label".into(), json!("Export Timelapse…"));
    fields.insert("format".into(), json!(fmt));
    fields.insert("fps".into(), json!(fps));
    fields.insert("path".into(), json!(""));
    let dialog = app.ui.open_dialog(DialogKind::Command, fields);
    Ok(json!({ "dialog": dialog }))
}

/// Renders the `Export Timelapse` dialog body.
pub fn body(app: &mut PhotocraftApp, ui: &mut egui::Ui, fields: &mut Map<String, Value>) {
    let t = Tokens::get(ui.ctx());
    let (frame_count, res_label, qual_label) = app.session.active().map_or((0, "1920 (1080p)", "High"), |st| {
        let res = match st.timelapse.resolution {
            TimelapseResolution::Res2560 => "2560 (2K)",
            TimelapseResolution::Res1920 => "1920 (1080p)",
        };
        let qual = match st.timelapse.quality {
            TimelapseQuality::High => "High",
            TimelapseQuality::Medium => "Medium",
            TimelapseQuality::Low => "Low",
        };
        (st.timelapse.frames.len(), res, qual)
    });

    ui.horizontal(|ui| {
        ui.label(RichText::new(tl!("Frames Captured")).color(t.text_dim));
        ui.label(RichText::new(frame_count.to_string()).font(mono(12.0)).color(t.text));
        ui.separator();
        ui.label(RichText::new(tl!(res_label)).color(t.text_dim));
        ui.label(RichText::new("·").color(t.text_faint));
        ui.label(RichText::new(tl!(qual_label)).color(t.text_dim));
    });
    ui.add_space(8.0);

    let mut format = fields
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("mp4")
        .to_string();
    let formats = [
        ("mp4".to_string(), "MP4 (H.264 / MPEG-4)"),
        ("webm".to_string(), "WebM"),
        ("avi".to_string(), "AVI (Motion)"),
        ("gif".to_string(), "Animated GIF"),
    ];
    egui::Grid::new("timelapse-export-grid")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            ui.label(RichText::new(tl!("Format")).color(t.text_dim));
            if widgets::dropdown(ui, "timelapse-export-fmt", tl!("Format"), &mut format, &formats, 180.0) {
                fields.insert("format".into(), json!(format));
            }
            ui.end_row();

            let mut fps = fields
                .get("fps")
                .and_then(Value::as_f64)
                .unwrap_or(24.0)
                .clamp(1.0, 60.0) as f32;
            ui.label(RichText::new(tl!("Frame Rate (FPS)")).color(t.text_dim));
            if widgets::value_field(ui, &mut fps, 1.0..=60.0, "fps", 80.0).changed() {
                fields.insert("fps".into(), json!(fps.round() as u32));
            }
            ui.end_row();
        });
}

/// Confirms the `Export Timelapse` dialog: runs directly when `path` was supplied (e.g. automation),
/// otherwise asks the host file-save picker for a destination path and exports there.
pub fn confirm(app: &mut PhotocraftApp, fields: &Map<String, Value>) -> Result<Value, String> {
    let format = fields
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("mp4")
        .to_string();
    let fps = fields
        .get("fps")
        .and_then(Value::as_u64)
        .unwrap_or(24)
        .clamp(1, 60) as u32;
    if let Some(path) = fields.get("path").and_then(Value::as_str).filter(|s| !s.trim().is_empty()) {
        return app.run("file.export.timelapse", json!({ "path": path, "format": format, "fps": fps }));
    }
    let ext = match format.as_str() {
        "webm" => "webm",
        "avi" => "avi",
        "gif" => "gif",
        _ => "mp4",
    };
    let stem = app
        .session
        .active()
        .map_or_else(|| "Untitled".to_string(), |st| st.doc.name.clone());
    let suggested = match crate::file_dialog::last_used_dir(&app.ui.recent_files) {
        Some(dir) => format!("{dir}/{stem}-timelapse.{ext}"),
        None => format!("{stem}-timelapse.{ext}"),
    };
    let request = FileDialogRequest::Save { suggested };
    app.ask_file(request, move |app, answer| match answer {
        FileDialogAnswer::SaveTo(dest) => {
            app.run("file.export.timelapse", json!({ "path": dest, "format": format, "fps": fps }))
        }
        FileDialogAnswer::Paths(mut paths) if !paths.is_empty() => {
            let dest = paths.remove(0);
            app.run("file.export.timelapse", json!({ "path": dest, "format": format, "fps": fps }))
        }
        _ => Err(crate::file_dialog::CANCELLED.into()),
    })
}

/// Floating `Window › Timelapse` panel rendered over the workspace when `app.ui.panels.timelapse` is open.
pub fn windows(app: &mut PhotocraftApp, ctx: &egui::Context) {
    if !app.ui.panels.timelapse {
        return;
    }
    let t = Tokens::get(ctx);
    let canvas = app.last_canvas_rect;
    let default_pos = if canvas.is_positive() {
        pos2((canvas.left() + 24.0).max(48.0), (canvas.bottom() - 240.0).max(72.0))
    } else {
        pos2(80.0, 140.0)
    };
    let card_id = egui::Id::unique("timelapse-floating-panel");
    let offset: egui::Vec2 = ctx.data(|m| m.get_temp(card_id)).unwrap_or_default();
    let mut drag = egui::Vec2::ZERO;

    egui::Area::new(card_id)
        .order(egui::Order::Middle)
        .fixed_pos(default_pos + offset)
        .show(ctx, |ui| {
            let frame = egui::Frame::NONE
                .fill(t.card)
                .stroke(Stroke::new(1.0, t.card_border))
                .corner_radius(CornerRadius::same(t.radius_lg as u8))
                .shadow(egui::Shadow { offset: [0, 10], blur: 28, spread: 0, color: t.shadow })
                .inner_margin(egui::Margin::same(12));
            frame.show(ui, |ui| {
                ui.set_width(284.0);
                let (header, _) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::hover());
                let close_rect = egui::Rect::from_min_max(pos2(header.right() - 20.0, header.top() + 1.0), pos2(header.right(), header.bottom() - 1.0));
                if icons::button(&mut ui.new_child(egui::UiBuilder::new().max_rect(close_rect)), "x", 20.0, false, tl!("Close")).clicked() {
                    app.ui.panels.timelapse = false;
                }
                let title_bar = egui::Rect::from_min_max(header.min, pos2(close_rect.left() - 4.0, header.bottom()));
                let grip = ui.interact(title_bar, card_id.with("grip"), Sense::drag());
                if grip.hovered() || grip.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                }
                drag = grip.drag_delta();
                ui.painter().text(
                    pos2(title_bar.left() + 2.0, title_bar.center().y),
                    egui::Align2::LEFT_CENTER,
                    tl!("Timelapse"),
                    semibold(12.5),
                    t.text,
                );

                ui.add_space(4.0);
                widgets::hairline(ui);
                ui.add_space(8.0);

                let Some((state, res_str, qual_str, locked, count, max_frames)) = app.session.active().map(|st| {
                    let r = match st.timelapse.resolution {
                        TimelapseResolution::Res2560 => "2560",
                        TimelapseResolution::Res1920 => "1920",
                    };
                    let q = match st.timelapse.quality {
                        TimelapseQuality::High => "high",
                        TimelapseQuality::Medium => "medium",
                        TimelapseQuality::Low => "low",
                    };
                    (
                        st.timelapse.state,
                        r.to_string(),
                        q.to_string(),
                        st.timelapse.locked_after_start,
                        st.timelapse.frames.len(),
                        st.timelapse.max_frames,
                    )
                }) else {
                    ui.label(RichText::new(tl!("No document open")).color(t.text_dim));
                    return;
                };

                let status_label = match state {
                    TimelapseState::Recording => tl!("Recording"),
                    TimelapseState::Paused => tl!("Paused"),
                    TimelapseState::Idle => tl!("Stopped"),
                };
                ui.horizontal(|ui| {
                    ui.label(RichText::new(status_label).font(semibold(11.5)).color(t.accent));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(format!("{count} / {max_frames}")).font(mono(11.5)).color(t.text_dim));
                        ui.label(RichText::new(tl!("Frames Captured")).color(t.text_faint));
                    });
                });
                ui.add_space(8.0);

                let mut res = res_str;
                let mut qual = qual_str;
                let res_opts = [
                    ("2560".to_string(), tl!("2560 (2K)")),
                    ("1920".to_string(), tl!("1920 (1080p)")),
                ];
                let qual_opts = [
                    ("high".to_string(), tl!("High")),
                    ("medium".to_string(), tl!("Medium")),
                    ("low".to_string(), tl!("Low")),
                ];
                let mut opts_changed = false;
                ui.add_enabled_ui(!locked, |ui| {
                    egui::Grid::new("timelapse-panel-opts")
                        .num_columns(2)
                        .spacing([10.0, 6.0])
                        .show(ui, |ui| {
                            ui.label(RichText::new(tl!("Timelapse Resolution")).color(t.text_dim));
                            if widgets::dropdown(ui, "tl-panel-res", tl!("Timelapse Resolution"), &mut res, &res_opts, 138.0) {
                                opts_changed = true;
                            }
                            ui.end_row();
                            ui.label(RichText::new(tl!("Timelapse Quality")).color(t.text_dim));
                            if widgets::dropdown(ui, "tl-panel-qual", tl!("Timelapse Quality"), &mut qual, &qual_opts, 138.0) {
                                opts_changed = true;
                            }
                            ui.end_row();
                        });
                });
                if opts_changed {
                    let _ = app.run("timelapse.configure", json!({ "resolution": res, "quality": qual }));
                }

                ui.add_space(10.0);
                let mut run_cmd: Option<&'static str> = None;
                ui.horizontal(|ui| {
                    match state {
                        TimelapseState::Idle => {
                            if widgets::secondary_button(ui, tl!("Start Timelapse"), 0.0).clicked() {
                                run_cmd = Some("timelapse.start");
                            }
                        }
                        TimelapseState::Recording => {
                            if widgets::secondary_button(ui, tl!("Pause Timelapse"), 0.0).clicked() {
                                run_cmd = Some("timelapse.pause");
                            }
                        }
                        TimelapseState::Paused => {
                            if widgets::secondary_button(ui, tl!("Resume Timelapse"), 0.0).clicked() {
                                run_cmd = Some("timelapse.resume");
                            }
                        }
                    }
                    if ui
                        .add_enabled_ui(count > 0 || locked || state != TimelapseState::Idle, |ui| {
                            widgets::secondary_button(ui, tl!("Clear Timelapse"), 0.0).clicked()
                        })
                        .inner
                    {
                        run_cmd = Some("timelapse.clear");
                    }
                });
                if let Some(cmd) = run_cmd
                    && let Err(e) = app.run(cmd, json!({}))
                {
                    app.ui.status = e;
                    app.ui.status_error = true;
                }

                ui.add_space(6.0);
                if ui
                    .add_enabled_ui(count > 0, |ui| {
                        widgets::secondary_button(ui, tl!("Export Timelapse…"), ui.available_width()).clicked()
                    })
                    .inner
                {
                    let c = ui.ctx().clone();
                    if let Err(e) = crate::menus::invoke(app, &c, "file.export.timelapse", json!({})) {
                        app.ui.status = e;
                        app.ui.status_error = true;
                    }
                }
            });
        });

    if drag != egui::Vec2::ZERO {
        ctx.data_mut(|m| m.insert_temp(card_id, offset + drag));
    }
}
