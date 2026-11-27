#[path = "snapshot/parameter_functions.rs"]
mod parameter_functions;

#[path = "snapshot/graph_items.rs"]
mod graph_items;

#[path = "snapshot/explorer.rs"]
mod explorer;

#[path = "snapshot/export.rs"]
mod export;

#[path = "snapshot/function_instances.rs"]
mod function_instances;

#[path = "snapshot/function_library.rs"]
mod function_library;

#[path = "snapshot/function_variables.rs"]
mod function_variables;

#[path = "snapshot/embedded_graphs.rs"]
mod embedded_graphs;

#[path = "snapshot/library.rs"]
mod library;

#[path = "snapshot/menu.rs"]
mod menu;

#[path = "snapshot/node_search.rs"]
mod node_search;

use std::time::Duration;

use egui::vec2;
use egui_kittest::{
    Harness,
    kittest::{By, NodeT, Queryable},
};
use materialcraft_ui_egui::{MaterialCraftApp, UiState};
use serde_json::json;

fn harness(app: MaterialCraftApp, w: f32, h: f32) -> Harness<'static, MaterialCraftApp> {
    Harness::builder().with_size(vec2(w, h)).with_pixels_per_point(1.0).wgpu().build_ui_state(
        |ui, app: &mut MaterialCraftApp| {
            app.step(&ui.ctx().clone());
            app.draw(ui);
        },
        app,
    )
}

/// Run frames until background work (cooks, previews) settles
fn settle(h: &mut Harness<'static, MaterialCraftApp>, frames: usize) {
    for _ in 0..frames {
        h.step();
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn save(h: &mut Harness<'static, MaterialCraftApp>, dir: &std::path::Path, name: &str) {
    let img = h.render().expect("render");
    img.save(dir.join(format!("{name}.png"))).expect("save");
    println!("wrote {}", dir.join(format!("{name}.png")).display());
}

fn assert_node_menu_order(harness: &Harness<'static, MaterialCraftApp>, labels: &[&str]) {
    let button = |label| By::new().role(egui::accesskit::Role::Button).label(label);
    // Canvas cards share these labels; compare the buttons within the submenu's own parent
    let menu = harness
        .get_all(button(labels[0]))
        .filter_map(|node| node.parent())
        .find(|parent| {
            let actual: Vec<_> = parent.query_all(By::new().role(egui::accesskit::Role::Button)).filter_map(|node| node.accesskit_node().label()).collect();
            actual.len() == labels.len() && labels.iter().all(|label| actual.iter().any(|actual| actual == label))
        })
        .expect("the submenu must contain every available native operation exactly once");
    let actual: Vec<_> = menu.query_all(By::new().role(egui::accesskit::Role::Button)).filter_map(|node| node.accesskit_node().label()).collect();

    assert_eq!(actual, labels, "native node menu order must match the catalog");
}

fn double_click(harness: &mut Harness<'static, MaterialCraftApp>, position: egui::Pos2) {
    harness.event(egui::Event::PointerMoved(position));
    harness.step();
    let start = harness.ctx.input(|input| input.time) + 1.0;
    for (index, pressed) in [true, false, true, false].into_iter().enumerate() {
        harness.input_mut().time = Some(start + index as f64 * 0.05);
        harness.event(egui::Event::PointerButton { pos: position, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        harness.step();
    }
    settle(harness, 3);
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn explorer_drags_a_linked_graph_between_open_packages() {
    use materialcraft_engine::{
        Session,
        core::{Graph, Package, Value},
        file_cmds, formats,
    };
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").expect("set MC_SNAPSHOT_DIR"));
    std::fs::create_dir_all(&directory).unwrap();
    let mut source = Package::new("Source file");
    let mut graph = Graph::new("Reusable tint");
    graph.size = [4, 4];
    let color = graph.add_node("uniform", [0.0; 2]);
    graph.node_mut(color).unwrap().params.insert("color".into(), Value::Color([0.2, 0.6, 0.8, 1.0]));
    let output = graph.add_node("output", [256.0, 0.0]);
    graph.connect(color, "output", output, "input").unwrap();
    source.graphs.push(graph);
    let mut session = Session::new(source);
    session.path = Some(directory.join("source.sbs").to_string_lossy().into_owned());
    let mut target = Package::new("Target file");
    target.graphs.push(Graph::new("Target canvas"));
    file_cmds::open_bytes(&mut session, &formats::sbs::write_sbs(&target).unwrap(), Some(directory.join("target.sbs").to_string_lossy().into_owned())).unwrap();
    session.preview_size = Some(16);
    let target_document = session.document_id;
    let mut app = MaterialCraftApp::headless(session, UiState::default());
    app.ui.dock.close(materialcraft_ui_egui::docking::Panel::Library);
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 6);
    let start = harness.get(By::new().role(egui::accesskit::Role::TreeItem).label("Reusable tint")).rect().center();
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
    settle(&mut harness, 8);
    assert_eq!(harness.state().session.document_id, target_document);
    assert_eq!(harness.state().session.graph().unwrap().nodes.len(), 1);
    let instance = harness.state().session.graph().unwrap().nodes[0].id;
    assert_eq!(harness.state().session.graph().unwrap().node(instance).unwrap().instance.as_ref().unwrap().graph, "Reusable tint");
    assert!(harness.state().session.cooked.nodes[&instance].error.is_none());
    save(&mut harness, &directory, "linked-instance-explorer-drop");
    let position = harness.get(By::new().role(egui::accesskit::Role::TreeItem).label("Reusable tint")).rect().center();
    double_click(&mut harness, position);
    assert_eq!(harness.state().session.document_id, 0);
    harness.state_mut().run("graph.select", json!({"ids": [color.0]})).unwrap();
    harness.state_mut().run("graph.node.param", json!({"id": color.0, "param": "color", "value": [0.9, 0.3, 0.1, 1.0]})).unwrap();
    let position = harness.get(By::new().role(egui::accesskit::Role::TreeItem).label("Target canvas")).rect().center();
    double_click(&mut harness, position);
    settle(&mut harness, 8);
    assert_eq!(harness.state().session.document_id, target_document);
    assert_eq!(harness.state().session.history(), ["Add graph instance"]);
    let value = harness.state().session.cooked.image(instance, 0).unwrap().get(0, 0, false);
    assert!(value.into_iter().zip([0.9, 0.3, 0.1, 1.0]).all(|(actual, expected)| (actual - expected).abs() < 1e-6));
    assert!(harness.state().session.modified);
    save(&mut harness, &directory, "linked-instance-source-edit");
    let position = harness.get(By::new().role(egui::accesskit::Role::Button).label("Reusable tint")).rect().center();
    double_click(&mut harness, position);
    assert_eq!(harness.state().session.document_id, target_document, "double-click must not open the instance source");
    assert_eq!(harness.state().session.graph().unwrap().name, "Target canvas");
    assert_eq!(harness.state().session.history(), ["Add graph instance"]);
    harness.key_press_modifiers(egui::Modifiers::CTRL | egui::Modifiers::COMMAND, egui::Key::E);
    settle(&mut harness, 3);
    assert_eq!(harness.state().session.document_id, target_document, "Ctrl+E must not open the instance source");
    assert_eq!(harness.state().session.graph().unwrap().name, "Target canvas");
    assert_eq!(harness.state().session.history(), ["Add graph instance"]);
    harness.get(By::new().role(egui::accesskit::Role::Button).label("Reusable tint")).click_secondary();
    settle(&mut harness, 2);
    for label in ["View output in 2D view", "Copy info to clipboard", "Open reference", "Create graph from selection (1)", "Duplicate selection Ctrl+D"] {
        assert!(harness.query(By::new().label(label)).is_some(), "missing instance action: {label}");
    }
    save(&mut harness, &directory, "linked-instance-context-menu");
    harness.get(By::new().label("Open reference")).click();
    settle(&mut harness, 4);
    assert_eq!(harness.state().session.document_id, 0);
    assert_eq!(harness.state().session.graph().unwrap().name, "Reusable tint");
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn function_output_highlight_follows_context_menu_and_undo() {
    use materialcraft_engine::core::{FunctionGraph, Value};

    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    app.ui.dock.close(materialcraft_ui_egui::docking::Panel::Library);
    app.run("file.new", json!({"kind": "function", "name": "Output selection"})).unwrap();
    let mut function = FunctionGraph::default();
    let input = function.add_constant(Value::Float(0.5));
    let add = function.add("add", &[input, input], None);
    let multiply = function.add("mul", &[input, input], None);
    let dot = function.add("passthrough", &[multiply], None);
    for (node, position) in function.nodes.iter_mut().zip([[0.0, 64.0], [256.0, 0.0], [256.0, 224.0], [480.0, 272.0]]) {
        node.pos = position;
    }
    function.output = Some(add);
    app.run("graph.function.set", json!({"function": function})).unwrap();
    app.session.reset_history();

    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 6);
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").expect("set MC_SNAPSHOT_DIR"));
    std::fs::create_dir_all(&directory).unwrap();
    let query = |label| By::new().role(egui::accesskit::Role::Button).label(label);
    let assert_highlight = |harness: &mut Harness<'static, MaterialCraftApp>, label, output| {
        let bounds = harness.get(query(label)).rect();
        let sample = if label == "Dot" { bounds.center() } else { bounds.min + bounds.size() * egui::vec2(0.2, 0.4) };
        let image = harness.render().unwrap();
        let pixel = image.get_pixel(sample.x as u32, sample.y as u32).0;
        assert_eq!(pixel == [255, 163, 26, 255], output, "{label}: {pixel:?}");
    };

    harness.get(query("Multiplication")).click_secondary();
    settle(&mut harness, 3);
    let labels = [
        "Add Comment",
        "Add Frame",
        "Copy info to clipboard",
        "Set as output node",
        "Create graph from selection (1)",
        "Copy selection Ctrl+C",
        "Delete selection Delete",
        "Delete and relink Backspace",
        "Duplicate selection Ctrl+D",
        "Duplicate selection without link Ctrl+Shift+D",
        "Select upstream nodes",
        "Select downstream nodes",
        "Disable selection Shift+D",
    ];
    let rows: Vec<_> = labels.iter().map(|label| harness.get(query(label)).rect().center().y).collect();
    assert!(rows.windows(2).all(|pair| pair[0] < pair[1]));
    let menu_column = harness.get(query("Copy info to clipboard")).rect();
    for label in ["Add node", "Add Dot node", "Add Navigation pin", "Save package", "Paste", "Cut selection", "Select all", "Previous pin", "Next pin"] {
        assert!(
            harness.query_all(By::new().role(egui::accesskit::Role::Button).label_contains(label)).all(|button| {
                let center = button.rect().center();
                center.x < menu_column.left() || center.x > menu_column.right() || center.y < rows[0] || center.y > *rows.last().unwrap()
            }),
            "unexpected node action {label}"
        );
    }
    save(&mut harness, &directory, "function-output-context-menu");
    assert_highlight(&mut harness, "Add", true);
    harness.get(By::new().role(egui::accesskit::Role::Button).label_contains("Set as output node")).click();
    settle(&mut harness, 3);
    assert_eq!(harness.state().session.graph().unwrap().function.as_ref().unwrap().output, Some(multiply));
    assert_eq!(harness.state().session.history().len(), 1);

    harness.get(query("Add")).click();
    settle(&mut harness, 3);
    assert_highlight(&mut harness, "Add", false);
    assert_highlight(&mut harness, "Multiplication", true);
    save(&mut harness, &directory, "function-output-separate-selection");

    harness.state_mut().run("edit.undo", json!({})).unwrap();
    settle(&mut harness, 3);
    assert_eq!(harness.state().session.graph().unwrap().function.as_ref().unwrap().output, Some(add));
    assert_highlight(&mut harness, "Add", true);
    assert_highlight(&mut harness, "Multiplication", false);
    harness.state_mut().run("edit.redo", json!({})).unwrap();
    settle(&mut harness, 3);
    assert_highlight(&mut harness, "Multiplication", true);

    harness.get(query("Dot")).click_secondary();
    settle(&mut harness, 3);
    assert!(harness.query(query("Set as output node")).is_none());
    assert_eq!(harness.state().session.graph().unwrap().function.as_ref().unwrap().output, Some(multiply));
    assert_highlight(&mut harness, "Dot", false);
    assert_highlight(&mut harness, "Multiplication", true);
    save(&mut harness, &directory, "function-dot-context-menu");
    let history = harness.state().session.history();
    harness.get(query("Copy info to clipboard")).click();
    let mut copied = None;
    for _ in 0..3 {
        harness.step();
        for command in &harness.output().platform_output.commands {
            if let egui::OutputCommand::CopyText(text) = command {
                copied = Some(serde_json::from_str::<serde_json::Value>(text).unwrap());
            }
        }
    }
    assert_eq!(copied.unwrap()["node"]["id"], dot);
    assert_eq!(harness.state().session.history(), history);

    harness.state_mut().run("graph.select", json!({"ids": [add, multiply]})).unwrap();
    settle(&mut harness, 2);
    harness.get(query("Multiplication")).click_secondary();
    settle(&mut harness, 3);
    assert!(harness.get(query("Set as output node")).accesskit_node().is_disabled());
    harness.get(query("Create graph from selection (2)"));
    assert!(harness.query(query("Swap links X")).is_none());
    save(&mut harness, &directory, "function-multiple-node-context-menu");
    harness.key_press(egui::Key::Escape);
    settle(&mut harness, 2);

    harness.state_mut().run("edit.selectNone", json!({})).unwrap();
    settle(&mut harness, 2);
    let background = harness.get(By::new().role(egui::accesskit::Role::Canvas).label("Function Graph")).rect().min + vec2(24.0, 24.0);
    harness.event(egui::Event::PointerMoved(background));
    harness.event(egui::Event::PointerButton { pos: background, button: egui::PointerButton::Secondary, pressed: true, modifiers: egui::Modifiers::NONE });
    harness.step();
    harness.event(egui::Event::PointerButton { pos: background, button: egui::PointerButton::Secondary, pressed: false, modifiers: egui::Modifiers::NONE });
    settle(&mut harness, 3);
    let labels = [
        "Add node",
        "Add Comment",
        "Add Frame",
        "Add Navigation pin",
        "Add Dot node",
        "Save package Ctrl+S",
        "Paste Ctrl+V",
        "Paste without link Ctrl+Shift+V",
        "Select all Ctrl+A",
        "Previous pin Shift+F2",
        "Next pin F2",
        "Copy selection Ctrl+C",
        "Delete selection Delete",
        "Delete and relink Backspace",
        "Duplicate selection Ctrl+D",
        "Duplicate selection without link Ctrl+Shift+D",
        "Select upstream nodes",
        "Select downstream nodes",
        "Disable selection Shift+D",
    ];
    let rows: Vec<_> = labels.iter().map(|label| harness.get(query(label)).rect().center().y).collect();
    assert!(rows.windows(2).all(|pair| pair[0] < pair[1]));
    for label in &labels[11..] {
        assert!(harness.get(query(label)).accesskit_node().is_disabled(), "{label}");
    }
    for label in ["Copy info to clipboard", "Set as output node", "Cut selection"] {
        assert!(harness.query(query(label)).is_none());
    }
    save(&mut harness, &directory, "function-background-context-menu");
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn selected_function_links_expose_swap_in_the_context_menu() {
    use materialcraft_engine::core::{FunctionGraph, NodeId, Value};

    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    app.ui.dock.close(materialcraft_ui_egui::docking::Panel::Library);
    app.run("file.new", json!({"kind": "function", "name": "Swap links"})).unwrap();
    let mut graph = FunctionGraph::default();
    let a = graph.add_constant(Value::Float(1.0));
    let b = graph.add_constant(Value::Float(2.0));
    let subtract = graph.add("sub", &[a, b], None);
    let interpolation = graph.add("lerp", &[b], None);
    graph.output = Some(subtract);
    for (node, position) in graph.nodes.iter_mut().zip([[0.0, -150.0], [0.0, 150.0], [450.0, -100.0], [450.0, 180.0]]) {
        node.pos = position;
    }
    app.run("graph.function.set", json!({"function": graph})).unwrap();
    app.session.reset_history();
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 4);
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    let click = |harness: &mut Harness<'static, MaterialCraftApp>, node, input: &str, button, modifiers| {
        let curve = harness.state().graph.wires.iter().find(|(wire, _)| wire.0 == NodeId(u64::from(node)) && wire.1 == input).unwrap().1;
        let position = curve[0].lerp(curve[3], 0.5);
        harness.event(egui::Event::ModifiersChanged(modifiers));
        harness.event(egui::Event::PointerMoved(position));
        harness.step();
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton { pos: position, button, pressed, modifiers });
            harness.step();
        }
        settle(harness, 2);
    };
    let query = || By::new().role(egui::accesskit::Role::Button).label("Swap links X");
    click(&mut harness, subtract, "0", egui::PointerButton::Secondary, egui::Modifiers::NONE);
    assert!(harness.query(query()).is_none());
    harness.key_press(egui::Key::Escape);
    settle(&mut harness, 2);
    click(&mut harness, subtract, "1", egui::PointerButton::Primary, egui::Modifiers::SHIFT);
    save(&mut harness, &directory, "function-swap-links-before");
    click(&mut harness, subtract, "0", egui::PointerButton::Secondary, egui::Modifiers::NONE);
    assert!(!harness.get(query()).accesskit_node().is_disabled());
    save(&mut harness, &directory, "function-selected-links-context-menu");
    harness.get(query()).click();
    settle(&mut harness, 3);
    assert_eq!(harness.state().session.graph().unwrap().function.as_ref().unwrap().node(subtract).unwrap().inputs, vec![Some(b), Some(a)]);
    assert_eq!(harness.state().session.history().len(), 1);
    save(&mut harness, &directory, "function-swap-links-after");
    harness.key_press(egui::Key::X);
    settle(&mut harness, 2);
    assert_eq!(harness.state().session.graph().unwrap().function.as_ref().unwrap().node(subtract).unwrap().inputs, vec![Some(a), Some(b)]);

    click(&mut harness, subtract, "0", egui::PointerButton::Primary, egui::Modifiers::CTRL);
    click(&mut harness, interpolation, "0", egui::PointerButton::Primary, egui::Modifiers::SHIFT);
    click(&mut harness, interpolation, "0", egui::PointerButton::Secondary, egui::Modifiers::NONE);
    let inspection = materialcraft_ui_egui::control::handle(harness.state_mut(), &egui::Context::default(), "ui.inspect", &json!({}));
    assert_eq!(inspection["result"]["subgraph"]["selectedWires"], json!([[subtract, "1"], [interpolation, "0"]]));
    assert!(harness.query(query()).is_none());
    save(&mut harness, &directory, "function-same-output-links-context-menu");
    harness.key_press(egui::Key::Escape);
    settle(&mut harness, 2);
    let history = harness.state().session.history();
    let package = harness.state().session.pkg.clone();
    harness.key_press(egui::Key::X);
    settle(&mut harness, 2);
    assert_eq!(harness.state().session.history(), history);
    assert_eq!(harness.state().session.pkg, package);
    click(&mut harness, interpolation, "0", egui::PointerButton::Primary, egui::Modifiers::CTRL);
    click(&mut harness, subtract, "0", egui::PointerButton::Primary, egui::Modifiers::SHIFT);

    click(&mut harness, subtract, "1", egui::PointerButton::Primary, egui::Modifiers::CTRL);
    click(&mut harness, interpolation, "0", egui::PointerButton::Primary, egui::Modifiers::SHIFT);
    click(&mut harness, interpolation, "0", egui::PointerButton::Secondary, egui::Modifiers::NONE);
    harness.get(query()).click();
    settle(&mut harness, 3);
    let function = harness.state().session.graph().unwrap().function.as_ref().unwrap();
    assert_eq!(function.node(subtract).unwrap().inputs, vec![Some(b), Some(b)]);
    assert_eq!(function.node(interpolation).unwrap().inputs, vec![Some(a)]);
    save(&mut harness, &directory, "function-cross-node-swap-links-after");
    harness.key_press(egui::Key::X);
    settle(&mut harness, 2);
    let function = harness.state().session.graph().unwrap().function.as_ref().unwrap();
    assert_eq!(function.node(subtract).unwrap().inputs, vec![Some(a), Some(b)]);
    assert_eq!(function.node(interpolation).unwrap().inputs, vec![Some(b)]);
    click(&mut harness, interpolation, "0", egui::PointerButton::Primary, egui::Modifiers::CTRL);
    click(&mut harness, subtract, "1", egui::PointerButton::Primary, egui::Modifiers::SHIFT);

    let mut graph = harness.state().session.graph().unwrap().function.clone().unwrap();
    let node = graph.nodes.iter_mut().find(|node| node.id == subtract).unwrap();
    node.op = "lerp".into();
    node.inputs.push(Some(a));
    harness.state_mut().run("graph.function.set", json!({"function": graph})).unwrap();
    settle(&mut harness, 3);
    click(&mut harness, subtract, "2", egui::PointerButton::Primary, egui::Modifiers::SHIFT);
    click(&mut harness, subtract, "0", egui::PointerButton::Secondary, egui::Modifiers::NONE);
    let inspection = materialcraft_ui_egui::control::handle(harness.state_mut(), &egui::Context::default(), "ui.inspect", &json!({}));
    assert_eq!(inspection["result"]["subgraph"]["selectedWires"].as_array().unwrap().len(), 3);
    assert!(harness.query(query()).is_none());
    save(&mut harness, &directory, "function-three-selected-links-context-menu");
    harness.key_press(egui::Key::Escape);
    settle(&mut harness, 2);
    let history = harness.state().session.history();
    harness.key_press(egui::Key::X);
    settle(&mut harness, 2);
    assert_eq!(harness.state().session.history(), history);
    assert_eq!(harness.state().session.graph().unwrap().function.as_ref().unwrap(), &graph);
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn pattern_links_share_the_pair_context_menu() {
    use materialcraft_engine::core::{NodeId, PatternGraph, PatternNodeKind, Value};

    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    app.ui.dock.close(materialcraft_ui_egui::docking::Panel::Library);
    let owner = NodeId(app.run("graph.node.add", json!({"def": "fx_map"})).unwrap()["id"].as_u64().unwrap());
    let mut graph = PatternGraph::with_root(PatternNodeKind::Switch);
    let a = graph.add(PatternNodeKind::Quadrant).unwrap();
    let b = graph.add(PatternNodeKind::Quadrant).unwrap();
    graph.nodes[0].children = vec![Some(a), Some(b)];
    for (node, position) in graph.nodes.iter_mut().zip([[0.0, 0.0], [-200.0, 300.0], [200.0, 300.0]]) {
        node.position = position;
    }
    app.run("graph.pattern.set", json!({"id": owner.0, "pattern": graph})).unwrap();
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 4);
    let context = harness.ctx.clone();
    materialcraft_ui_egui::menus::invoke(harness.state_mut(), &context, "graph.node.edit");
    settle(&mut harness, 3);
    for (port, button, modifiers) in [
        ("output0", egui::PointerButton::Primary, egui::Modifiers::NONE),
        ("output1", egui::PointerButton::Primary, egui::Modifiers::SHIFT),
        ("output0", egui::PointerButton::Secondary, egui::Modifiers::NONE),
    ] {
        let curve = harness.state().graph.wires.iter().find(|(wire, _)| wire.0 == NodeId(1) && wire.1 == port).unwrap().1;
        let position = curve[0].lerp(curve[3], 0.5);
        harness.event(egui::Event::ModifiersChanged(modifiers));
        harness.event(egui::Event::PointerMoved(position));
        harness.step();
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton { pos: position, button, pressed, modifiers });
            harness.step();
        }
        settle(&mut harness, 2);
    }
    let query = By::new().role(egui::accesskit::Role::Button).label("Swap links X");
    assert!(!harness.get(query.clone()).accesskit_node().is_disabled());
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    save(&mut harness, &directory, "pattern-selected-links-context-menu");
    harness.get(query).click();
    settle(&mut harness, 3);
    let Value::PatternGraph(swapped) = &harness.state().session.graph().unwrap().node(owner).unwrap().params["pattern"] else { panic!() };
    assert_eq!(swapped.nodes[0].children, vec![Some(b), Some(a)]);
    save(&mut harness, &directory, "pattern-swap-links-after");
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn incomplete_functions_warn_in_standalone_and_processor_canvases() {
    use materialcraft_engine::{
        FunctionGraphTarget,
        core::{FunctionGraph, NodeId, Value},
    };

    for definition in ["function", "pixel_processor", "value_processor"] {
        let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
        app.ui.dock.close(materialcraft_ui_egui::docking::Panel::Library);
        let target = if definition == "function" {
            app.run("file.new", json!({"kind": "function", "name": "Incomplete function"})).unwrap();
            FunctionGraphTarget::Graph
        } else {
            let owner = NodeId(app.run("graph.node.add", json!({"def": definition})).unwrap()["id"].as_u64().unwrap());
            if definition == "pixel_processor" {
                app.run("graph.node.param", json!({"id": owner.0, "param": "color_mode", "value": false})).unwrap();
            }
            FunctionGraphTarget::Parameter { node: owner, parameter: "function".into() }
        };
        let mut graph = FunctionGraph::default();
        let source = graph.add_constant(Value::Float(0.5));
        let multiply = graph.add("mul", &[], None);
        graph.nodes[1].pos = [256.0, 0.0];
        app.run("graph.function.set", json!({"target": target, "function": graph})).unwrap();
        let mut harness = harness(app, 1600.0, 960.0);
        settle(&mut harness, 4);
        if definition != "function" {
            let context = harness.ctx.clone();
            materialcraft_ui_egui::menus::invoke(harness.state_mut(), &context, "graph.node.edit");
            settle(&mut harness, 3);
        }
        let missing_output = || By::new().label_contains("No output node defined");
        let missing_source = || By::new().label_contains("Output node has no connected value source");
        let warning_label = format!("Warnings for {}", harness.state().session.graph().unwrap().name);
        let explorer_warning = || By::new().label(&warning_label);
        assert!(harness.query(explorer_warning()).is_some(), "{definition}");
        assert!(harness.query(missing_output()).is_some(), "{definition}");
        assert!(harness.query(missing_source()).is_none(), "{definition}");
        let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
        std::fs::create_dir_all(&directory).unwrap();
        save(&mut harness, &directory, &format!("{definition}-warnings"));

        harness.get(By::new().role(egui::accesskit::Role::Button).label("Multiplication")).click_secondary();
        settle(&mut harness, 2);
        harness.get(By::new().role(egui::accesskit::Role::Button).label_contains("Set as output node")).click();
        settle(&mut harness, 3);
        assert!(harness.query(missing_output()).is_none());
        assert!(harness.query(missing_source()).is_some());
        save(&mut harness, &directory, &format!("{definition}-missing-output-source"));
        harness
            .state_mut()
            .run("graph.function.edit", json!({"target": target, "edit": {"action": "connect", "from": source, "to": multiply, "input": 1}}))
            .unwrap();
        settle(&mut harness, 3);
        assert!(harness.query(missing_source()).is_none());
        assert!(harness.query(explorer_warning()).is_none());
        save(&mut harness, &directory, &format!("{definition}-partial-output-with-source"));
        harness.state_mut().run("edit.undo", json!({})).unwrap();
        settle(&mut harness, 3);
        assert!(harness.query(missing_source()).is_some());
        assert!(harness.query(explorer_warning()).is_some());
        harness.state_mut().run("file.new", json!({"kind": "function", "name": "Another graph"})).unwrap();
        settle(&mut harness, 3);
        assert!(harness.query(explorer_warning()).is_some());
    }
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn output_menu_and_package_warnings_follow_parameter_types() {
    use materialcraft_engine::{
        FunctionGraphTarget,
        core::{FunctionGraph, NodeId, Value},
    };

    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    app.ui.dock.close(materialcraft_ui_egui::docking::Panel::Library);
    let owner = NodeId(app.run("graph.node.add", json!({"def": "pixel_processor", "params": {"color_mode": false}})).unwrap()["id"].as_u64().unwrap());
    let target = FunctionGraphTarget::Parameter { node: owner, parameter: "function".into() };
    let mut function = FunctionGraph::default();
    function.add_constant(Value::Float3([0.5; 3]));
    let scalar = function.add_constant(Value::Float(0.5));
    function.nodes[1].pos = [280.0, 0.0];
    app.run("graph.function.set", json!({"target": target, "function": function})).unwrap();
    let package_warning_label = format!("Warnings for {}*", app.session.pkg.name);
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 4);
    let context = harness.ctx.clone();
    materialcraft_ui_egui::menus::invoke(harness.state_mut(), &context, "graph.node.edit");
    settle(&mut harness, 3);
    assert!(harness.query(By::new().label("Expected output: Float")).is_some());
    assert!(harness.query(By::new().label(&package_warning_label)).is_some());
    harness.get(By::new().role(egui::accesskit::Role::Button).label("Float3")).click_secondary();
    settle(&mut harness, 3);
    assert!(harness.get(By::new().role(egui::accesskit::Role::Button).label("Set as output node")).accesskit_node().is_disabled());
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    save(&mut harness, &directory, "incompatible-function-output");
    harness.event(egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
    settle(&mut harness, 3);
    harness.state_mut().run("graph.function.edit", json!({"target": target, "edit": {"action": "output", "id": scalar}})).unwrap();
    settle(&mut harness, 3);
    assert!(harness.query(By::new().label(&package_warning_label)).is_none());
    harness.state_mut().run("graph.node.param", json!({"id": owner.0, "param": "color_mode", "value": true})).unwrap();
    settle(&mut harness, 3);
    assert!(harness.query(By::new().label("Expected output: Float4")).is_some());
    assert!(harness.query(By::new().label_contains("Output node returns Float; expected Float4")).is_some());
    assert!(harness.query(By::new().label(&package_warning_label)).is_some());
    save(&mut harness, &directory, "changed-function-output-type");
    harness.state_mut().run("edit.undo", json!({})).unwrap();
    settle(&mut harness, 3);
    assert!(harness.query(By::new().label(&package_warning_label)).is_none());
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn explorer_disclosure_tracks_input_and_output_entries() {
    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    app.session.pkg.graphs[0].name = "Graph entries".into();
    app.ui.dock.close(materialcraft_ui_egui::docking::Panel::Library);
    app.run("graph.node.add", json!({"def": "uniform"})).unwrap();
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 4);
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    let arrow_visible = |harness: &mut Harness<'static, MaterialCraftApp>| {
        let name = &harness.state().session.graph().unwrap().name;
        let row = harness.get(By::new().role(egui::accesskit::Role::TreeItem).label(name)).rect();
        let image = harness.render().unwrap();
        // The disclosure gutter contains a light triangle only when the graph has children
        ((row.left() as u32 - 16)..(row.left() as u32 - 2))
            .any(|x| ((row.top() as u32 + 3)..(row.bottom() as u32 - 3)).any(|y| image.get_pixel(x, y).0[..3].iter().all(|channel| *channel > 150)))
    };
    assert!(!arrow_visible(&mut harness));
    save(&mut harness, &directory, "explorer-no-inputs-or-outputs");

    for definition in ["input_color", "input_grayscale", "input_value", "output"] {
        harness.state_mut().run("graph.node.add", json!({"def": definition})).unwrap();
        settle(&mut harness, 3);
        assert!(arrow_visible(&mut harness), "{definition}");
        if definition == "output" {
            save(&mut harness, &directory, "explorer-with-output");
        }
        harness.state_mut().run("edit.delete", json!({})).unwrap();
        settle(&mut harness, 3);
        assert!(!arrow_visible(&mut harness), "{definition}");
        harness.state_mut().run("edit.undo", json!({})).unwrap();
        settle(&mut harness, 3);
        assert!(arrow_visible(&mut harness), "{definition}");
        harness.state_mut().run("edit.redo", json!({})).unwrap();
        settle(&mut harness, 3);
        assert!(!arrow_visible(&mut harness), "{definition}");
    }
    harness.state_mut().run("file.new", json!({"kind": "function", "name": "Function leaf"})).unwrap();
    let function = materialcraft_engine::core::FunctionGraph::constant(materialcraft_engine::core::Value::Float(0.5));
    harness.state_mut().run("graph.function.set", json!({"function": function})).unwrap();
    settle(&mut harness, 3);
    assert!(!arrow_visible(&mut harness));
    save(&mut harness, &directory, "explorer-function-leaf");
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn explorer_open_targets_the_clicked_graph() {
    let mut app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), UiState::default());
    app.run("graph.duplicate", json!({"graph": 0}));
    app.session.pkg.graphs[1].name = "Export target".into();
    app.run("graph.open", json!({"graph": 0}));
    let mut h = harness(app, 1600.0, 960.0);
    settle(&mut h, 5);
    h.get_by_label("Export target").click_secondary();
    settle(&mut h, 3);
    // Explorer menu items name their shortcut after the label
    let open = format!("Open {}", materialcraft_ui_egui::menus::shortcut_label(&h.ctx, "Enter"));
    h.get(By::new().role(egui::accesskit::Role::Button).label(&open)).click();
    settle(&mut h, 3);
    assert_eq!(h.state().session.graph, 1);
    assert!(!h.state().ui.windows.export);
    if let Some(dir) = std::env::var_os("MC_SNAPSHOT_DIR").map(std::path::PathBuf::from) {
        std::fs::create_dir_all(&dir).unwrap();
        save(&mut h, &dir, "explorer-open");
    }
}

/// The startup graph stays empty through real UI frames; templates are still available on demand
#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn empty_startup_can_create_a_template() {
    let app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 40);
    assert_eq!(harness.state().session.pkg.graphs.len(), 1);
    assert!(harness.state().session.graph().unwrap().nodes.is_empty());
    assert!(harness.state().graph.rects.is_empty());
    if let Some(dir) = std::env::var_os("MC_SNAPSHOT_DIR").map(std::path::PathBuf::from) {
        std::fs::create_dir_all(&dir).unwrap();
        save(&mut harness, &dir, "empty-startup");
    }
    harness.get_by_label("New graph...").click();
    settle(&mut harness, 4);
    harness.get(By::new().role(egui::accesskit::Role::ComboBox).label("Template")).click();
    settle(&mut harness, 2);
    harness.get_by_label("Atomic sample").click();
    settle(&mut harness, 4);
    harness.get_by_label("Create").click();
    settle(&mut harness, 40);
    assert_eq!(harness.state().session.pkg.graphs.len(), 2);
    assert!(!harness.state().session.graph().unwrap().nodes.is_empty());
    assert!(!harness.state().graph.rects.is_empty());
    if let Some(dir) = std::env::var_os("MC_SNAPSHOT_DIR").map(std::path::PathBuf::from) {
        save(&mut harness, &dir, "chosen-template");
    }
}

#[test]
#[ignore]
fn snapshots() {
    let Some(dir) = std::env::var_os("MC_SNAPSHOT_DIR").map(std::path::PathBuf::from) else {
        eprintln!("set MC_SNAPSHOT_DIR");
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), UiState::default());
    let mut h = harness(app, 1600.0, 960.0);
    settle(&mut h, 40);
    // Select a node so the properties panel shows parameters
    let id = h.state().session.graph().unwrap().nodes[0].id;
    h.state_mut().run("graph.select", json!({"ids": [id.0]}));
    let preview = h.state().session.graph().unwrap().nodes.iter().find(|node| node.def == "blur").unwrap().id;
    h.state_mut().run("view.2d", json!({"node": preview.0, "histogram": true}));
    settle(&mut h, 30);
    save(&mut h, &dir, "workspace");
    h.state_mut().ui.windows.new_graph = true;
    settle(&mut h, 40);
    save(&mut h, &dir, "new-graph");
    h.state_mut().ui.windows.new_graph = false;

    // A cut-out material in the 3D view (an opacity output), a reroute dot pulled out of a wire and another wire selected; then the alpha channel view
    h.state_mut().run("file.new", json!({"template": "atomics", "size": 512}));
    let holes = h.state_mut().run("graph.node.add", json!({"def": "pixel_processor", "at": [0, -260]})).unwrap()["id"].as_u64().unwrap();
    let out = h.state_mut().run("graph.node.add", json!({"def": "output", "at": [420, -260], "connectFrom": {"node": holes}, "params": {"usage": 7}})).unwrap()
        ["id"]
        .as_u64()
        .unwrap();
    h.state_mut().run("graph.wire.insertDot", json!({"to": out, "toPort": "input", "at": [210, -330]}));
    let wire = h.state().session.graph().unwrap().connections.iter().find(|c| c.to.0 != out).map(|c| (c.to, c.to_port.clone()));
    h.state_mut().graph.selected_wires = wire.into_iter().collect();
    h.state_mut().run("view.3d", json!({"scene": "sphere"}));
    h.state_mut().ui.frame_all = true;
    settle(&mut h, 60);
    save(&mut h, &dir, "alpha-transparency");
    h.state_mut().run("view.3d", json!({"alphaView": true}));
    settle(&mut h, 40);
    save(&mut h, &dir, "alpha-channel");
    h.state_mut().run("view.3d", json!({"alphaView": false, "scene": "rounded_cube"}));

    // A pixel processor drawing rings, with its function graph open
    h.state_mut().run("file.new", json!({"template": "empty", "name": "Rings"}));
    let f = json!({"t": "function", "v": {"nodes": [
        {"id": 1, "op": "get_float2", "value": {"t": "text", "v": "$pos"}, "pos": [0, 0]},
        {"id": 2, "op": "const_float2", "value": {"t": "float2", "v": [0.5, 0.5]}, "pos": [0, 110]},
        {"id": 3, "op": "sub", "inputs": [1, 2], "pos": [170, 40]},
        {"id": 8, "op": "dot", "inputs": [3, 3], "pos": [340, 40]},
        {"id": 4, "op": "sqrt", "inputs": [8], "pos": [340, -80]},
        {"id": 5, "op": "const_float1", "value": {"t": "float", "v": 60.0}, "pos": [340, 130]},
        {"id": 6, "op": "mul", "inputs": [4, 5], "pos": [510, 60]},
        {"id": 7, "op": "sin", "inputs": [6], "pos": [680, 60]}
    ], "output": 7}});
    let id =
        h.state_mut().run("graph.node.add", json!({"def": "pixel_processor", "at": [0, 0], "params": {"color_mode": false}})).unwrap()["id"].as_u64().unwrap();
    h.state_mut().run("graph.node.param", json!({"id": id, "param": "function", "value": f}));
    let out =
        h.state_mut().run("graph.node.add", json!({"def": "output", "connectFrom": {"node": id}, "params": {"usage": 4}})).unwrap()["id"].as_u64().unwrap();
    let _ = out;
    h.state_mut().run("graph.select", json!({"ids": [id]}));
    h.state_mut().run("view.2d", json!({"node": id}));
    h.state_mut().function_editor = Some(materialcraft_ui_egui::function::FunctionEditor::new(
        h.state().session.graph,
        materialcraft_engine::core::NodeId(id),
        materialcraft_ui_egui::function::FunctionTarget::Parameter("function".into()),
    ));
    h.state_mut().ui.frame_all = true;
    settle(&mut h, 40);
    save(&mut h, &dir, "function-editor");
    h.state_mut().function_editor = None;

    h.state_mut().run("file.newAtomicSample", json!({})).unwrap();
    let input = h.state().session.graph().unwrap().nodes.iter().find(|node| node.def == "pixel_processor").unwrap().id;
    let id = h.state_mut().run("graph.node.add", json!({"def": "levels", "connectFrom": {"node": input.0}})).unwrap()["id"].as_u64().unwrap();
    h.state_mut().run("graph.select", json!({"ids": [id]})).unwrap();
    h.state_mut().run("graph.node.param", json!({"id": id, "param": "in_low", "value": 0.3})).unwrap();
    h.state_mut().run("graph.node.param", json!({"id": id, "param": "in_mid", "value": 0.35})).unwrap();
    h.state_mut().run("graph.node.param", json!({"id": id, "param": "in_high", "value": 0.85})).unwrap();
    h.state_mut().run("view.2d", json!({"node": id})).unwrap();
    h.state_mut().ui.frame_all = true;
    settle(&mut h, 40);
    save(&mut h, &dir, "levels-editor");

    h.state_mut().run("file.newAtomicSample", json!({}));
    h.state_mut().ui.frame_all = true;
    settle(&mut h, 60);
    save(&mut h, &dir, "atomic-sample");
    let pattern = h.state().session.graph().unwrap().nodes.iter().find(|node| node.def == "fx_map").unwrap().id;
    h.state_mut().pattern_editor = Some(materialcraft_ui_egui::pattern::PatternEditor::new(h.state().session.graph, pattern));
    settle(&mut h, 20);
    save(&mut h, &dir, "pattern-editor");
}

/// Steady-state UI build time per frame with each panel closed in turn (release): `cargo test -p materialcraft-ui-egui --release --test snapshot perf -- --ignored --nocapture`
#[test]
#[ignore]
fn perf() {
    use materialcraft_ui_egui::docking::Panel;
    let app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), UiState::default());
    let mut h = harness(app, 1600.0, 960.0);
    settle(&mut h, 60);
    let measure = |h: &mut Harness<'static, MaterialCraftApp>| {
        let t = std::time::Instant::now();
        for _ in 0..30 {
            h.step();
        }
        t.elapsed().as_secs_f64() * 1000.0 / 30.0
    };
    println!("all panels: {:.2} ms/frame", measure(&mut h));
    for p in [Panel::Library, Panel::View3d, Panel::View2d, Panel::Properties, Panel::Explorer] {
        h.state_mut().ui.dock.close(p);
        settle(&mut h, 3);
        println!("without {p:?}: {:.2} ms/frame", measure(&mut h));
        h.state_mut().ui.dock.focus(p);
        settle(&mut h, 3);
    }
}

#[test]
#[ignore = "requires an offscreen GPU"]
fn graph_context_menu_has_flat_node_list() {
    let app = MaterialCraftApp::headless(materialcraft_engine::sample_session(), UiState::default());
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 8);
    harness.get(By::new().role(egui::accesskit::Role::Button).label("Brightness")).click_secondary();
    settle(&mut harness, 3);
    let directory = std::env::var_os("MC_SNAPSHOT_DIR").map(std::path::PathBuf::from);
    if let Some(directory) = &directory {
        std::fs::create_dir_all(directory).unwrap();
        save(&mut harness, directory, "graph-context-menu");
    }

    for label in [
        "Add Comment",
        "Add Frame",
        "Add Navigation pin",
        "Add Dot node",
        "View in 3D view",
        "Reset and view in 3D view",
        "View output in 2D view",
        "Compute nodes thumbnails",
        "Clear nodes thumbnails",
        "Save package",
        "Paste without link",
        "Previous pin",
        "Next pin",
        "Delete and relink",
        "Duplicate selection without link",
        "Select upstream nodes",
        "Select downstream nodes",
        "Disable selection",
    ] {
        assert!(harness.query_all(By::new().label_contains(label)).next().is_some(), "missing {label}");
    }

    harness.get_by_label("Add node").click();
    settle(&mut harness, 3);
    let primitive_labels = [
        "Bitmap",
        "Blend",
        "Blur",
        "Channels shuffle",
        "Curve",
        "Directional blur",
        "Directional warp",
        "Distance",
        "Emboss",
        "FX-Map",
        "Gradient (Dynamic)",
        "Gradient map",
        "Grayscale conversion",
        "HSL",
        "Levels",
        "Normal",
        "Pixel processor",
        "SVG",
        "Sharpen",
        "Text",
        "Transformation 2D",
        "Uniform color",
        "Value processor",
        "Warp",
        "Input color",
        "Input grayscale",
        "Input value",
        "Output",
    ];
    if let Some(directory) = &directory {
        save(&mut harness, directory, "unfiltered-add-node-menu");
    }
    assert_node_menu_order(&harness, &primitive_labels);
    assert_eq!(
        materialcraft_engine::nodes::catalog().iter().filter(|definition| definition.id != "dot").map(|definition| definition.label).collect::<Vec<_>>(),
        primitive_labels
    );
    harness.key_press(egui::Key::S);
    settle(&mut harness, 3);
    let button = |label| By::new().role(egui::accesskit::Role::Button).label(label);
    assert!(!harness.get_all(button("SVG")).last().unwrap().accesskit_node().is_disabled());
    assert!(!harness.get_all(button("Sharpen")).last().unwrap().accesskit_node().is_disabled());
    assert!(harness.get_all(button("Grayscale conversion")).last().unwrap().accesskit_node().is_disabled());
    if let Some(directory) = &directory {
        save(&mut harness, directory, "flat-add-node-menu");
    }

    let before_clear = harness.state().session.pkg.clone();
    harness.key_press(egui::Key::Backspace);
    settle(&mut harness, 3);
    assert!(!harness.get_all(button("Grayscale conversion")).last().unwrap().accesskit_node().is_disabled());
    assert_eq!(harness.state().session.pkg, before_clear);

    for _ in 0..2 {
        harness.key_press(egui::Key::Escape);
        settle(&mut harness, 2);
    }
    let graph = harness.state().session.graph().unwrap();
    let selected = graph.nodes.iter().find(|node| node.def == "fx_map").unwrap().id;
    let other = graph.nodes.iter().find(|node| node.def != "fx_map").unwrap().id;
    harness.state_mut().run("graph.select", json!({"ids": [selected.0, other.0]}));
    harness.state_mut().ui.frame_all = true;
    settle(&mut harness, 3);
    assert!(harness.query_all(By::new().label("Edit FX-Map graph")).next().is_none());
    let position = harness.state().graph.rects.iter().find(|(id, _)| *id == selected).unwrap().1.center();
    harness.event(egui::Event::PointerMoved(position));
    harness.step();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: position, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE });
        harness.step();
    }
    settle(&mut harness, 3);
    if let Some(directory) = &directory {
        save(&mut harness, directory, "pattern-node-context-menu");
    }
    assert!(
        harness.query_all(By::new().label_contains("Edit graph")).next().is_some(),
        "selection: {:?}, target: {selected:?}, position: {position:?}",
        harness.state().session.selection
    );
    assert_eq!(harness.state().session.selection.last(), Some(&selected));

    harness.get(By::new().role(egui::accesskit::Role::Button).label_contains("Edit graph")).click();
    settle(&mut harness, 3);
    assert_eq!(harness.state().pattern_editor.as_ref().map(|editor| editor.node), Some(selected));
    assert!(harness.query_all(By::new().role(egui::accesskit::Role::Window).label("FX-Map graph")).next().is_none());
    assert!(harness.query_all(By::new().role(egui::accesskit::Role::Canvas).label("FX-Map graph")).next().is_some());
    assert_eq!(harness.state().graph.rects.len(), 3);
    let mut graph = match &harness.state().session.graph().unwrap().node(selected).unwrap().params["pattern"] {
        materialcraft_engine::core::Value::PatternGraph(graph) => (**graph).clone(),
        _ => panic!("missing pattern"),
    };
    let dot = graph.add(materialcraft_engine::core::PatternNodeKind::Dot).unwrap();
    graph.nodes[0].children[1] = Some(dot);
    graph.nodes[3].position = [288.0, 240.0];
    graph.nodes[3].children[0] = Some(graph.nodes[2].id);
    let quadrant_id = graph.nodes[2].id;
    harness.state_mut().run("graph.pattern.set", json!({"id": selected.0, "pattern": graph})).unwrap();
    settle(&mut harness, 3);
    let quadrant = harness.state().graph.rects.iter().find(|(id, _)| id.0 == u64::from(quadrant_id)).unwrap().1.center();
    harness.event(egui::Event::PointerMoved(quadrant));
    harness.step();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: quadrant, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        harness.step();
    }
    settle(&mut harness, 3);
    assert!(harness.query_all(By::new().label("Pattern size")).next().is_some());
    if let Some(directory) = &directory {
        save(&mut harness, directory, "pattern-graph-editor");
    }
    harness.get(By::new().role(egui::accesskit::Role::Button).label("Return to parent graph")).click();
    settle(&mut harness, 3);
    assert!(harness.state().pattern_editor.is_none());

    harness.state_mut().run("file.new", json!({"template": "empty"})).unwrap();
    harness.state_mut().run("graph.node.add", json!({"def": "fx_map"})).unwrap();
    harness.state_mut().ui.frame_all = true;
    harness.event(egui::Event::PointerGone);
    settle(&mut harness, 3);
    assert_eq!(harness.state().graph.ports.len(), 3);
    if let Some(directory) = &directory {
        save(&mut harness, directory, "default-pattern-inputs");
    }
    let position = harness.state().graph.rects[0].1.center();
    harness.event(egui::Event::PointerMoved(position));
    settle(&mut harness, 3);
    if let Some(directory) = &directory {
        save(&mut harness, directory, "hovered-pattern-inputs");
    }

    harness.state_mut().ui.dock.focus(materialcraft_ui_egui::docking::Panel::Graph);
    harness.key_press_modifiers(egui::Modifiers::CTRL | egui::Modifiers::COMMAND, egui::Key::E);
    settle(&mut harness, 3);
    assert_eq!(harness.state().graph.rects.len(), 1);
    assert_eq!(harness.state().graph.ports.len(), 5);
    harness.event(egui::Event::PointerGone);
    settle(&mut harness, 3);
    assert!(harness.query_all(By::new().role(egui::accesskit::Role::Button).label("Quadrant")).next().is_some());
    if let Some(directory) = &directory {
        save(&mut harness, directory, "default-pattern-root");
    }
    let position = harness.state().graph.rects[0].1.center();
    harness.event(egui::Event::PointerMoved(position));
    settle(&mut harness, 3);
    if let Some(directory) = &directory {
        save(&mut harness, directory, "hovered-pattern-root");
    }

    harness.get_all(By::new().role(egui::accesskit::Role::Button).label("Quadrant")).last().unwrap().click();
    settle(&mut harness, 3);
    let blank = harness.state().graph.view.left_bottom() + vec2(12.0, -12.0);
    click_canvas(&mut harness, blank);
    if let Some(directory) = &directory {
        save(&mut harness, directory, "deselected-pattern-properties");
    }
    harness.get_all(By::new().role(egui::accesskit::Role::Button).label("Quadrant")).last().unwrap().click_secondary();
    settle(&mut harness, 3);
    harness.get_by_label("Add node").click();
    settle(&mut harness, 3);
    assert_node_menu_order(&harness, &["Quadrant", "Switch", "Iterate"]);
    harness.key_press(egui::Key::S);
    settle(&mut harness, 3);
    if let Some(directory) = &directory {
        save(&mut harness, directory, "filtered-pattern-menu");
    }
    assert!(!harness.get_all(button("Switch")).last().unwrap().accesskit_node().is_disabled());
    assert!(harness.query_all(button("Quadrant")).any(|node| node.accesskit_node().is_disabled()));
    assert!(harness.query_all(button("Iterate")).any(|node| node.accesskit_node().is_disabled()));

    harness.key_press(egui::Key::Enter);
    settle(&mut harness, 3);
    let owner = harness.state().pattern_editor.as_ref().unwrap().node;
    let materialcraft_engine::core::Value::PatternGraph(pattern) = &harness.state().session.graph().unwrap().node(owner).unwrap().params["pattern"] else {
        panic!("missing pattern");
    };

    assert_eq!(pattern.nodes.len(), 2);
    assert_eq!(pattern.nodes[1].kind, materialcraft_engine::core::PatternNodeKind::Switch);
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn annotations_and_function_graph_creation() {
    use materialcraft_engine::core::{FunctionGraph, GraphKind, Value};

    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    app.run("graph.node.add", json!({"def": "uniform", "at": [64, 64]})).unwrap();
    let frame =
        app.run("graph.frame.add", json!({"rect": [0, -64, 500, 404], "title": "Frame", "description": "Frame descriptions appear inside the frame and wrap as the frame is resized. Nodes and comments move together with this frame."})).unwrap()["id"].as_u64().unwrap();
    let comment = app.run("graph.comment.add", json!({"at": [64, 240], "text": "Comment"})).unwrap()["id"].as_u64().unwrap();
    app.run("graph.annotation.select", json!({"kind": "frame", "id": frame}));
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 8);
    assert!(harness.query_all(By::new().label("Frame title")).next().is_some());
    assert!(harness.query_all(By::new().label("Title Visible")).next().is_some());
    save(&mut harness, &directory, "selected-frame-properties");

    harness.state_mut().run("graph.annotation.select", json!({"kind": "comment", "id": comment}));
    settle(&mut harness, 3);
    assert!(harness.query_all(By::new().label("Comment description")).next().is_some());
    assert!(harness.query_all(By::new().label("Frame title")).next().is_none());
    save(&mut harness, &directory, "plain-comment-properties");

    let processor = harness.state_mut().run("graph.node.add", json!({"def": "value_processor", "at": [256, 64]})).unwrap()["id"].as_u64().unwrap();
    harness.state_mut().run("graph.node.value_input", json!({"id": processor, "name": "roughness_scale", "type": "Float"}));
    let value = harness.state_mut().run("graph.node.add", json!({"def": "input_value", "at": [64, 440]})).unwrap()["id"].as_u64().unwrap();
    harness.state_mut().run("graph.connect", json!({"from": value, "to": processor, "toPort": "value1"})).unwrap();
    harness.state_mut().run("graph.select", json!({"ids": [processor]}));
    settle(&mut harness, 3);
    harness.get_by_label("Value inputs").click();
    settle(&mut harness, 3);
    assert!(harness.query_all(By::new().label("Value 1")).next().is_some());
    assert!(harness.query_all(By::new().label("Roughness Scale")).next().is_some());
    save(&mut harness, &directory, "processor-port-labels");

    let inspection = materialcraft_ui_egui::control::handle(harness.state_mut(), &egui::Context::default(), "ui.inspect", &json!({}));
    let ports = inspection["result"]["ports"].as_array().unwrap();
    assert!(ports.iter().any(|port| port["node"] == processor && port["port"] == "value1" && port["label"] == "Value 1 (Float)"));
    assert!(ports.iter().any(|port| port["node"] == processor && port["port"] == "roughness_scale" && port["label"] == "Roughness Scale (Float)"));

    harness.state_mut().ui.windows.new_graph = true;
    settle(&mut harness, 3);
    harness.get_by_label("Function Graph").click();
    settle(&mut harness, 3);
    save(&mut harness, &directory, "new-function-graph");
    harness.get_by_label("Create").click();
    settle(&mut harness, 3);
    assert_eq!(harness.state().session.graph().unwrap().kind(), GraphKind::Function);

    let mut function = FunctionGraph::default();
    let first = function.add_constant(Value::Float(2.0));
    let second = function.add_constant(Value::Float(3.0));
    let output = function.add("mul", &[first, second], None);
    function.output = Some(output);
    function.nodes[1].pos = [0.0, 128.0];
    function.nodes[2].pos = [256.0, 64.0];
    let condition = function.add_constant(Value::Bool(true));
    let branch = function.add("ifelse", &[condition, output, second], None);
    function.nodes[3].pos = [256.0, -128.0];
    function.nodes[4].pos = [512.0, 64.0];
    function.output = Some(branch);
    harness.state_mut().run("graph.function.set", json!({"function": function}));
    harness.state_mut().ui.frame_all = true;
    settle(&mut harness, 5);
    save(&mut harness, &directory, "standalone-function-graph");
    let inspection = materialcraft_ui_egui::control::handle(harness.state_mut(), &egui::Context::default(), "ui.inspect", &json!({}));
    let ports = inspection["result"]["ports"].as_array().unwrap();
    assert!(ports.iter().any(|port| port["node"] == branch && port["port"] == "1" && port["label"] == "If"));
    assert!(ports.iter().any(|port| port["node"] == branch && port["port"] == "output" && port["label"] == "Output"));

    let position = harness.state().graph.view.right_bottom() - vec2(20.0, 20.0);
    harness.event(egui::Event::PointerMoved(position));
    harness.step();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: position, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE });
        harness.step();
    }
    settle(&mut harness, 3);
    save(&mut harness, &directory, "function-context-menu");
    harness.get_by_label("Add node").click();
    settle(&mut harness, 3);
    for label in ["Float", "Float2", "Multiplication", "Division", "Ceil", "Vector Float2", "Swizzle Integer1", "To Float", "If...Else", "While loop"] {
        assert!(harness.query_all(By::new().label(label)).next().is_some(), "missing {label}");
    }
    let labels: Vec<_> = materialcraft_engine::nodes::function::FUNCTIONS
        .iter()
        .filter(|definition| definition.id != "passthrough")
        .map(|definition| definition.label)
        .collect();
    assert_eq!(labels.len(), 84);
    assert!(harness.query(By::new().role(egui::accesskit::Role::Button).label("Dot")).is_none());
    save(&mut harness, &directory, "flat-function-node-menu");
    assert_node_menu_order(&harness, &labels);
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn function_cards_and_dot_share_the_regular_graph_style() {
    use materialcraft_engine::core::{FunctionGraph, Value};

    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    app.run("file.new", json!({"kind": "function", "name": "Function_cards"})).unwrap();
    let mut function = FunctionGraph::default();
    let value = function.add_constant(Value::Float4([1.0, 0.5, 0.25, 1.0]));
    let dot = function.add("passthrough", &[value], None);
    let multiply = function.add("mul", &[dot, dot], None);
    function.nodes[0].pos = [0.0, 0.0];
    function.nodes[1].pos = [224.0, 64.0];
    function.nodes[2].pos = [416.0, 0.0];
    function.output = Some(multiply);
    app.run("graph.function.set", json!({"function": function})).unwrap();
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 5);
    assert!(harness.query_all(By::new().role(egui::accesskit::Role::Button).label("Dot")).next().is_some());
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    save(&mut harness, &directory, "function-cards-and-dot");
    harness.get_all(By::new().role(egui::accesskit::Role::Button).label("Multiplication")).last().unwrap().click();
    settle(&mut harness, 3);
    let blank = harness.state().graph.view.left_bottom() + vec2(12.0, -12.0);
    click_canvas(&mut harness, blank);
    save(&mut harness, &directory, "deselected-function-properties");

    let position = harness.state().graph.view.right_bottom() - vec2(20.0, 20.0);
    harness.event(egui::Event::PointerMoved(position));
    harness.step();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: position, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE });
        harness.step();
    }
    settle(&mut harness, 3);
    harness.get_by_label("Add node").click();
    settle(&mut harness, 3);
    save(&mut harness, &directory, "grouped-function-menu");
    harness.key_press(egui::Key::M);
    settle(&mut harness, 3);
    save(&mut harness, &directory, "filtered-function-menu");
    assert!(harness.get_all(By::new().role(egui::accesskit::Role::Button).label("Boolean")).last().unwrap().accesskit_node().is_disabled());
    harness.key_press(egui::Key::Enter);
    settle(&mut harness, 3);
    let function = harness.state().session.graph().unwrap().function.as_ref().unwrap();
    assert_eq!(function.nodes.len(), 4);
    let added = function.nodes.last().unwrap();
    assert!(materialcraft_engine::nodes::function::definition(&added.op).unwrap().label.starts_with('M'));
}

fn click_canvas(harness: &mut Harness<'static, MaterialCraftApp>, position: egui::Pos2) {
    harness.event(egui::Event::PointerMoved(position));
    harness.step();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: position, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        harness.step();
    }
    settle(harness, 3);
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn named_outputs_select_without_loading_and_open_on_double_click() {
    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    let source = app.run("graph.node.add", json!({"def": "uniform", "at": [0, 0]})).unwrap()["id"].as_u64().unwrap();
    let output = app
        .run(
            "graph.node.add",
            json!({
                "def": "output", "at": [224, 0], "connectFrom": {"node": source}, "params": {"identifier": "base_color"}
            }),
        )
        .unwrap()["id"]
        .as_u64()
        .unwrap();
    app.run("view.2d", json!({"node": source})).unwrap();
    app.run("graph.select", json!({"ids": [source]})).unwrap();
    app.session.cook();
    let material = app.session.material_maps().into_iter().map(|(name, image)| (name, image.data.clone())).collect::<Vec<_>>();
    let view3d = serde_json::to_value(&app.session.view3d).unwrap();
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 5);
    let query = By::new().role(egui::accesskit::Role::TreeItem).label("Base Color");
    let position = harness.get(query.clone()).rect().center();
    click_canvas(&mut harness, position);
    assert_eq!(harness.state().session.selection, vec![materialcraft_engine::core::NodeId(source)]);
    assert_eq!(harness.state().session.view2d.node, Some(materialcraft_engine::core::NodeId(source)));
    assert!(!harness.state().graph.ports.iter().any(|port| port.node.0 == output && port.output));

    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    save(&mut harness, &directory, "named-output-selection");
    let start = harness.ctx.input(|input| input.time) + 1.0;
    for (index, pressed) in [true, false, true, false].into_iter().enumerate() {
        harness.input_mut().time = Some(start + index as f64 * 0.05);
        harness.event(egui::Event::PointerButton { pos: position, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        harness.step();
    }
    settle(&mut harness, 3);
    assert_eq!(harness.state().session.view2d.node, Some(materialcraft_engine::core::NodeId(output)));
    assert_eq!(harness.state().session.selection, vec![materialcraft_engine::core::NodeId(output)]);
    assert!(harness.query_all(By::new().label("Identifier")).next().is_some());
    assert_eq!(serde_json::to_value(&harness.state().session.view3d).unwrap(), view3d);
    let current = harness.state().session.material_maps().into_iter().map(|(name, image)| (name, image.data.clone())).collect::<Vec<_>>();
    assert_eq!(current, material);
    save(&mut harness, &directory, "named-output-preview");
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn shared_toolbar_targets_each_graph_and_preserves_undo() {
    use materialcraft_engine::core::{FunctionGraph, NodeId, PatternGraph, PatternNodeKind, Value};

    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    let mut common_controls = None;
    for kind in ["material", "function", "pattern"] {
        let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
        let mut owner = 0;
        match kind {
            "function" => {
                app.run("file.new", json!({"kind": "function"})).unwrap();
                let mut graph = FunctionGraph::default();
                for position in [[0.0, 0.0], [256.0, 160.0]] {
                    graph.add_constant(Value::Float(1.0));
                    graph.nodes.last_mut().unwrap().pos = position;
                }
                graph.output = Some(2);
                app.run("graph.function.set", json!({"function": graph})).unwrap();
            }
            "pattern" => {
                owner = app.run("graph.node.add", json!({"def": "fx_map", "at": [700, 800]})).unwrap()["id"].as_u64().unwrap();
                let mut graph = PatternGraph::default();
                for position in [[0.0, 0.0], [256.0, 160.0]] {
                    graph.add(PatternNodeKind::Quadrant).unwrap();
                    graph.nodes.last_mut().unwrap().position = position;
                }
                app.run("graph.pattern.set", json!({"id": owner, "pattern": graph})).unwrap();
                app.pattern_editor = Some(materialcraft_ui_egui::pattern::PatternEditor::new(app.session.graph, NodeId(owner)));
            }
            _ => {
                for position in [[0, 0], [256, 160]] {
                    app.run("graph.node.add", json!({"def": "uniform", "at": position})).unwrap();
                }
            }
        }
        let mut harness = harness(app, 1600.0, 960.0);
        settle(&mut harness, 4);
        let query = |label| By::new().role(egui::accesskit::Role::Button).label(label);
        let controls = ["Frame all (F)", "Align left", "Distribute horizontally", "Snap to grid"].map(|label| harness.get(query(label)).rect());
        if let Some(expected) = common_controls {
            assert_eq!(controls, expected, "{kind} toolbar drift");
        }
        common_controls = Some(controls);
        let context = harness.ctx.clone();
        materialcraft_ui_egui::menus::invoke(harness.state_mut(), &context, "edit.selectAll");
        settle(&mut harness, 2);
        let original = harness.state().session.pkg.clone();
        harness.get(query("Align left")).click();
        settle(&mut harness, 3);
        let graph = harness.state().session.graph().unwrap();
        let positions: Vec<_> = match kind {
            "function" => graph.function.as_ref().unwrap().nodes.iter().map(|node| node.pos).collect(),
            "pattern" => {
                let Value::PatternGraph(pattern) = &graph.node(NodeId(owner)).unwrap().params["pattern"] else { panic!("missing pattern") };
                assert_eq!(graph.node(NodeId(owner)).unwrap().pos, [704.0, 800.0]);
                pattern.nodes.iter().map(|node| node.position).collect()
            }
            _ => graph.nodes.iter().map(|node| node.pos).collect(),
        };
        assert_eq!(positions, vec![[0.0, 0.0], [0.0, 160.0]], "{kind} alignment");
        harness.state_mut().session.undo().unwrap();
        assert_eq!(harness.state().session.pkg, original);
        let snap = harness.state().ui.prefs.snap_to_grid;
        harness.get(query("Snap to grid")).click();
        settle(&mut harness, 2);
        assert_eq!(harness.state().ui.prefs.snap_to_grid, !snap);
        assert_eq!(harness.state().session.pkg, original);
        harness.get(query("Frame all (F)")).click();
        settle(&mut harness, 2);
        save(&mut harness, &directory, &format!("shared-toolbar-{kind}"));
        if kind == "pattern" {
            materialcraft_ui_egui::menus::invoke(harness.state_mut(), &context, "edit.delete");
            settle(&mut harness, 2);
            assert!(harness.query_all(By::new().label("Set a node as the root")).next().is_some());
            harness.state_mut().session.undo().unwrap();
            harness.get(query("Return to parent graph")).click();
            settle(&mut harness, 2);
            assert!(harness.state().pattern_editor.is_none());
            assert_eq!(harness.state().session.pkg, original);
        } else if kind == "function" {
            harness.get(query("Add frame around the selection")).click();
            settle(&mut harness, 2);
            assert_eq!(harness.state().session.graph().unwrap().frames.len(), 1);
            assert_eq!(harness.state().session.graph().unwrap().function, original.graphs[harness.state().session.graph].function);
            save(&mut harness, &directory, "shared-toolbar-function-frame");
        }
    }
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn embedded_function_toolbar_and_panels_share_the_active_target() {
    use materialcraft_engine::{
        FunctionGraphTarget,
        core::{FunctionGraph, NodeId, Value},
    };
    for definition in ["pixel_processor", "value_processor"] {
        let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
        let owner = NodeId(app.run("graph.node.add", json!({"def": definition, "at": [700, 800]})).unwrap()["id"].as_u64().unwrap());
        let target = FunctionGraphTarget::Parameter { node: owner, parameter: "function".into() };
        let mut graph = FunctionGraph::default();
        for position in [[0.0, 0.0], [256.0, 160.0]] {
            graph.add_constant(Value::Float(0.5));
            graph.nodes.last_mut().unwrap().pos = position;
        }
        app.run("graph.function.set", json!({"target": target, "function": graph})).unwrap();
        let mut harness = harness(app, 1600.0, 960.0);
        settle(&mut harness, 4);
        let context = harness.ctx.clone();
        materialcraft_ui_egui::menus::invoke(harness.state_mut(), &context, "graph.node.edit");
        materialcraft_ui_egui::menus::invoke(harness.state_mut(), &context, "edit.selectAll");
        settle(&mut harness, 2);
        assert!(harness.query_all(By::new().role(egui::accesskit::Role::Window)).next().is_none());
        assert_eq!(harness.query_all(By::new().role(egui::accesskit::Role::Canvas).label("Function Graph")).count(), 1);
        let query = |label| By::new().role(egui::accesskit::Role::Button).label(label);
        for label in ["Frame all (F)", "Align left", "Distribute horizontally", "Snap to grid", "Show node thumbnails"] {
            assert_eq!(harness.query_all(query(label)).count(), 1, "shared control {label}");
        }
        let original = harness.state().session.pkg.clone();
        harness.get(query("Align left")).click();
        settle(&mut harness, 2);
        let function = target.current(&harness.state().session).unwrap();
        assert_eq!(function.nodes.iter().map(|node| node.pos).collect::<Vec<_>>(), vec![[0.0, 0.0], [0.0, 160.0]]);
        assert_eq!(harness.state().session.graph().unwrap().node(owner).unwrap().pos, [704.0, 800.0]);
        materialcraft_ui_egui::menus::invoke(harness.state_mut(), &context, "edit.undo");
        settle(&mut harness, 2);
        assert_eq!(harness.state().session.pkg, original);
        let snap = harness.state().ui.prefs.snap_to_grid;
        harness.get(query("Snap to grid")).click();
        settle(&mut harness, 2);
        assert_eq!(harness.state().ui.prefs.snap_to_grid, !snap);
        assert!(harness.query_all(By::new().label("Get Float")).next().is_some());
        assert!(materialcraft_ui_egui::properties::title(harness.state()).starts_with("Float"));
        let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
        std::fs::create_dir_all(&directory).unwrap();
        save(&mut harness, &directory, &format!("{definition}-inline-function"));
        harness.state_mut().ui.library_search = "Sample".into();
        settle(&mut harness, 2);
        for (label, operation) in [("Sample gray", "samplelum"), ("Sample color", "samplecol")] {
            let tile = harness.get(By::new().role(egui::accesskit::Role::ListItem).label(label));
            let position = tile.rect().center();

            double_click(&mut harness, position);

            assert!(target.current(&harness.state().session).unwrap().nodes.iter().any(|node| node.op == operation));
            materialcraft_ui_egui::menus::invoke(harness.state_mut(), &context, "edit.undo");
            settle(&mut harness, 2);
            assert_eq!(harness.state().session.pkg, original);
        }
        save(&mut harness, &directory, &format!("{definition}-library-samplers"));

        harness.get(By::new().role(egui::accesskit::Role::Canvas).label("Function Graph")).click_secondary();
        settle(&mut harness, 2);
        harness.get_by_label("Add node").click();
        settle(&mut harness, 2);
        for label in ["Sample gray", "Sample color"] {
            assert!(harness.query_all(query(label)).next().is_some());
        }
        let labels: Vec<_> =
            target.available_operations(&harness.state().session).filter(|operation| operation.id != "passthrough").map(|operation| operation.label).collect();
        assert_eq!(labels.len(), materialcraft_engine::nodes::function::FUNCTIONS.iter().filter(|operation| operation.id != "passthrough").count());
        assert!(harness.query(query("Dot")).is_none());
        save(&mut harness, &directory, &format!("{definition}-node-menu"));
        assert_node_menu_order(&harness, &labels);
        harness.key_press(egui::Key::Escape);
        harness.key_press(egui::Key::Escape);
        settle(&mut harness, 2);
        harness.get(query("Return to parent graph")).click();
        settle(&mut harness, 3);
        assert!(harness.state().function_editor.is_none());
        assert_eq!(harness.state().session.selection, vec![owner]);
        assert_eq!(harness.state().session.pkg, original);
    }
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn material_assignment_previews_outputs_and_survives_function_editing() {
    use materialcraft_engine::core::NodeId;

    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    app.session.preview_size = Some(64);
    app.ui.prefs.preview_size = 64;
    let source =
        app.run("graph.node.add", json!({"def": "uniform", "at": [0, 0], "params": {"color_mode": true, "color": [0.8, 0.04, 0.02, 1.0]}})).unwrap()["id"]
            .as_u64()
            .unwrap();
    let output = app
        .run("graph.node.add", json!({"def": "output", "at": [224, 0], "connectFrom": {"node": source}, "params": {"identifier": "preview_image", "usage": 9}}))
        .unwrap()["id"]
        .as_u64()
        .unwrap();
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 45);
    assert!(harness.state().session.material_maps().is_empty());
    let before = harness.state().view3d.shown.clone().unwrap();
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    save(&mut harness, &directory, "material-unassigned");

    let selection = harness.state().session.selection.clone();
    harness.get(By::new().role(egui::accesskit::Role::TreeItem).label("Preview Image")).click_secondary();
    settle(&mut harness, 3);
    harness.get_by_label("View in 3D view").click();
    settle(&mut harness, 3);
    save(&mut harness, &directory, "material-channel-menu");
    harness.get(By::new().role(egui::accesskit::Role::Button).label("Base color")).click();
    settle(&mut harness, 45);
    assert_eq!(harness.state().session.selection, selection, "channel menu clicks must not reach the graph beneath them");
    assert_eq!(harness.state().session.material_maps()[0].0, "baseColor");
    let red_frame = harness.state().view3d.shown.clone().unwrap();
    assert_ne!(before.2, red_frame.2, "assignment must invalidate the rendered frame");
    let red_pixels = red_frame.2.as_chunks::<4>().0.iter().filter(|pixel| u16::from(pixel[0]) > u16::from(pixel[1]) + 30).count();
    assert!(red_pixels > 100, "the rendered mesh must show the assigned red image");
    save(&mut harness, &directory, "material-assigned-red");

    harness.state_mut().run("graph.node.param", json!({"id": source, "param": "color", "value": [0.02, 0.04, 0.8, 1.0]})).unwrap();
    settle(&mut harness, 45);
    let blue_frame = harness.state().view3d.shown.clone().unwrap();
    let blue_pixels = blue_frame.2.as_chunks::<4>().0.iter().filter(|pixel| u16::from(pixel[2]) > u16::from(pixel[0]) + 30).count();
    assert!(blue_pixels > 100, "edits must refresh the assigned material");
    assert_ne!(red_frame.2, blue_frame.2);
    save(&mut harness, &directory, "material-assigned-blue");

    harness.state_mut().run("file.new", json!({"kind": "function", "name": "Function preview"})).unwrap();
    settle(&mut harness, 45);
    let maps = harness.state().session.material_maps();
    assert_eq!(maps[0].1.get(0, 0, false), [0.02, 0.04, 0.8, 1.0]);
    assert_eq!(harness.state().session.material_preview.channels[&materialcraft_engine::core::MaterialChannel::BaseColor].node, NodeId(output));
    save(&mut harness, &directory, "material-retained-in-function");
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn explorer_inputs_and_graph_drop_share_labels_and_material_assignment() {
    use materialcraft_engine::core::{ComponentUsage, MaterialChannel, NodeId};

    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    app.session.pkg.graphs[0].name = "Atomic_sample".into();
    app.ui.prefs.preview_size = 64;
    let mut inputs = Vec::new();
    for (definition, identifier, label) in [
        ("input_color", "color_input", "Color Input"),
        ("input_grayscale", "gray_input", "Gray Input"),
        ("input_value", "brightness_value", "Brightness Value"),
    ] {
        let id = app.run("graph.node.add", json!({"def": definition, "at": [0, inputs.len() * 192], "params": {"identifier": identifier}})).unwrap()["id"]
            .as_u64()
            .unwrap();
        inputs.push((NodeId(id), label));
    }
    let source = app.run("graph.node.add", json!({"def": "uniform", "at": [224, 0], "params": {"color_mode": true, "color": [0.8, 0.04, 0.02, 1.0]}})).unwrap()
        ["id"]
        .as_u64()
        .unwrap();
    let output = app.run("graph.node.add", json!({"def": "output", "at": [448, 0], "connectFrom": {"node": source}, "params": {"identifier": "base_color", "usage": ComponentUsage::index("diffuse")}})).unwrap()["id"].as_u64().unwrap();
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 8);
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    let graph_row = harness.get(By::new().role(egui::accesskit::Role::TreeItem).label("Atomic_sample")).rect();
    for (id, label) in inputs {
        let row = harness.get(By::new().role(egui::accesskit::Role::TreeItem).label(label));
        assert!(row.rect().left() > graph_row.left());
        let position = row.rect().center();
        double_click(&mut harness, position);
        assert_eq!(harness.state().session.selection, vec![id]);
        assert_eq!(harness.get(By::new().role(egui::accesskit::Role::TextInput).label("Node label")).value().as_deref(), Some(label));
        assert!(harness.query_all(By::new().label("Identifier")).next().is_some());
        let before = harness.state().session.graph().unwrap().node(id).unwrap().clone();
        let default = &materialcraft_engine::nodes::find(&before.def).unwrap().param("usage").unwrap().default;
        assert_eq!(before.params.get("usage").unwrap_or(default), &materialcraft_engine::core::Value::Enum(ComponentUsage::ANY_INDEX));
        harness.get(By::new().role(egui::accesskit::Role::ComboBox).label("Component Usage")).click();
        settle(&mut harness, 2);
        harness.get_by_label("Base color").scroll_to_me();
        settle(&mut harness, 2);
        harness.get_by_label("Base color").click();
        settle(&mut harness, 2);
        assert_eq!(harness.state().session.graph().unwrap().node(id).unwrap().params.get("usage"), Some(&materialcraft_engine::core::Value::Enum(0)));
        save(&mut harness, &directory, &format!("component-usage-{}", before.def));
        harness.state_mut().run("edit.undo", json!({})).unwrap();
        assert_eq!(harness.state().session.graph().unwrap().node(id).unwrap(), &before);
    }
    let position = harness.get(By::new().role(egui::accesskit::Role::TreeItem).label("Base Color")).rect().center();
    double_click(&mut harness, position);
    harness.get(By::new().role(egui::accesskit::Role::ComboBox).label("Component Usage")).click();
    settle(&mut harness, 3);
    harness.get_by_label("Glossiness").scroll_to_me();
    settle(&mut harness, 3);
    harness.get_by_label("Glossiness").click();
    settle(&mut harness, 3);
    let node = harness.state().session.graph().unwrap().node(NodeId(output)).unwrap();
    assert_eq!(materialcraft_engine::eval::output_usage(node), "glossiness");
    harness.state_mut().run("graph.node.param", json!({"id": output, "param": "usage", "value": ComponentUsage::index("diffuse")})).unwrap();

    settle(&mut harness, 3);
    save(&mut harness, &directory, "explorer-inputs-and-usages");
    harness.state_mut().run("file.new", json!({"kind": "function", "name": "Function editing"})).unwrap();
    harness.state_mut().run("view.3d.clear", json!({})).unwrap();
    settle(&mut harness, 8);
    let graph = harness.state().session.graph;
    let camera = harness.state().session.view3d.clone();
    let package = harness.state().session.pkg.clone();
    let start = harness.get(By::new().role(egui::accesskit::Role::TreeItem).label("Atomic_sample")).rect().center();
    let end = harness.get(By::new().role(egui::accesskit::Role::Image).label("3D view")).rect().center();
    harness.event(egui::Event::PointerMoved(start));
    harness.step();
    harness.event(egui::Event::PointerButton { pos: start, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    harness.step();
    for position in [start + vec2(12.0, 0.0), end] {
        harness.event(egui::Event::PointerMoved(position));
        harness.step();
    }
    save(&mut harness, &directory, "graph-drop-target");
    harness.event(egui::Event::PointerButton { pos: end, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    settle(&mut harness, 45);
    assert_eq!(harness.state().session.graph, graph, "dropping a graph must not open it");
    assert_eq!(harness.state().session.view3d, camera, "dropping must not orbit the camera");
    assert_eq!(harness.state().session.pkg, package);
    let binding = &harness.state().session.material_preview.channels[&MaterialChannel::BaseColor];
    assert_eq!((binding.graph, binding.node), (0, NodeId(output)));
    let frame = &harness.state().view3d.shown.as_ref().unwrap().2;
    assert!(frame.as_chunks::<4>().0.iter().filter(|pixel| u16::from(pixel[0]) > u16::from(pixel[1]) + 30).count() > 100);
    save(&mut harness, &directory, "graph-dropped-on-material");
}
