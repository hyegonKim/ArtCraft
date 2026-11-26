use super::{By, MaterialCraftApp, Queryable, UiState, harness, json, save, settle};
use materialcraft_engine::core::{FunctionGraph, Value};

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn get_properties_offer_typed_choices_and_accept_custom_names() {
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    app.run("file.new", json!({"kind": "function", "name": "Variable choices"})).unwrap();
    let mut function = FunctionGraph::default();
    let float = function.add("get_float1", &[], Some(Value::Text("ITypedThis".into())));
    let integer = function.add("get_integer1", &[], Some(Value::Text("".into())));
    function.nodes[1].pos = [240.0, 0.0];
    function.output = Some(float);
    app.run("graph.function.set", json!({"function": function})).unwrap();
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 5);
    harness.get(By::new().role(egui::accesskit::Role::Button).label("Get Float")).click();
    settle(&mut harness, 2);
    harness.get(By::new().role(egui::accesskit::Role::Button).label("Choose variable")).click();
    settle(&mut harness, 2);

    harness.get_by_label("$time");
    harness.get_by_label("ITypedThis");
    let field = harness.get(By::new().role(egui::accesskit::Role::TextInput).label("Variable name")).rect();
    let choice = harness.get_by_label("$time").rect();
    assert!((choice.left() - field.left()).abs() < 16.0, "the dropdown must align with the editable field");
    assert!(choice.width() >= field.width() - 16.0, "the dropdown must span the editable field");
    assert!(harness.query_all(By::new().label("$depth")).next().is_none());
    assert!(harness.query_all(By::new().label("$tiling")).next().is_none());
    save(&mut harness, &directory, "get-float-variable-dropdown");
    harness.get_by_label("$time").click();
    settle(&mut harness, 2);
    assert_eq!(harness.state().session.graph().unwrap().function.as_ref().unwrap().node(float).unwrap().value, Some(Value::Text("$time".into())));

    harness.get(By::new().role(egui::accesskit::Role::Button).label("Get Integer")).click();
    settle(&mut harness, 2);
    harness.get(By::new().role(egui::accesskit::Role::Button).label("Choose variable")).click();
    settle(&mut harness, 2);
    let choices = ["$normalformat", "$tiling", "$pixelratio"].map(|name| harness.get_by_label(name).rect());
    assert!(choices[0].top() < choices[1].top() && choices[1].top() < choices[2].top());
    save(&mut harness, &directory, "get-integer-variable-dropdown");
    harness.get_by_label("$tiling").click();
    settle(&mut harness, 2);
    assert_eq!(harness.state().session.graph().unwrap().function.as_ref().unwrap().node(integer).unwrap().value, Some(Value::Text("$tiling".into())));

    let field = harness.get(By::new().role(egui::accesskit::Role::TextInput).label("Variable name"));
    field.click();
    harness.key_press_modifiers(egui::Modifiers::CTRL | egui::Modifiers::COMMAND, egui::Key::A);
    harness.event(egui::Event::Text("MyInteger".into()));
    settle(&mut harness, 2);
    assert_eq!(harness.state().session.graph().unwrap().function.as_ref().unwrap().node(integer).unwrap().value, Some(Value::Text("MyInteger".into())));
    save(&mut harness, &directory, "get-integer-typed-variable");
}
