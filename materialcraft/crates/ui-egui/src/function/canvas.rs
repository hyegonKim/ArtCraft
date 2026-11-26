use super::{FunctionEditor, FunctionTarget};
use crate::{MaterialCraftApp, graph::GraphCamera};
use egui::Ui;
use materialcraft_engine::FunctionEdit;
use serde_json::json;

pub(super) fn canvas(app: &mut MaterialCraftApp, editor: &mut FunctionEditor, ui: &mut Ui) {
    let Some(frame) = super::render::paint(app, editor, ui) else { return };

    let standalone = matches!(editor.target, FunctionTarget::Graph);
    app.ui.frame_all = false;
    if standalone {
        app.ui.graph_camera = frame.camera.state();
    }
    app.graph.frames = frame.frames.iter().map(|frame| (frame.id, frame.rect)).collect();
    app.graph.comments = frame.comments.clone();
    app.graph.pins = frame.pins.clone();
    if frame.clear_annotation {
        app.run("graph.annotation.select", json!({}));
    }
    for (id, value) in frame.values {
        editor.edit(app, FunctionEdit::Value { id, value }, true);
    }
    for (id, response) in &frame.nodes {
        if response.secondary_clicked()
            && let Some(pointer) = response.interact_pointer_pos()
        {
            editor.menu_at = frame.camera.to_graph(pointer);
        }
        response.context_menu(|ui| {
            super::context_menu::show(app, editor, ui, Some(*id));
        });
    }
    let on_annotation = ui.input(|input| input.pointer.hover_pos()).is_some_and(|pointer| {
        frame.frames.iter().any(|frame| frame.contains(pointer))
            || frame.comments.iter().any(|(_, rect)| rect.contains(pointer))
            || frame.pins.iter().any(|(_, rect)| rect.contains(pointer))
    });
    super::input::handle(app, editor, ui, &frame.response, &frame.ports, &frame.geometry, on_annotation);
    let camera = GraphCamera { origin: frame.response.rect.min, pan: editor.pan, zoom: editor.zoom };
    super::search::show(app, editor, ui, &camera);
    frame.geometry.publish(&mut app.graph, frame.response.rect);
    if editor.drag.is_none() {
        let camera = GraphCamera { origin: frame.response.rect.min, pan: editor.pan, zoom: editor.zoom };
        crate::graph::annotation_input::handle(app, ui, &camera, &frame.frames, &frame.comments, &frame.pins);
        if app.session.annotation_selection.is_some() {
            editor.selection.clear();
            editor.selected_wires.clear();
        }
    }
    if standalone {
        app.ui.graph_camera = editor.camera();
        app.session.selection = editor.selection.iter().map(|id| materialcraft_engine::core::NodeId(u64::from(*id))).collect();
    }
    editor.inspected_node = crate::graph::node_selection::inspected(&editor.selection, editor.inspected_node);
}
