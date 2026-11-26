use materialcraft_engine::{Session, core::Package, execute, file_cmds};
use serde_json::json;

const SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="red"/></svg>"#;

#[test]
fn imports_remain_undoable_in_packages_with_many_resources() {
    use materialcraft_engine::core::{Resource, ResourceBytes};
    let mut package = Package::new("Resources");
    let bytes = ResourceBytes::new(SVG.to_vec());
    for index in 0..4096 {
        package.resources.push(Resource::new(format!("Shape_{index}"), "shape.svg", bytes.clone()));
    }
    let mut session = Session::new(package);

    file_cmds::add_resource(&mut session, "new.svg", SVG.to_vec(), Some("New")).unwrap();

    assert_eq!(session.pkg.resources.len(), 4097);
    assert_eq!(session.pkg.resource("New").unwrap().bytes, bytes);

    execute(&mut session, "edit.undo", &json!({})).unwrap();

    assert_eq!(session.pkg.resources.len(), 4096);
    assert!(session.pkg.resource("New").is_none());
}

#[test]
fn resource_nodes_use_the_source_package_and_undo_as_one_edit() {
    use materialcraft_engine::core::Value;
    let mut session = Session::new(Package::new("Source"));
    execute(&mut session, "file.new", &json!({"template": "empty"})).unwrap();
    file_cmds::add_resource(&mut session, "mask.svg", SVG.to_vec(), Some("Mask")).unwrap();
    let source = session.document_id;
    execute(&mut session, "graph.resource.add", &json!({"resource": "Mask", "at": [120, 80]})).unwrap();

    assert_eq!(session.pkg.resources.len(), 1);
    assert_eq!(session.graph().unwrap().nodes[0].pos, [120.0, 80.0]);

    execute(&mut session, "edit.undo", &json!({})).unwrap();

    assert!(session.graph().unwrap().nodes.is_empty());

    execute(&mut session, "file.newPackage", &json!({"name": "Destination"})).unwrap();
    execute(&mut session, "file.new", &json!({"template": "empty"})).unwrap();
    let destination = session.document_id;
    file_cmds::add_resource(&mut session, "other.svg", SVG.to_vec(), Some("Mask")).unwrap();
    execute(&mut session, "graph.resource.add", &json!({"document": source, "resource": "Mask"})).unwrap();

    assert_eq!(session.document_id, destination);
    let node = &session.graph().unwrap().nodes[0];
    assert_eq!(node.def, "svg");
    assert_eq!(node.params["resource"], Value::Text("Mask_1".into()));
    assert_eq!(session.pkg.resource("Mask_1").unwrap().bytes.as_slice(), SVG);

    execute(&mut session, "edit.undo", &json!({})).unwrap();

    assert_eq!(session.pkg.resources.len(), 1);
    assert!(session.graph().unwrap().nodes.is_empty());
    assert_eq!(session.package(source).unwrap().resources.len(), 1);

    let before = session.pkg.clone();
    for parameters in [json!({"resource": "missing"}), json!({"document": u64::MAX, "resource": "Mask"})] {
        assert!(execute(&mut session, "graph.resource.add", &parameters).is_err());
        assert_eq!(session.pkg, before);
    }
}

#[test]
fn asset_subfolders_and_attributes_persist_in_both_project_formats() {
    let root = std::env::temp_dir().join(format!("materialcraft-resource-folders-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut session = Session::new(Package::new("Folders"));
    execute(&mut session, "file.new", &json!({"template": "empty"})).unwrap();
    execute(&mut session, "folder.create", &json!({"parent": "Resources", "name": "Masks"})).unwrap();
    execute(&mut session, "folder.create", &json!({"parent": "Resources/Masks", "name": "Empty"})).unwrap();
    execute(&mut session, "folder.attributes", &json!({"folder": "Resources", "description": "Imported assets"})).unwrap();
    let description = "Masks & <shapes>\nSecond line: café";
    execute(&mut session, "folder.attributes", &json!({"folder": "Resources/Masks", "description": description})).unwrap();
    file_cmds::add_resource_into(&mut session, "mask.svg", SVG.to_vec(), Some("Mask"), "Resources/Masks").unwrap();
    execute(&mut session, "graph.resource.add", &json!({"resource": "Mask"})).unwrap();
    execute(&mut session, "folder.attributes", &json!({"folder": "Resources/Masks", "identifier": "Shapes"})).unwrap();

    assert!(!session.pkg.folder_descriptions.contains_key("Resources/Masks"));
    assert_eq!(session.pkg.resource("Mask").unwrap().folder, "Resources/Shapes");
    assert_eq!(session.pkg.folder_descriptions["Resources/Shapes"], description);

    execute(&mut session, "explorer.move", &json!({"resource": "Mask", "parent": "Resources/Shapes/Empty"})).unwrap();
    execute(&mut session, "edit.undo", &json!({})).unwrap();

    assert_eq!(session.pkg.resource("Mask").unwrap().folder, "Resources/Shapes");

    for extension in ["sbs", "mcraft"] {
        let path = root.join(format!("folders.{extension}"));
        execute(&mut session, "file.saveAs", &json!({"path": path})).unwrap();
        let mut reopened = Session::new(Package::new("Empty"));
        file_cmds::open_file(&mut reopened, path.to_str().unwrap()).unwrap();

        assert_eq!(reopened.pkg.folder_descriptions, session.pkg.folder_descriptions);
        assert!(reopened.pkg.folder_paths().contains("Resources/Shapes/Empty"));
        assert_eq!(reopened.pkg.resource("Mask").unwrap().folder, "Resources/Shapes");
        assert_eq!(reopened.pkg.resource("Mask").unwrap().bytes.as_slice(), SVG);
    }
    execute(&mut session, "folder.remove", &json!({"folder": "Resources/Shapes"})).unwrap();

    assert!(!session.pkg.folder_descriptions.contains_key("Resources/Shapes"));
    assert!(session.pkg.resources.is_empty());

    execute(&mut session, "edit.undo", &json!({})).unwrap();

    assert_eq!(session.pkg.folder_descriptions["Resources/Shapes"], description);

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn resources_root_is_unique_and_its_subtree_stays_asset_only() {
    let mut session = Session::new(Package::new("Reserved"));
    execute(&mut session, "file.new", &json!({"template": "empty"})).unwrap();
    execute(&mut session, "folder.create", &json!({"name": "Tools"})).unwrap();
    execute(&mut session, "folder.create", &json!({"parent": "Resources", "name": "Images"})).unwrap();
    for name in ["Resources", "resources", "RESOURCES"] {
        for (command, parameters) in [
            ("folder.create", json!({"name": name})),
            ("folder.rename", json!({"folder": "Tools", "name": name})),
            ("folder.attributes", json!({"folder": "Tools", "identifier": name})),
        ] {
            assert!(execute(&mut session, command, &parameters).is_err(), "{command}: {parameters}");
        }
    }
    for (command, parameters) in [
        ("file.new", json!({"folder": "Resources/Images"})),
        ("explorer.move", json!({"graph": 0, "parent": "Resources/Images"})),
        ("explorer.move", json!({"folder": "Tools", "parent": "Resources"})),
        ("explorer.move", json!({"folder": "Resources/Images", "parent": ""})),
        ("folder.attributes", json!({"folder": "Resources", "identifier": "Assets"})),
    ] {
        assert!(execute(&mut session, command, &parameters).is_err(), "{command}: {parameters}");
    }

    assert!(file_cmds::add_resource_into(&mut session, "mask.svg", SVG.to_vec(), None, "Tools").is_err());
    assert_eq!(file_cmds::add_resource(&mut session, "Images.svg", SVG.to_vec(), None).unwrap(), "Images_1");
}

#[test]
fn folder_attributes_target_inactive_packages_without_switching_the_canvas() {
    let mut session = Session::new(Package::new("Source"));
    execute(&mut session, "folder.create", &json!({"name": "Tools"})).unwrap();
    let source = session.document_id;
    execute(&mut session, "file.newPackage", &json!({"name": "Active"})).unwrap();
    let active = session.document_id;
    execute(&mut session, "folder.attributes", &json!({"document": source, "folder": "Tools", "identifier": "Helpers", "description": "Utility graphs"}))
        .unwrap();

    assert_eq!(session.document_id, active);

    let properties = execute(&mut session, "folder.properties", &json!({"document": source, "folder": "Helpers"})).unwrap();

    assert_eq!(properties["description"], "Utility graphs");

    execute(&mut session, "edit.undo", &json!({"document": source})).unwrap();

    assert!(session.package(source).unwrap().folder_paths().contains("Tools"));
    assert!(session.package(source).unwrap().folder_descriptions.is_empty());
    assert_eq!(session.document_id, active);
}

#[test]
fn resources_is_reserved_for_assets() {
    let mut session = Session::new(Package::new("Resources test"));

    assert!(session.pkg.folder_paths().contains("Resources"));

    for (command, params) in [
        ("file.new", json!({"folder": "Resources"})),
        ("folder.create", json!({"name": "Resources"})),
        ("folder.rename", json!({"folder": "Resources", "name": "Other"})),
        ("folder.remove", json!({"folder": "Resources"})),
        ("explorer.move", json!({"folder": "Resources", "parent": ""})),
        ("explorer.copy", json!({"folder": "Resources"})),
    ] {
        assert!(execute(&mut session, command, &params).is_err(), "{command}: {params}");
    }
    execute(&mut session, "file.new", &json!({"template": "empty"})).unwrap();
    execute(&mut session, "folder.create", &json!({"name": "Other"})).unwrap();
    execute(&mut session, "explorer.copy", &json!({"graph": 0})).unwrap();
    for (command, params) in [
        ("folder.rename", json!({"folder": "Other", "name": "Resources"})),
        ("explorer.move", json!({"graph": 0, "parent": "Resources"})),
        ("explorer.paste", json!({"folder": "Resources"})),
    ] {
        assert!(execute(&mut session, command, &params).is_err());
    }
}

#[test]
fn unsaved_resources_save_independently_as_sbs_and_mcraft() {
    let root = std::env::temp_dir().join(format!("materialcraft-resources-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut session = Session::new(Package::new("Wood"));
    file_cmds::add_resource(&mut session, "mask.svg", SVG.to_vec(), Some("Mask")).unwrap();

    assert!(session.path.is_none());
    assert!(session.modified);

    execute(&mut session, "edit.undo", &json!({})).unwrap();

    assert!(session.pkg.resources.is_empty());

    execute(&mut session, "edit.redo", &json!({})).unwrap();

    for extension in ["sbs", "mcraft"] {
        let directory = root.join(extension);
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join(format!("wood.{extension}"));
        execute(&mut session, "file.saveAs", &json!({"path": path})).unwrap();

        assert!(path.is_file(), "save must honor the requested format");

        if extension == "sbs" {
            let xml = std::fs::read_to_string(&path).unwrap();

            assert!(xml.contains("resources/"));
            assert!(!xml.contains("binembedded"));

            let files: Vec<_> = std::fs::read_dir(directory.join("resources")).unwrap().collect();

            assert_eq!(files.len(), 1);
            assert_eq!(std::fs::read(files[0].as_ref().unwrap().path()).unwrap(), SVG);
        } else {
            assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1, "an archive needs no external SBS or resources");
        }
        let mut reopened = Session::new(Package::new("Other"));
        file_cmds::open_file(&mut reopened, path.to_str().unwrap()).unwrap();

        assert_eq!(reopened.pkg.resources.len(), 1);
        assert_eq!(reopened.pkg.resources[0].bytes.as_slice(), SVG);

        execute(&mut reopened, "file.save", &json!({})).unwrap();
        if extension == "sbs" {
            assert_eq!(std::fs::read_dir(directory.join("resources")).unwrap().count(), 1, "repeat saves reuse resource names");
        }
        execute(&mut reopened, "folder.create", &json!({"name": "Unsaved"})).unwrap();
        let copy = directory.join(format!("copy.{extension}"));
        let saved_path = reopened.path.clone();
        execute(&mut reopened, "file.saveCopyAs", &json!({"path": copy})).unwrap();

        assert_eq!(reopened.path, saved_path);
        assert!(reopened.modified);

        execute(&mut reopened, "file.reload", &json!({"discard": true})).unwrap();

        assert!(!reopened.pkg.folder_paths().contains("Unsaved"));
        assert_eq!(reopened.pkg.resources[0].bytes.as_slice(), SVG);
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn vector_import_is_undoable_and_does_not_require_a_graph_or_a_saved_package() {
    let root = std::env::temp_dir().join(format!("materialcraft-vector-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("shape.SVG");
    std::fs::write(&path, SVG).unwrap();
    let mut session = Session::new(Package::new("Unsaved"));

    assert!(execute(&mut session, "file.importBitmap", &json!({"path": path})).is_err());

    execute(&mut session, "file.importVector", &json!({"path": path})).unwrap();

    assert!(session.pkg.graphs.is_empty());
    assert!(session.path.is_none());
    assert_eq!(session.pkg.resources[0].folder, "Resources");
    assert_eq!(session.pkg.resources[0].bytes.as_slice(), SVG);
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1, "unsaved import creates no sidecars");

    execute(&mut session, "edit.undo", &json!({})).unwrap();

    assert!(session.pkg.resources.is_empty());

    for extension in ["psd", "psb", "surface", "raw"] {
        assert!(file_cmds::add_resource(&mut session, &format!("shape.{extension}"), SVG.to_vec(), None).is_err());
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn failed_save_keeps_the_document_and_existing_sbs_intact() {
    let root = std::env::temp_dir().join(format!("materialcraft-failed-save-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("safe.sbs");
    let mut session = Session::new(Package::new("Safe"));
    execute(&mut session, "file.save", &json!({"path": path})).unwrap();
    let original = std::fs::read(&path).unwrap();
    file_cmds::add_resource(&mut session, "shape.svg", SVG.to_vec(), None).unwrap();
    let files = file_cmds::save_files_at(&session, path.to_str().unwrap()).unwrap();
    let resource = files.files.keys().find(|name| name.starts_with("resources/")).unwrap();
    std::fs::write(root.join(resource), b"different existing file").unwrap();

    assert!(execute(&mut session, "file.save", &json!({})).is_err());
    assert!(session.modified);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(std::fs::read(root.join(resource)).unwrap(), b"different existing file");

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn reload_refreshes_assets_in_archived_dependencies() {
    use materialcraft_engine::{
        core::{Graph, GraphInstance, Resource, ResourceBytes},
        formats::project::ProjectFiles,
    };
    let root = std::env::temp_dir().join(format!("materialcraft-reload-archive-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("linked.mcraft");
    let mut source = Package::new("Source");
    source.graphs.push(Graph::new("Material"));
    source.resources.push(Resource::new("Mask", "mask.svg", ResourceBytes::new(SVG.to_vec())));
    let mut package = Package::new("Consumer");
    let mut graph = Graph::new("Graph");
    let id = graph.add_node("graph_instance", [0.0; 2]);
    graph.node_mut(id).unwrap().instance = Some(GraphInstance {
        label: String::new(),
        package: "/source/material.sbs".into(),
        graph: "Material".into(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        defaults: Default::default(),
    });
    package.graphs.push(graph);
    package.dependencies.insert("/source/material.sbs".into(), std::sync::Arc::new(source));
    let mut files = ProjectFiles::build(&package, "linked.mcraft", true).unwrap();
    std::fs::write(&path, files.zip().unwrap()).unwrap();
    let mut session = Session::new(Package::new("Empty"));
    file_cmds::open_file(&mut session, path.to_str().unwrap()).unwrap();
    let reference = session.pkg.graphs[0].nodes[0].instance.as_ref().unwrap().package.clone();

    assert_eq!(session.pkg.dependencies[&reference].resources[0].bytes.as_slice(), SVG);

    // External edits can replace an asset without changing its path inside the archive
    let changed = std::str::from_utf8(SVG).unwrap().replace("red", "blue").into_bytes();
    let resource = files.files.values_mut().find(|bytes| bytes.as_slice() == SVG).unwrap();
    *resource = ResourceBytes::new(changed.clone());
    std::fs::write(&path, files.zip().unwrap()).unwrap();
    execute(&mut session, "file.reload", &json!({})).unwrap();

    assert_eq!(session.pkg.dependencies[&reference].resources[0].bytes.as_slice(), changed);
    assert!(!session.modified);

    std::fs::remove_dir_all(root).unwrap();
}
