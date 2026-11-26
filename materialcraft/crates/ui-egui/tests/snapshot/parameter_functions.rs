use super::{harness, save, settle};
use egui_kittest::kittest::{By, Queryable};
use materialcraft_engine::core::{BaseParameter, NodeId};
use materialcraft_ui_egui::{MaterialCraftApp, UiState};
use serde_json::json;

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn parameter_functions_create_edit_reset_and_return_to_the_owner() {
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").expect("set MC_SNAPSHOT_DIR"));
    std::fs::create_dir_all(&directory).unwrap();
    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    app.ui.prefs.preview_size = 32;
    let owner = NodeId(app.run("graph.node.add", json!({"def": "directional_blur"})).unwrap()["id"].as_u64().unwrap());
    let mut harness = harness(app, 1600.0, 1100.0);
    settle(&mut harness, 8);
    let button = |label| By::new().role(egui::accesskit::Role::Button).label(label);
    for parameter in BaseParameter::ALL {
        let label = format!("Parameter actions: {}", parameter.label());
        assert!(
            harness.get(By::new().role(egui::accesskit::Role::Button).label(&label)).rect().bottom() < 1000.0,
            "{} must fit in Properties",
            parameter.label()
        );
    }
    save(&mut harness, &directory, "parameter-controls");

    harness.get(button("Parameter actions: Output size")).click();
    settle(&mut harness, 8);
    for label in ["Empty function", "Constant value", "Reset to default"] {
        assert!(harness.query(button(label)).is_some());
    }
    assert!(harness.query(button("Expose as new graph input")).is_none());
    save(&mut harness, &directory, "parameter-function-menu");
    harness.get(button("Empty function")).click();
    settle(&mut harness, 5);

    assert!(harness.state().session.graph().unwrap().node(owner).unwrap().base.functions[&BaseParameter::OutputSize].nodes.is_empty());
    assert_eq!(harness.state().function_editor.as_ref().unwrap().node, Some(owner));
    assert!(harness.query(By::new().label_contains("No output node defined")).is_some());
    assert!(harness.query(By::new().label("Expected output: Integer2")).is_some());
    assert!(harness.query(button("Parameter actions: Output size")).is_some(), "owner Properties must remain available");
    save(&mut harness, &directory, "parameter-empty-function");

    save(&mut harness, &directory, "inheritance-relative-input");
    harness.get(button("Output size inheritance: Relative to input")).click();
    settle(&mut harness, 3);
    harness.get_by_label("Relative to parent").click();
    settle(&mut harness, 3);
    save(&mut harness, &directory, "inheritance-relative-parent");
    harness.get(button("Output size inheritance: Relative to parent")).click();
    settle(&mut harness, 3);
    harness.get(button("Absolute")).click();
    settle(&mut harness, 3);
    assert_eq!(harness.state().session.graph().unwrap().node(owner).unwrap().base.size_mode, materialcraft_engine::core::SizeMode::Absolute);
    save(&mut harness, &directory, "inheritance-absolute");
    harness.get(button("Output size inheritance: Absolute")).click();
    settle(&mut harness, 3);
    harness.get_by_label("Relative to parent").click();
    settle(&mut harness, 3);
    let base = &harness.state().session.graph().unwrap().node(owner).unwrap().base;
    assert_eq!(base.size_mode, materialcraft_engine::core::SizeMode::RelativeToParent);
    assert!(base.functions[&BaseParameter::OutputSize].nodes.is_empty());

    harness.get(button("Return to parent graph")).click();
    settle(&mut harness, 8);
    harness.get(button("Parameter actions: Output size")).click();
    settle(&mut harness, 8);
    harness.get(button("Reset to default")).click();
    settle(&mut harness, 8);
    harness.get(button("Parameter actions: Output size")).click();
    settle(&mut harness, 8);
    save(&mut harness, &directory, "parameter-after-reset-menu");
    assert!(harness.query(button("Constant value")).is_some(), "the reset parameter menu must offer a constant");
    harness.get(button("Constant value")).click();
    settle(&mut harness, 4);

    let function = &harness.state().session.graph().unwrap().node(owner).unwrap().base.functions[&BaseParameter::OutputSize];
    assert_eq!(function.nodes.len(), 1);
    assert_eq!(function.output, Some(function.nodes[0].id));
    assert!(harness.query(By::new().label_contains("No output node defined")).is_none());
    save(&mut harness, &directory, "parameter-constant-function");

    harness.get(button("Integer2")).click();
    settle(&mut harness, 3);
    assert!(harness.query(By::new().label("Output node")).is_some());
    assert!(harness.query_all(By::new().role(egui::accesskit::Role::SpinButton).label("X")).next().is_some());
    save(&mut harness, &directory, "parameter-constant-properties");

    harness.get(button("Return to parent graph")).click();
    settle(&mut harness, 5);
    harness.get(button("Parameter actions: Angle")).click();
    settle(&mut harness, 3);
    harness.get(button("Constant value")).click();
    settle(&mut harness, 4);
    assert!(harness.query(By::new().label("Expected output: Float")).is_some());
    assert!(harness.state().session.graph().unwrap().node(owner).unwrap().functions.contains_key("angle"));
    save(&mut harness, &directory, "parameter-specific-function");
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn exposing_a_specific_parameter_waits_for_confirmation() {
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").expect("set MC_SNAPSHOT_DIR"));
    std::fs::create_dir_all(&directory).unwrap();
    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    app.ui.prefs.preview_size = 32;
    let owner = NodeId(app.run("graph.node.add", json!({"def": "blur"})).unwrap()["id"].as_u64().unwrap());
    app.run("graph.node.param", json!({"id": owner, "param": "intensity", "value": 10.0}));
    let original = app.session.graph().unwrap().clone();
    let mut harness = harness(app, 1600.0, 1100.0);
    settle(&mut harness, 8);
    let button = |label| By::new().role(egui::accesskit::Role::Button).label(label);
    let input = |label| By::new().role(egui::accesskit::Role::TextInput).label(label);

    harness.get(button("Parameter actions: Intensity")).click();
    settle(&mut harness, 4);
    harness.get(button("Expose as new graph input")).click();
    settle(&mut harness, 4);

    assert_eq!(*harness.state().session.graph().unwrap(), original, "opening the dialog must not edit the graph");
    assert!(harness.query(By::new().label("Expose Parameter")).is_some());
    harness.get(By::new().role(egui::accesskit::Role::Slider).label("Default")).set_value_accesskit(12.0);
    settle(&mut harness, 2);
    assert_eq!(*harness.state().session.graph().unwrap(), original, "editing the default must remain a draft");
    harness.get(input("Description")).click();
    settle(&mut harness, 1);
    harness.get(input("Description")).type_text("Controls the blur radius");
    settle(&mut harness, 2);
    save(&mut harness, &directory, "expose-parameter-dialog");
    harness.get(button("Cancel")).click();
    settle(&mut harness, 4);
    assert_eq!(*harness.state().session.graph().unwrap(), original);

    harness.get(button("Parameter actions: Intensity")).click();
    settle(&mut harness, 4);
    harness.get(button("Expose as new graph input")).click();
    settle(&mut harness, 4);
    harness.get(input("Description")).click();
    settle(&mut harness, 1);
    harness.get(input("Description")).type_text("Controls the blur radius");
    harness.get(input("Group")).click();
    settle(&mut harness, 1);
    harness.get(input("Group")).type_text("Surface/Blur");
    settle(&mut harness, 2);
    harness.get(button("OK")).click();
    settle(&mut harness, 6);

    let graph = harness.state().session.graph().unwrap();
    assert_eq!(graph.exposed.len(), 1);
    assert_eq!(graph.exposed[0].default, materialcraft_engine::core::Value::Float(10.0));
    assert_eq!(graph.exposed[0].group, "Surface/Blur");
    let exposed = serde_json::to_value(&graph.exposed[0]).unwrap();
    assert_eq!(exposed["description"], "Controls the blur radius");
    assert_eq!(graph.node(owner).unwrap().functions["intensity"].input_identifier(), Some(graph.exposed[0].id.as_str()));
    harness.state_mut().run("edit.undo", json!({}));
    assert_eq!(*harness.state().session.graph().unwrap(), original, "exposure is one undo step");
}
