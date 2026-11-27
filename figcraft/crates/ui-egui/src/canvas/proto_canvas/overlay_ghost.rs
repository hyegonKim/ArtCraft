//! Ghost preview and on-canvas dragging for `OverlayPosition::Manual` interactions while editing
//! an interaction in the Prototype panel.

use egui::{Color32, Context, CornerRadius, Id, Painter, PointerButton, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};
use figcraft_doc::proto::{Action, OverlayPosition};
use figcraft_doc::{Document, NodeId};
use serde_json::json;

use crate::FigApp;

#[derive(Clone, Copy, Debug)]
struct GhostDrag {
    source: NodeId,
    index: usize,
    start_pointer: Pos2,
    start_offset: [f64; 2],
}

fn drag_id() -> Id {
    Id::unique("proto_overlay_ghost_drag")
}

/// Active manual-overlay `(source, index, dest_frame, ghost_screen_rect, current_offset)` for the
/// interaction currently open in the Prototype editor, when on `page`.
fn active_ghost(app: &FigApp, ctx: &Context, doc: &Document, page: NodeId) -> Option<(NodeId, usize, Rect, [f64; 2])> {
    let (source, index) = crate::proto_panel::editor(ctx)?;
    if doc.page_of(source) != Some(page) {
        return None;
    }
    let interaction = doc.node(source)?.interactions.get(index)?;
    let (dest, offset) = interaction.actions.iter().find_map(|a| match a {
        Action::OpenOverlay { dest, position: OverlayPosition::Manual, offset, .. } => Some((*dest, *offset)),
        _ => None,
    })?;
    let dest_node = doc.node(dest)?;
    let src_bounds = doc.world_bounds(source)?;
    let off = offset.unwrap_or([0.0, src_bounds.height()]);
    let gx = src_bounds.x0 + off[0];
    let gy = src_bounds.y0 + off[1];
    let screen = Rect::from_two_pos(app.to_screen(gx, gy), app.to_screen(gx + dest_node.width, gy + dest_node.height));
    Some((source, index, screen, off))
}

pub(super) fn draw(app: &FigApp, painter: &Painter, doc: &Document, page: NodeId) {
    let ctx = painter.ctx();
    let Some((_, _, rect, _)) = active_ghost(app, ctx, doc, page) else { return };
    let accent = crate::theme::canvas_accent(ctx);
    painter.rect_filled(rect, CornerRadius::ZERO, accent.gamma_multiply(0.08));
    painter.rect_stroke(rect, CornerRadius::ZERO, Stroke::new(1.5, accent), StrokeKind::Inside);
    // Crosshair mark at the ghost's top-left corner.
    let tl = rect.min;
    painter.line_segment([tl - vec2(5.0, 0.0), tl + vec2(5.0, 0.0)], Stroke::new(1.5, accent));
    painter.line_segment([tl - vec2(0.0, 5.0), tl + vec2(0.0, 5.0)], Stroke::new(1.5, accent));
    let _ = Color32::TRANSPARENT;
    let _ = pos2;
}

pub(super) fn end(app: &mut FigApp, ctx: &Context) {
    if ctx.data(|d| d.get_temp::<GhostDrag>(drag_id()).is_some()) {
        ctx.data_mut(|d| d.remove::<GhostDrag>(drag_id()));
        let _ = app.session.cancel_interaction();
    }
}

pub(super) fn input(app: &mut FigApp, ctx: &Context, pos: Option<Pos2>, hovered: bool) -> bool {
    let existing: Option<GhostDrag> = ctx.data(|d| d.get_temp(drag_id()));
    let (pressed, down, released) = ctx
        .input(|i| (i.pointer.button_pressed(PointerButton::Primary), i.pointer.primary_down(), i.pointer.button_released(PointerButton::Primary)));

    if let Some(drag) = existing {
        if let Some(p) = pos {
            let zoom = app.session.ds().map(|d| d.view.zoom).unwrap_or(1.0).max(1e-6);
            let dx = f64::from(p.x - drag.start_pointer.x) / zoom;
            let dy = f64::from(p.y - drag.start_pointer.y) / zoom;
            let next = [(drag.start_offset[0] + dx).round(), (drag.start_offset[1] + dy).round()];
            let _ = app.session.preview(|doc, _| {
                if let Some(n) = doc.node_mut(drag.source)
                    && let Some(it) = n.interactions.get_mut(drag.index)
                {
                    for a in &mut it.actions {
                        if let Action::OpenOverlay { position: OverlayPosition::Manual, offset, .. } = a {
                            *offset = Some(next);
                            break;
                        }
                    }
                }
                Ok(())
            });
        }
        ctx.request_repaint();
        if released || !down {
            ctx.data_mut(|d| d.remove::<GhostDrag>(drag_id()));
            let _ = app.session.commit_interaction();
        }
        return true;
    }

    if !pressed || !hovered {
        return false;
    }
    let Some(p) = pos else { return false };
    let Ok(ds) = app.session.ds() else { return false };
    let (doc, page) = (ds.doc.clone(), ds.page);
    let Some((source, index, rect, off)) = active_ghost(app, ctx, &doc, page) else {
        return false;
    };
    if !rect.contains(p) || app.session.active_read_only().is_some() {
        return false;
    }
    if app.session.begin_interaction("Move overlay").is_err() {
        return false;
    }
    let _ = json!({});
    ctx.data_mut(|d| {
        d.insert_temp(drag_id(), GhostDrag { source, index, start_pointer: p, start_offset: off });
    });
    ctx.request_repaint();
    true
}
