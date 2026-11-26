//! Copy, Cut and Paste of graph nodes through the platform's clipboard events and key presses

use egui::{Event, Key, Modifiers, OutputCommand, vec2};
use egui_kittest::{Harness, kittest::Queryable};
use materialcraft_ui_egui::{MaterialCraftApp, UiState, menus};
use serde_json::json;

/// Two wired nodes, both selected
fn harness(render: bool) -> Harness<'static, MaterialCraftApp> {
    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    app.session.preview_size = Some(32);
    let source = app.run("graph.node.add", json!({"def": "uniform", "at": [0, 0]})).unwrap()["id"].as_u64().unwrap();
    let levels = app.run("graph.node.add", json!({"def": "levels", "connectFrom": {"node": source}})).unwrap()["id"].as_u64().unwrap();
    app.run("graph.select", json!({"ids": [source, levels]})).unwrap();

    let builder = Harness::builder().with_size(vec2(1600.0, 960.0));
    let builder = if render { builder.wgpu() } else { builder };
    let mut harness = builder.build_ui_state(
        |ui, app: &mut MaterialCraftApp| {
            app.step(&ui.ctx().clone());
            app.draw(ui);
        },
        app,
    );
    harness.run_steps(8);
    harness
}

/// The text the last frame put on the system clipboard
fn copied_text(harness: &Harness<'static, MaterialCraftApp>) -> String {
    harness
        .output()
        .platform_output
        .commands
        .iter()
        .find_map(|command| if let OutputCommand::CopyText(text) = command { Some(text.clone()) } else { None })
        .expect("a node copy fills the system clipboard, so the desktop's Paste key sends an event")
}

fn node_count(harness: &Harness<'static, MaterialCraftApp>) -> usize {
    harness.state().session.graph().unwrap().nodes.len()
}

fn wire_count(harness: &Harness<'static, MaterialCraftApp>) -> usize {
    harness.state().session.graph().unwrap().connections.len()
}

fn chord(key: Key) -> Event {
    Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND }
}

fn copy_paste_cut(harness: &mut Harness<'static, MaterialCraftApp>) {
    harness.event(Event::Copy);
    harness.step();
    assert!(harness.state().session.clipboard.is_some(), "the desktop's Copy event copies the nodes");
    let payload = copied_text(harness);
    assert_eq!(serde_json::from_str::<serde_json::Value>(&payload).unwrap()["nodes"].as_array().unwrap().len(), 2);

    harness.event(Event::Paste(payload));
    harness.step();
    assert_eq!((node_count(harness), wire_count(harness)), (4, 2));

    harness.event(Event::Cut);
    harness.step();
    assert_eq!(node_count(harness), 2);

    harness.event(Event::Paste(copied_text(harness)));
    harness.step();
    assert_eq!((node_count(harness), wire_count(harness)), (4, 2));

    harness.state_mut().run("edit.undo", json!({})).unwrap();
    assert_eq!(node_count(harness), 2);
    harness.state_mut().run("edit.redo", json!({})).unwrap();
    assert_eq!(node_count(harness), 4);
}

#[test]
fn desktop_clipboard_events_copy_paste_and_cut_nodes_with_their_wires() {
    copy_paste_cut(&mut harness(false));
}

#[test]
fn edit_menu_copy_fills_the_system_clipboard() {
    let mut harness = harness(false);
    let label = format!("Copy {}", menus::shortcut_label(&harness.ctx, "Ctrl+C"));

    harness.get_by_label("Edit").click();
    harness.run_steps(4);
    harness.get_by_label(&label).click();
    harness.step();

    assert!(!copied_text(&harness).is_empty());
}

#[test]
fn node_context_menu_copy_fills_the_system_clipboard() {
    let mut harness = harness(false);
    let label = format!("Copy selection {}", menus::shortcut_label(&harness.ctx, "Ctrl+C"));
    let node = harness.state().graph.rects[0].1.center();

    harness.event(Event::PointerMoved(node));
    harness.step();
    for pressed in [true, false] {
        harness.event(Event::PointerButton { pos: node, button: egui::PointerButton::Secondary, pressed, modifiers: Modifiers::NONE });
        harness.step();
    }
    harness.run_steps(4);
    harness.get_by_label(&label).click();
    harness.step();

    assert!(!copied_text(&harness).is_empty());
}

#[test]
fn a_web_press_and_its_clipboard_event_run_once() {
    let mut harness = harness(false);

    // The web build sends the key press, then the clipboard event it causes
    harness.input_mut().events.extend([Event::ModifiersChanged(Modifiers::COMMAND), chord(Key::C), Event::Copy]);
    harness.step();
    let payload = copied_text(&harness);
    harness.input_mut().events.extend([chord(Key::V), Event::Paste(payload)]);
    harness.step();

    assert_eq!(node_count(&harness), 4);
}

#[test]
fn command_key_presses_still_edit_nodes() {
    let mut harness = harness(false);

    harness.input_mut().events.extend([Event::ModifiersChanged(Modifiers::COMMAND), chord(Key::C)]);
    harness.step();
    assert!(!copied_text(&harness).is_empty());
    harness.input_mut().events.push(chord(Key::V));
    harness.step();
    assert_eq!(node_count(&harness), 4);
    harness.input_mut().events.push(chord(Key::X));
    harness.step();

    assert_eq!(node_count(&harness), 2);
}

#[test]
fn clipboard_events_edit_a_focused_text_field_and_leave_the_nodes() {
    let mut harness = harness(false);
    harness.get_by_label("Node label").click();
    harness.step();
    let selected = *harness.state().session.selection.last().unwrap();
    let label = |harness: &Harness<'static, MaterialCraftApp>| harness.state().session.graph().unwrap().node(selected).unwrap().label.clone();

    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.step();
    harness.event(Event::Paste("Edited label".into()));
    harness.step();
    assert_eq!(label(&harness), "Edited label");

    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.step();
    harness.event(Event::Copy);
    harness.step();
    assert_eq!(copied_text(&harness), "Edited label");
    assert!(harness.state().session.clipboard.is_none());

    harness.event(Event::Cut);
    harness.step();
    assert_eq!(node_count(&harness), 2);
    assert_eq!(label(&harness), "");
}

#[test]
#[ignore = "requires an offscreen GPU"]
fn clipboard_graph_snapshot() {
    let mut harness = harness(true);
    copy_paste_cut(&mut harness);
    harness.run_steps(10);

    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").expect("MC_SNAPSHOT_DIR"));
    std::fs::create_dir_all(&directory).unwrap();
    harness.render().unwrap().save(directory.join("clipboard-shortcuts.png")).unwrap();
}
