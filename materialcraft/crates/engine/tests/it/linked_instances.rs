use materialcraft_engine::{
    Session,
    core::{Graph, NodeId, Package, Value},
    execute, file_cmds, formats,
};
use serde_json::json;

fn source() -> (Package, NodeId) {
    let mut package = Package::new("Source file");
    let mut graph = Graph::new("Source");
    graph.size = [4, 4];
    let value = graph.add_node("uniform", [0.0; 2]);
    graph.node_mut(value).unwrap().params.insert("color".into(), Value::Color([0.25, 0.5, 0.75, 1.0]));
    let output = graph.add_node("output", [200.0, 0.0]);
    graph.node_mut(output).unwrap().params.insert("identifier".into(), Value::Text("color".into()));
    graph.connect(value, "output", output, "input").unwrap();
    package.graphs.push(graph);
    (package, value)
}

fn open(session: &mut Session, package: &Package, file: &str) -> u64 {
    file_cmds::open_bytes(session, &formats::sbs::write_sbs(package).unwrap(), Some(file.into())).unwrap();
    session.document_id
}

#[test]
fn source_edits_update_instances_and_dirty_dependents_before_saving() {
    let (source, value) = source();
    let mut session = Session::new(source);
    session.selection = vec![value];
    session.path = Some(std::env::temp_dir().join("materialcraft-instance-source.sbs").to_string_lossy().into_owned());
    let mut target = Package::new("Target file");
    target.graphs.push(Graph::new("Target"));
    let target_id = open(&mut session, &target, "target.sbs");
    let instance = NodeId(execute(&mut session, "graph.instance.add", &json!({"document": 0, "graph": "Source"})).unwrap()["id"].as_u64().unwrap());
    session.preview_size = Some(16);
    session.cook();
    assert_eq!(session.output_ports(instance), ["color"]);
    assert!(session.cooked.nodes[&instance].error.is_none(), "{:?}", session.cooked.nodes[&instance].error);
    assert_eq!(session.cooked.image(instance, 0).unwrap().get(0, 0, false), [0.25, 0.5, 0.75, 1.0]);
    let history = session.history();
    session.modified = false;
    execute(&mut session, "package.activate", &json!({"document": 0})).unwrap();
    execute(&mut session, "graph.select", &json!({"ids": [value.0]})).unwrap();
    execute(&mut session, "graph.node.param", &json!({"id": value.0, "param": "color", "value": [0.8, 0.2, 0.1, 1.0]})).unwrap();
    assert!(session.open_packages().iter().find(|entry| entry.0 == target_id).unwrap().3);
    execute(&mut session, "package.activate", &json!({"document": target_id})).unwrap();
    assert_eq!(session.history(), history);
    session.cook();
    assert_eq!(session.cooked.image(instance, 0).unwrap().get(0, 0, false), [0.8, 0.2, 0.1, 1.0]);
    execute(&mut session, "edit.undo", &json!({})).unwrap();
    assert!(session.graph().unwrap().nodes.is_empty());
    execute(&mut session, "edit.redo", &json!({})).unwrap();
    session.cook();
    assert_eq!(session.cooked.image(instance, 0).unwrap().get(0, 0, false), [0.8, 0.2, 0.1, 1.0]);
}

#[test]
fn creation_rejects_cycles_missing_outputs_and_incompatible_graph_kinds_atomically() {
    let (package, _) = source();
    let mut session = Session::new(package);
    let before = session.pkg.clone();
    assert!(execute(&mut session, "graph.instance.add", &json!({"document": 0, "graph": "Source"})).is_err());
    assert_eq!(session.pkg, before);
    assert!(!session.can_undo());
    execute(&mut session, "file.new", &json!({"name": "Incomplete", "template": "empty"})).unwrap();
    assert!(execute(&mut session, "graph.instance.add", &json!({"document": 0, "graph": "Incomplete"})).is_err());
    assert!(execute(&mut session, "graph.instance.add", &json!({"document": 0, "graph": "Source", "pattern": 1})).is_err());
    execute(&mut session, "file.new", &json!({"name": "Function", "kind": "function"})).unwrap();
    assert!(execute(&mut session, "graph.instance.add", &json!({"document": 0, "graph": "Source"})).is_err());
}

#[test]
fn failed_edit_groups_do_not_publish_source_changes() {
    let (source, value) = source();
    let mut session = Session::new(source);
    session.selection = vec![value];
    session.path = Some(std::env::temp_dir().join("materialcraft-group-source.sbs").to_string_lossy().into_owned());
    let mut target = Package::new("Target");
    target.graphs.push(Graph::new("Target"));
    let target_id = open(&mut session, &target, "group-target.sbs");
    let instance = NodeId(execute(&mut session, "graph.instance.add", &json!({"document": 0, "graph": "Source"})).unwrap()["id"].as_u64().unwrap());
    session.modified = false;
    execute(&mut session, "package.activate", &json!({"document": 0})).unwrap();
    let result = session.group("Failed source edit", |session| {
        execute(session, "graph.node.param", &json!({"id": value.0, "param": "color", "value": [1.0, 0.0, 0.0, 1.0]})).unwrap();
        execute(session, "graph.node.param", &json!({"id": 99999, "param": "color", "value": [0.0, 0.0, 0.0, 0.0]}))
    });
    assert!(result.is_err());
    assert!(!session.open_packages().iter().find(|entry| entry.0 == target_id).unwrap().3);
    execute(&mut session, "package.activate", &json!({"document": target_id})).unwrap();
    session.preview_size = Some(16);
    session.cook();
    assert_eq!(session.cooked.image(instance, 0).unwrap().get(0, 0, false), [0.25, 0.5, 0.75, 1.0]);
    execute(&mut session, "package.activate", &json!({"document": 0})).unwrap();
    session.begin_group("Complete source edit on package switch");
    execute(&mut session, "graph.node.param", &json!({"id": value.0, "param": "color", "value": [1.0, 0.0, 0.0, 1.0]})).unwrap();
    execute(&mut session, "package.activate", &json!({"document": target_id})).unwrap();
    assert!(session.modified);
    session.cook();
    assert_eq!(session.cooked.image(instance, 0).unwrap().get(0, 0, false), [1.0, 0.0, 0.0, 1.0]);
}

#[test]
fn instance_info_is_read_only_and_describes_the_source_reference() {
    let (source, _) = source();
    let mut session = Session::new(source);
    execute(&mut session, "file.new", &json!({"name": "Target", "template": "empty"})).unwrap();
    let instance = execute(&mut session, "graph.instance.add", &json!({"document": 0, "graph": "Source"})).unwrap()["id"].as_u64().unwrap();
    let before = session.pkg.clone();
    let history = session.history();
    let info = execute(&mut session, "graph.node.info", &json!({"id": instance})).unwrap();
    let info: serde_json::Value = serde_json::from_str(info["text"].as_str().unwrap()).unwrap();
    assert_eq!(info["instance"]["graph"], "Source");
    assert_eq!(session.pkg, before);
    assert_eq!(session.history(), history);
}

#[test]
fn updates_propagate_through_three_packages_and_cycles_are_rejected() {
    let (source, value) = source();
    let mut session = Session::new(source);
    session.selection = vec![value];
    session.path = Some(std::env::temp_dir().join("materialcraft-transitive-source.sbs").to_string_lossy().into_owned());
    let mut target = Package::new("Target");
    target.graphs.push(Graph::new("Target"));
    let middle = open(&mut session, &target, "transitive-middle.sbs");
    let instance = execute(&mut session, "graph.instance.add", &json!({"document": 0, "graph": "Source"})).unwrap()["id"].as_u64().unwrap();
    let output = execute(&mut session, "graph.node.add", &json!({"def": "output"})).unwrap()["id"].as_u64().unwrap();
    execute(&mut session, "graph.connect", &json!({"from": instance, "fromPort": "color", "to": output})).unwrap();
    session.modified = false;
    let outer = open(&mut session, &target, "transitive-outer.sbs");
    let instance = NodeId(execute(&mut session, "graph.instance.add", &json!({"document": middle, "graph": "Target"})).unwrap()["id"].as_u64().unwrap());
    let output = execute(&mut session, "graph.node.add", &json!({"def": "output"})).unwrap()["id"].as_u64().unwrap();
    execute(&mut session, "graph.connect", &json!({"from": instance, "to": output})).unwrap();
    session.modified = false;
    session.preview_size = Some(16);
    session.cook();
    assert_eq!(session.cooked.image(instance, 0).unwrap().get(0, 0, false), [0.25, 0.5, 0.75, 1.0]);
    execute(&mut session, "package.activate", &json!({"document": 0})).unwrap();
    assert!(execute(&mut session, "graph.instance.add", &json!({"document": outer, "graph": "Target"})).is_err());
    execute(&mut session, "graph.node.param", &json!({"id": value.0, "param": "color", "value": [0.1, 0.2, 0.3, 1.0]})).unwrap();
    assert!(session.open_packages().iter().filter(|entry| entry.0 != 0).all(|entry| entry.3));
    execute(&mut session, "package.activate", &json!({"document": outer})).unwrap();
    session.cook();
    assert_eq!(session.cooked.image(instance, 0).unwrap().get(0, 0, false), [0.1, 0.2, 0.3, 1.0]);
}

#[test]
fn closing_a_modified_source_discards_its_unsaved_changes_in_dependents() {
    let directory = std::env::temp_dir().join(format!("materialcraft-linked-discard-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let source_path = directory.join("source.sbs");
    let (source, value) = source();
    let mut session = Session::new(source);
    session.selection = vec![value];
    execute(&mut session, "file.save", &json!({"path": source_path})).unwrap();
    let mut target = Package::new("Target");
    target.graphs.push(Graph::new("Target"));
    let target_id = open(&mut session, &target, "discard-target.sbs");
    let instance = NodeId(execute(&mut session, "graph.instance.add", &json!({"document": 0, "graph": "Source"})).unwrap()["id"].as_u64().unwrap());
    execute(&mut session, "package.activate", &json!({"document": 0})).unwrap();
    execute(&mut session, "graph.node.param", &json!({"id": value.0, "param": "color", "value": [1.0, 0.0, 0.0, 1.0]})).unwrap();
    assert!(execute(&mut session, "package.close", &json!({})).is_err());
    execute(&mut session, "package.close", &json!({"discard": true})).unwrap();
    assert_eq!(session.document_id, target_id);
    session.preview_size = Some(16);
    session.cook();
    let color = session.cooked.image(instance, 0).unwrap().get(0, 0, false);
    assert!(color.into_iter().zip([0.25, 0.5, 0.75, 1.0]).all(|(actual, expected)| (actual - expected).abs() < 1e-6));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn named_value_inputs_and_outputs_keep_their_type_across_files() {
    let mut source = Package::new("Value source");
    let mut graph = Graph::new("Source");
    let input = graph.add_node("input_value", [0.0; 2]);
    graph.node_mut(input).unwrap().params.extend([("identifier".into(), Value::Text("number".into())), ("default".into(), Value::Float(0.25))]);
    let output = graph.add_node("output", [200.0, 0.0]);
    graph.node_mut(output).unwrap().params.insert("identifier".into(), Value::Text("result".into()));
    graph.connect(input, "output", output, "input").unwrap();
    source.graphs.push(graph);
    let mut session = Session::new(source);
    session.path = Some(std::env::temp_dir().join("materialcraft-value-source.sbs").to_string_lossy().into_owned());
    let mut target = Package::new("Target");
    target.graphs.push(Graph::new("Target"));
    open(&mut session, &target, "value-target.sbs");
    let instance = NodeId(execute(&mut session, "graph.instance.add", &json!({"document": 0, "graph": "Source"})).unwrap()["id"].as_u64().unwrap());
    let value = execute(&mut session, "graph.node.add", &json!({"def": "input_value", "params": {"default": 0.75}})).unwrap()["id"].as_u64().unwrap();
    execute(&mut session, "graph.connect", &json!({"from": value, "to": instance, "toPort": "number"})).unwrap();
    session.preview_size = Some(16);
    session.cook();
    let cooked = &session.cooked.nodes[&instance];
    assert!(cooked.error.is_none(), "{:?}", cooked.error);
    assert_eq!(cooked.outputs[0].as_value(), Some(&Value::Float(0.75)));
    assert_eq!(session.output_ports(instance), ["result"]);
    assert!(!materialcraft_engine::MaterialSource { graph: session.graph, node: instance, port: "result".into() }.is_image(&session.pkg));
    let mut local = session.open_packages().into_iter().find(|entry| entry.0 == 0).unwrap().1.clone();
    let mut caller = session.graph().unwrap().clone();
    caller.node_mut(instance).unwrap().instance.as_mut().unwrap().package.clear();
    local.graphs.push(caller);
    local.dependencies.clear();
    let bytes = formats::sbs::write_sbs(&local).unwrap();
    let mut reopened = Session::new(formats::sbs::read_sbs(&bytes, "local.sbs", &|_| None).unwrap().pkg);
    execute(&mut reopened, "graph.open", &json!({"graph": "Target"})).unwrap();
    reopened.preview_size = Some(16);
    reopened.cook();
    let instance = reopened.graph().unwrap().nodes.iter().find(|node| node.def == "graph_instance").unwrap().id;
    let cooked = &reopened.cooked.nodes[&instance];
    assert!(cooked.error.is_none(), "{:?}", cooked.error);
    assert_eq!(cooked.outputs[0].as_value(), Some(&Value::Float(0.75)));
}

#[test]
fn removing_a_source_output_does_not_erase_connected_instance_wires_on_save() {
    let (source, _) = source();
    let mut session = Session::new(source);
    session.path = Some(std::env::temp_dir().join("materialcraft-port-source.sbs").to_string_lossy().into_owned());
    let source_output = session.graph().unwrap().nodes.iter().find(|node| node.def == "output").unwrap().id;
    let mut target = Package::new("Target");
    target.graphs.push(Graph::new("Target"));
    let target_id = open(&mut session, &target, "port-target.sbs");
    let instance = NodeId(execute(&mut session, "graph.instance.add", &json!({"document": 0, "graph": "Source"})).unwrap()["id"].as_u64().unwrap());
    let output = execute(&mut session, "graph.node.add", &json!({"def": "output"})).unwrap()["id"].as_u64().unwrap();
    execute(&mut session, "graph.connect", &json!({"from": instance, "fromPort": "color", "to": output})).unwrap();
    execute(&mut session, "package.activate", &json!({"document": 0})).unwrap();
    execute(&mut session, "graph.select", &json!({"ids": [source_output]})).unwrap();
    execute(&mut session, "edit.delete", &json!({})).unwrap();
    execute(&mut session, "package.activate", &json!({"document": target_id})).unwrap();
    let bytes = file_cmds::save_bytes(&session).unwrap();
    let read = formats::sbs::read_sbs(&bytes, "target.sbs", &|_| None).unwrap();
    assert_eq!(read.pkg.graphs[0].connections.len(), 1);
    session.preview_size = Some(16);
    session.cook();
    assert!(session.cooked.nodes[&instance].error.is_some());
}

#[test]
fn local_instances_follow_graph_renames_and_undo() {
    let (source, _) = source();
    let mut session = Session::new(source);
    execute(&mut session, "file.new", &json!({"name": "Target", "template": "empty"})).unwrap();
    let instance = NodeId(execute(&mut session, "graph.instance.add", &json!({"document": 0, "graph": "Source"})).unwrap()["id"].as_u64().unwrap());
    execute(&mut session, "graph.rename", &json!({"graph": "Source", "name": "Renamed"})).unwrap();
    assert_eq!(session.graph().unwrap().node(instance).unwrap().instance.as_ref().unwrap().graph, "Renamed");
    execute(&mut session, "edit.undo", &json!({})).unwrap();
    assert_eq!(session.graph().unwrap().node(instance).unwrap().instance.as_ref().unwrap().graph, "Source");
}

#[test]
fn extracting_a_selection_cannot_create_an_instance_without_outputs() {
    let mut package = Package::new("Source");
    package.graphs.push(Graph::new("Graph"));
    let mut session = Session::new(package);
    execute(&mut session, "graph.node.add", &json!({"def": "uniform"})).unwrap();
    let before = session.pkg.clone();
    let history = session.history();
    assert!(execute(&mut session, "graph.createFromSelection", &json!({})).is_err());
    assert_eq!(session.pkg, before);
    assert_eq!(session.history(), history);
}

#[test]
fn saved_dependencies_reopen_without_source_files_in_explorer_and_missing_sources_keep_wires() {
    let directory = std::env::temp_dir().join(format!("materialcraft-linked-roundtrip-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let source_path = directory.join("source.sbs");
    let target_path = directory.join("target.sbs");
    let (source, _) = source();
    let mut session = Session::new(source);
    execute(&mut session, "file.save", &json!({"path": source_path})).unwrap();
    execute(&mut session, "file.newPackage", &json!({"name": "Target"})).unwrap();
    execute(&mut session, "file.new", &json!({"name": "Target", "template": "empty"})).unwrap();
    let instance = execute(&mut session, "graph.instance.add", &json!({"document": 0, "graph": "Source"})).unwrap()["id"].as_u64().unwrap();
    let output = execute(&mut session, "graph.node.add", &json!({"def": "output"})).unwrap()["id"].as_u64().unwrap();
    execute(&mut session, "graph.connect", &json!({"from": instance, "fromPort": "color", "to": output})).unwrap();
    execute(&mut session, "file.save", &json!({"path": target_path})).unwrap();
    let saved = std::fs::read_to_string(&target_path).unwrap();
    assert!(saved.contains("filename v=\"source.sbs\""));
    assert!(!saved.contains(&directory.to_string_lossy().to_string()));
    let mut reopened = Session::new(Package::new("Initial"));
    file_cmds::open_file(&mut reopened, target_path.to_str().unwrap()).unwrap();
    assert!(reopened.load_warnings.is_empty(), "{:?}", reopened.load_warnings);
    assert_eq!(reopened.open_packages().len(), 2);
    execute(&mut reopened, "graph.open", &json!({"graph": "Target"})).unwrap();
    reopened.preview_size = Some(16);
    reopened.cook();
    let graph = reopened.graph().unwrap();
    let output = graph.nodes.iter().find(|node| node.def == "output").unwrap().id;
    let color = reopened.cooked.image(output, 0).unwrap().get(0, 0, false);
    assert!(color.into_iter().zip([0.25, 0.5, 0.75, 1.0]).all(|(actual, expected)| (actual - expected).abs() < 1e-6));
    std::fs::remove_file(&source_path).unwrap();
    let mut missing = Session::new(Package::new("Initial"));
    file_cmds::open_file(&mut missing, target_path.to_str().unwrap()).unwrap();
    execute(&mut missing, "graph.open", &json!({"graph": "Target"})).unwrap();
    assert_eq!(missing.graph().unwrap().connections.len(), 1);
    let resaved = file_cmds::save_bytes(&missing).unwrap();
    let resaved = formats::sbs::read_sbs(&resaved, "target.sbs", &|_| None).unwrap();
    assert_eq!(resaved.pkg.graph_by_name("Target").unwrap().1.connections.len(), 1);
    assert!(missing.path.is_some());
    std::fs::remove_dir_all(directory).unwrap();
}

fn assert_instance_color(session: &mut Session, instance: NodeId, expected: [f32; 4]) {
    let cooked = session.cook();
    assert_eq!(cooked.nodes[&instance].error, None);
    let actual = cooked.image(instance, 0).unwrap().get(0, 0, false);
    assert!(actual.into_iter().zip(expected).all(|(actual, expected)| (actual - expected).abs() < 1e-6), "{actual:?} != {expected:?}");
}

#[test]
fn instance_defaults_follow_current_source_inputs_before_and_after_save() {
    for linked in [false, true] {
        let directory = std::env::temp_dir().join(format!("materialcraft-current-inputs-{}-{linked}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let source_path = directory.join("source.sbs");
        let target_path = directory.join("target.sbs");
        let (package, value) = source();
        let mut session = Session::new(package);
        session.path = Some(source_path.to_string_lossy().into_owned());
        execute(&mut session, "graph.exposed.add", &json!({"id": "tint", "bind": {"node": value, "param": "color"}})).unwrap();
        execute(&mut session, "graph.exposed.set", &json!({"id": "tint", "value": [0.2, 0.3, 0.4, 1.0]})).unwrap();
        let target_document = if linked {
            let mut target = Package::new("Target");
            target.graphs.push(Graph::new("Target"));
            open(&mut session, &target, target_path.to_str().unwrap())
        } else {
            execute(&mut session, "file.new", &json!({"name": "Target", "template": "empty"})).unwrap();
            session.document_id
        };
        let target_graph = session.graph;
        let instance = NodeId(execute(&mut session, "graph.instance.add", &json!({"document": 0, "graph": "Source"})).unwrap()["id"].as_u64().unwrap());
        session.preview_size = Some(16);

        for expected in [[0.2, 0.3, 0.4, 1.0], [0.8, 0.6, 0.4, 1.0]] {
            execute(&mut session, "package.activate", &json!({"document": 0})).unwrap();
            session.graph = 0;
            execute(&mut session, "graph.exposed.set", &json!({"id": "tint", "value": expected})).unwrap();
            execute(&mut session, "package.activate", &json!({"document": target_document})).unwrap();
            session.graph = target_graph;
            let cooked = session.cook();

            assert_eq!(cooked.nodes[&instance].error, None);
            assert_eq!(cooked.nodes[&instance].parameters["tint"], Value::Color(expected));
            assert_instance_color(&mut session, instance, expected);
        }

        let source_bytes = formats::sbs::write_sbs(session.open_packages().iter().find(|entry| entry.0 == 0).unwrap().1).unwrap();
        std::fs::write(&source_path, source_bytes).unwrap();
        std::fs::write(&target_path, file_cmds::save_bytes(&session).unwrap()).unwrap();
        let mut reopened = Session::new(Package::new("Initial"));
        file_cmds::open_file(&mut reopened, target_path.to_str().unwrap()).unwrap();
        assert!(reopened.load_warnings.is_empty(), "{:?}", reopened.load_warnings);
        reopened.graph = target_graph;
        reopened.preview_size = Some(16);
        let instance = reopened.graph().unwrap().nodes.iter().find(|node| node.def == "graph_instance").unwrap().id;
        assert_instance_color(&mut reopened, instance, [0.8, 0.6, 0.4, 1.0]);

        execute(&mut reopened, "graph.node.param", &json!({"id": instance, "param": "tint", "value": [0.1, 0.2, 0.3, 1.0]})).unwrap();
        assert_instance_color(&mut reopened, instance, [0.1, 0.2, 0.3, 1.0]);
        execute(&mut reopened, "graph.node.reset", &json!({"id": instance, "param": "tint"})).unwrap();
        assert_instance_color(&mut reopened, instance, [0.8, 0.6, 0.4, 1.0]);
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn instance_constant_capture_uses_the_current_source_input_without_a_prior_cook() {
    let (package, value) = source();
    let mut session = Session::new(package);
    execute(&mut session, "graph.exposed.add", &json!({"id": "tint", "bind": {"node": value, "param": "color"}})).unwrap();
    execute(&mut session, "graph.exposed.set", &json!({"id": "tint", "value": [0.8, 0.6, 0.4, 1.0]})).unwrap();
    execute(&mut session, "file.new", &json!({"name": "Target", "template": "empty"})).unwrap();
    let instance = NodeId(execute(&mut session, "graph.instance.add", &json!({"document": 0, "graph": "Source"})).unwrap()["id"].as_u64().unwrap());

    let info = execute(&mut session, "graph.node.parameterInfo", &json!({"id": instance, "param": "tint"})).unwrap();
    assert_eq!(info["value"], json!(Value::Color([0.8, 0.6, 0.4, 1.0])));
    execute(&mut session, "graph.node.function", &json!({"id": instance, "param": "tint", "mode": "constant"})).unwrap();
    session.graph = 0;
    execute(&mut session, "graph.exposed.set", &json!({"id": "tint", "value": [0.0, 0.0, 0.0, 1.0]})).unwrap();
    session.graph = 1;
    session.preview_size = Some(16);

    assert_instance_color(&mut session, instance, [0.8, 0.6, 0.4, 1.0]);
}
