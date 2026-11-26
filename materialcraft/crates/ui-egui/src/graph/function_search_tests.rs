//! The node search of Function canvases: keys, double-click and wires dropped on empty canvas

use egui::{Event, Key, Modifiers, PointerButton, Pos2, vec2};
use materialcraft_engine::core::{FunctionGraph, Value};
use materialcraft_engine::new_session;
use serde_json::json;

use super::frames::Frames;
use crate::function::search::{FunctionSearch, SearchWire};

fn press(key: Key) -> Event {
    Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }
}

/// A Function Graph holding one constant, framed in the canvas
fn function_canvas() -> (Frames, u32) {
    let mut frames = Frames::new(new_session());
    frames.app.run("file.new", json!({"kind": "function"})).unwrap();
    let mut graph = FunctionGraph::default();
    let constant = graph.add_constant(Value::Float(2.0));
    graph.output = Some(constant);
    frames.app.run("graph.function.set", json!({"function": graph})).unwrap();
    frames.app.ui.frame_all = true;
    for _ in 0..4 {
        frames.run(Vec::new(), Modifiers::NONE);
    }
    (frames, constant)
}

fn search(frames: &Frames) -> Option<FunctionSearch> {
    frames.app.function_editor.as_ref().and_then(|editor| editor.search.clone())
}

fn empty(frames: &Frames) -> Pos2 {
    frames.app.graph.view.min + vec2(40.0, 40.0)
}

fn port(frames: &Frames, node: u32, port: &str) -> Pos2 {
    frames
        .app
        .graph
        .ports
        .iter()
        .find(|hit| hit.node.0 == u64::from(node) && hit.port == port)
        .map(|hit| hit.pos)
        .unwrap_or_else(|| panic!("no port {node}:{port}"))
}

fn drag(frames: &mut Frames, from: Pos2, to: Pos2) {
    frames.run(vec![Event::PointerMoved(from)], Modifiers::NONE);
    frames.run(vec![Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], Modifiers::NONE);
    for step in 1..=8 {
        frames.run(vec![Event::PointerMoved(from.lerp(to, step as f32 / 8.0))], Modifiers::NONE);
    }
    frames.run(vec![Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], Modifiers::NONE);
    frames.run(Vec::new(), Modifiers::NONE);
}

/// Type `text` into the open search and pick the first result with Enter
fn choose(frames: &mut Frames, text: &str) {
    frames.run(Vec::new(), Modifiers::NONE);
    frames.run(vec![Event::Text(text.into())], Modifiers::NONE);
    frames.run(Vec::new(), Modifiers::NONE);
    frames.run(vec![press(Key::Enter)], Modifiers::NONE);
    frames.run(Vec::new(), Modifiers::NONE);
}

fn nodes(frames: &Frames) -> Vec<(u32, String, Vec<Option<u32>>)> {
    let function = frames.app.session.graph().unwrap().function.as_ref().unwrap();
    function.nodes.iter().map(|node| (node.id, node.op.clone(), node.inputs.clone())).collect()
}

#[test]
fn space_tab_and_double_click_open_the_search_and_enter_adds() {
    let (mut frames, _) = function_canvas();
    for key in [Key::Space, Key::Tab] {
        frames.run(vec![Event::PointerMoved(empty(&frames))], Modifiers::NONE);
        frames.run(vec![press(key)], Modifiers::NONE);
        let opened = search(&frames).unwrap_or_else(|| panic!("{key:?} opens the search"));
        assert_eq!(opened.wire, None);
        frames.run(vec![press(Key::Escape)], Modifiers::NONE);
        assert!(search(&frames).is_none(), "Escape closes it");
    }

    // Away from the canvas, the keys open nothing
    frames.run(vec![Event::PointerMoved(Pos2::new(5.0, 5.0))], Modifiers::NONE);
    frames.run(vec![press(Key::Space)], Modifiers::NONE);
    assert!(search(&frames).is_none());

    let at = empty(&frames);
    frames.time += 1.0;
    frames.run(vec![Event::PointerMoved(at)], Modifiers::NONE);
    for pressed in [true, false, true, false] {
        frames.run(vec![Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }], Modifiers::NONE);
    }
    assert!(search(&frames).is_some(), "a double-click on empty canvas opens the search");

    let before = nodes(&frames).len();
    choose(&mut frames, "add");
    assert!(search(&frames).is_none());
    let nodes = nodes(&frames);
    assert_eq!(nodes.len(), before + 1);
    assert!(nodes.iter().any(|(_, op, _)| op == "add"));
}

#[test]
fn wires_dropped_on_empty_canvas_search_and_connect_the_new_node() {
    let (mut frames, constant) = function_canvas();

    // From an output: the new node takes the wire on its first input
    let from = port(&frames, constant, "output");
    drag(&mut frames, from, from + vec2(260.0, -60.0));
    assert_eq!(search(&frames).unwrap().wire, Some(SearchWire::From(constant)));
    choose(&mut frames, "add");
    let (add, _, inputs) = nodes(&frames).into_iter().find(|(_, op, _)| op == "add").unwrap();
    assert_eq!(inputs, [Some(constant), None]);

    // From an unconnected input: the new node's output feeds it
    frames.run(Vec::new(), Modifiers::NONE);
    let input = port(&frames, add, "1");
    let drop = input + vec2(-40.0, 110.0);
    assert!(frames.app.graph.view.contains(input) && frames.app.graph.view.contains(drop));
    drag(&mut frames, input, drop);
    assert_eq!(search(&frames).unwrap().wire, Some(SearchWire::To(add, 1)));
    choose(&mut frames, "Float");
    let nodes = nodes(&frames);
    let (_, _, inputs) = nodes.iter().find(|(id, _, _)| *id == add).unwrap();
    let fed = inputs[1].unwrap();
    assert_eq!(nodes.iter().find(|(id, _, _)| *id == fed).unwrap().1, "const_float1");

    // A click on a port is not a drop on empty canvas
    frames.click(port(&frames, constant, "output"), Modifiers::NONE);
    assert!(search(&frames).is_none());
}

#[test]
fn the_toolbar_adds_common_function_nodes_wired_from_the_selection() {
    let (mut frames, constant) = function_canvas();
    let editor = frames.app.function_editor.as_mut().unwrap();
    editor.selection = vec![constant];
    crate::function::library::add_near_selection(&mut frames.app, "add");
    let (_, _, inputs) = nodes(&frames).into_iter().find(|(_, op, _)| op == "add").unwrap();
    assert_eq!(inputs, [Some(constant), None]);

    // With nothing selected, a constant goes in the middle of the view, unwired
    frames.app.function_editor.as_mut().unwrap().selection.clear();
    crate::function::library::add_near_selection(&mut frames.app, "const_float2");
    assert!(nodes(&frames).iter().any(|(_, op, _)| op == "const_float2"));
}

#[test]
fn function_search_lists_each_tagged_library_function_once() {
    use materialcraft_engine::core::{Graph, Package};
    use std::collections::BTreeSet;
    use std::sync::Arc;

    let (mut frames, _) = function_canvas();
    let mut graph = Graph::new("TaggedSearchFixture");
    graph.function = Some(FunctionGraph::constant(Value::Float(0.5)));
    graph.attributes.tags = "Maths;Constant".into();
    let mut package = Package::new("Search fixture");
    package.graphs.push(graph);
    for path in ["sbs://search-first.sbs", "sbs://search-second.sbs"] {
        frames.app.session.pkg.dependencies.insert(path.into(), Arc::new(package.clone()));
    }
    let editor = frames.app.function_editor.as_ref().unwrap();

    for text in ["TaggedSearchFixture", "TaggedSearchFixture maths", "TaggedSearchFixture constant"] {
        let candidates = crate::function::search::candidates(&frames.app, editor, text);

        assert_eq!(candidates.len(), 2, "one result per package, matching every category: {text}");
        let sources: BTreeSet<_> = candidates
            .iter()
            .map(|candidate| match &candidate.entry {
                crate::function::library::FunctionLibraryDrag::Instance { package, .. } => package.as_str(),
                _ => panic!("the fixture must be a library function"),
            })
            .collect();
        assert_eq!(sources, BTreeSet::from(["sbs://search-first.sbs", "sbs://search-second.sbs"]));
    }
}

#[test]
fn function_search_uses_text_typed_in_the_enter_frame() {
    let (mut frames, _) = function_canvas();
    frames.run(vec![Event::PointerMoved(empty(&frames))], Modifiers::NONE);
    frames.run(vec![press(Key::Space)], Modifiers::NONE);
    frames.run(Vec::new(), Modifiers::NONE);
    assert!(search(&frames).is_some());

    frames.run(vec![Event::Text("sample color".into()), press(Key::Enter)], Modifiers::NONE);

    assert!(search(&frames).is_none());
    let nodes = nodes(&frames);
    assert_eq!(nodes.len(), 2);
    assert!(nodes.iter().any(|(_, operation, _)| operation == "samplecol"));
}

#[test]
fn processor_search_and_toolbar_keep_both_samplers_available() {
    use materialcraft_engine::{FunctionGraphTarget, core::NodeId};

    for definition in ["pixel_processor", "value_processor"] {
        let mut frames = Frames::new(new_session());
        let owner = NodeId(frames.app.run("graph.node.add", json!({"def": definition})).unwrap()["id"].as_u64().unwrap());
        crate::menus::invoke(&mut frames.app, &frames.ctx, "graph.node.edit");
        frames.run(Vec::new(), Modifiers::NONE);
        let target = FunctionGraphTarget::Parameter { node: owner, parameter: "function".into() };
        let parent_count = frames.app.session.graph().unwrap().nodes.len();
        let editor = frames.app.function_editor.as_ref().unwrap();
        let candidates = crate::function::search::candidates(&frames.app, editor, "sample");
        for operation in ["samplelum", "samplecol"] {
            assert!(
                candidates.iter().any(|candidate| matches!(&candidate.entry,
                crate::function::library::FunctionLibraryDrag::Operation(found) if found == operation)),
                "{definition}: {operation}"
            );

            crate::function::library::add_near_selection(&mut frames.app, operation);

            assert!(target.current(&frames.app.session).unwrap().nodes.iter().any(|node| node.op == operation));
        }
        assert_eq!(frames.app.session.graph().unwrap().nodes.len(), parent_count);
    }
}
