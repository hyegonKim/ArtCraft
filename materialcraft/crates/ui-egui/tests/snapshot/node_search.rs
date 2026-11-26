use super::{By, MaterialCraftApp, Queryable, UiState, harness, json, save, settle};
use egui::accesskit::Role;

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn graph_toolbars_and_the_function_node_search() {
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    let app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 6);
    // Every atomic node has a toolbar button
    for label in ["Bitmap", "FX-Map", "SVG", "Text", "Value processor", "Input color", "Input value", "Output"] {
        assert!(harness.query_all(By::new().role(Role::Button).label(label)).next().is_some(), "{label}");
    }
    save(&mut harness, &directory, "graph-toolbar-material");

    harness.state_mut().run("file.new", json!({"kind": "function", "name": "Search"})).unwrap();
    settle(&mut harness, 5);
    for label in ["Float", "Add", "Division", "Linear interpolation", "If...Else", "Sample color"] {
        let label = format!("New {label} node");
        assert!(harness.query(By::new().role(Role::Button).label(&label)).is_some(), "function toolbar: {label}");
    }
    harness.get(By::new().role(Role::Button).label("New Float node")).click();
    settle(&mut harness, 3);
    let center = harness.state().graph.view.center();
    harness.event(egui::Event::PointerMoved(center + egui::vec2(-200.0, -120.0)));
    harness.step();
    harness.key_press(egui::Key::Space);
    settle(&mut harness, 3);
    assert!(harness.query(By::new().label("Search functions")).is_some());
    harness.event(egui::Event::Text("pi".into()));
    settle(&mut harness, 3);
    save(&mut harness, &directory, "function-node-search");
    harness.key_press(egui::Key::Enter);
    settle(&mut harness, 3);
    let function = harness.state().session.graph().unwrap().function.as_ref().unwrap();
    assert_eq!(function.nodes.len(), 2, "the toolbar's Float and the searched node");
    save(&mut harness, &directory, "graph-toolbar-function");
}
