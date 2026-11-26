use super::{WireRef, node_selection, wire_geometry};
use egui::{Color32, Modifiers, Pos2, Response, Stroke, Ui};

pub(crate) fn select<N>(wires: &mut Vec<WireRef>, nodes: &mut Vec<N>, wire: WireRef, modifiers: Modifiers, secondary: bool) {
    let keep_nodes = modifiers.ctrl || modifiers.command || modifiers.shift || (secondary && wires.contains(&wire));
    node_selection::select(wires, Some(wire), modifiers);
    if !keep_nodes {
        nodes.clear();
    }
}

pub(crate) fn handle<N>(wires: &mut Vec<WireRef>, nodes: &mut Vec<N>, ui: &Ui, response: &Response, curves: &[(WireRef, [Pos2; 4])]) -> bool {
    let wire = ui.input(|input| input.pointer.hover_pos()).and_then(|pointer| wire_geometry::wire_at(curves, pointer));
    let Some(wire) = wire.filter(|_| response.hovered()) else { return false };

    if ui.input(|input| input.pointer.primary_pressed()) || response.secondary_clicked() {
        let modifiers = if response.secondary_clicked() { Modifiers::NONE } else { ui.input(|input| input.modifiers) };
        select(wires, nodes, wire.clone(), modifiers, response.secondary_clicked());
    }
    true
}

pub(crate) fn stroke(selected: bool, width: f32, color: Color32, accent: Color32) -> Stroke {
    if selected { Stroke::new(width + 1.0, accent) } else { Stroke::new(width, color) }
}
