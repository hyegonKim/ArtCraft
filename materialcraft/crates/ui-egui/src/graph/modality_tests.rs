use egui::{Event, Key, Modifiers, Pos2, vec2};
use materialcraft_engine::core::{FunctionGraph, NodeId, Value};
use materialcraft_engine::new_session;
use serde_json::json;

use super::frames::Frames;
use crate::unsaved::Guarded;

const KINDS: [&str; 3] = ["graph", "function", "pattern"];

fn press(key: Key) -> Event {
    Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }
}

/// A canvas of `kind` holding a node, with the pointer on an empty corner away from the centered prompt
fn canvas(kind: &str) -> Frames {
    let mut frames = Frames::new(new_session());
    match kind {
        "function" => {
            frames.app.run("file.new", json!({"kind": "function"})).unwrap();
            let mut graph = FunctionGraph::default();
            let source = graph.add_constant(Value::Float(2.0));
            graph.output = Some(source);
            frames.app.run("graph.function.set", json!({"function": graph})).unwrap();
        }
        "pattern" => {
            let owner = NodeId(frames.app.run("graph.node.add", json!({"def": "fx_map"})).unwrap()["id"].as_u64().unwrap());
            frames.app.pattern_editor = Some(crate::pattern::PatternEditor::new(frames.app.session.graph, owner));
        }
        _ => {
            frames.app.run("graph.node.add", json!({"def": "uniform"})).unwrap();
        }
    }
    frames.app.ui.frame_all = true;
    for _ in 0..3 {
        frames.run(Vec::new(), Modifiers::NONE);
    }
    assert!(!frames.app.graph.rects.is_empty(), "{kind}");
    let corner = corner(&frames);
    frames.run(vec![Event::PointerMoved(corner)], Modifiers::NONE);
    frames
}

fn corner(frames: &Frames) -> Pos2 {
    frames.app.graph.view.min + vec2(24.0, 24.0)
}

fn add_menu_open(frames: &Frames) -> bool {
    frames.app.graph.search.is_some()
        || frames.app.function_editor.as_ref().is_some_and(|editor| editor.search.is_some())
        || egui::Popup::is_any_open(&frames.ctx)
}

#[test]
fn space_does_not_open_the_node_menu_behind_a_prompt() {
    for kind in KINDS {
        let mut frames = canvas(kind);
        frames.app.ui.windows.unsaved = Some(Guarded::Quit);
        frames.run(vec![press(Key::Space)], Modifiers::NONE);
        assert!(!add_menu_open(&frames), "{kind}");

        frames.app.ui.windows.unsaved = None;
        frames.run(Vec::new(), Modifiers::NONE);
        frames.run(vec![press(Key::Space)], Modifiers::NONE);
        assert!(add_menu_open(&frames), "{kind}: Space works once the prompt is answered");
    }
}

#[test]
fn f_does_not_move_the_camera_behind_a_prompt() {
    for kind in KINDS {
        let mut frames = canvas(kind);
        // Zoom away from the framed view, so that F has a camera to change
        frames.run(vec![Event::Zoom(1.4)], Modifiers::NONE);
        frames.run(Vec::new(), Modifiers::NONE);
        let before = frames.app.graph.rects.clone();

        frames.app.ui.windows.unsaved = Some(Guarded::Quit);
        frames.run(vec![press(Key::F)], Modifiers::NONE);
        frames.run(Vec::new(), Modifiers::NONE);
        assert_eq!(frames.app.graph.rects, before, "{kind}");

        frames.app.ui.windows.unsaved = None;
        frames.run(Vec::new(), Modifiers::NONE);
        frames.run(vec![press(Key::F)], Modifiers::NONE);
        frames.run(Vec::new(), Modifiers::NONE);
        assert_ne!(frames.app.graph.rects, before, "{kind}: F works once the prompt is answered");
    }
}

#[test]
fn enter_in_the_node_search_waits_behind_a_prompt() {
    let mut frames = canvas("graph");
    frames.run(vec![press(Key::Space)], Modifiers::NONE);
    assert!(frames.app.graph.search.is_some());
    let nodes = frames.app.session.graph().unwrap().nodes.len();

    frames.app.ui.windows.unsaved = Some(Guarded::Quit);
    frames.run(vec![press(Key::Enter)], Modifiers::NONE);
    assert_eq!(frames.app.session.graph().unwrap().nodes.len(), nodes);

    frames.app.ui.windows.unsaved = None;
    frames.run(Vec::new(), Modifiers::NONE);
    frames.run(vec![press(Key::Enter)], Modifiers::NONE);
    assert_eq!(frames.app.session.graph().unwrap().nodes.len(), nodes + 1, "Enter works once the prompt is answered");
}

#[test]
fn the_node_search_takes_no_keys_behind_a_prompt() {
    let mut frames = canvas("graph");
    frames.run(vec![press(Key::Space)], Modifiers::NONE);
    frames.run(Vec::new(), Modifiers::NONE);
    assert!(frames.app.graph.search.is_some());

    frames.app.ui.windows.unsaved = Some(Guarded::Quit);
    frames.run(Vec::new(), Modifiers::NONE);
    frames.run(vec![press(Key::ArrowDown)], Modifiers::NONE);
    assert_eq!(frames.app.graph.search.as_ref().unwrap().sel, 0, "arrows wait behind the prompt");
    frames.run(vec![Event::Text("x".into())], Modifiers::NONE);
    assert_eq!(frames.app.graph.search.as_ref().unwrap().text, "", "typing waits behind the prompt");

    frames.run(vec![press(Key::Escape)], Modifiers::NONE);
    assert!(frames.app.graph.search.is_some(), "Escape answers the prompt and leaves the search open");

    frames.app.ui.windows.unsaved = None;
    frames.run(Vec::new(), Modifiers::NONE);
    frames.run(Vec::new(), Modifiers::NONE);
    frames.run(vec![Event::Text("n".into())], Modifiers::NONE);
    assert_eq!(frames.app.graph.search.as_ref().unwrap().text, "n", "the search has the keyboard once the prompt is answered");
    frames.run(vec![press(Key::Escape)], Modifiers::NONE);
    assert!(frames.app.graph.search.is_none(), "Escape closes the search once the prompt is answered");
}

#[test]
fn a_double_click_does_not_open_the_node_search_behind_a_prompt() {
    let double_click = |frames: &mut Frames| {
        let pos = corner(frames);
        frames.time += 1.0;
        frames.run(vec![Event::PointerMoved(pos)], Modifiers::NONE);
        for pressed in [true, false, true, false] {
            frames.run(vec![Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE }], Modifiers::NONE);
        }
    };
    let mut frames = canvas("graph");
    frames.app.ui.windows.unsaved = Some(Guarded::Quit);
    frames.run(Vec::new(), Modifiers::NONE);
    double_click(&mut frames);
    assert!(frames.app.graph.search.is_none());

    frames.app.ui.windows.unsaved = None;
    frames.run(Vec::new(), Modifiers::NONE);
    double_click(&mut frames);
    assert!(frames.app.graph.search.is_some(), "a double-click opens the search once the prompt is answered");
}
