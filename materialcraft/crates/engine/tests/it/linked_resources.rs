use materialcraft_engine::{
    Session,
    core::{FunctionGraph, Graph, Package, Resource, ResourceBytes, Value},
    execute, file_cmds,
    formats::sbs,
};
use serde_json::json;

const TILE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="red"/></svg>"#;

fn resource_package(source: &str) -> Package {
    let mut package = Package::new("Resource resolution");
    let mut graph = Graph::new("Constant");
    graph.function = Some(FunctionGraph::constant(Value::Float(0.5)));
    package.graphs.push(graph);
    let mut resource = Resource::new("Tile", "tile.svg", ResourceBytes::new(Vec::new()));
    resource.source = source.into();
    package.resources.push(resource);
    package
}

#[test]
fn library_resources_resolve_next_to_their_package() {
    const CHILD_ROOT: &str = "MC_LINKED_RESOURCES_TEST_ROOT";
    if let Ok(root) = std::env::var(CHILD_ROOT) {
        verify_library_resources(std::path::Path::new(&root));
        return;
    }

    // A child process isolates the library root without mutating the test runner's environment
    let root = std::env::temp_dir().join(format!("materialcraft-linked-resources-{}", std::process::id()));
    std::fs::create_dir_all(root.join("assets/sbs/resources")).unwrap();
    std::fs::create_dir_all(root.join("assets/sbs/nested/resources")).unwrap();
    std::fs::write(root.join("assets/sbs/resources/tile.svg"), TILE).unwrap();
    std::fs::write(root.join("assets/sbs/nested/resources/tile.svg"), TILE).unwrap();
    let package = sbs::write_sbs(&resource_package("resources/tile.svg")).unwrap();
    std::fs::write(root.join("assets/sbs/functions.sbs"), &package).unwrap();
    std::fs::write(root.join("assets/sbs/nested/source.sbs"), &package).unwrap();

    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "linked_resources::library_resources_resolve_next_to_their_package", "--nocapture"])
        .env(CHILD_ROOT, &root)
        .env("MATERIALCRAFT_SBS_DIR", root.join("assets/sbs"))
        .status()
        .unwrap();
    std::fs::remove_dir_all(root).unwrap();

    assert!(status.success());
}

fn verify_library_resources(root: &std::path::Path) {
    let mut session = Session::new(Package::new("Caller"));
    let library = session.pkg.dependencies.get("sbs://functions.sbs").unwrap();
    assert_eq!(library.resources[0].bytes.as_slice(), TILE);

    file_cmds::open_file(&mut session, "sbs://nested/source.sbs").unwrap();
    assert!(session.load_warnings.is_empty(), "{:?}", session.load_warnings);
    assert_eq!(session.pkg.resources[0].bytes.as_slice(), TILE);
    execute(&mut session, "file.reload", &json!({})).unwrap();
    assert_eq!(session.pkg.resources[0].bytes.as_slice(), TILE);

    let read = file_cmds::linked_files("sbs://nested/source.sbs");
    assert_eq!(read("resources/tile.svg").unwrap(), TILE);
    assert_eq!(read("sbs://resources/tile.svg").unwrap(), TILE);
    assert_eq!(read("../resources/tile.svg").unwrap(), TILE);
    assert!(read("../../private.svg").is_none());
    assert!(read("resources/absent.svg").is_none());

    // Saving embeds loaded resources so a relocated package remains self-contained
    let saved = file_cmds::save_bytes(&session).unwrap();
    let reopened = sbs::read_sbs(&saved, "elsewhere/copy.sbs", &|_| None).unwrap();
    assert!(reopened.warnings.is_empty(), "{:?}", reopened.warnings);
    assert_eq!(reopened.pkg.resources[0].bytes.as_slice(), TILE);

    let source_document = session.document_id;
    execute(&mut session, "file.newPackage", &json!({})).unwrap();
    execute(&mut session, "file.new", &json!({"kind": "function"})).unwrap();
    execute(&mut session, "graph.instance.add", &json!({"document": source_document, "graph": "Constant"})).unwrap();
    let caller = file_cmds::save_bytes(&session).unwrap();
    let mut dependent = Session::new(Package::new("Dependent"));
    file_cmds::open_bytes(&mut dependent, &caller, Some(root.join("dependent.sbs").to_string_lossy().into_owned())).unwrap();
    assert_eq!(dependent.pkg.dependencies["sbs://nested/source.sbs"].resources[0].bytes.as_slice(), TILE);

    let missing = sbs::write_sbs(&resource_package("resources/absent.svg")).unwrap();
    file_cmds::open_bytes_linked(&mut session, &missing, Some("sbs://nested/missing.sbs".into()), &read).unwrap();
    assert!(session.pkg.resources[0].bytes.is_empty());
    assert_eq!(session.pkg.resources[0].source, "sbs://nested/resources/absent.svg");
    assert!(!session.load_warnings.is_empty());

    let local = root.join("caller.sbs");
    std::fs::write(&local, sbs::write_sbs(&resource_package("sbs://resources/tile.svg")).unwrap()).unwrap();
    file_cmds::open_file(&mut session, local.to_str().unwrap()).unwrap();
    assert_eq!(session.pkg.resources[0].bytes.as_slice(), TILE);
}
