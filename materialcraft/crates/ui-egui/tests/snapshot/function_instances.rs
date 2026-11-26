use super::{By, Harness, MaterialCraftApp, Queryable, UiState, double_click, harness, json, save, settle, vec2};
use materialcraft_engine::{
    FunctionGraphTarget, Session,
    core::{FunctionGraph, Graph, NodeId, Package, Value},
    file_cmds, formats,
};

fn drag(harness: &mut Harness<'static, MaterialCraftApp>, name: &str) {
    let start = harness.get(By::new().role(egui::accesskit::Role::TreeItem).label(name)).rect().center();
    let end = harness.state().graph.view.center();
    harness.event(egui::Event::PointerMoved(start));
    harness.step();
    harness.event(egui::Event::PointerButton { pos: start, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    harness.step();
    for position in [start + vec2(12.0, 0.0), end] {
        harness.event(egui::Event::PointerMoved(position));
        harness.step();
    }
    harness.event(egui::Event::PointerButton { pos: end, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    settle(harness, 5);
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn function_instances_drop_into_compatible_canvases_and_return_from_their_source() {
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    for kind in ["function", "pixel_processor", "value_processor", "fx_map", "material"] {
        let mut source = Package::new("Function sources");
        let mut tint = Graph::new("Reusable color");
        tint.function = Some(FunctionGraph::constant(Value::Float4([0.2, 0.5, 0.8, 1.0])));
        source.graphs.push(tint);
        let mut incomplete = Graph::new("Incomplete source");
        incomplete.function = Some(FunctionGraph::default());
        source.graphs.push(incomplete);
        let mut session = Session::new(source);
        session.path = Some(directory.join(format!("source-{kind}.sbs")).to_string_lossy().into());
        let mut target = Package::new("Instance destination");
        let mut graph = Graph::new("Instance canvas");
        if kind == "function" {
            graph.function = Some(FunctionGraph::default());
        }
        target.graphs.push(graph);
        file_cmds::open_bytes(
            &mut session,
            &formats::sbs::write_sbs(&target).unwrap(),
            Some(directory.join(format!("target-{kind}.sbs")).to_string_lossy().into()),
        )
        .unwrap();
        let target_document = session.document_id;
        let mut app = MaterialCraftApp::headless(session, UiState::default());
        app.ui.dock.close(materialcraft_ui_egui::docking::Panel::Library);
        let owner = if matches!(kind, "pixel_processor" | "value_processor" | "fx_map") {
            Some(NodeId(app.run("graph.node.add", json!({"def": kind})).unwrap()["id"].as_u64().unwrap()))
        } else {
            None
        };
        let mut harness = harness(app, 1600.0, 960.0);
        settle(&mut harness, 5);
        if owner.is_some() {
            harness.key_press_modifiers(egui::Modifiers::CTRL | egui::Modifiers::COMMAND, egui::Key::E);
            settle(&mut harness, 4);
            if kind == "fx_map" {
                assert!(harness.state().pattern_editor.is_some());
            } else {
                assert_eq!(harness.state().function_editor.as_ref().unwrap().node, owner);
            }
        }
        let original = harness.state().session.pkg.clone();
        drag(&mut harness, "Incomplete source");
        assert_eq!(harness.state().session.pkg, original, "an outputless graph must not produce a drag payload");
        drag(&mut harness, "Reusable color");
        if matches!(kind, "fx_map" | "material") {
            assert_eq!(harness.state().session.pkg, original, "{kind} must reject Function instances");
            continue;
        }
        let target = owner.map_or(FunctionGraphTarget::Graph, |node| FunctionGraphTarget::Parameter { node, parameter: "function".into() });
        let instance = target.current(&harness.state().session).unwrap().nodes.iter().find(|node| node.instance.is_some()).unwrap().id;
        assert_eq!(harness.state().session.document_id, target_document);
        assert_eq!(harness.state().session.graph, 0);
        harness.get(By::new().role(egui::accesskit::Role::Button).label("Reusable color")).click_secondary();
        settle(&mut harness, 2);
        harness.get(By::new().label("Set as output node")).click();
        settle(&mut harness, 3);
        assert_eq!(target.current(&harness.state().session).unwrap().output, Some(instance));
        save(&mut harness, &directory, &format!("function-instance-{kind}"));
        let position = harness.get(By::new().role(egui::accesskit::Role::Button).label("Reusable color")).rect().center();
        let history = harness.state().session.history();
        double_click(&mut harness, position);
        assert_eq!(harness.state().session.document_id, target_document, "double-click must not open the {kind} instance source");
        assert_eq!(harness.state().function_editor.as_ref().unwrap().node, owner);
        assert_eq!(target.current(&harness.state().session).unwrap().output, Some(instance));
        assert_eq!(harness.state().session.history(), history);
        harness.key_press_modifiers(egui::Modifiers::CTRL | egui::Modifiers::COMMAND, egui::Key::E);
        settle(&mut harness, 3);
        assert_eq!(harness.state().session.document_id, target_document, "Ctrl+E must not open the {kind} instance source");
        assert_eq!(harness.state().function_editor.as_ref().unwrap().node, owner);
        assert_eq!(target.current(&harness.state().session).unwrap().output, Some(instance));
        assert_eq!(harness.state().session.history(), history);
        harness.get(By::new().role(egui::accesskit::Role::Button).label("Reusable color")).click_secondary();
        settle(&mut harness, 2);
        save(&mut harness, &directory, &format!("function-instance-{kind}-menu"));
        harness.get(By::new().label("Open reference")).click();
        settle(&mut harness, 4);
        assert_eq!(harness.state().session.document_id, 0);
        assert_eq!(harness.state().session.graph().unwrap().name, "Reusable color");
        harness.state_mut().run("graph.function.edit", json!({"edit": {"action": "value", "id": 1, "value": Value::Float4([0.7,0.3,0.2,1.0])}})).unwrap();
        harness.get(By::new().role(egui::accesskit::Role::Button).label("Return to parent graph")).click();
        settle(&mut harness, 5);
        assert_eq!(harness.state().session.document_id, target_document);
        assert!(harness.state().session.modified);
        assert_eq!(harness.state().function_editor.as_ref().unwrap().node, owner);
        assert_eq!(target.current(&harness.state().session).unwrap().output, Some(instance));
        assert_eq!(harness.state().session.history().last().unwrap(), "Edit Function Graph");
        save(&mut harness, &directory, &format!("function-instance-{kind}-return"));
    }
}
