use super::geometry::category_color;
use crate::{graph::node_card, theme::Tokens};
use egui::{Align2, Color32, FontId, Painter, Rect, Stroke};
use materialcraft_engine::{core::FunctionNode, nodes::function::FunctionDefinition};

const OUTPUT_COLOR: Color32 = Color32::from_rgb(255, 163, 26);

pub(super) fn paint(
    painter: &Painter,
    bounds: Rect,
    zoom: f32,
    node: &FunctionNode,
    definition: Option<&FunctionDefinition>,
    selected: bool,
    output: bool,
    tokens: &Tokens,
) {
    let dot = node.op == "passthrough";
    let outline = if selected {
        tokens.accent
    } else if dot {
        Color32::from_gray(0xb4)
    } else {
        Color32::from_gray(0x10)
    };

    if dot {
        node_card::paint_dot(painter, bounds, outline);
        return;
    }

    let label = materialcraft_engine::nodes::function::node_label(node);
    let category = definition.map_or("", |definition| definition.category);
    let missing = node.missing.is_some() || (node.instance.is_some() && !matches!(node.source, Some(Ok(_))));
    let color = if missing {
        Color32::from_rgb(210, 35, 125)
    } else if node.instance.is_some() {
        crate::graph::header_color("graph_instance")
    } else {
        category_color(category)
    };
    node_card::paint(painter, bounds, zoom, label, color);
    let body = node_card::body(bounds, zoom);
    if output {
        painter.rect_filled(body, 0.0, OUTPUT_COLOR);
    }
    node_card::outline(painter, bounds, zoom, Stroke::new(if selected { 2.0 } else { 1.0 }, outline));

    if missing {
        painter.text(body.center(), Align2::CENTER_CENTER, crate::i18n::tr("MISSING"), FontId::proportional(15.0 * zoom), Color32::WHITE);
    } else if node.value.is_none() {
        let text_color = if output { Color32::from_gray(0x20) } else { tokens.text_dim };
        painter.text(body.center(), Align2::CENTER_CENTER, egui_phosphor::regular::CODE, FontId::proportional(30.0 * zoom), text_color);
    }
}
