//! The menu bar as the title bar, where the desktop leaves the window frame to the app (GNOME on Wayland)

use craft_window::Button;
use egui::viewport::ResizeDirection;
use egui::{Context, CursorIcon, Id, LayerId, Order, PointerButton, Rect, Sense, Ui, ViewportCommand, pos2, vec2};
use egui_phosphor::regular as icon;

use crate::i18n::tr;

fn label(button: Button) -> &'static str {
    match button {
        Button::Minimize => "Minimize",
        Button::Maximize => "Maximize",
        Button::Close => "Close",
    }
}

fn glyph(button: Button) -> &'static str {
    match button {
        Button::Minimize => icon::MINUS,
        Button::Maximize => icon::SQUARE,
        Button::Close => icon::X,
    }
}

fn command(button: Button, maximized: bool) -> ViewportCommand {
    match button {
        Button::Minimize => ViewportCommand::Minimized(true),
        Button::Maximize => ViewportCommand::Maximized(!maximized),
        Button::Close => ViewportCommand::Close,
    }
}

const EDGE: f32 = 5.0;
const CORNER: f32 = 12.0;

/// The edge and corner zones of a window with content rect `w`; corners come last and win where they overlap the edges
fn zones(w: Rect) -> [(Rect, ResizeDirection, CursorIcon); 8] {
    use ResizeDirection::*;
    [
        (Rect::from_min_max(w.left_top(), pos2(w.right(), w.top() + EDGE)), North, CursorIcon::ResizeVertical),
        (Rect::from_min_max(pos2(w.left(), w.bottom() - EDGE), w.right_bottom()), South, CursorIcon::ResizeVertical),
        (Rect::from_min_max(w.left_top(), pos2(w.left() + EDGE, w.bottom())), West, CursorIcon::ResizeHorizontal),
        (Rect::from_min_max(pos2(w.right() - EDGE, w.top()), w.right_bottom()), East, CursorIcon::ResizeHorizontal),
        (Rect::from_min_size(w.left_top(), vec2(CORNER, CORNER)), NorthWest, CursorIcon::ResizeNwSe),
        (Rect::from_min_size(w.right_top() - vec2(CORNER, 0.0), vec2(CORNER, CORNER)), NorthEast, CursorIcon::ResizeNeSw),
        (Rect::from_min_size(w.left_bottom() - vec2(0.0, CORNER), vec2(CORNER, CORNER)), SouthWest, CursorIcon::ResizeNeSw),
        (Rect::from_min_size(w.right_bottom() - vec2(CORNER, CORNER), vec2(CORNER, CORNER)), SouthEast, CursorIcon::ResizeNwSe),
    ]
}

/// Invisible resize zones along the window's edges, above everything else; none while the window is maximized or full screen
pub(crate) fn resize_zones(ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let (maximized, fullscreen) = ctx.input(|input| (input.viewport().maximized.unwrap_or(false), input.viewport().fullscreen.unwrap_or(false)));
    if maximized || fullscreen {
        return;
    }
    let window = ctx.content_rect();
    let layer = LayerId::new(Order::Foreground, Id::unique("window-resize-zones"));
    let zones_ui = ui.new_child(egui::UiBuilder::new().layer_id(layer).max_rect(window));
    for (index, (rect, direction, cursor)) in zones(window).into_iter().enumerate() {
        let response = zones_ui.interact(rect, Id::unique(("window-resize-zone", index)), Sense::DRAG).on_hover_cursor(cursor);
        if response.drag_started_by(PointerButton::Primary) {
            ctx.send_viewport_cmd(ViewportCommand::BeginResize(direction));
            // The window manager takes the pointer and on some desktops its release as well
            ctx.stop_dragging();
            ctx.input_mut(|input| input.pointer = Default::default());
        }
    }
}

/// Draw `buttons` in the given left-to-right order
pub(crate) fn buttons(ui: &mut Ui, ctx: &Context, buttons: &[Button]) {
    for &button in buttons {
        let response = crate::widgets::icon_button(ui, glyph(button), tr(label(button)), false, 18.0);
        crate::widgets::name(&response, egui::Role::Button, label(button));
        if response.clicked() {
            let maximized = ctx.input(|input| input.viewport().maximized).unwrap_or(false);
            ctx.send_viewport_cmd(command(button, maximized));
        }
    }
}

#[cfg(test)]
#[path = "window_controls.test.rs"]
mod tests;
