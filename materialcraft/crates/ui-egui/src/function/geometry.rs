use super::{FunctionDrag, FunctionEditor};
use crate::{MaterialCraftApp, graph::node_card};
use egui::{Color32, Pos2, Rect, vec2};
use materialcraft_engine::core::FunctionNode;

pub(super) fn category_color(category: &str) -> Color32 {
    match category {
        "Constant" => Color32::from_rgb(0x5c, 0x62, 0x6e),
        "Variables" => Color32::from_rgb(0x6d, 0x52, 0xb5),
        "Samplers" => Color32::from_rgb(0xa8, 0x3c, 0x34),
        "Vector" | "Cast" => Color32::from_rgb(0x3a, 0x66, 0xb0),
        "Operator" => Color32::from_rgb(0x3b, 0x7f, 0x4c),
        "Logical" | "Comparison" | "Control" => Color32::from_rgb(0xb3, 0x5a, 0x2c),
        _ => Color32::from_rgb(0x1e, 0x8a, 0xa0),
    }
}

fn input_count(node: &FunctionNode) -> usize {
    materialcraft_engine::nodes::function::input_identifiers(node).count()
}

pub(super) fn node_rect(node: &FunctionNode, previews: bool) -> Rect {
    node_card::bounds(node.pos, input_count(node), 1, previews, node.op == "passthrough")
}

pub(super) fn in_port(node: &FunctionNode, bounds: Rect, index: usize) -> Pos2 {
    node_card::port(bounds, index, input_count(node), false, node.op == "passthrough")
}

pub(super) fn out_port(node: &FunctionNode, bounds: Rect) -> Pos2 {
    node_card::port(bounds, 0, 1, true, node.op == "passthrough")
}

pub(super) fn preview_rect(app: &MaterialCraftApp, editor: &FunctionEditor, node: &FunctionNode, snap: bool, selected: bool) -> Rect {
    let mut bounds = node_rect(node, app.ui.prefs.thumbnails);
    if let Some(FunctionDrag::Node(d, delta)) = &editor.drag
        && (*d == node.id || selected)
    {
        let position = materialcraft_engine::graph_grid::translate(node.pos, (*delta).into(), snap);
        bounds = bounds.translate(vec2(position[0] - node.pos[0], position[1] - node.pos[1]));
    }
    bounds = bounds.translate(crate::graph::annotation_canvas::offset(app, |contents| contents.function_nodes.contains(&node.id)));
    bounds
}
