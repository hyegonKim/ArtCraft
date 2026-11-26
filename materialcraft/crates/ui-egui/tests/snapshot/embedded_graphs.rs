use super::{double_click, harness, save, settle};
use egui_kittest::kittest::{By, Queryable};
use materialcraft_engine::core::NodeId;
use materialcraft_ui_egui::{MaterialCraftApp, UiState};
use serde_json::json;

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn fresh_embedded_graphs_open_only_after_explicit_editing() {
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").expect("set MC_SNAPSHOT_DIR"));
    std::fs::create_dir_all(&directory).unwrap();

    for (definition, label) in [("fx_map", "FX-Map"), ("pixel_processor", "Pixel processor"), ("value_processor", "Value processor")] {
        let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
        let owner = NodeId(app.run("graph.node.add", json!({"def": definition})).unwrap()["id"].as_u64().unwrap());
        app.ui.frame_all = true;
        let original = app.session.pkg.clone();
        let mut harness = harness(app, 1600.0, 960.0);
        settle(&mut harness, 4);
        let canvas = harness.get(By::new().role(egui::accesskit::Role::Canvas).label("Node graph")).rect();
        let position = harness
            .query_all(By::new().role(egui::accesskit::Role::Button).label(label))
            .map(|node| node.rect().center())
            .find(|position| canvas.contains(*position))
            .expect("the newly created node must be on the canvas");

        double_click(&mut harness, position);

        assert!(harness.state().function_editor.is_none(), "{definition}");
        assert!(harness.state().pattern_editor.is_none(), "{definition}");
        assert_eq!(harness.state().session.selection, vec![owner]);
        save(&mut harness, &directory, &format!("{definition}-after-double-click"));

        harness.key_press_modifiers(egui::Modifiers::CTRL | egui::Modifiers::COMMAND, egui::Key::E);
        settle(&mut harness, 3);

        if definition == "fx_map" {
            assert_eq!(harness.state().pattern_editor.as_ref().unwrap().node, owner);
            let canvas = harness.get(By::new().role(egui::accesskit::Role::Canvas).label("FX-Map graph")).rect();
            assert!(harness.query_all(By::new().role(egui::accesskit::Role::Button).label("Quadrant")).any(|node| canvas.contains(node.rect().center())));
        } else {
            assert_eq!(harness.state().function_editor.as_ref().unwrap().node, Some(owner));
            assert!(harness.query(By::new().label_contains("No output node defined")).is_some());
        }
        assert_eq!(harness.state().session.pkg, original);
        save(&mut harness, &directory, &format!("{definition}-initial-embedded-graph"));
    }
}
