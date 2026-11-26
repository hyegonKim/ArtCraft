use super::{By, MaterialCraftApp, NodeT, Queryable, UiState, harness, json, save, settle};
use egui::accesskit::Role;

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn explorer_can_close_the_last_package_and_all_packages() {
    use materialcraft_engine::{Session, core::Package};
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    let mut app = MaterialCraftApp::headless(Session::new(Package::new("Untitled package")), UiState::default());
    app.ui.dock.close(materialcraft_ui_egui::docking::Panel::Library);
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 4);
    harness.get(By::new().role(Role::TreeItem).label("Untitled package")).click_secondary();
    settle(&mut harness, 3);
    harness.get(By::new().role(Role::Button).label("Close package")).click();
    settle(&mut harness, 4);

    assert!(harness.state().session.open_packages().is_empty());
    assert_eq!(harness.query_all(By::new().role(Role::TreeItem)).count(), 0);
    assert!(harness.get(By::new().label("Save package")).accesskit_node().is_disabled());

    save(&mut harness, &directory, "explorer-no-open-projects");

    harness.get_by_label("New graph...").click();
    settle(&mut harness, 3);
    harness.get_by_label("Create").click();
    settle(&mut harness, 5);

    assert_eq!(harness.state().session.open_packages().len(), 1);
    assert!(harness.state().session.graph().is_ok());

    harness.state_mut().run("file.newPackage", json!({"name": "Second"})).unwrap();
    let context = harness.ctx.clone();
    materialcraft_ui_egui::menus::invoke(harness.state_mut(), &context, "ui.closeAll");
    settle(&mut harness, 3);

    assert!(harness.state().ui.windows.unsaved.is_some());

    harness.get_by_label("Cancel").click();
    settle(&mut harness, 3);

    assert_eq!(harness.state().session.open_packages().len(), 2);

    materialcraft_ui_egui::menus::invoke(harness.state_mut(), &context, "ui.closeAll");
    settle(&mut harness, 3);
    harness.get_by_label("Discard").click();
    settle(&mut harness, 5);

    assert!(harness.state().session.open_packages().is_empty());
    assert_eq!(harness.query_all(By::new().role(Role::TreeItem)).count(), 0);
    assert!(harness.state().ui.windows.unsaved.is_none());

    save(&mut harness, &directory, "explorer-after-close-all");
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn explorer_empty_packages_have_no_disclosure() {
    use materialcraft_engine::{Session, core::Package};
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    let app = MaterialCraftApp::headless(Session::new(Package::new("Empty package")), UiState::default());
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 4);
    let arrow_visible = |harness: &mut egui_kittest::Harness<'static, MaterialCraftApp>| {
        let label = if harness.state().session.modified { "Empty package*" } else { "Empty package" };
        let row = harness.get(By::new().role(Role::TreeItem).label(label)).rect();
        let image = harness.render().unwrap();

        // The disclosure gutter must stay blank when the package has no visible children
        ((row.left() as u32 - 16)..(row.left() as u32 - 2))
            .any(|x| ((row.top() as u32 + 3)..(row.bottom() as u32 - 3)).any(|y| image.get_pixel(x, y).0[..3].iter().all(|channel| *channel > 150)))
    };

    assert!(!arrow_visible(&mut harness));

    save(&mut harness, &directory, "explorer-empty-package");

    // An empty reserved group can be present in loaded SBS XML without appearing in the tree
    harness.state_mut().session.pkg.folders.push("Resources".into());
    settle(&mut harness, 3);

    assert!(harness.query(By::new().role(Role::TreeItem).label("Resources")).is_none());
    assert!(!arrow_visible(&mut harness));

    harness.get(By::new().role(Role::TreeItem).label("Empty package")).click_secondary();
    settle(&mut harness, 3);
    for label in ["Import", "New", "Save as...", "Close package"] {
        assert!(harness.query_all(By::new().role(Role::Button).label_contains(label)).next().is_some(), "{label}");
    }
    harness.key_press(egui::Key::Escape);
    settle(&mut harness, 3);

    for (command, parameters, child) in [
        ("file.new", json!({"template": "empty", "name": "Surface"}), "Surface"),
        ("folder.create", json!({"name": "Tools"}), "Tools"),
        ("folder.create", json!({"parent": "Resources", "name": "Masks"}), "Masks"),
    ] {
        harness.state_mut().run(command, parameters).unwrap();
        settle(&mut harness, 4);

        assert!(arrow_visible(&mut harness), "{child} must make the package expandable");
        assert!(harness.query(By::new().role(Role::TreeItem).label(child)).is_some());

        harness.state_mut().run("edit.undo", json!({})).unwrap();
        settle(&mut harness, 4);

        assert!(!arrow_visible(&mut harness), "undoing {child} must remove the disclosure");
    }

    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2"/></svg>"#;
    materialcraft_engine::file_cmds::add_resource(&mut harness.state_mut().session, "Mask.svg", svg.to_vec(), None).unwrap();
    settle(&mut harness, 4);

    assert!(arrow_visible(&mut harness));
    assert!(harness.query(By::new().role(Role::TreeItem).label("Mask")).is_some());

    save(&mut harness, &directory, "explorer-package-with-resource");
    harness.state_mut().run("edit.undo", json!({})).unwrap();
    settle(&mut harness, 4);

    assert!(!arrow_visible(&mut harness));
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn explorer_resources_drag_into_the_graph() {
    use materialcraft_engine::{core::Value, file_cmds, formats};
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    app.run("file.newPackage", json!({"name": "Drag resources"})).unwrap();
    app.run("file.new", json!({"template": "empty"})).unwrap();
    app.session.preview_size = Some(16);
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="red"/></svg>"#;
    file_cmds::add_resource(&mut app.session, "Mask.svg", svg.to_vec(), None).unwrap();
    let bitmap = formats::encode(&formats::decode(svg, false).unwrap(), "png", formats::Depth::U8, true).unwrap();
    file_cmds::add_resource(&mut app.session, "Paint.png", bitmap, None).unwrap();
    app.ui.dock.close(materialcraft_ui_egui::docking::Panel::Library);
    let document = app.session.document_id;
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 6);

    for (index, (name, definition)) in [("Mask", "svg"), ("Paint", "bitmap")].into_iter().enumerate() {
        let start = harness.get(By::new().role(Role::TreeItem).label(name)).rect().center();
        let end = harness.state().graph.view.center() + egui::vec2(index as f32 * 200.0 - 100.0, 0.0);
        harness.event(egui::Event::PointerMoved(start));
        harness.step();
        harness.event(egui::Event::PointerButton { pos: start, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
        harness.step();
        for position in [start + egui::vec2(12.0, 0.0), end] {
            harness.event(egui::Event::PointerMoved(position));
            harness.step();
        }
        harness.event(egui::Event::PointerButton { pos: end, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
        settle(&mut harness, 8);

        // A drop starts an asynchronous cook; other GPU tests may still be using the worker pool
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while (harness.state().cooker.busy() || harness.state().session.dirty) && std::time::Instant::now() < deadline {
            settle(&mut harness, 1);
        }
        assert!(!harness.state().cooker.busy() && !harness.state().session.dirty, "the resource preview cook must complete");

        let session = &harness.state().session;
        assert_eq!(session.document_id, document);
        let graph = session.graph().unwrap();
        assert_eq!(graph.nodes.len(), index + 1, "dragging {name} must add exactly one node");
        let node = graph.nodes.last().unwrap();
        assert_eq!(node.def, definition);
        assert_eq!(node.params["resource"], Value::Text(name.into()));
        assert!(session.cooked.nodes[&node.id].error.is_none());
        let bounds = harness.state().graph.rects.iter().find(|(id, _)| *id == node.id).unwrap().1;
        assert!((bounds.center().x - end.x).abs() < 2.0, "the node must land at the pointer");

        harness.state_mut().run("edit.undo", json!({})).unwrap();

        assert_eq!(harness.state().session.graph().unwrap().nodes.len(), index);
        assert_eq!(harness.state().session.pkg.resources.len(), 2);

        harness.state_mut().run("edit.redo", json!({})).unwrap();
        settle(&mut harness, 5);
    }
    save(&mut harness, &directory, "explorer-resource-drops");
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn explorer_resources_folder_is_hidden_until_an_asset_is_imported() {
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    let mut harness = harness(MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default()), 1600.0, 960.0);
    settle(&mut harness, 4);
    let resources = || By::new().role(Role::TreeItem).label("Resources");

    assert!(harness.query(resources()).is_none());

    save(&mut harness, &directory, "explorer-empty-resources-hidden");
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2"/></svg>"#;
    materialcraft_engine::file_cmds::add_resource(&mut harness.state_mut().session, "Mask.svg", svg.to_vec(), None).unwrap();
    settle(&mut harness, 4);

    assert!(harness.query(resources()).is_some());

    harness.state_mut().run("edit.undo", json!({})).unwrap();
    settle(&mut harness, 4);

    assert!(harness.query(resources()).is_none());
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn explorer_folder_properties_and_saved_package_names() {
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    app.session.pkg.name = "Untitled".into();
    app.run("folder.create", json!({"name": "Tools"})).unwrap();
    app.run("folder.create", json!({"parent": "Resources", "name": "Masks"})).unwrap();
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 5);
    let tree = |name| By::new().role(Role::TreeItem).label(name);
    let input = |name| By::new().role(Role::TextInput).label(name);

    assert!(harness.query(tree("Untitled*")).is_some());

    let position = harness.get(tree("Resources")).rect().center();
    super::click_canvas(&mut harness, position);

    assert_eq!(harness.get(input("Identifier")).value().as_deref(), Some("Resources"));

    let position = harness.get(input("Description")).rect().center();
    super::click_canvas(&mut harness, position);
    harness.get(input("Description")).type_text("Original texture assets");
    settle(&mut harness, 2);
    let position = harness.get(tree("Tools")).rect().center();
    super::click_canvas(&mut harness, position);

    assert_eq!(harness.state().session.pkg.folder_descriptions["Resources"], "Original texture assets");

    let position = harness.get(input("Description")).rect().center();
    super::click_canvas(&mut harness, position);
    harness.get(input("Description")).type_text("Reusable graph helpers");
    settle(&mut harness, 2);

    // Saving while a field still has focus must commit its draft
    harness.state_mut().run("file.saveAs", json!({"path": directory.join("Folder attributes.sbs")})).unwrap();
    settle(&mut harness, 4);

    assert_eq!(harness.state().session.pkg.folder_descriptions["Tools"], "Reusable graph helpers");
    assert!(harness.query(tree("Folder attributes.sbs")).is_some());

    save(&mut harness, &directory, "explorer-folder-properties");
    harness.state_mut().run("file.saveAs", json!({"path": directory.join("Folder attributes.mcraft")})).unwrap();
    settle(&mut harness, 4);

    assert!(harness.query(tree("Folder attributes.mcraft")).is_some());

    let position = harness.get(tree("Resources")).rect().center();
    super::click_canvas(&mut harness, position);
    save(&mut harness, &directory, "explorer-resources-properties");
    harness.get(tree("Resources")).click_secondary();
    settle(&mut harness, 2);
    harness.get(By::new().role(Role::Button).label("New")).click();
    settle(&mut harness, 2);
    for name in ["Graph", "Function Graph"] {
        assert!(harness.query(By::new().role(Role::Button).label(name)).is_none());
    }
    save(&mut harness, &directory, "explorer-resources-new-folder");
    harness.get(By::new().role(Role::Button).label("Folder")).click();
    settle(&mut harness, 4);

    assert!(harness.state().session.pkg.folder_paths().contains("Resources/Untitled_Folder_1"));
    assert_eq!(harness.get(input("Identifier")).value().as_deref(), Some("Untitled_Folder_1"));

    let position = harness.get(input("Identifier")).rect().center();
    super::click_canvas(&mut harness, position);
    harness.key_press_modifiers(egui::Modifiers::CTRL | egui::Modifiers::COMMAND, egui::Key::A);
    harness.get(input("Identifier")).type_text("Textures");
    settle(&mut harness, 2);
    let position = harness.get(tree("Tools")).rect().center();
    super::click_canvas(&mut harness, position);

    assert!(harness.state().session.pkg.folder_paths().contains("Resources/Textures"));
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn explorer_selection_does_not_open_documents_or_graphs() {
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    app.session.pkg.name = "Source".into();
    app.run("graph.rename", json!({"name": "Material"})).unwrap();
    let output = app.run("graph.node.add", json!({"def": "output", "params": {"identifier": "result"}})).unwrap()["id"].as_u64().unwrap();
    app.run("graph.node.add", json!({"def": "input_color", "params": {"identifier": "input"}})).unwrap();
    app.run("folder.create", json!({"name": "Tools"})).unwrap();
    app.run("file.new", json!({"folder": "Tools", "kind": "function", "name": "Helper"})).unwrap();
    let source = app.session.document_id;
    app.run("file.newPackage", json!({"name": "Active"})).unwrap();
    let active = (app.session.document_id, app.session.graph);
    let selection = app.session.selection.clone();
    let view = app.session.view2d.node;
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 5);

    for label in ["Source*", "Tools", "Material", "Helper", "Result", "Input"] {
        let position = harness.get(By::new().role(Role::TreeItem).label(label)).rect().center();
        super::click_canvas(&mut harness, position);

        assert_eq!((harness.state().session.document_id, harness.state().session.graph), active, "single click on {label}");
        assert_eq!(harness.state().session.selection, selection, "single click on {label}");
        assert_eq!(harness.state().session.view2d.node, view, "single click on {label}");
    }
    save(&mut harness, &directory, "explorer-select-without-opening");

    let position = harness.get(By::new().role(Role::TreeItem).label("Helper")).rect().center();
    super::double_click(&mut harness, position);
    assert_eq!(harness.state().session.document_id, source);
    assert_eq!(harness.state().session.graph().unwrap().name, "Helper");

    let position = harness.get(By::new().role(Role::TreeItem).label("Material")).rect().center();
    super::click_canvas(&mut harness, position);
    assert_eq!(harness.state().session.graph().unwrap().name, "Helper");
    super::double_click(&mut harness, position);
    assert_eq!(harness.state().session.graph().unwrap().name, "Material");

    let position = harness.get(By::new().role(Role::TreeItem).label("Result")).rect().center();
    super::double_click(&mut harness, position);
    assert_eq!(harness.state().session.selection, vec![materialcraft_engine::core::NodeId(output)]);
    assert_eq!(harness.state().session.view2d.node, Some(materialcraft_engine::core::NodeId(output)));
    save(&mut harness, &directory, "explorer-double-click-open");
}

#[test]
#[ignore = "requires an offscreen GPU; run explicitly with --ignored"]
fn explorer_folder_tree_and_context_menus() {
    let directory = std::path::PathBuf::from(std::env::var_os("MC_SNAPSHOT_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    let mut app = MaterialCraftApp::headless(materialcraft_engine::new_session(), UiState::default());
    app.session.pkg.name = "Explorer menus".into();
    app.run("folder.create", json!({"name": "Tools"})).unwrap();
    app.run("folder.create", json!({"parent": "Tools", "name": "Math"})).unwrap();
    app.run("file.new", json!({"folder": "Tools/Math", "kind": "function", "name": "Helper"})).unwrap();
    app.run("file.new", json!({"folder": "Tools", "template": "empty", "name": "Surface"})).unwrap();
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2"/></svg>"#;
    materialcraft_engine::file_cmds::add_resource(&mut app.session, "Mask.svg", svg.to_vec(), None).unwrap();
    let mut harness = harness(app, 1600.0, 960.0);
    settle(&mut harness, 6);
    fn tree(label: &str) -> By<'_> {
        By::new().role(Role::TreeItem).label(label)
    }
    fn button(label: &str) -> By<'_> {
        By::new().role(Role::Button).label(label)
    }
    let tools = harness.get(tree("Tools")).rect();
    let math = harness.get(tree("Math")).rect();
    let helper = harness.get(tree("Helper")).rect();

    assert!(tools.left() < math.left() && math.left() < helper.left());

    save(&mut harness, &directory, "explorer-folders");

    for (label, expected, screenshot) in [
        ("Tools", vec!["New", "Copy", "Paste", "Rename", "Remove"], "explorer-folder-menu"),
        (
            "Explorer menus*",
            vec![
                "New",
                "Import",
                "Link",
                "Reload",
                "Save",
                "Save as...",
                "Save a copy as...",
                "Export material...",
                "Export with dependencies...",
                "Send to",
                "Copy",
                "Paste",
                "Close package",
                "Compute outputs",
                "Show in folder...",
                "Dependency manager...",
                "Open dependencies",
            ],
            "explorer-package-menu",
        ),
        (
            "Helper",
            vec!["Open", "Copy", "Paste", "Remove", "Rename", "View outputs in 3D view", "Compute outputs", "Export outputs..."],
            "explorer-function-menu",
        ),
        (
            "Surface",
            vec!["Open", "Copy", "Paste", "Remove", "Rename", "View outputs in 3D view", "Compute outputs", "Export outputs..."],
            "explorer-graph-menu",
        ),
    ] {
        harness.get(tree(label)).click_secondary();
        settle(&mut harness, 3);
        for label in expected {
            assert!(harness.query_all(By::new().label_contains(label)).next().is_some(), "{screenshot}: {label}");
        }
        for label in ["Duplicate", "Delete"] {
            assert!(harness.query(button(label)).is_none(), "{screenshot}: unexpected {label}");
        }
        if screenshot == "explorer-package-menu" {
            for unavailable in ["Export with dependencies...", "Dependency manager...", "Show in folder..."] {
                assert!(harness.get(button(unavailable)).accesskit_node().is_disabled(), "{unavailable}");
            }
        }
        // A Function Graph has no image outputs to view, compute or export
        if matches!(screenshot, "explorer-function-menu" | "explorer-graph-menu") {
            let disabled = harness.get(button("Compute outputs")).accesskit_node().is_disabled();
            assert_eq!(disabled, screenshot == "explorer-function-menu", "{screenshot}");
        }

        assert!(harness.get(button("Paste")).accesskit_node().is_disabled());

        save(&mut harness, &directory, screenshot);
        harness.key_press(egui::Key::Escape);
        settle(&mut harness, 2);
    }

    harness.get(tree("Resources")).click_secondary();
    settle(&mut harness, 3);
    for label in ["Copy", "Paste", "Rename", "Remove"] {
        assert!(harness.query(button(label)).is_none(), "Resources must not offer {label}");
    }
    harness.get(By::new().label("Import")).click();
    settle(&mut harness, 3);
    for label in ["Bitmap", "Vector graphics"] {
        assert!(harness.query_all(button(label)).next().is_some());
    }
    save(&mut harness, &directory, "explorer-resources-import");
    harness.key_press(egui::Key::Escape);
    harness.key_press(egui::Key::Escape);
    settle(&mut harness, 2);

    harness.get(tree("Mask")).click_secondary();
    settle(&mut harness, 2);
    save(&mut harness, &directory, "explorer-resource-menu");
    harness.get(button("Add to graph")).click();
    settle(&mut harness, 3);

    assert!(harness.state().session.graph().unwrap().nodes.iter().any(|node| node.def == "svg"));

    harness.get(tree("Helper")).click_secondary();
    settle(&mut harness, 2);
    harness.get(button("Copy")).click();
    settle(&mut harness, 2);
    harness.get(tree("Tools")).click_secondary();
    settle(&mut harness, 2);
    harness.get(button("Paste")).click();
    settle(&mut harness, 3);

    assert!(harness.state().session.pkg.graph_by_name("Tools/Helper").is_some());

    harness.get(tree("Math")).click_secondary();
    settle(&mut harness, 2);
    harness.get(By::new().label("New")).click();
    settle(&mut harness, 3);
    for label in ["Graph", "Function Graph", "Folder"] {
        assert!(harness.query(button(label)).is_some());
    }
    save(&mut harness, &directory, "explorer-new-submenu");
    harness.get(button("Function Graph")).click();
    settle(&mut harness, 3);

    assert!(!harness.state().ui.windows.new_graph);
    assert!(harness.state().ui.explorer.rename.is_none());
    assert_eq!(harness.state().session.graph().unwrap().folder, "Tools/Math");
    assert_eq!(harness.state().session.graph().unwrap().name, "Untitled_Function_Graph_1");
    assert!(harness.state().session.graph().unwrap().function.is_some());

    for (label, expected) in [
        ("Function Graph", "Untitled_Function_Graph_2"),
        ("Graph", "Untitled_Graph_1"),
        ("Graph", "Untitled_Graph_2"),
        ("Folder", "Untitled_Folder_1"),
        ("Folder", "Untitled_Folder_2"),
    ] {
        harness.get(tree("Math")).click_secondary();
        settle(&mut harness, 2);
        harness.get(By::new().label("New")).click();
        settle(&mut harness, 2);
        harness.get(button(label)).click();
        settle(&mut harness, 3);

        assert!(!harness.state().ui.windows.new_graph);
        assert!(harness.state().ui.explorer.rename.is_none());

        let path = format!("Tools/Math/{expected}");
        if label == "Folder" {
            assert!(harness.state().session.pkg.folder_paths().contains(&path));
        } else {
            assert!(harness.state().session.pkg.graph_by_name(&path).is_some());
        }
    }
    save(&mut harness, &directory, "explorer-create-without-prompts");
}
