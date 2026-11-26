use materialcraft_engine::{Session, core::Package, execute};
use serde_json::{Value, json};

fn run(session: &mut Session, command: &str, parameters: Value) -> Value {
    execute(session, command, &parameters).unwrap()
}

#[test]
fn folders_contain_both_graph_kinds_and_removal_is_undoable() {
    let mut session = Session::new(Package::new("Explorer"));
    run(&mut session, "folder.create", json!({"name": "Tools"}));
    run(&mut session, "folder.create", json!({"parent": "Tools", "name": "Math"}));
    run(&mut session, "file.new", json!({"folder": "Tools", "name": "Surface", "template": "empty"}));
    run(&mut session, "file.new", json!({"folder": "Tools/Math", "name": "Helper", "kind": "function"}));

    assert!(session.pkg.graph_by_name("Tools/Surface").is_some());
    assert!(session.pkg.graph_by_name("Tools/Math/Helper").is_some());
    let before = session.pkg.clone();
    run(&mut session, "folder.remove", json!({"folder": "Tools"}));
    assert!(session.pkg.graphs.is_empty());

    run(&mut session, "edit.undo", json!({}));
    assert_eq!(session.pkg, before);
}

#[test]
fn numbered_explorer_names_increment_across_conflicts() {
    let mut session = Session::new(Package::new("Names"));
    run(&mut session, "folder.create", json!({"name": "Tools"}));
    run(&mut session, "folder.create", json!({"parent": "Tools", "name": "Untitled_Graph_2"}));

    for (kind, prefix) in [("material", "Untitled_Graph"), ("function", "Untitled_Function_Graph")] {
        for expected in if kind == "material" { [1, 3] } else { [1, 2] } {
            let result = run(&mut session, "file.new", json!({"folder": "Tools", "name": format!("{prefix}_1"), "kind": kind, "template": "empty"}));
            assert_eq!(result["name"], format!("{prefix}_{expected}"));
        }
    }
    for expected in [1, 2] {
        let result = run(&mut session, "folder.create", json!({"parent": "Tools", "name": "Untitled_Folder_1"}));
        assert_eq!(result["folder"], format!("Tools/Untitled_Folder_{expected}"));
    }
    let result = run(&mut session, "folder.create", json!({"name": "Untitled_Folder_1"}));
    assert_eq!(result["folder"], "Untitled_Folder_1");
}

#[test]
fn graph_duplication_uses_the_destination_item_namespace() {
    use materialcraft_engine::core::{Graph, Resource, ResourceBytes};
    let mut package = Package::new("Names");
    let mut graph = Graph::new("Surface");
    graph.folder = "Tools".into();
    package.graphs.push(graph);
    package.folders.push("Tools/Surface_1".into());
    let mut resource = Resource::new("Surface_2", "tile.png", ResourceBytes::new(vec![1]));
    resource.folder = "Tools".into();
    package.resources.push(resource);
    package.graphs.push(Graph::new("Surface_3"));
    let mut session = Session::new(package);

    run(&mut session, "graph.duplicate", json!({"graph": 0}));

    assert_eq!(session.graph().unwrap().sbs_path(), "Tools/Surface_3");
    assert!(!session.pkg.folder_paths().contains(&session.graph().unwrap().sbs_path()));
}

#[test]
fn graph_from_selection_avoids_folder_and_resource_names() {
    use materialcraft_engine::core::{Graph, Resource, ResourceBytes};
    let mut package = Package::new("Names");
    let mut graph = Graph::new("Canvas");
    let node = graph.add_node("output", [0.0, 0.0]);
    package.graphs.push(graph);
    package.folders.push("Sub_graph".into());
    let mut resource = Resource::new("Sub_graph_1", "tile.png", ResourceBytes::new(vec![1]));
    resource.folder.clear();
    package.resources.push(resource);
    let mut session = Session::new(package);
    session.selection = vec![node];

    run(&mut session, "graph.createFromSelection", json!({}));

    assert!(session.pkg.graph_by_name("Sub_graph_2").is_some());
    assert!(!session.pkg.graphs.iter().any(|graph| session.pkg.folder_paths().contains(&graph.sbs_path())));
}

#[test]
fn explorer_moves_and_renames_cannot_collide_with_resources() {
    use materialcraft_engine::core::{Resource, ResourceBytes};
    let mut session = Session::new(Package::new("Names"));
    run(&mut session, "folder.create", json!({"name": "Folder"}));
    run(&mut session, "file.new", json!({"name": "Graph", "template": "empty"}));
    for (folder, name) in [("", "Taken"), ("Folder", "Graph")] {
        let mut resource = Resource::new(name, "tile.png", ResourceBytes::new(vec![1]));
        resource.folder = folder.into();
        session.pkg.resources.push(resource);
    }
    let before = materialcraft_engine::file_cmds::save_bytes(&session).unwrap();

    assert!(execute(&mut session, "folder.rename", &json!({"folder": "Folder", "name": "Taken"})).is_err());
    assert!(execute(&mut session, "graph.rename", &json!({"graph": 0, "name": "Taken"})).is_err());
    assert!(execute(&mut session, "explorer.move", &json!({"graph": 0, "parent": "Folder"})).is_err());
    assert_eq!(materialcraft_engine::file_cmds::save_bytes(&session).unwrap(), before);
}

#[test]
fn package_paste_places_root_resources_in_the_destination_folder() {
    use materialcraft_engine::core::{Resource, ResourceBytes};
    let mut session = Session::new(Package::new("Copy"));
    let mut resource = Resource::new("Tile", "tile.png", ResourceBytes::new(vec![1]));
    resource.folder = String::new();
    session.pkg.resources.push(resource);
    run(&mut session, "explorer.copy", json!({}));
    run(&mut session, "file.newPackage", json!({}));
    run(&mut session, "folder.create", json!({"name": "Destination"}));
    run(&mut session, "file.new", json!({"folder": "Destination", "name": "Tile", "template": "empty"}));

    run(&mut session, "explorer.paste", json!({"folder": "Destination"}));

    assert_eq!(session.pkg.resources[0].sbs_path(), "Destination/Tile_1");
}

fn linked_functions() -> Session {
    use materialcraft_engine::core::{FunctionGraph, Graph, Value as GraphValue};
    let mut package = Package::new("Linked functions");
    let mut source = Graph::new("Source");
    source.folder = "Tools/Math".into();
    source.function = Some(FunctionGraph::constant(GraphValue::Float(0.5)));
    package.graphs.push(source);
    let mut session = Session::new(package);
    run(&mut session, "file.new", json!({"kind": "function", "name": "Caller", "folder": "Tools"}));
    run(&mut session, "graph.instance.add", json!({"document": 0, "graph": "Tools/Math/Source"}));
    session
}

#[test]
fn renamed_and_moved_folders_rewrite_nested_function_references() {
    let mut session = linked_functions();
    run(&mut session, "folder.rename", json!({"folder": "Tools", "name": "Helpers"}));
    let caller = &session.pkg.graphs[1].function.as_ref().unwrap().nodes[0];
    assert_eq!(caller.instance.as_ref().unwrap().graph, "Helpers/Math/Source");
    assert!(matches!(caller.source, Some(Ok(_))));

    run(&mut session, "explorer.move", json!({"folder": "Helpers/Math", "parent": ""}));
    assert_eq!(session.pkg.graphs[0].folder, "Math");
    let caller = &session.pkg.graphs[1].function.as_ref().unwrap().nodes[0];
    assert_eq!(caller.instance.as_ref().unwrap().graph, "Math/Source");
    assert!(matches!(caller.source, Some(Ok(_))));
    assert!(session.pkg.folder_paths().contains("Helpers"));

    let before = session.pkg.clone();
    assert!(execute(&mut session, "explorer.move", &json!({"folder": "Math", "parent": "Math"})).is_err());
    assert_eq!(session.pkg, before);
}

#[test]
fn folder_copy_keeps_empty_descendants_and_retargets_its_internal_instances() {
    let mut session = linked_functions();
    run(&mut session, "folder.create", json!({"parent": "Tools", "name": "Empty"}));
    run(&mut session, "explorer.copy", json!({"folder": "Tools"}));
    let before = session.pkg.clone();

    run(&mut session, "explorer.paste", json!({}));

    assert!(session.pkg.folder_paths().contains("Tools_1/Empty"));
    let (_, caller) = session.pkg.graph_by_name("Tools_1/Caller").unwrap();
    let node = &caller.function.as_ref().unwrap().nodes[0];
    assert_eq!(node.instance.as_ref().unwrap().graph, "Tools_1/Math/Source");
    assert!(matches!(node.source, Some(Ok(_))));
    run(&mut session, "edit.undo", json!({}));
    assert_eq!(session.pkg, before);
}

#[test]
fn explorer_clipboard_survives_document_switch_and_checks_external_references() {
    let mut session = linked_functions();
    run(&mut session, "explorer.copy", json!({"graph": 1}));
    run(&mut session, "file.newPackage", json!({"name": "Destination"}));
    let destination = session.document_id;
    let before = session.pkg.clone();

    assert!(execute(&mut session, "explorer.paste", &json!({})).is_err());
    assert_eq!(session.pkg, before);
    run(&mut session, "package.activate", json!({"document": 0}));
    run(&mut session, "explorer.copy", json!({"folder": "Tools"}));
    run(&mut session, "package.activate", json!({"document": destination}));
    run(&mut session, "explorer.paste", json!({}));

    let (_, caller) = session.pkg.graph_by_name("Tools/Caller").unwrap();
    assert!(matches!(caller.function.as_ref().unwrap().nodes[0].source, Some(Ok(_))));
}

#[test]
fn folder_removal_does_not_remove_similarly_named_siblings() {
    let mut session = Session::new(Package::new("Siblings"));
    for folder in ["A", "AB"] {
        run(&mut session, "folder.create", json!({"name": folder}));
        run(&mut session, "file.new", json!({"folder": folder, "name": "Graph", "template": "empty"}));
    }
    run(&mut session, "folder.remove", json!({"folder": "A"}));

    assert_eq!(session.pkg.graphs.len(), 1);
    assert_eq!(session.pkg.graphs[0].sbs_path(), "AB/Graph");
    assert!(session.pkg.folder_paths().contains("AB"));
}

#[test]
fn save_copy_keeps_document_identity_and_reload_is_atomic() {
    let directory = std::env::temp_dir().join(format!("materialcraft-explorer-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let original = directory.join("original.sbs");
    let copy = directory.join("copy.sbs");
    let mut session = linked_functions();
    for path in ["", "   "] {
        assert!(execute(&mut session, "file.saveCopyAs", &json!({"path": path})).is_err());
    }

    run(&mut session, "file.save", json!({"path": original}));
    let document = session.document_id;
    let path = session.path.clone();
    run(&mut session, "folder.create", json!({"name": "Unsaved"}));
    let history = session.history().len();

    run(&mut session, "file.saveCopyAs", json!({"path": copy}));
    assert_eq!(session.path, path);
    assert!(session.modified);
    assert_eq!(session.history().len(), history);
    let copied = materialcraft_engine::formats::sbs::read_sbs(&std::fs::read(&copy).unwrap(), "copy.sbs", &|_| None).unwrap();
    assert!(copied.pkg.folder_paths().contains("Unsaved"));
    assert!(execute(&mut session, "file.reload", &json!({})).is_err());

    run(&mut session, "file.reload", json!({"discard": true}));
    assert_eq!(session.document_id, document);
    assert!(!session.modified);
    assert!(!session.pkg.folder_paths().contains("Unsaved"));
    let before = session.pkg.clone();
    std::fs::write(&original, "broken package").unwrap();
    assert!(execute(&mut session, "file.reload", &json!({})).is_err());
    assert_eq!(session.pkg, before);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn folder_names_and_depth_are_bounded() {
    let mut session = Session::new(Package::new("Validation"));
    run(&mut session, "file.new", json!({"name": " Surface ", "template": "empty"}));
    assert_eq!(session.graph().unwrap().name, "Surface");

    for name in ["", "../escape", "A/B", "A\\B", ".", "\0"] {
        assert!(execute(&mut session, "folder.create", &json!({"name": name})).is_err(), "{name:?}");
    }
    let path = std::iter::repeat_n("A", 64).collect::<Vec<_>>().join("/");
    session.pkg.folders.push(path.clone());
    assert!(execute(&mut session, "folder.create", &json!({"parent": path})).is_err());
}

#[test]
fn copied_folders_keep_unreferenced_resources_in_the_copied_subtree() {
    use materialcraft_engine::core::{Resource, ResourceBytes};
    let mut package = Package::new("Resources");
    let mut resource = Resource::new("Mask", "mask.png", ResourceBytes::new(vec![1, 2, 3]));
    resource.folder = "Tools/Images".into();
    package.resources.push(resource);
    let mut session = Session::new(package);
    run(&mut session, "explorer.copy", json!({"folder": "Tools"}));

    run(&mut session, "explorer.paste", json!({}));
    assert_eq!(session.pkg.resources.len(), 2);
    assert_eq!(session.pkg.resources[1].folder, "Tools_1/Images");
    assert_eq!(session.pkg.resources[1].bytes.as_slice(), &[1, 2, 3]);
    run(&mut session, "folder.rename", json!({"folder": "Tools_1", "name": "Copy"}));
    assert_eq!(session.pkg.resources[1].folder, "Copy/Images");
    run(&mut session, "folder.remove", json!({"folder": "Copy"}));
    assert_eq!(session.pkg.resources.len(), 1);
}
