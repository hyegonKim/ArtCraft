use super::function_port::FunctionPort;
use super::search::{FunctionSearch, SearchWire};
use super::{FunctionDrag, FunctionEditor};
use crate::{
    MaterialCraftApp,
    graph::{GraphCamera, canvas_geometry::CanvasGeometry},
    theme::Tokens,
};
use egui::{Pos2, Stroke, Ui};
use materialcraft_engine::FunctionEdit;

pub(super) fn handle(
    app: &mut MaterialCraftApp,
    editor: &mut FunctionEditor,
    ui: &mut Ui,
    response: &egui::Response,
    ports: &[FunctionPort],
    geometry: &CanvasGeometry,
    on_annotation: bool,
) {
    let canvas_bounds = response.rect;
    let pointer = ui.input(|input| input.pointer.hover_pos());
    let painter = ui.painter_at(canvas_bounds);
    let tokens = Tokens::get(ui.ctx());
    let snap = app.ui.prefs.snap_to_grid;
    if crate::graph::gesture::cancelled(ui) {
        editor.drag = None;
        return;
    }

    let on_wire = editor.drag.is_none()
        && !on_annotation
        && !ui.input(|input| input.modifiers.alt)
        && crate::graph::wire_selection::handle(&mut editor.selected_wires, &mut editor.selection, ui, response, &geometry.wires);
    if on_wire && app.session.annotation_selection.is_some() && (ui.input(|input| input.pointer.primary_pressed()) || response.secondary_clicked()) {
        app.run("graph.annotation.select", serde_json::json!({}));
    }

    // Gestures: wires from output dots and unconnected inputs, panning
    let near = |pointer: Pos2| crate::graph::port_hit::nearest_by(ports, pointer, |port| (port.position, port.radius)).map(|port| (port.node, port.slot));
    if editor.drag.is_none() && GraphCamera::pan_started(ui, canvas_bounds) {
        editor.drag = Some(FunctionDrag::Pan(crate::graph::gesture::pan_button(ui)));
    } else if ui.input(|input| input.pointer.primary_pressed())
        && editor.drag.is_none()
        && !app.graph.is_dragging()
        && let Some(pointer_position) = pointer
        && ui.rect_contains_pointer(canvas_bounds)
        && let Some((id, slot)) = near(pointer_position)
    {
        match slot {
            None => editor.drag = Some(FunctionDrag::Wire(id)),
            Some(input) => {
                let connected = editor.current(app).and_then(|graph| graph.node(id)).is_some_and(|node| node.inputs.get(input).is_some_and(Option::is_some));
                if !connected {
                    editor.drag = Some(FunctionDrag::InputWire(id, input));
                }
            }
        }
    }

    if editor.drag.is_none()
        && !app.graph.is_dragging()
        && response.hovered()
        && ui.input(|input| input.pointer.primary_pressed())
        && let Some(start) = pointer
        && !on_annotation
        && !on_wire
    {
        if app.session.annotation_selection.is_some() {
            app.run("graph.annotation.select", serde_json::json!({}));
        }
        editor.drag = Some(FunctionDrag::Box { start });
        crate::graph::node_selection::select(&mut editor.selected_wires, None, ui.input(|input| input.modifiers));
    }

    let delta = ui.input(|input| input.pointer.delta());
    match &mut editor.drag {
        Some(FunctionDrag::Node(_, offset)) => *offset += delta / editor.zoom,
        Some(FunctionDrag::Pan(_)) => editor.pan -= delta / editor.zoom,
        Some(FunctionDrag::Wire(id)) => {
            if let (Some(pointer_position), Some(source_id)) = (pointer, ports.iter().find(|port| port.node == *id && port.slot.is_none())) {
                crate::graph::wire_geometry::bezier(&painter, source_id.position, pointer_position, Stroke::new(2.0, tokens.accent));
            }
        }
        Some(FunctionDrag::InputWire(id, input)) => {
            if let (Some(pointer_position), Some(destination)) = (pointer, ports.iter().find(|port| port.node == *id && port.slot == Some(*input))) {
                crate::graph::wire_geometry::bezier(&painter, pointer_position, destination.position, Stroke::new(2.0, tokens.accent));
            }
        }
        Some(FunctionDrag::Box { start }) => {
            if let Some(pointer) = pointer {
                crate::graph::node_selection::paint_box(&painter, *start, pointer, tokens.accent);
            }
        }
        None => {}
    }

    let camera = GraphCamera { origin: canvas_bounds.min, pan: editor.pan, zoom: editor.zoom };
    let owner = match editor.drag {
        Some(FunctionDrag::Pan(button)) => button,
        _ => egui::PointerButton::Primary,
    };
    if crate::graph::gesture::finished(ui, owner) {
        match editor.drag.take() {
            Some(FunctionDrag::Node(id, offset)) => {
                if offset.length() > 0.5 {
                    let mut ids = editor.selection.clone();
                    if !ids.contains(&id) {
                        ids.push(id);
                    }
                    editor.edit(app, FunctionEdit::Move { ids, by: offset.into(), snap }, false);
                }
            }
            Some(FunctionDrag::Box { start }) => {
                if let Some(pointer) = pointer {
                    let nodes = geometry.rects.iter().map(|(id, bounds)| (id.0 as u32, *bounds));
                    crate::graph::node_selection::select_box(&mut editor.selection, nodes, start, pointer, ui.input(|input| input.modifiers));
                }
            }
            Some(FunctionDrag::Wire(from)) => match pointer.map(|pointer| (pointer, near(pointer))) {
                Some((_, Some((to, Some(input))))) if from != to => editor.edit(app, FunctionEdit::Connect { from, to, input }, false),
                // Dropped on empty canvas: search for a node that takes the wire
                Some((pointer, None)) if canvas_bounds.contains(pointer) => {
                    editor.search = Some(FunctionSearch::at(camera.to_graph(pointer), Some(SearchWire::From(from))));
                }
                _ => {}
            },
            Some(FunctionDrag::InputWire(to, input)) => match pointer.map(|pointer| (pointer, near(pointer))) {
                Some((_, Some((from, None)))) if from != to => editor.edit(app, FunctionEdit::Connect { from, to, input }, false),
                // Dropped on empty canvas: search for a node whose output feeds the input
                Some((pointer, None)) if canvas_bounds.contains(pointer) => {
                    editor.search = Some(FunctionSearch::at(camera.to_graph(pointer), Some(SearchWire::To(to, input))));
                }
                _ => {}
            },
            _ => {}
        }
    }

    if response.clicked() && !on_wire && !ui.input(|input| input.modifiers.alt) {
        crate::graph::node_selection::select(&mut editor.selection, None, ui.input(|input| input.modifiers));
        crate::graph::node_selection::select(&mut editor.selected_wires, None, ui.input(|input| input.modifiers));
    }

    if let Some(payload) = crate::widgets::dnd_release::<super::function_library_drag::FunctionLibraryDrag>(response)
        && let Some(pointer) = pointer
    {
        let position = camera.to_graph(pointer);
        super::library::insert(app, editor, &payload, [position.x - crate::graph::NODE_W * 0.5, position.y - 20.0]);
    }

    if let Some(payload) = crate::widgets::dnd_release::<crate::graph_drag_payload::GraphDragPayload>(response)
        && let Some(pointer) = pointer
        && let Some(target) = editor.command_target()
    {
        let at = camera.to_graph(pointer);
        if let Some(result) = app.run(
            "graph.instance.add",
            serde_json::json!({"document": payload.document, "graph": payload.name,
            "function": target, "at": [at.x - crate::graph::NODE_W * 0.5, at.y - 20.0], "snapToGrid": snap}),
        ) && let Some(id) = result.get("id").and_then(serde_json::Value::as_u64).and_then(|id| u32::try_from(id).ok())
        {
            editor.selection = vec![id];
            editor.selected_wires.clear();
            editor.inspected_node = Some(id);
        }
    }

    if let Some(payload) = crate::widgets::dnd_release::<crate::graph::LibraryDrag>(response)
        && let Some(pointer) = pointer
    {
        let at = camera.to_graph(pointer);
        if let Some(result) = app.run("library.add", serde_json::json!({"entry": payload.0, "at": [at.x, at.y], "snapToGrid": snap})) {
            editor.selection = if app.session.annotation_selection.is_some() {
                Vec::new()
            } else {
                result["result"]["id"].as_u64().and_then(|id| u32::try_from(id).ok()).into_iter().collect()
            };
            editor.selected_wires.clear();
        }
    }

    // Adding operations
    if response.secondary_clicked()
        && let Some(pointer_position) = pointer
    {
        editor.menu_at = camera.to_graph(pointer_position);
    }
    // Space or Tab over the canvas, or a double-click on empty canvas, searches for a node to add
    // Tab moves focus between widgets, so it waits only for text fields and focus held before this frame
    let space = !ui.ctx().egui_wants_keyboard_input() && ui.input(|input| input.key_pressed(egui::Key::Space));
    let tab = !app.graph.keyboard_was_focused && !ui.ctx().text_edit_focused() && ui.input(|input| input.key_pressed(egui::Key::Tab));
    let keys = space || tab;
    let on_node = pointer.is_some_and(|pointer| geometry.rects.iter().any(|(_, bounds)| bounds.contains(pointer)));
    let double_click = response.double_clicked() && !on_wire && !on_annotation && !on_node && pointer.and_then(near).is_none();
    if editor.search.is_none()
        && ((response.hovered() && keys) || double_click)
        && !crate::shell::modality::Modality::capture(app, ui.ctx()).is_blocked()
        && let Some(pointer_position) = pointer
    {
        editor.menu_at = camera.to_graph(pointer_position);
        editor.search = Some(FunctionSearch::at(editor.menu_at, None));
    }
    response.context_menu(|ui| {
        super::context_menu::show(app, editor, ui, None);
    });
}
