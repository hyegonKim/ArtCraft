use super::geometry::{in_port, node_rect, out_port, preview_rect};
use super::{FunctionDrag, FunctionEditor, port_label::port_label};
use super::{function_canvas::FunctionCanvas, function_port::FunctionPort};
use crate::graph::canvas_geometry::CanvasGeometry;
use crate::{MaterialCraftApp, graph::node_card, theme::Tokens};
use egui::{Align2, Color32, Rect, Sense, Stroke, Ui, Vec2, pos2, vec2};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn paint(app: &mut MaterialCraftApp, editor: &mut FunctionEditor, ui: &mut Ui) -> Option<FunctionCanvas> {
    let (canvas_bounds, response) = ui.allocate_exact_size(ui.available_size().max(vec2(200.0, 200.0)), Sense::click_and_drag());
    app.graph.view = canvas_bounds;
    let graph = editor.current(app)?;
    let snap = app.ui.prefs.snap_to_grid;
    let selected: BTreeSet<_> = editor.selection.iter().copied().collect();
    let layouts: BTreeMap<_, _> =
        graph.nodes.iter().map(|node| (node.id, (node, preview_rect(app, editor, node, snap, selected.contains(&node.id))))).collect();
    editor.selection.retain(|id| layouts.contains_key(id));
    editor.selected_wires.retain(|(id, port)| {
        u32::try_from(id.0)
            .ok()
            .and_then(|id| layouts.get(&id))
            .and_then(|(node, _)| port.parse::<usize>().ok().and_then(|slot| node.inputs.get(slot)))
            .is_some_and(Option::is_some)
    });
    editor.inspected_node = crate::graph::node_selection::inspected(&editor.selection, editor.inspected_node);
    let document = app.graph.annotation_target.items(&app.session).ok();
    let tokens = Tokens::get(ui.ctx());
    crate::widgets::name(&response, egui::Role::Canvas, "Function Graph");
    let painter = ui.painter_at(canvas_bounds);
    painter.rect_filled(canvas_bounds, 4.0, Color32::from_rgb(0x26, 0x28, 0x2c));
    let mut geometry = CanvasGeometry::default();
    let mut values = Vec::new();
    let mut nodes = Vec::new();
    let mut clear_annotation = false;

    let frame_key = crate::graph::GraphCamera::frame_key(ui, canvas_bounds) && !crate::shell::modality::Modality::capture(app, ui.ctx()).is_blocked();
    if !editor.framed || app.ui.frame_all || frame_key {
        editor.framed = true;
        let selected_only = frame_key && !editor.selection.is_empty();
        let mut destination = graph
            .nodes
            .iter()
            .filter(|node| !selected_only || selected.contains(&node.id))
            .map(|node| node_rect(node, app.ui.prefs.thumbnails))
            .fold(Rect::NOTHING, |combined, bounds| combined.union(bounds));
        if !selected_only && let Some(document) = &document {
            for frame in &document.frames {
                destination = destination.union(Rect::from_min_size(pos2(frame.rect[0], frame.rect[1] - 32.0), vec2(frame.rect[2], frame.rect[3] + 32.0)));
            }
        }
        let camera = crate::graph::GraphCamera::fit(canvas_bounds, destination);
        editor.pan = camera.pan;
        editor.zoom = camera.zoom;
    }

    let mut camera = crate::graph::GraphCamera { origin: canvas_bounds.min, pan: editor.pan, zoom: editor.zoom };
    camera.zoom_from_input(ui, canvas_bounds);
    editor.pan = camera.pan;
    editor.zoom = camera.zoom;

    let frame_hits = document.as_ref().map(|graph| crate::graph::annotation_canvas::frames(app, ui, &graph.frames, &camera)).unwrap_or_default();
    crate::graph::grid::paint(&painter, &camera, canvas_bounds);

    // Wires use the same prepared bounds as the cards
    for node in &graph.nodes {
        let Some((_, bounds)) = layouts.get(&node.id) else { continue };

        for (input_index, source_id) in node.inputs.iter().enumerate() {
            if let Some((source, source_bounds)) = source_id.and_then(|source| layouts.get(&source)) {
                let source_position = camera.to_screen(out_port(source, *source_bounds));
                let destination = camera.to_screen(in_port(node, *bounds, input_index));
                let points = crate::graph::wire_geometry::bezier_points(source_position, destination);
                geometry.wires.push(((materialcraft_engine::core::NodeId(u64::from(node.id)), input_index.to_string()), points));
                let selected = editor.selected_wires.contains(&(materialcraft_engine::core::NodeId(u64::from(node.id)), input_index.to_string()));
                painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
                    points,
                    false,
                    Color32::TRANSPARENT,
                    crate::graph::wire_selection::stroke(selected, 2.0, Color32::from_gray(0xb4), tokens.accent),
                ));
            }
        }
    }

    let mut ports = Vec::new();
    for node in &graph.nodes {
        let id = node.id;
        let Some((_, bounds)) = layouts.get(&id) else { continue };

        let bounds = *bounds;
        let screen_rect = Rect::from_min_max(camera.to_screen(bounds.min), camera.to_screen(bounds.max));
        geometry.rects.push((materialcraft_engine::core::NodeId(u64::from(id)), screen_rect));

        let definition = materialcraft_engine::nodes::function::definition(&node.op);
        let label = materialcraft_engine::nodes::function::node_label(node);
        let dot = node.op == "passthrough";
        let show_labels = crate::graph::port_label::visible(ui, screen_rect);
        super::node_canvas::paint(&painter, screen_rect, editor.zoom, node, definition, selected.contains(&id), graph.output == Some(id), &tokens);

        let port_radius = (4.5 * editor.zoom).clamp(2.5, 7.0);
        let port_color = crate::graph::port_color(materialcraft_engine::nodes::PortKind::Value);
        {
            for (input_index, identifier) in materialcraft_engine::nodes::function::input_identifiers(node).enumerate() {
                let position = camera.to_screen(in_port(node, bounds, input_index));
                painter.circle(position, port_radius, port_color, Stroke::new(1.0, Color32::from_gray(0x18)));
                if show_labels {
                    crate::graph::port_label::paint(
                        &painter,
                        position - vec2(10.0, 0.0),
                        Align2::RIGHT_CENTER,
                        port_label(identifier),
                        editor.zoom,
                        tokens.text_strong,
                    );
                }
                ports.push(FunctionPort {
                    node: id,
                    slot: Some(input_index),
                    position,
                    radius: if dot { (3.5 * editor.zoom).max(2.0) } else { 9.0 },
                    label: port_label(identifier).into(),
                });
            }
        }

        let output_position = camera.to_screen(out_port(node, bounds));
        painter.circle(output_position, port_radius, port_color, Stroke::new(1.0, Color32::from_gray(0x18)));
        if show_labels {
            crate::graph::port_label::paint(
                &painter,
                output_position + vec2(10.0, 0.0),
                Align2::LEFT_CENTER,
                port_label("output"),
                editor.zoom,
                tokens.text_strong,
            );
        }
        ports.push(FunctionPort {
            node: id,
            slot: None,
            position: output_position,
            radius: if dot { (3.5 * editor.zoom).max(2.0) } else { 9.0 },
            label: port_label("output").into(),
        });

        let body = ui.interact(screen_rect, ui.scope_id().with(("fn-node", id)), Sense::click_and_drag());
        crate::widgets::name(&body, egui::Role::Button, label);

        if !dot && node.missing.is_none() && node.value.is_some() {
            let content = node_card::body(screen_rect, editor.zoom).shrink(6.0 * editor.zoom);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(content));
            child.set_clip_rect(content.intersect(canvas_bounds));
            if let Some(mut value) = node.value.clone()
                && crate::value_editor::show(&mut child, &mut value)
            {
                values.push((id, value));
            }
        }

        if (body.contains_pointer() && ui.input(|input| input.pointer.primary_pressed() && !input.modifiers.alt)) || body.secondary_clicked() {
            let modifiers = if body.secondary_clicked() { egui::Modifiers::NONE } else { ui.input(|input| input.modifiers) };
            let keep_wires = modifiers.ctrl || modifiers.command || modifiers.shift || (body.secondary_clicked() && editor.selection.contains(&id));

            crate::graph::node_selection::select(&mut editor.selection, Some(id), modifiers);
            if !keep_wires {
                editor.selected_wires.clear();
            }
            if app.session.annotation_selection.is_some() {
                clear_annotation = true;
            }
        }

        if body.drag_started() && !ui.input(|input| input.modifiers.alt) && editor.selection.contains(&id) && editor.drag.is_none() && !app.graph.is_dragging()
        {
            editor.drag = Some(FunctionDrag::Node(id, Vec2::ZERO));
        }

        nodes.push((id, body));
    }

    geometry.ports = ports
        .iter()
        .map(|port| crate::graph::PortHit {
            node: materialcraft_engine::core::NodeId(u64::from(port.node)),
            port: port.slot.map_or_else(|| "output".into(), |slot| slot.to_string()),
            label: port.label.clone(),
            output: port.slot.is_none(),
            kind: materialcraft_engine::nodes::PortKind::Value,
            pos: port.position,
        })
        .collect();

    let comments = document.as_ref().map(|document| crate::graph::annotation_canvas::comments(app, ui, &document.comments, &camera)).unwrap_or_default();
    let pins = document.as_ref().map(|document| crate::graph::annotation_canvas::pins(app, ui, &document.pins, &camera)).unwrap_or_default();
    Some(FunctionCanvas { response, camera, geometry, ports, nodes, values, frames: frame_hits, comments, pins, clear_annotation })
}
