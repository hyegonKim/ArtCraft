use egui::{Align2, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui, vec2};

use crate::{theme::Tokens, widgets};

pub(super) fn show(ui: &mut Ui, name: &str, icon: &str, selected: bool, warning: Option<&str>) -> Response {
    let theme = Tokens::get(ui.ctx());
    let (bounds, response) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::click_and_drag());
    widgets::name(&response, egui::Role::TreeItem, name);
    if selected {
        ui.painter().rect_filled(bounds, 4.0, theme.row_selected);
        ui.painter().rect_stroke(bounds, 4.0, Stroke::new(1.0, theme.accent), StrokeKind::Inside);
    } else if response.hovered() {
        ui.painter().rect_filled(bounds, 4.0, theme.row_hover);
    }

    let mut text_bounds = bounds.intersect(ui.clip_rect());
    if let Some(warning) = warning {
        let center = bounds.right_center() - vec2(12.0, 0.0);
        let warning_response = ui.interact(Rect::from_center_size(center, vec2(20.0, 20.0)), response.id.with("warning"), Sense::hover());
        widgets::name(&warning_response, egui::Role::Label, &format!("Warnings for {name}"));
        ui.ctx().accesskit_node_builder(warning_response.id, |node| node.set_description(warning));
        ui.painter().text(center, Align2::CENTER_CENTER, egui_phosphor::regular::WARNING, FontId::proportional(14.0), theme.warning);
        warning_response.on_hover_text(warning);
        text_bounds.max.x = text_bounds.max.x.min(bounds.right() - 24.0);
    }

    ui.painter().with_clip_rect(text_bounds).text(
        bounds.left_center() + vec2(4.0, 0.0),
        Align2::LEFT_CENTER,
        format!("{icon}  {name}"),
        FontId::proportional(12.0),
        if selected { theme.text_strong } else { theme.text },
    );
    response
}
