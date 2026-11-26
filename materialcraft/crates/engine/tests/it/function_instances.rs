use materialcraft_engine::{
    Session,
    core::{ExposedParameter, FunctionGraph, Graph, GraphInstance, GraphPort, NodeId, Package, Value},
    execute, file_cmds, formats,
    nodes::function::{self, EvaluationContext, EvaluationState},
};
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;

fn source() -> Package {
    let mut package = Package::new("Functions");
    let mut graph = Graph::new("Twice");
    graph.exposed.push(ExposedParameter {
        description: String::new(),
        editor: None,
        clamp: false,
        step: None,
        id: "amount".into(),
        label: "Amount".into(),
        group: String::new(),
        default: Value::Float(0.5),
        min: 0.0,
        max: 1.0,
        options: Vec::new(),
    });
    let mut function = FunctionGraph::default();
    let input = function.add("get_float1", &[], Some(Value::Text("amount".into())));
    let factor = function.add_constant(Value::Float(2.0));
    function.output = Some(function.add("mul", &[input, factor], None));
    graph.function = Some(function);
    package.graphs.push(graph);
    let mut graph = Graph::new("Tint");
    graph.function = Some(FunctionGraph::constant(Value::Float4([0.2, 0.4, 0.6, 1.0])));
    package.graphs.push(graph);
    package
}

fn edit(session: &mut Session, target: &Json, edit: Json) -> Json {
    execute(session, "graph.function.edit", &json!({"target": target, "edit": edit})).unwrap()
}

fn call(session: &mut Session, document: u64, graph: &str, target: &Json) -> u32 {
    execute(session, "graph.instance.add", &json!({"document": document, "graph": graph, "function": target})).unwrap()["id"].as_u64().unwrap() as u32
}

fn evaluate(graph: &FunctionGraph) -> Value {
    function::compile(graph, 0)
        .unwrap()
        .evaluate(
            &EvaluationContext { scope: Default::default(), position: [0.5; 2], size: [16.0; 2], images: &[], tile: true, variables: &BTreeMap::new() },
            &mut EvaluationState::default(),
        )
        .unwrap()
}

#[test]
fn redundant_legacy_function_connectors_preserve_wires_without_repeating_evaluation() {
    let mut package = source();
    let mut graph = Graph::new("Caller");
    let mut body = FunctionGraph::default();
    let maximum = body.add_constant(Value::Float(1.0));
    let random = body.add("rand", &[maximum], None);
    let instance = body.add("instance", &[random], None);
    let call = body.nodes.iter_mut().find(|node| node.id == instance).unwrap();
    call.instance = Some(GraphInstance {
        label: String::new(),
        package: String::new(),
        graph: "Twice".into(),
        inputs: vec![GraphPort { identifier: "amount".into(), type_code: 256 }],
        outputs: vec![GraphPort { identifier: "output".into(), type_code: 256 }],
        defaults: BTreeMap::new(),
    });
    let next_random = body.add("rand", &[maximum], None);
    body.output = Some(body.add("add", &[instance, next_random], None));
    graph.function = Some(body);
    package.graphs.push(graph);
    let canonical = Session::new(package.clone());
    let expected = evaluate(canonical.pkg.graph_by_name("Caller").unwrap().1.function.as_ref().unwrap());
    let caller = package.graphs.last_mut().unwrap().function.as_mut().unwrap();
    let call = caller.nodes.iter_mut().find(|node| node.id == instance).unwrap();
    call.instance.as_mut().unwrap().inputs.push(GraphPort { identifier: "old_amount".into(), type_code: 0 });
    call.inputs.push(Some(random));

    let session = Session::new(package.clone());
    let function = session.pkg.graph_by_name("Caller").unwrap().1.function.as_ref().unwrap();
    assert_eq!(evaluate(function), expected);
    assert!(function::warnings(function).is_empty());
    let saved = formats::sbs::write_sbs(&session.pkg).unwrap();
    let reopened = Session::new(formats::sbs::read_sbs(&saved, "caller.sbs", &|_| None).unwrap().pkg);
    let function = reopened.pkg.graph_by_name("Caller").unwrap().1.function.as_ref().unwrap();
    let call = function.nodes.iter().find(|node| node.instance.is_some()).unwrap();
    assert_eq!(call.instance.as_ref().unwrap().inputs.iter().map(|port| port.identifier.as_str()).collect::<Vec<_>>(), ["amount", "old_amount"]);
    assert_eq!(call.inputs[0], call.inputs[1]);
    assert_eq!(evaluate(function), expected);

    let caller = package.graphs.last_mut().unwrap().function.as_mut().unwrap();
    *caller.nodes.iter_mut().find(|node| node.id == instance).unwrap().inputs.last_mut().unwrap() = Some(maximum);
    let conflicting = Session::new(package);
    let function = conflicting.pkg.graph_by_name("Caller").unwrap().1.function.as_ref().unwrap();
    assert!(function::compile(function, 0).err().unwrap().contains("connected input no longer exists"));
}

#[test]
fn functions_link_across_documents_and_keep_independent_undo() {
    let mut session = Session::new(source());
    session.path = Some(std::env::temp_dir().join("materialcraft-function-source.sbs").to_string_lossy().into_owned());
    let mut target_package = Package::new("Caller");
    let mut graph = Graph::new("Caller");
    graph.function = Some(FunctionGraph::default());
    target_package.graphs.push(graph);
    file_cmds::open_bytes(&mut session, &formats::sbs::write_sbs(&target_package).unwrap(), Some("function-target.sbs".into())).unwrap();
    let document = session.document_id;
    let target = json!({"kind": "graph"});
    let instance = call(&mut session, 0, "Twice", &target);
    let constant = edit(&mut session, &target, json!({"action": "add", "operation": "const_float1", "at": [0,0]}))["id"].as_u64().unwrap();
    edit(&mut session, &target, json!({"action": "value", "id": constant, "value": Value::Float(0.25)}));
    edit(&mut session, &target, json!({"action": "output", "id": instance}));
    assert!(function::warnings(session.graph().unwrap().function.as_ref().unwrap()).is_empty());
    edit(&mut session, &target, json!({"action": "connect", "from": constant, "to": instance, "input": 0}));
    assert_eq!(evaluate(session.graph().unwrap().function.as_ref().unwrap()), Value::Float(0.5));
    let history = session.history();
    session.modified = false;
    execute(&mut session, "package.activate", &json!({"document": 0})).unwrap();
    edit(&mut session, &target, json!({"action": "value", "id": 2, "value": Value::Float(3.0)}));
    execute(&mut session, "package.activate", &json!({"document": document})).unwrap();
    assert!(session.modified);
    assert_eq!(session.history(), history);
    assert_eq!(evaluate(session.graph().unwrap().function.as_ref().unwrap()), Value::Float(0.75));
    execute(&mut session, "edit.undo", &json!({})).unwrap();
    assert_eq!(evaluate(session.graph().unwrap().function.as_ref().unwrap()), Value::Float(1.5));
    execute(&mut session, "edit.redo", &json!({})).unwrap();
    assert_eq!(evaluate(session.graph().unwrap().function.as_ref().unwrap()), Value::Float(0.75));
}

#[test]
fn processor_calls_recook_when_source_changes_and_round_trip_without_copying_source() {
    let mut session = Session::new(source());
    let directory = std::env::temp_dir().join(format!("materialcraft-function-links-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let source_path = directory.join("source.sbs");
    let target_path = directory.join("target.sbs");
    execute(&mut session, "file.saveAs", &json!({"path": source_path})).unwrap();
    let mut target_package = Package::new("Processors");
    target_package.graphs.push(Graph::new("Processors"));
    file_cmds::open_bytes(&mut session, &formats::sbs::write_sbs(&target_package).unwrap(), Some(target_path.to_string_lossy().into())).unwrap();
    let document = session.document_id;
    let mut processors = Vec::new();
    for def in ["pixel_processor", "value_processor"] {
        let id = execute(&mut session, "graph.node.add", &json!({"def": def})).unwrap()["id"].as_u64().unwrap();
        let target = json!({"kind": "parameter", "node": id, "parameter": "function"});
        let instance = call(&mut session, 0, "Tint", &target);
        edit(&mut session, &target, json!({"action": "output", "id": instance}));
        processors.push(NodeId(id));
    }
    session.preview_size = Some(16);
    session.cook();
    assert_eq!(session.cooked.image(processors[0], 0).unwrap().get(0, 0, false), [0.2, 0.4, 0.6, 1.0]);
    assert_eq!(session.cooked.value(processors[1], 0), Some(&Value::Float4([0.2, 0.4, 0.6, 1.0])));
    execute(&mut session, "package.activate", &json!({"document": 0})).unwrap();
    execute(&mut session, "graph.open", &json!({"graph": 1})).unwrap();
    edit(&mut session, &json!({"kind": "graph"}), json!({"action": "value", "id": 1, "value": Value::Float4([0.7,0.3,0.1,1.0])}));
    execute(&mut session, "file.save", &json!({})).unwrap();
    execute(&mut session, "package.activate", &json!({"document": document})).unwrap();
    session.cook();
    assert_eq!(session.cooked.image(processors[0], 0).unwrap().get(0, 0, false), [0.7, 0.3, 0.1, 1.0]);
    assert_eq!(session.cooked.value(processors[1], 0), Some(&Value::Float4([0.7, 0.3, 0.1, 1.0])));
    execute(&mut session, "file.saveAs", &json!({"path": target_path})).unwrap();
    let saved = std::fs::read(&target_path).unwrap();
    let xml = String::from_utf8(saved.clone()).unwrap();
    assert!(xml.contains("source.sbs") && !xml.contains(source_path.to_str().unwrap()));
    assert!(xml.contains("pkg:///Tint?dependency="));
    assert!(!xml.contains("0.7 0.3 0.1"));
    let mut reopened = Session::new(Package::new("Empty"));
    execute(&mut reopened, "file.open", &json!({"path": target_path})).unwrap();
    reopened.preview_size = Some(16);
    reopened.cook();
    for result in reopened.cooked.nodes.values() {
        assert!(result.error.is_none(), "{:?}", result.error);
    }
    std::fs::remove_file(&source_path).unwrap();
    let mut missing = Session::new(Package::new("Empty"));
    file_cmds::open_bytes(&mut missing, &saved, Some(target_path.to_string_lossy().into())).unwrap();
    assert!(missing.graph().unwrap().functions().all(|function| function.nodes.iter().any(|node| node.instance.is_some())));
    assert!(missing.graph().unwrap().functions().all(|function| !function::warnings(function).is_empty()));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn function_calls_reject_wrong_canvas_cycles_and_outputless_sources() {
    let mut package = source();
    let mut incomplete = Graph::new("Incomplete");
    incomplete.function = Some(FunctionGraph::default());
    package.graphs.push(incomplete);
    package.graphs.push(Graph::new("Material"));
    let mut session = Session::new(package);
    let before = session.pkg.clone();
    for name in ["Twice", "Incomplete", "Material"] {
        assert!(execute(&mut session, "graph.instance.add", &json!({"document": 0, "graph": name})).is_err());
    }
    assert_eq!(before, session.pkg);
    execute(&mut session, "graph.open", &json!({"graph": 3})).unwrap();
    assert!(execute(&mut session, "graph.instance.add", &json!({"document": 0, "graph": "Tint"})).is_err());
    assert!(execute(&mut session, "graph.instance.add", &json!({"document": 0, "graph": "Tint", "pattern": 1})).is_err());
    execute(&mut session, "graph.open", &json!({"graph": 2})).unwrap();
    call(&mut session, 0, "Tint", &json!({"kind": "graph"}));
    assert!(session.graph().unwrap().function.as_ref().unwrap().output.is_none());
}

#[test]
fn calls_isolate_variables_and_evaluate_arguments_once() {
    let mut package = source();
    let mut source = FunctionGraph::default();
    let input = source.add("get_float1", &[], Some(Value::Text("amount".into())));
    let private = source.add_constant(Value::Float(99.0));
    let write = source.add("set", &[private], Some(Value::Text("private".into())));
    let twice = source.add("add", &[input, input], None);
    source.output = Some(source.add("sequence", &[write, twice], None));
    package.graphs[0].function = Some(source);
    let mut caller = FunctionGraph::default();
    let one = caller.add_constant(Value::Float(1.0));
    let init = caller.add("set", &[one], Some(Value::Text("private".into())));
    let get = caller.add("get_float1", &[], Some(Value::Text("private".into())));
    let next = caller.add("add", &[get, one], None);
    let argument = caller.add("set", &[next], Some(Value::Text("private".into())));
    let call = caller.add("instance", &[argument], None);
    caller.nodes.last_mut().unwrap().instance = Some(GraphInstance {
        label: String::new(),
        defaults: Default::default(),
        package: String::new(),
        graph: "Twice".into(),
        inputs: vec![GraphPort { identifier: "amount".into(), type_code: 256 }],
        outputs: vec![GraphPort { identifier: "output".into(), type_code: 256 }],
    });
    let total = caller.add("add", &[call, get], None);
    caller.output = Some(caller.add("sequence", &[init, total], None));
    function::resolve_instances(&mut caller, &package, &BTreeMap::new());
    assert_eq!(evaluate(&caller), Value::Float(6.0));
}

#[test]
fn unconnected_instance_inputs_use_the_source_defaults() {
    let mut session = Session::new(source());
    execute(&mut session, "file.new", &json!({"name": "Caller", "kind": "function"})).unwrap();
    let target = json!({"kind": "graph"});
    let instance = call(&mut session, 0, "Twice", &target);
    edit(&mut session, &target, json!({"action": "output", "id": instance}));

    let graph = session.graph().unwrap().function.as_ref().unwrap();
    assert_eq!(evaluate(graph), Value::Float(1.0));
    assert!(function::warnings(graph).is_empty());
}

#[test]
fn function_instance_inputs_follow_current_source_values() {
    for linked in [false, true] {
        let mut session = Session::new(source());
        session.path = Some(std::env::temp_dir().join("materialcraft-function-input-source.sbs").to_string_lossy().into_owned());
        execute(&mut session, "graph.exposed.set", &json!({"id": "amount", "value": 0.25})).unwrap();
        if linked {
            let mut package = Package::new("Calls");
            let mut graph = Graph::new("Caller");
            graph.function = Some(FunctionGraph::default());
            package.graphs.push(graph);
            file_cmds::open_bytes(&mut session, &formats::sbs::write_sbs(&package).unwrap(), Some("function-input-target.sbs".into())).unwrap();
        } else {
            execute(&mut session, "file.new", &json!({"name": "Caller", "kind": "function"})).unwrap();
        }
        let document = session.document_id;
        let target = json!({"kind": "graph"});
        let instance = call(&mut session, 0, "Twice", &target);
        edit(&mut session, &target, json!({"action": "output", "id": instance}));

        assert_eq!(evaluate(session.graph().unwrap().function.as_ref().unwrap()), Value::Float(0.5));

        execute(&mut session, "file.new", &json!({"name": "Outer", "kind": "function"})).unwrap();
        let outer = call(&mut session, document, "Caller", &target);
        edit(&mut session, &target, json!({"action": "output", "id": outer}));
        execute(&mut session, "package.activate", &json!({"document": 0})).unwrap();
        execute(&mut session, "graph.open", &json!({"graph": "Twice"})).unwrap();
        execute(&mut session, "graph.exposed.set", &json!({"id": "amount", "value": 0.75})).unwrap();
        execute(&mut session, "package.activate", &json!({"document": document})).unwrap();
        execute(&mut session, "graph.open", &json!({"graph": "Outer"})).unwrap();

        assert_eq!(evaluate(session.graph().unwrap().function.as_ref().unwrap()), Value::Float(1.5));
        assert!(function::warnings(session.graph().unwrap().function.as_ref().unwrap()).is_empty());

        execute(&mut session, "graph.open", &json!({"graph": "Caller"})).unwrap();
        let constant = edit(&mut session, &target, json!({"action": "add", "operation": "const_float1", "at": [0, 0]}))["id"].as_u64().unwrap();
        edit(&mut session, &target, json!({"action": "value", "id": constant, "value": Value::Float(0.125)}));
        edit(&mut session, &target, json!({"action": "connect", "from": constant, "to": instance, "input": 0}));
        execute(&mut session, "package.activate", &json!({"document": 0})).unwrap();
        execute(&mut session, "graph.open", &json!({"graph": "Twice"})).unwrap();
        execute(&mut session, "graph.exposed.set", &json!({"id": "amount", "value": 1.0})).unwrap();
        execute(&mut session, "package.activate", &json!({"document": document})).unwrap();
        execute(&mut session, "graph.open", &json!({"graph": "Outer"})).unwrap();

        assert_eq!(evaluate(session.graph().unwrap().function.as_ref().unwrap()), Value::Float(0.25));
    }
}

#[test]
fn function_instance_input_changes_recook_parameter_functions_and_processors() {
    let mut session = Session::new(source());
    execute(&mut session, "graph.exposed.set", &json!({"id": "amount", "value": 0.25})).unwrap();
    execute(&mut session, "file.new", &json!({"name": "Material", "template": "empty"})).unwrap();
    let blur = NodeId(execute(&mut session, "graph.node.add", &json!({"def": "blur"})).unwrap()["id"].as_u64().unwrap());
    execute(&mut session, "graph.node.function", &json!({"id": blur, "param": "intensity", "mode": "empty"})).unwrap();
    let processor = NodeId(execute(&mut session, "graph.node.add", &json!({"def": "value_processor"})).unwrap()["id"].as_u64().unwrap());
    for target in [json!({"kind": "dynamic", "node": blur, "parameter": "intensity"}), json!({"kind": "parameter", "node": processor, "parameter": "function"})]
    {
        let instance = call(&mut session, 0, "Twice", &target);
        edit(&mut session, &target, json!({"action": "output", "id": instance}));
    }
    session.preview_size = Some(16);
    let cooked = session.cook();

    assert_eq!(cooked.nodes[&blur].error, None);
    assert_eq!(cooked.nodes[&blur].parameters["intensity"], Value::Float(0.5));
    assert_eq!(cooked.value(processor, 0), Some(&Value::Float(0.5)));

    execute(&mut session, "graph.open", &json!({"graph": "Twice"})).unwrap();
    execute(&mut session, "graph.exposed.set", &json!({"id": "amount", "value": 0.75})).unwrap();
    execute(&mut session, "graph.open", &json!({"graph": "Material"})).unwrap();
    let cooked = session.cook();

    assert_eq!(cooked.nodes[&blur].error, None);
    assert_eq!(cooked.nodes[&blur].parameters["intensity"], Value::Float(1.5));
    assert_eq!(cooked.nodes[&processor].error, None);
    assert_eq!(cooked.value(processor, 0), Some(&Value::Float(1.5)));
    assert!(!cooked.nodes[&blur].cached);
    assert!(!cooked.nodes[&processor].cached);
    let cached = session.cook();
    assert!(cached.nodes[&blur].cached);
    assert!(cached.nodes[&processor].cached);
    assert_eq!(cached.value(processor, 0), Some(&Value::Float(1.5)));

    let saved = formats::sbs::write_sbs(&session.pkg).unwrap();
    let mut reopened = Session::new(formats::sbs::read_sbs(&saved, "function-inputs.sbs", &|_| None).unwrap().pkg);
    execute(&mut reopened, "graph.open", &json!({"graph": "Material"})).unwrap();
    reopened.preview_size = Some(16);
    let blur = reopened.graph().unwrap().nodes.iter().find(|node| node.def == "blur").unwrap().id;
    let processor = reopened.graph().unwrap().nodes.iter().find(|node| node.def == "value_processor").unwrap().id;
    let cooked = reopened.cook();

    assert_eq!(cooked.nodes[&blur].parameters["intensity"], Value::Float(1.5));
    assert_eq!(cooked.value(processor, 0), Some(&Value::Float(1.5)));
}

#[test]
fn connected_inputs_survive_source_interface_changes_and_missing_sources() {
    let mut session = Session::new(source());
    execute(&mut session, "file.new", &json!({"name": "Caller", "kind": "function"})).unwrap();
    let target = json!({"kind": "graph"});
    let instance = call(&mut session, 0, "Twice", &target);
    let constant = edit(&mut session, &target, json!({"action": "add", "operation": "const_float1", "at": [0,0]}))["id"].as_u64().unwrap();
    edit(&mut session, &target, json!({"action": "connect", "from": constant, "to": instance, "input": 0}));
    edit(&mut session, &target, json!({"action": "output", "id": instance}));
    execute(&mut session, "graph.open", &json!({"graph": 0})).unwrap();
    execute(&mut session, "graph.exposed.delete", &json!({"id": "amount"})).unwrap();
    let caller = session.pkg.graphs[2].function.as_ref().unwrap();
    assert_eq!(caller.node(instance).unwrap().inputs, vec![Some(constant as u32)]);
    assert!(function::warnings(caller)[0].contains("no longer exists"));
    let saved = formats::sbs::write_sbs(&session.pkg).unwrap();
    let read = formats::sbs::read_sbs(&saved, "interface.sbs", &|_| None).unwrap();
    assert!(read.warnings.is_empty(), "{:?}", read.warnings);
    let reopened = Session::new(read.pkg);
    let caller = reopened.pkg.graphs[2].function.as_ref().unwrap();
    let instance = caller.nodes.iter().find(|node| node.instance.is_some()).unwrap();
    assert_eq!(instance.inputs.len(), 1);
    assert!(instance.inputs[0].is_some());
    assert!(function::warnings(caller)[0].contains("no longer exists"));
    execute(&mut session, "edit.undo", &json!({})).unwrap();
    assert!(function::warnings(session.pkg.graphs[2].function.as_ref().unwrap()).is_empty());
    execute(&mut session, "graph.rename", &json!({"name": "Renamed function"})).unwrap();
    let caller = session.pkg.graphs[2].function.as_ref().unwrap();
    assert_eq!(caller.node(instance_id(caller)).unwrap().instance.as_ref().unwrap().graph, "Renamed function");
    assert!(function::warnings(caller).is_empty());
}

fn instance_id(function: &FunctionGraph) -> u32 {
    function.nodes.iter().find(|node| node.instance.is_some()).unwrap().id
}

#[test]
fn nested_calls_propagate_edits_and_reject_indirect_cycles() {
    let mut session = Session::new(source());
    let target = json!({"kind": "graph"});
    execute(&mut session, "file.new", &json!({"name": "Middle", "kind": "function"})).unwrap();
    let inner = call(&mut session, 0, "Tint", &target);
    edit(&mut session, &target, json!({"action": "output", "id": inner}));
    execute(&mut session, "file.new", &json!({"name": "Outer", "kind": "function"})).unwrap();
    let outer = call(&mut session, 0, "Middle", &target);
    edit(&mut session, &target, json!({"action": "output", "id": outer}));
    assert_eq!(evaluate(session.graph().unwrap().function.as_ref().unwrap()), Value::Float4([0.2, 0.4, 0.6, 1.0]));
    execute(&mut session, "graph.open", &json!({"graph": 1})).unwrap();
    assert!(execute(&mut session, "graph.instance.add", &json!({"document": 0, "graph": "Outer"})).is_err());
    edit(&mut session, &target, json!({"action": "value", "id": 1, "value": Value::Float4([0.1,0.2,0.3,1.0])}));
    assert_eq!(evaluate(session.pkg.graphs[3].function.as_ref().unwrap()), Value::Float4([0.1, 0.2, 0.3, 1.0]));
    let reopened = Session::new(formats::sbs::read_sbs(&formats::sbs::write_sbs(&session.pkg).unwrap(), "nested.sbs", &|_| None).unwrap().pkg);
    assert_eq!(evaluate(reopened.pkg.graphs[3].function.as_ref().unwrap()), Value::Float4([0.1, 0.2, 0.3, 1.0]));
    execute(&mut session, "graph.open", &json!({"graph": 3})).unwrap();
    let before = session.pkg.clone();
    let history = session.history();
    let reference = execute(&mut session, "graph.instance.reference", &json!({"id": outer, "function": target})).unwrap();
    assert_eq!(reference["graph"], 2);
    assert_eq!(reference["document"], 0);
    assert_eq!(session.pkg, before);
    assert_eq!(session.history(), history);
}

#[test]
fn function_edits_refresh_main_instances_through_a_closed_middle_package() {
    let directory = std::env::temp_dir().join(format!("materialcraft-nested-function-files-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let mut session = Session::new(source());
    execute(&mut session, "file.saveAs", &json!({"path": directory.join("source.sbs")})).unwrap();
    let mut middle = Package::new("Middle");
    middle.graphs.push(Graph::new("Material"));
    file_cmds::open_bytes(&mut session, &formats::sbs::write_sbs(&middle).unwrap(), Some(directory.join("middle.sbs").to_string_lossy().into())).unwrap();
    let middle_document = session.document_id;
    let pixel = execute(&mut session, "graph.node.add", &json!({"def": "pixel_processor"})).unwrap()["id"].as_u64().unwrap();
    let target = json!({"kind": "parameter", "node": pixel, "parameter": "function"});
    let instance = call(&mut session, 0, "Tint", &target);
    edit(&mut session, &target, json!({"action": "output", "id": instance}));
    let output = execute(&mut session, "graph.node.add", &json!({"def": "output"})).unwrap()["id"].as_u64().unwrap();
    execute(&mut session, "graph.connect", &json!({"from": pixel, "to": output})).unwrap();
    execute(&mut session, "file.save", &json!({})).unwrap();
    let mut outer = Package::new("Outer");
    outer.graphs.push(Graph::new("Outer"));
    let outer_path = directory.join("outer.sbs");
    file_cmds::open_bytes(&mut session, &formats::sbs::write_sbs(&outer).unwrap(), Some(outer_path.to_string_lossy().into())).unwrap();
    let outer_document = session.document_id;
    let instance =
        NodeId(execute(&mut session, "graph.instance.add", &json!({"document": middle_document, "graph": "Material"})).unwrap()["id"].as_u64().unwrap());
    execute(&mut session, "file.save", &json!({})).unwrap();
    execute(&mut session, "package.close", &json!({"document": middle_document})).unwrap();
    session.preview_size = Some(16);
    session.cook();
    assert_eq!(session.cooked.image(instance, 0).unwrap().get(0, 0, false), [0.2, 0.4, 0.6, 1.0]);
    execute(&mut session, "package.activate", &json!({"document": 0})).unwrap();
    execute(&mut session, "graph.open", &json!({"graph": 1})).unwrap();
    edit(&mut session, &json!({"kind": "graph"}), json!({"action": "value", "id": 1, "value": Value::Float4([0.6,0.4,0.2,1.0])}));
    execute(&mut session, "file.save", &json!({})).unwrap();
    execute(&mut session, "package.activate", &json!({"document": outer_document})).unwrap();
    assert!(session.modified);
    session.cook();
    assert_eq!(session.cooked.image(instance, 0).unwrap().get(0, 0, false), [0.6, 0.4, 0.2, 1.0]);
    let mut reopened = Session::new(Package::new("Empty"));
    execute(&mut reopened, "file.open", &json!({"path": outer_path})).unwrap();
    reopened.preview_size = Some(16);
    reopened.cook();
    let result = reopened.cooked.nodes.values().next().unwrap();
    assert!(result.error.is_none(), "{:?}", result.error);
    assert_eq!(result.outputs[0].as_image().unwrap().get(0, 0, false), [0.6, 0.4, 0.2, 1.0]);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn malformed_instance_metadata_is_rejected_without_changing_the_document() {
    let mut session = Session::new(source());
    execute(&mut session, "file.new", &json!({"name": "Caller", "kind": "function"})).unwrap();
    let instance = call(&mut session, 0, "Tint", &json!({"kind": "graph"}));
    let before = session.pkg.clone();
    let mut function = session.graph().unwrap().function.clone().unwrap();
    function.nodes.iter_mut().find(|node| node.id == instance).unwrap().op = "const_float4".into();
    assert!(execute(&mut session, "graph.function.set", &json!({"function": function})).is_err());
    assert_eq!(session.pkg, before);
}

#[test]
fn cyclic_references_loaded_from_sbs_report_errors_without_recursing_forever() {
    let mut package = Package::new("Malformed reference");
    let mut graph = Graph::new("Recursive function");
    let mut function = FunctionGraph::default();
    let id = function.add("instance", &[], None);
    function.nodes[0].instance = Some(GraphInstance {
        label: String::new(),
        defaults: Default::default(),
        package: String::new(),
        graph: graph.name.clone(),
        inputs: Vec::new(),
        outputs: vec![GraphPort { identifier: "output".into(), type_code: 256 }],
    });
    function.output = Some(id);
    graph.function = Some(function);
    package.graphs.push(graph);
    let bytes = formats::sbs::write_sbs(&package).unwrap();
    let report = formats::sbs::read_sbs(&bytes, "cycle.sbs", &|_| None).unwrap();
    assert!(report.warnings.is_empty());
    let session = Session::new(report.pkg);
    let function = session.graph().unwrap().function.as_ref().unwrap();
    assert!(function::warnings(function)[0].contains("cycle"));
    let error = function::compile(function, 0).err().unwrap();
    assert!(error.contains("cycle"), "{error}");
}
