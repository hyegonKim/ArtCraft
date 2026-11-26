//! The four "+" handles beside a board's selected sticky note or shape (#1597, #1919).
//!
//! Clicking a handle adds the next sticky or shape on that side joined by a connector and begins
//! typing in it; dragging a handle drops the next one centred where it is let go (or joins an
//! existing sticky or shape under the pointer).

use egui::{Color32, Context, CornerRadius, Id, Order, Pos2, Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use figcraft_doc::{BoardItem, NodeId};
use figcraft_engine::tools::ToolId;
use serde_json::json;

use crate::FigApp;

/// Distance (screen points) from each side of the selected object's box to the centre of its "+"
/// handle. `board_bar` expands the selection box by this so the floating bar sits clear above it.
pub const REACH: f32 = 28.0;

const HANDLE_SIZE: f32 = 22.0;
const DOT_RADIUS: f32 = 8.0;

#[derive(Clone, Debug)]
struct DragState {
    source: NodeId,
    side: &'static str,
    pos: Pos2,
}

fn drag_id() -> Id {
    Id::unique("board_connect_drag")
}

fn eligible_target(app: &FigApp) -> Option<NodeId> {
    if app.session.active_read_only().is_some()
        || app.session.tool.tool != ToolId::Move
        || app.session.tool.busy()
    {
        return None;
    }
    let ds = app.session.ds().ok()?;
    if !figcraft_engine::board::is_board(&ds.doc) || ds.text_edit.is_some() {
        return None;
    }
    let [id] = ds.selection.as_slice() else { return None };
    let n = ds.doc.node(*id)?;
    if n.locked || !n.visible {
        return None;
    }
    matches!(n.board, Some(BoardItem::Sticky { .. } | BoardItem::Shape { .. })).then_some(*id)
}

/// Whether the four "+" handles should show around the current selection.
pub fn shown(app: &FigApp) -> bool {
    eligible_target(app).is_some()
}

/// Whether one of the "+" handles is currently being dragged.
pub fn dragging(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<DragState>(drag_id()).is_some())
}

fn side_to_connector_end(side: &str) -> &'static str {
    match side {
        "above" => "top",
        "below" => "bottom",
        "left" => "left",
        _ => "right",
    }
}

/// Draw the four "+" handles and handle clicks / drags on them.
pub fn show(app: &mut FigApp, ctx: &Context) {
    let active_drag: Option<DragState> = ctx.data(|d| d.get_temp(drag_id()));
    let current = eligible_target(app);

    if let Some(d) = &active_drag
        && current != Some(d.source)
    {
        ctx.data_mut(|m| m.remove::<DragState>(drag_id()));
        ctx.stop_dragging();
        return;
    }

    let Some(target) = current else { return };
    let Some(bounds) = app.session.doc().ok().and_then(|doc| doc.world_bounds(target)) else {
        return;
    };
    let sel_rect = Rect::from_two_pos(app.to_screen(bounds.x0, bounds.y0), app.to_screen(bounds.x1, bounds.y1));
    if !sel_rect.expand(REACH + HANDLE_SIZE).intersects(app.canvas.rect) && active_drag.is_none() {
        return;
    }

    let sides: [(&'static str, Pos2); 4] = [
        ("above", pos2(sel_rect.center().x, sel_rect.min.y - REACH)),
        ("right", pos2(sel_rect.max.x + REACH, sel_rect.center().y)),
        ("below", pos2(sel_rect.center().x, sel_rect.max.y + REACH)),
        ("left", pos2(sel_rect.min.x - REACH, sel_rect.center().y)),
    ];

    let accent = crate::theme::canvas_accent(ctx);
    let mut clicked_side: Option<&'static str> = None;
    let mut started_drag: Option<DragState> = None;
    let mut updated_pos: Option<Pos2> = None;
    let mut released_drag = false;

    for (side, center) in sides {
        let area_pos = center - vec2(HANDLE_SIZE * 0.5, HANDLE_SIZE * 0.5);
        egui::Area::new(Id::unique(("board_connect", side)))
            .fixed_pos(area_pos)
            .order(Order::Middle)
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(vec2(HANDLE_SIZE, HANDLE_SIZE), Sense::click_and_drag());
                let resp = resp.on_hover_text(crate::i18n::tr("Add a connected shape"));
                let is_this_drag = active_drag.as_ref().is_some_and(|d| d.side == side);
                let hot = resp.hovered() || is_this_drag;
                let p = ui.painter();
                let c = rect.center();
                if hot {
                    p.circle_filled(c, DOT_RADIUS + 1.5, accent);
                    p.circle_stroke(c, DOT_RADIUS + 1.5, Stroke::new(1.5, Color32::WHITE));
                    p.line_segment([c - vec2(4.0, 0.0), c + vec2(4.0, 0.0)], Stroke::new(1.5, Color32::WHITE));
                    p.line_segment([c - vec2(0.0, 4.0), c + vec2(0.0, 4.0)], Stroke::new(1.5, Color32::WHITE));
                } else {
                    p.circle_filled(c, DOT_RADIUS, Color32::WHITE);
                    p.circle_stroke(c, DOT_RADIUS, Stroke::new(1.5, accent));
                    p.line_segment([c - vec2(3.5, 0.0), c + vec2(3.5, 0.0)], Stroke::new(1.5, accent));
                    p.line_segment([c - vec2(0.0, 3.5), c + vec2(0.0, 3.5)], Stroke::new(1.5, accent));
                }

                if resp.clicked() && active_drag.is_none() {
                    clicked_side = Some(side);
                }
                if resp.drag_started() && active_drag.is_none() {
                    let at = ctx.input(|i| i.pointer.interact_pos()).unwrap_or(c);
                    started_drag = Some(DragState { source: target, side, pos: at });
                }
                if is_this_drag {
                    if let Some(at) = ctx.input(|i| i.pointer.interact_pos().or(i.pointer.latest_pos())) {
                        updated_pos = Some(at);
                    }
                    if resp.drag_stopped() || !ctx.input(|i| i.pointer.primary_down()) {
                        released_drag = true;
                    }
                }
            });
    }

    if let Some(d) = started_drag {
        ctx.data_mut(|m| m.insert_temp(drag_id(), d));
        ctx.request_repaint();
        return;
    }

    if let Some(mut d) = active_drag {
        if let Some(p) = updated_pos {
            d.pos = p;
        }
        if !released_drag {
            // Draw preview ghost / connector line on the foreground painter while dragging.
            let fg = ctx.layer_painter(egui::LayerId::new(Order::Foreground, Id::unique("board_connect_preview")));
            let from = match d.side {
                "above" => pos2(sel_rect.center().x, sel_rect.min.y),
                "below" => pos2(sel_rect.center().x, sel_rect.max.y),
                "left" => pos2(sel_rect.min.x, sel_rect.center().y),
                _ => pos2(sel_rect.max.x, sel_rect.center().y),
            };
            fg.line_segment([from, d.pos], Stroke::new(2.0, accent));
            let preview_size = sel_rect.size();
            let ghost = Rect::from_center_size(d.pos, preview_size);
            fg.rect_stroke(ghost, CornerRadius::same(4), Stroke::new(1.5, accent.gamma_multiply(0.6)), StrokeKind::Inside);
            ctx.data_mut(|m| m.insert_temp(drag_id(), d));
            ctx.request_repaint();
            return;
        }

        ctx.data_mut(|m| m.remove::<DragState>(drag_id()));
        let drop_pos = d.pos;
        let pill = crate::toolbar::pill_rect(app.canvas.rect, crate::toolbar::bar_width(app));
        if !app.canvas.rect.contains(drop_pos) || pill.contains(drop_pos) {
            return;
        }
        let page_pt = app.to_page(drop_pos);
        let hit = app.session.ds().ok().and_then(|ds| {
            figcraft_engine::board::sticky_or_shape_at(&ds.doc, ds.page, page_pt, Some(d.source))
        });
        if let Some(existing) = hit {
            app.run(
                "board.connector",
                json!({
                    "from": d.source,
                    "to": existing,
                    "fromSide": side_to_connector_end(d.side),
                }),
            );
        } else {
            app.run(
                "board.addConnected",
                json!({
                    "id": d.source,
                    "side": d.side,
                    "at": [page_pt.x, page_pt.y],
                    "edit": true,
                }),
            );
        }
        return;
    }

    if let Some(side) = clicked_side {
        app.run("board.addConnected", json!({"id": target, "side": side, "edit": true}));
    }
}
