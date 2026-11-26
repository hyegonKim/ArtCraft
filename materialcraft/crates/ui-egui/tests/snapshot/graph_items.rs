use super::{By, MaterialCraftApp, Queryable, UiState, harness, save, settle};
use egui::{Event, Key, Modifiers, PointerButton, accesskit::Role, vec2};
use serde_json::json;

/// A Library pin can be dropped, renamed, reselected and deleted on every canvas.
#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn navigation_pin_library_drop_and_properties_in_every_canvas() {
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    for kind in ["material", "function", "pixel_processor", "value_processor", "fx_map"] {
        let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
        if kind == "function" {
            app.run("file.new", json!({"kind": "function"})).unwrap();
        } else if kind != "material" {
            app.run("graph.node.add", json!({"def": kind})).unwrap();
            materialcraft_ui_egui::menus::invoke(&mut app, &egui::Context::default(), "graph.node.edit");
        }
        app.ui.library_category = "Graph items".into();
        app.ui.library_show_other = false;
        let mut harness = harness(app, 1600.0, 960.0);
        settle(&mut harness, 6);
        for label in ["Comment", "Dot node", "Frame", "Pin"] {
            assert!(harness.query(By::new().role(Role::ListItem).label(label)).is_some(), "{kind}: {label}");
        }
        let start = harness.get(By::new().role(Role::ListItem).label("Pin")).rect().center();
        let end = harness.state().graph.view.center() + vec2(130.0, 80.0);
        harness.event(Event::PointerMoved(start));
        harness.step();
        harness.event(Event::PointerButton { pos: start, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
        harness.step();
        for position in [start + vec2(15.0, 0.0), end] {
            harness.event(Event::PointerMoved(position));
            harness.step();
        }
        harness.event(Event::PointerButton { pos: end, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
        settle(&mut harness, 4);
        let target = harness.state().session.annotation_target.clone();
        assert_eq!(target.items(&harness.state().session).unwrap().pins.len(), 1, "{kind}");
        let input = By::new().role(Role::TextInput).label("Navigation Pin description");
        assert!(harness.query(input.clone()).is_some(), "{kind}");
        assert!(harness.ctx.egui_wants_keyboard_input(), "new pin description must receive focus: {kind}");
        harness.event(Event::ModifiersChanged(Modifiers::COMMAND));
        harness.event(Event::Key { key: Key::A, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND });
        harness.step();
        harness.event(Event::Key { key: Key::A, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::COMMAND });
        harness.event(Event::ModifiersChanged(Modifiers::NONE));
        harness.event(Event::Text("Review checkpoint".into()));
        settle(&mut harness, 3);
        assert_eq!(target.items(&harness.state().session).unwrap().pins[0].name, "Review checkpoint", "{kind}");
        save(&mut harness, &directory, &format!("navigation-pin-{kind}"));

        let marker = harness.state().graph.pins[0].1.left_center() + vec2(6.0, 0.0);
        harness.event(Event::PointerMoved(marker));
        harness.step();
        for pressed in [true, false] {
            harness.event(Event::PointerButton { pos: marker, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
            harness.step();
        }
        harness.event(Event::Key { key: Key::Delete, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE });
        harness.step();
        harness.event(Event::Key { key: Key::Delete, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE });
        settle(&mut harness, 3);
        assert!(target.items(&harness.state().session).unwrap().pins.is_empty(), "Delete must remove the selected pin in {kind}");
        harness.state_mut().session.undo().unwrap();
        assert_eq!(target.items(&harness.state().session).unwrap().pins[0].name, "Review checkpoint");
    }
}
