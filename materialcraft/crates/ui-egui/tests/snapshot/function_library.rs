use super::{By, Harness, MaterialCraftApp, Queryable, UiState, harness, json, save, settle, vec2};
use materialcraft_engine::{FunctionGraphTarget, core::NodeId};

fn drag(harness: &mut Harness<'static, MaterialCraftApp>, label: &str, offset: egui::Vec2) {
    let start = harness.get(By::new().role(egui::accesskit::Role::ListItem).label(label)).rect().center();
    let end = harness.state().graph.view.center() + offset;
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
fn bottom_library_drags_functions_into_function_and_processor_canvases() {
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    for kind in ["function", "pixel_processor", "value_processor"] {
        let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
        app.run("file.new", json!({"kind": if kind == "function" { "function" } else { "material" }, "name": "Library instance"})).unwrap();
        let owner = if kind == "function" { None } else { Some(NodeId(app.run("graph.node.add", json!({"def": kind})).unwrap()["id"].as_u64().unwrap())) };
        app.ui.library_search = "Pi".into();
        let mut harness = harness(app, 1600.0, 960.0);
        settle(&mut harness, 5);
        if owner.is_some() {
            harness.key_press_modifiers(egui::Modifiers::CTRL | egui::Modifiers::COMMAND, egui::Key::E);
            settle(&mut harness, 4);
        }
        let target = owner.map_or(FunctionGraphTarget::Graph, |node| FunctionGraphTarget::Parameter { node, parameter: "function".into() });
        let previous = target.current(&harness.state().session).unwrap().nodes.len();

        drag(&mut harness, "Pi", vec2(-140.0, 0.0));

        let function = target.current(&harness.state().session).unwrap();
        assert_eq!(function.nodes.len(), previous + 1, "{kind}");
        let node = function.nodes.iter().find(|node| node.instance.is_some()).unwrap();
        assert_eq!(node.instance.as_ref().unwrap().package, "sbs://functions.sbs");
        assert_eq!(node.instance.as_ref().unwrap().graph, "Functions/Math/Pi");
        assert!(matches!(node.source, Some(Ok(_))));
        save(&mut harness, &directory, &format!("library-drag-{kind}"));

        harness.state_mut().ui.library_search = "Division".into();
        settle(&mut harness, 3);
        drag(&mut harness, "Division", vec2(20.0, 120.0));

        let function = target.current(&harness.state().session).unwrap();
        let division = function.nodes.iter().find(|node| node.op == "div").unwrap();
        assert!(division.instance.is_none(), "native operations must not become SBS instances");
        assert_eq!(division.inputs.len(), 2);
        harness.get(By::new().role(egui::accesskit::Role::Button).label("Division"));
        save(&mut harness, &directory, &format!("library-native-{kind}"));

        harness.state_mut().ui.library_search = "GlobalRandom".into();
        settle(&mut harness, 3);
        drag(&mut harness, "Global random", vec2(160.0, 0.0));

        let random = target
            .current(&harness.state().session)
            .unwrap()
            .nodes
            .iter()
            .find(|node| node.instance.as_ref().is_some_and(|instance| instance.graph.ends_with("GlobalRandom")))
            .unwrap();
        assert!(matches!(random.source, Some(Ok(_))));
        save(&mut harness, &directory, &format!("library-random-{kind}"));

        let mut missing = harness.state().session.pkg.dependencies["sbs://functions.sbs"].graph_by_name("Functions/Random/GlobalRandom").unwrap().1.clone();
        missing.name = "MissingFunction".into();
        missing.attributes.label.clear();
        missing.folder = "Functions/Test".into();
        missing.function = Some(Default::default());
        let mut package = materialcraft_engine::core::Package::new("Missing library");
        package.graphs.push(missing);
        harness.state_mut().session.pkg.dependencies.insert("sbs://missing-functions.sbs".into(), std::sync::Arc::new(package));

        harness.state_mut().ui.library_search = "MissingFunction".into();
        settle(&mut harness, 3);
        // The canvas sits under the two toolbar rows; drop above its lower edge
        drag(&mut harness, "MissingFunction", vec2(160.0, 110.0));

        let missing = target
            .current(&harness.state().session)
            .unwrap()
            .nodes
            .iter()
            .find(|node| node.instance.as_ref().is_some_and(|instance| instance.graph.ends_with("MissingFunction")))
            .unwrap();
        assert!(matches!(missing.source, Some(Err(_))));
        let bounds = harness.get(By::new().role(egui::accesskit::Role::Button).label("MissingFunction")).rect();
        let image = harness.render().unwrap();
        let magenta = (2..12).any(|y| {
            (2..60).any(|x| {
                let pixel = image.get_pixel((bounds.min.x + x as f32) as u32, (bounds.min.y + y as f32) as u32).0;
                pixel[0] > 150 && pixel[1] < 80 && pixel[2] > 80
            })
        });
        assert!(magenta, "missing header must be magenta");
        save(&mut harness, &directory, &format!("library-missing-{kind}"));
    }
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn bundled_even_count_opens_as_an_editable_arithmetic_tree() {
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    app.run("file.open", json!({"path": "sbs://functions.sbs"})).unwrap();
    app.run("graph.open", json!({"graph": "Functions/Math/Parity/EvenCount"})).unwrap();
    app.ui.library_search = "EvenCount".into();
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 5);

    for label in ["Get Float", "Floor", "Float", "Division", "Ceil"] {
        harness.get(By::new().role(egui::accesskit::Role::Button).label(label));
    }
    let function = harness.state().session.graph().unwrap().function.as_ref().unwrap();
    assert_eq!(function.nodes.len(), 5);
    assert_eq!(function.nodes.iter().flat_map(|node| node.inputs.iter().flatten()).count(), 4);
    assert_eq!(function.node(function.output.unwrap()).unwrap().op, "ceil");
    save(&mut harness, &directory, "even-count-arithmetic-tree");
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn samplers_drag_into_both_processor_canvases() {
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    for definition in ["pixel_processor", "value_processor"] {
        let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
        let owner = NodeId(app.run("graph.node.add", json!({"def": definition})).unwrap()["id"].as_u64().unwrap());
        app.ui.library_search = "Sample".into();
        app.ui.prefs.snap_to_grid = false;
        let mut harness = harness(app, 1600.0, 960.0);
        settle(&mut harness, 5);
        harness.key_press_modifiers(egui::Modifiers::CTRL | egui::Modifiers::COMMAND, egui::Key::E);
        settle(&mut harness, 4);
        let target = FunctionGraphTarget::Parameter { node: owner, parameter: "function".into() };
        let parent_position = harness.state().session.graph().unwrap().node(owner).unwrap().pos;
        let parent_count = harness.state().session.graph().unwrap().nodes.len();

        for (label, operation, offset) in [("Sample gray", "samplelum", vec2(-150.0, 0.0)), ("Sample color", "samplecol", vec2(150.0, 0.0))] {
            let before = target.current(&harness.state().session).unwrap().clone();

            drag(&mut harness, label, offset);

            let function = target.current(&harness.state().session).unwrap();
            assert_eq!(function.nodes.len(), before.nodes.len() + 1, "{definition}: {label}");
            let added = function.nodes.iter().find(|node| !before.nodes.iter().any(|previous| previous.id == node.id)).unwrap();
            assert_eq!(added.op, operation);
            harness.get(By::new().role(egui::accesskit::Role::Button).label(label));
            let after = function.clone();
            harness.state_mut().session.undo().unwrap();
            assert_eq!(target.current(&harness.state().session).unwrap(), &before);
            harness.state_mut().session.redo().unwrap();
            assert_eq!(target.current(&harness.state().session).unwrap(), &after);
            settle(&mut harness, 2);
        }
        assert_eq!(harness.state().session.graph().unwrap().nodes.len(), parent_count);
        assert_eq!(harness.state().session.graph().unwrap().node(owner).unwrap().pos, parent_position);
        save(&mut harness, &directory, &format!("library-sampler-drag-{definition}"));
    }
}
