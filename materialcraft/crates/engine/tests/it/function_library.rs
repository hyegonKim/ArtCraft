use materialcraft_engine::{
    FunctionGraphTarget, Session,
    core::{FunctionGraph, Graph, NodeId, Package, Value},
    execute, file_cmds,
    nodes::function::{self, EvaluationContext, EvaluationState},
};
use serde_json::json;
use std::collections::BTreeMap;

fn session() -> Session {
    let mut package = Package::new("Library integration");
    let mut graph = Graph::new("Caller");
    graph.function = Some(FunctionGraph::default());
    package.graphs.push(graph);
    Session::new(package)
}

fn value(function: &FunctionGraph) -> Value {
    let context = EvaluationContext { scope: Default::default(), position: [0.5; 2], size: [16.0; 2], images: &[], tile: true, variables: &BTreeMap::new() };
    function::compile(function, 0).unwrap().evaluate(&context, &mut EvaluationState::default()).unwrap()
}

#[test]
fn library_instances_are_discoverable_undoable_and_portable() {
    let mut session = session();
    let entries = execute(&mut session, "library.functions", &json!({})).unwrap();
    assert_eq!(entries.as_array().unwrap().len(), 171);
    let pi = entries.as_array().unwrap().iter().find(|entry| entry["graph"] == "Functions/Math/Pi").unwrap();
    assert_eq!(pi["outputs"][0]["type_code"], 256);

    let node = execute(&mut session, "library.function.add", &json!({"graph": "Functions/Math/Pi", "at": [64.0, 128.0]})).unwrap()["id"].as_u64().unwrap();
    execute(&mut session, "graph.function.edit", &json!({"edit": {"action": "output", "id": node}})).unwrap();
    assert_eq!(value(session.graph().unwrap().function.as_ref().unwrap()), Value::Float(std::f32::consts::PI));
    execute(&mut session, "edit.undo", &json!({})).unwrap();
    execute(&mut session, "edit.undo", &json!({})).unwrap();
    assert!(session.graph().unwrap().function.as_ref().unwrap().nodes.is_empty());
    execute(&mut session, "edit.redo", &json!({})).unwrap();
    execute(&mut session, "edit.redo", &json!({})).unwrap();

    let bytes = file_cmds::save_bytes(&session).unwrap();
    let xml = std::str::from_utf8(&bytes).unwrap();
    assert!(xml.contains("sbs://functions.sbs"));
    assert!(!xml.contains("assets/sbs"));
    assert!(!xml.contains("<identifier v=\"Pi\""));
    let mut reopened = self::session();
    file_cmds::open_bytes(&mut reopened, &bytes, Some("library-caller.sbs".into())).unwrap();
    assert_eq!(value(reopened.graph().unwrap().function.as_ref().unwrap()), Value::Float(std::f32::consts::PI));
}

#[test]
fn every_bundled_function_has_a_resolved_executable_body() {
    let mut session = session();
    let entries = execute(&mut session, "library.functions", &json!({})).unwrap();
    let entries = entries.as_array().unwrap();
    let missing: Vec<_> = entries.iter().filter(|entry| entry["missing"] != false).map(|entry| &entry["graph"]).collect();

    assert_eq!(entries.len(), 171);
    assert!(missing.is_empty(), "missing bundled functions: {missing:?}");
    for entry in entries {
        session.graph_mut().unwrap().function = Some(FunctionGraph::default());
        session.reset_history();
        let identifier = execute(&mut session, "library.function.add", &json!({"graph": entry["graph"]})).unwrap()["id"].as_u64().unwrap();
        execute(&mut session, "graph.function.edit", &json!({"edit": {"action": "output", "id": identifier}})).unwrap();
        let function = session.graph().unwrap().function.as_ref().unwrap();

        assert!(matches!(function.nodes[0].source, Some(Ok(_))), "{}", entry["graph"]);
        function::compile(function, 0).unwrap_or_else(|error| panic!("{}: {error}", entry["graph"]));
    }
}

#[test]
fn parity_library_instances_evaluate_connected_inputs_after_reopen() {
    for (name, expected, default) in [
        ("EvenCount", Value::Float(3.0), Value::Float(0.0)),
        ("OddCount", Value::Float(2.0), Value::Float(0.0)),
        ("ParityTest", Value::Bool(true), Value::Bool(true)),
    ] {
        let mut session = session();
        let entries = execute(&mut session, "library.functions", &json!({"filter": name})).unwrap();
        assert_eq!(entries[0]["missing"], false, "{name} must be usable from the Library");

        let instance =
            execute(&mut session, "library.function.add", &json!({"graph": format!("Functions/Math/Parity/{name}")})).unwrap()["id"].as_u64().unwrap();
        execute(&mut session, "graph.function.edit", &json!({"edit": {"action": "output", "id": instance}})).unwrap();
        assert_eq!(value(session.graph().unwrap().function.as_ref().unwrap()), default);
        let constant = execute(&mut session, "graph.function.edit", &json!({"edit": {"action": "add", "operation": "const_float1", "at": [-200.0, 0.0]}}))
            .unwrap()["id"]
            .as_u64()
            .unwrap();
        execute(&mut session, "graph.function.edit", &json!({"edit": {"action": "value", "id": constant, "value": Value::Float(5.75)}})).unwrap();
        execute(&mut session, "graph.function.edit", &json!({"edit": {"action": "connect", "from": constant, "to": instance, "input": 0}})).unwrap();
        let bytes = file_cmds::save_bytes(&session).unwrap();
        let mut reopened = self::session();
        file_cmds::open_bytes(&mut reopened, &bytes, Some("parity-caller.sbs".into())).unwrap();

        assert_eq!(value(session.graph().unwrap().function.as_ref().unwrap()), expected, "{name}");
        assert_eq!(value(reopened.graph().unwrap().function.as_ref().unwrap()), expected, "reopened {name}");
    }
}

#[test]
fn missing_library_implementations_remain_instances_with_typed_ports() {
    let mut session = session();
    let mut missing = session.pkg.dependencies["sbs://functions.sbs"].graph_by_name("Functions/Random/GlobalRandom").unwrap().1.clone();
    missing.name = "MissingFunction".into();
    missing.folder = "Functions/Test".into();
    missing.function = Some(FunctionGraph::default());
    let mut package = Package::new("Missing library");
    package.graphs.push(missing);
    session.pkg.dependencies.insert("sbs://missing-functions.sbs".into(), std::sync::Arc::new(package));

    let entries = execute(&mut session, "library.functions", &json!({"filter": "MissingFunction"})).unwrap();
    assert_eq!(entries[0]["missing"], true);
    assert_eq!(entries[0]["inputs"].as_array().unwrap().len(), 2);

    let id = execute(&mut session, "library.function.add", &json!({"package": "sbs://missing-functions.sbs", "graph": "Functions/Test/MissingFunction"}))
        .unwrap()["id"]
        .as_u64()
        .unwrap();
    let node = session.graph().unwrap().function.as_ref().unwrap().node(id as u32).unwrap();
    assert!(matches!(node.source, Some(Err(_))));
    assert_eq!(node.instance.as_ref().unwrap().inputs.len(), 2);
    let saved = file_cmds::save_bytes(&session).unwrap();
    let mut reopened = self::session();
    file_cmds::open_bytes(&mut reopened, &saved, None).unwrap();
    assert!(matches!(reopened.graph().unwrap().function.as_ref().unwrap().nodes[0].source, Some(Err(_))));
}

#[test]
fn function_library_accepts_processor_canvases_and_rejects_material_canvases() {
    let mut package = Package::new("Processors");
    package.graphs.push(Graph::new("Material"));
    let mut session = Session::new(package);
    assert!(execute(&mut session, "library.function.add", &json!({"graph": "Functions/Math/Pi"})).is_err());

    for kind in ["pixel_processor", "value_processor"] {
        let owner = execute(&mut session, "graph.node.add", &json!({"def": kind})).unwrap()["id"].as_u64().unwrap();
        if kind == "pixel_processor" {
            execute(&mut session, "graph.node.param", &json!({"id": owner, "param": "color_mode", "value": false})).unwrap();
        }

        let target = FunctionGraphTarget::Parameter { node: NodeId(owner), parameter: "function".into() };
        let node = execute(&mut session, "library.function.add", &json!({"graph": "Functions/Math/Pi", "function": target})).unwrap()["id"].as_u64().unwrap();
        execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "output", "id": node}})).unwrap();

        assert_eq!(value(target.current(&session).unwrap()), Value::Float(std::f32::consts::PI));
    }
}

#[test]
fn library_aliases_open_as_references_and_require_a_local_save_path() {
    let mut session = session();
    file_cmds::open_file(&mut session, "sbs://functions.sbs").unwrap();

    assert_eq!(session.pkg.graphs.len(), 171);
    let error = execute(&mut session, "file.save", &json!({})).unwrap_err().to_string();
    assert!(error.contains("local .sbs"), "{error}");
}

#[test]
fn renaming_a_nested_source_updates_its_local_reference() {
    let mut session = session();
    session.pkg.graphs[0].folder = "Functions/Local".into();
    session.pkg.graphs[0].function = Some(FunctionGraph::constant(Value::Float(0.75)));
    execute(&mut session, "file.new", &json!({"kind": "function", "name": "Consumer"})).unwrap();
    execute(&mut session, "graph.instance.add", &json!({"document": 0, "graph": "Functions/Local/Caller"})).unwrap();

    execute(&mut session, "graph.rename", &json!({"graph": 0, "name": "Renamed"})).unwrap();

    let node = &session.graph().unwrap().function.as_ref().unwrap().nodes[0];
    assert_eq!(node.instance.as_ref().unwrap().graph, "Functions/Local/Renamed");
    assert!(matches!(node.source, Some(Ok(_))));
}

#[test]
#[ignore = "local interoperability input; set MC_FUNCTIONS_SBS to an external package"]
fn supplied_functions_evaluate_before_and_after_save() {
    let path = std::env::var("MC_FUNCTIONS_SBS").unwrap();
    let read = materialcraft_engine::formats::sbs::read_sbs(&std::fs::read(&path).unwrap(), &path, &|_| None).unwrap();
    let before = Session::new(read.pkg);
    let bytes = materialcraft_engine::formats::sbs::write_sbs(&before.pkg).unwrap();
    let after = Session::new(materialcraft_engine::formats::sbs::read_sbs(&bytes, "saved.sbs", &|_| None).unwrap().pkg);
    let bundled = Session::new(
        materialcraft_engine::formats::sbs::read_sbs(include_bytes!("../../../../assets/sbs/functions.sbs"), "functions.sbs", &|_| None).unwrap().pkg,
    );
    let mut failures = Vec::new();
    let mut defaults = 0;

    for graph in &before.pkg.graphs {
        let function = graph.function.as_ref().unwrap();
        for node in function.nodes.iter().filter(|node| node.instance.is_some()) {
            assert!(matches!(node.source, Some(Ok(_))), "{}: unresolved instance {}", graph.sbs_path(), node.id);
        }
        let source = function::compile(function, 0).unwrap();
        let saved = after.pkg.graph_by_name(&graph.sbs_path()).unwrap().1;
        let saved = function::compile(saved.function.as_ref().unwrap(), 0).unwrap();
        let converted = bundled.pkg.graph_by_name(&graph.sbs_path()).unwrap().1;
        let converted = function::compile(converted.function.as_ref().unwrap(), 0).unwrap();
        let mut errors = Vec::new();
        for profile in 0..4 {
            let mut variables: BTreeMap<_, _> = graph
                .exposed
                .iter()
                .enumerate()
                .map(|(index, parameter)| {
                    let scalar = (profile as f32 + index as f32) * 0.25;
                    let value = if profile == 0 {
                        parameter.default.clone()
                    } else {
                        match parameter.default {
                            Value::Float(_) => Value::Float(scalar),
                            Value::Float2(_) => Value::Float2([scalar, scalar * 2.0]),
                            Value::Float3(_) => Value::Float3([scalar, scalar * 2.0, scalar * 3.0]),
                            Value::Float4(_) => Value::Float4([scalar, scalar * 2.0, scalar * 3.0, scalar * 4.0]),
                            Value::Int(_) => Value::Int(profile),
                            Value::Int2(_) => Value::Int2([profile, profile + 1]),
                            Value::Int3(_) => Value::Int3([profile, profile + 1, profile + 2]),
                            Value::Int4(_) => Value::Int4([profile, profile + 1, profile + 2, profile + 3]),
                            Value::Bool(_) => Value::Bool(profile % 2 != 0),
                            _ => parameter.default.clone(),
                        }
                    };
                    (parameter.id.clone(), value)
                })
                .collect();
            // This hidden helper requires a render region supplied by its caller
            if graph.name == "compute_total_number" {
                variables.insert("render_region".into(), Value::Float4([-0.5, 0.5, -0.5, 0.5]));
            }
            let context =
                EvaluationContext { scope: Default::default(), position: [0.5; 2], size: [256.0, 128.0], images: &[], tile: true, variables: &variables };
            match source.evaluate(&context, &mut EvaluationState::default()) {
                Ok(expected) => {
                    assert_eq!(saved.evaluate(&context, &mut EvaluationState::default()).unwrap(), expected, "{}", graph.sbs_path());
                    assert_eq!(converted.evaluate(&context, &mut EvaluationState::default()).unwrap(), expected, "converted {}", graph.sbs_path());
                    defaults += usize::from(profile == 0);
                    errors.clear();
                    break;
                }
                Err(error) => errors.push(error),
            }
        }
        if !errors.is_empty() {
            failures.push(format!("{}: {}", graph.sbs_path(), errors.join("; ")));
        }
    }

    eprintln!(
        "Evaluated {} of {} supplied functions before/after save and in the converted library; {defaults} used their declared defaults",
        before.pkg.graphs.len() - failures.len(),
        before.pkg.graphs.len()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
