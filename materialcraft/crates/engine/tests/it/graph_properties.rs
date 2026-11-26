use materialcraft_engine::{
    Session,
    core::{Graph, Package},
    execute,
};
use serde_json::json;

#[path = "graph_property/history_preferences.rs"]
mod history_preferences;

#[test]
fn inactive_seed_gestures_merge_and_finish_independently() {
    let mut package = Package::new("Source");
    package.graphs.push(Graph::new("Inspected"));
    let mut session = Session::new(package);
    execute(&mut session, "file.newPackage", &json!({"name": "Canvas"})).unwrap();
    let active = session.document_id;
    let before = session.pkg.clone();

    for seed in [1, 2, 3] {
        execute(&mut session, "graph.seed", &json!({"document": 0, "graph": 0, "seed": seed})).unwrap();
    }
    session.finish_edit();
    for seed in [4, 5] {
        execute(&mut session, "graph.seed", &json!({"document": 0, "graph": 0, "seed": seed})).unwrap();
    }
    execute(&mut session, "edit.undo", &json!({"document": 0})).unwrap();

    let target = materialcraft_engine::GraphTarget { document: 0, graph: 0 };
    assert_eq!(target.get(&session).unwrap().seed, 3);
    execute(&mut session, "edit.undo", &json!({"document": 0})).unwrap();
    assert_eq!(target.get(&session).unwrap().seed, 0);
    execute(&mut session, "edit.redo", &json!({"document": 0})).unwrap();
    assert_eq!(target.get(&session).unwrap().seed, 3);
    execute(&mut session, "edit.redo", &json!({"document": 0})).unwrap();
    assert_eq!(target.get(&session).unwrap().seed, 5);
    assert_eq!(session.document_id, active);
    assert_eq!(session.pkg, before);

    execute(&mut session, "package.activate", &json!({"document": 0})).unwrap();
    execute(&mut session, "edit.undo", &json!({})).unwrap();
    assert_eq!(session.graph().unwrap().seed, 3);
    execute(&mut session, "edit.undo", &json!({})).unwrap();
    assert_eq!(session.graph().unwrap().seed, 0);
}

#[test]
fn history_commands_default_to_the_canvas_and_reject_invalid_targets() {
    let mut package = Package::new("Source");
    package.graphs.push(Graph::new("Inspected"));
    let mut session = Session::new(package);
    execute(&mut session, "file.newPackage", &json!({"name": "Canvas"})).unwrap();
    execute(&mut session, "file.new", &json!({"template": "empty"})).unwrap();
    session.reset_history();
    execute(&mut session, "graph.seed", &json!({"seed": 8})).unwrap();
    execute(&mut session, "graph.attributes", &json!({"document": 0, "graph": 0, "label": "Source label"})).unwrap();
    let source = materialcraft_engine::GraphTarget { document: 0, graph: 0 };
    let canvas = session.pkg.clone();

    for document in [json!(-1), json!("0"), json!(u64::MAX)] {
        assert!(execute(&mut session, "edit.undo", &json!({"document": document})).is_err());
    }
    assert_eq!(session.pkg, canvas);
    assert_eq!(source.get(&session).unwrap().attributes.label, "Source label");
    execute(&mut session, "edit.undo", &json!({})).unwrap();

    assert_eq!(session.graph().unwrap().seed, 0);
    assert_eq!(source.get(&session).unwrap().attributes.label, "Source label");
    assert!(session.can_undo_document(0));
    assert!(!session.can_undo());
    assert!(session.can_redo());
    execute(&mut session, "edit.redo", &json!({})).unwrap();
    assert_eq!(session.graph().unwrap().seed, 8);
}

#[test]
fn another_documents_properties_have_their_own_undo_history() {
    let mut package = Package::new("Source");
    package.graphs.push(Graph::new("Inspected"));
    let mut session = Session::new(package);
    let source = session.document_id;
    execute(&mut session, "file.newPackage", &json!({"name": "Canvas"})).unwrap();
    execute(&mut session, "file.new", &json!({"template": "empty"})).unwrap();
    let active = session.document_id;
    let history = session.history();
    let selection = session.selection.clone();

    execute(&mut session, "graph.attributes", &json!({"document": source, "graph": 0, "description": "Saved on source"})).unwrap();

    assert_eq!(session.document_id, active);
    assert_eq!(session.history(), history);
    assert_eq!(session.selection, selection);
    assert!(session.graph().unwrap().attributes.description.is_empty());
    execute(&mut session, "package.activate", &json!({"document": source})).unwrap();
    assert_eq!(session.graph().unwrap().attributes.description, "Saved on source");
    assert!(session.modified);
    execute(&mut session, "edit.undo", &json!({})).unwrap();
    assert!(session.graph().unwrap().attributes.description.is_empty());
    assert!(!session.modified);
    execute(&mut session, "edit.redo", &json!({})).unwrap();
    assert_eq!(session.graph().unwrap().attributes.description, "Saved on source");
}

#[test]
fn invalid_attribute_updates_are_atomic_in_inactive_documents() {
    let mut package = Package::new("Source");
    package.graphs.push(Graph::new("Inspected"));
    let mut session = Session::new(package);
    let source = session.document_id;
    execute(&mut session, "file.newPackage", &json!({"name": "Canvas"})).unwrap();
    let original = materialcraft_engine::GraphTarget { document: source, graph: 0 }.package(&session).unwrap().clone();

    assert!(execute(&mut session, "graph.attributes", &json!({"document": source, "graph": 0, "label": "Partial edit", "description": 42})).is_err());

    assert_eq!(*materialcraft_engine::GraphTarget { document: source, graph: 0 }.package(&session).unwrap(), original);
    assert!(!session.open_packages().iter().find(|(id, _, _, _)| *id == source).unwrap().3);
    assert!(execute(&mut session, "graph.properties", &json!({"document": u64::MAX})).is_err());
    assert!(execute(&mut session, "graph.properties", &json!({"document": source, "graph": u64::MAX})).is_err());
}

#[test]
fn inactive_material_base_parameters_and_identifier_target_the_inspected_graph() {
    let mut package = Package::new("Source");
    package.graphs = vec![Graph::new("Canvas"), Graph::new("Inspected")];
    let mut session = Session::new(package);
    let source = session.document_id;
    execute(&mut session, "file.newPackage", &json!({"name": "Other"})).unwrap();

    for (command, mut parameters) in [
        ("graph.size", json!({"log2": [8, 9]})),
        ("graph.seed", json!({"seed": 12})),
        ("graph.format", json!({"format": "f32"})),
        ("graph.rename", json!({"name": "Renamed"})),
    ] {
        parameters["document"] = json!(source);
        parameters["graph"] = json!(1);
        execute(&mut session, command, &parameters).unwrap();
    }

    let graph = materialcraft_engine::GraphTarget { document: source, graph: 1 }.get(&session).unwrap();
    assert_eq!(graph.name, "Renamed");
    assert_eq!(graph.size, [8, 9]);
    assert_eq!(graph.seed, 12);
    assert_eq!(graph.format, materialcraft_engine::core::OutputFormat::F32);
    assert_eq!(execute(&mut session, "graph.properties", &json!({"document": source, "graph": "Renamed"})).unwrap()["identifier"], "Renamed");
    assert_ne!(session.document_id, source);
}

#[test]
fn a_function_label_updates_instances_without_renaming_references() {
    use materialcraft_engine::core::{FunctionGraph, Value};
    let mut source = Graph::new("Stable_identifier");
    source.function = Some(FunctionGraph::constant(Value::Float(1.0)));
    let mut caller = Graph::new("Caller");
    caller.function = Some(FunctionGraph::default());
    let mut package = Package::new("Labels");
    package.graphs = vec![source, caller];
    let mut session = Session::new(package);
    execute(&mut session, "graph.open", &json!({"graph": "Caller"})).unwrap();
    execute(&mut session, "graph.instance.add", &json!({"document": 0, "graph": "Stable_identifier"})).unwrap();

    execute(&mut session, "graph.attributes", &json!({"graph": "Stable_identifier", "label": "Readable label"})).unwrap();

    let function = session.graph().unwrap().function.as_ref().unwrap();
    let node = &function.nodes[0];
    assert_eq!(materialcraft_engine::nodes::function::node_label(node), "Readable label");
    assert_eq!(node.instance.as_ref().unwrap().graph, "Stable_identifier");
    let bytes = materialcraft_engine::file_cmds::save_bytes(&session).unwrap();
    let reopened = Session::new(materialcraft_engine::formats::sbs::read_sbs(&bytes, "labels.sbs", &|_| None).unwrap().pkg);
    let node = &reopened.pkg.graphs[1].function.as_ref().unwrap().nodes[0];
    assert_eq!(materialcraft_engine::nodes::function::node_label(node), "Readable label");
    assert_eq!(node.instance.as_ref().unwrap().graph, "Stable_identifier");
}
