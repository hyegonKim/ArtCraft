use materialcraft_engine::{
    FunctionGraphTarget, Session,
    core::{FunctionGraph, Graph, NodeId, Package, ParameterInheritance, Value},
    execute,
    formats::sbs::{read_sbs, write_sbs},
    new_session,
};
use serde_json::json;

fn getter(operation: &str, name: &str) -> FunctionGraph {
    let mut function = FunctionGraph::default();
    function.output = Some(function.add(operation, &[], Some(Value::Text(name.into()))));
    function
}

fn processor(graph: &mut Graph, operation: &str, name: &str) -> NodeId {
    let id = graph.add_node("value_processor", [0.0; 2]);
    graph.node_mut(id).unwrap().params.insert("function".into(), Value::Function(Box::new(getter(operation, name))));
    id
}

#[test]
fn standalone_get_suggestions_match_each_type_and_order() {
    let mut session = new_session();
    execute(&mut session, "file.new", &json!({"kind": "function", "name": "Variables"})).unwrap();
    for (operation, expected) in [
        ("get_string", vec![]),
        ("get_bool", vec![]),
        ("get_integer4", vec![]),
        ("get_integer3", vec![]),
        ("get_integer2", vec!["$uvtile"]),
        ("get_integer1", vec!["$normalformat", "$tiling", "$pixelratio"]),
        ("get_float4", vec![]),
        ("get_float3", vec!["$physicalsize"]),
        ("get_float2", vec!["$size", "$sizelog2"]),
        ("get_float1", vec!["$time"]),
    ] {
        let function = getter(operation, "ITypedThis");
        let id = function.output.unwrap();
        execute(&mut session, "graph.function.set", &json!({"function": function})).unwrap();
        let before = session.pkg.clone();

        let result = execute(&mut session, "graph.function.variables", &json!({"id": id})).unwrap();
        let variables = result["variables"].as_array().unwrap();
        let actual: Vec<_> = variables.iter().filter(|variable| variable["system"] == true).map(|variable| variable["name"].as_str().unwrap()).collect();

        assert_eq!(actual, expected, "{operation}");
        assert_eq!(variables.last().unwrap()["name"], "ITypedThis");
        assert_eq!(session.pkg, before);
    }
}

#[test]
fn pixel_processor_suggestions_add_position_without_fx_variables() {
    let mut graph = Graph::new("Pixels");
    let owner = graph.add_node("pixel_processor", [0.0; 2]);
    graph.node_mut(owner).unwrap().params.insert("function".into(), Value::Function(Box::new(getter("get_float2", ""))));
    let mut package = Package::new("Pixels");
    package.graphs.push(graph);
    let session = Session::new(package);
    let target = FunctionGraphTarget::Parameter { node: owner, parameter: "function".into() };

    let variables = target.variables(&session, 1).unwrap();

    assert_eq!(variables.iter().map(|variable| variable.name.as_str()).collect::<Vec<_>>(), ["$size", "$sizelog2", "$pos"]);
}

#[test]
fn graph_metadata_and_host_values_cook_and_round_trip_as_native_get_nodes() {
    let mut graph = Graph::new("Context");
    graph.size = [5, 4];
    let cases = [
        ("get_float1", "$time", Value::Float(2.5)),
        ("get_integer2", "$uvtile", Value::Int2([7, 11])),
        ("get_integer1", "$normalformat", Value::Int(1)),
        ("get_integer1", "$tiling", Value::Int(2)),
        ("get_integer1", "$pixelratio", Value::Int(1)),
        ("get_float3", "$physicalsize", Value::Float3([20.0, 10.0, 0.5])),
        ("get_float2", "$size", Value::Float2([32.0, 16.0])),
        ("get_float2", "$sizelog2", Value::Float2([5.0, 4.0])),
    ];
    let ids: Vec<_> = cases.iter().map(|(operation, name, _)| processor(&mut graph, operation, name)).collect();
    let mut package = Package::new("Context");
    package.graphs.push(graph);
    let mut session = Session::new(package);
    execute(&mut session, "graph.context", &json!({"physicalSize": [20, 10, 0.5], "pixelRatio": 1, "tiling": [false, true]})).unwrap();
    let environment = json!({"timeSeconds": 2.5, "uvTile": [7, 11], "normalFormat": 1});
    execute(&mut session, "session.environment", &environment).unwrap();
    let bytes = write_sbs(&session.pkg).unwrap();
    let read = read_sbs(&bytes, "context.sbs", &|_| None).unwrap();
    assert!(read.warnings.is_empty(), "{:?}", read.warnings);
    let mut reopened = Session::new(read.pkg);
    execute(&mut reopened, "session.environment", &environment).unwrap();

    for session in [&mut session, &mut reopened] {
        let cooked = session.cook();
        for ((_, name, expected), id) in cases.iter().zip(&ids) {
            let result = &cooked.nodes[id];
            assert_eq!(result.error, None, "{name}");
            assert_eq!(result.outputs[0].as_value(), Some(expected), "{name}");
        }
        assert!(session.cook().nodes.values().all(|result| result.cached));
        let original = session.pkg.clone();
        execute(session, "session.environment", &json!({"timeSeconds": 4.0})).unwrap();
        let cooked = session.cook();
        assert_eq!(cooked.nodes[&ids[0]].outputs[0].as_value(), Some(&Value::Float(4.0)));
        assert!(!cooked.nodes[&ids[0]].cached);
        assert!(cooked.nodes[&ids[1]].cached, "unrelated host variables keep their cache entries");
        assert_eq!(session.pkg, original, "host values are not authored into the package");
    }
}

#[test]
fn pixel_ratio_and_tiling_inherit_and_preserve_native_relative_modes() {
    let mut graph = Graph::new("Inheritance");
    graph.size = [4, 4];
    graph.tiling = [true, true];
    let source = graph.add_node("uniform", [0.0; 2]);
    let input = graph.node_mut(source).unwrap();
    input.base.pixel_ratio = 1;
    input.base.pixel_ratio_mode = ParameterInheritance::Absolute;
    input.base.tiling = [true, false];
    input.base.tiling_mode = ParameterInheritance::Absolute;
    let pixel = graph.add_node("pixel_processor", [200.0, 0.0]);
    let mut function = getter("get_integer1", "$pixelratio");
    let ratio = function.output.unwrap();
    let tiling = function.add("get_integer1", &[], Some(Value::Text("$tiling".into())));
    let value = function.add("add", &[ratio, tiling], None);
    function.output = Some(function.add("tofloat", &[value], None));
    graph.node_mut(pixel).unwrap().params.insert("function".into(), Value::Function(Box::new(function)));
    graph.connect(source, "output", pixel, "input0").unwrap();
    let mut package = Package::new("Inheritance");
    package.graphs.push(graph);
    let mut session = Session::new(package);
    session.selection = vec![pixel];

    for (mode, expected) in [("relativeToInput", 2.0), ("relativeToParent", 3.0), ("absolute", 0.0)] {
        execute(&mut session, "graph.node.base", &json!({"pixelRatio": 0, "tiling": false, "pixelRatioMode": mode, "tilingMode": mode})).unwrap();
        let bytes = write_sbs(&session.pkg).unwrap();
        let read = read_sbs(&bytes, "inheritance.sbs", &|_| None).unwrap();
        assert!(read.warnings.is_empty(), "{:?}", read.warnings);
        assert_eq!(read.pkg.graphs[0].node(pixel).unwrap().base, session.pkg.graphs[0].node(pixel).unwrap().base);
        let mut reopened = Session::new(read.pkg);

        for session in [&mut session, &mut reopened] {
            let cooked = session.cook();
            assert_eq!(cooked.nodes[&pixel].error, None);
            assert!((cooked.image(pixel, 0).unwrap().mean()[0] - expected).abs() < 1e-6, "{mode}");
        }
    }
}

#[test]
fn nested_graph_instances_inherit_host_values_and_invalidate_their_cache() {
    let mut source = Graph::new("Source");
    source.size = [4, 4];
    let value = processor(&mut source, "get_float1", "$time");
    let output = source.add_node("output", [200.0, 0.0]);
    source.connect(value, "output", output, "input").unwrap();
    let mut package = Package::new("Nested");
    package.graphs.push(source);
    for (name, reference) in [("Middle", "Source"), ("Outer", "Middle")] {
        let mut graph = Graph::new(name);
        graph.size = [4, 4];
        let instance = graph.add_node("graph_instance", [0.0; 2]);
        graph.node_mut(instance).unwrap().params.insert("graph".into(), Value::Text(reference.into()));
        let output = graph.add_node("output", [200.0, 0.0]);
        graph.connect(instance, "output", output, "input").unwrap();
        package.graphs.push(graph);
    }
    let mut session = Session::new(package);
    session.graph = 2;
    for seconds in [1.25, 8.5] {
        execute(&mut session, "session.environment", &json!({"timeSeconds": seconds})).unwrap();

        let cooked = session.cook();

        for result in cooked.nodes.values() {
            assert_eq!(result.error, None);
            assert_eq!(result.outputs[0].as_value(), Some(&Value::Float(seconds)));
            assert!(!result.cached);
        }
        assert!(session.cook().nodes.values().all(|result| result.cached));
    }
}

fn function_instance(name: &str, type_code: u32) -> FunctionGraph {
    let mut function = FunctionGraph::default();
    let id = function.add("instance", &[], None);
    function.nodes[0].instance = Some(materialcraft_engine::core::GraphInstance {
        label: String::new(),
        package: String::new(),
        graph: name.into(),
        inputs: Vec::new(),
        outputs: vec![materialcraft_engine::core::GraphPort { identifier: "output".into(), type_code }],
        defaults: Default::default(),
    });
    function.output = Some(id);
    function
}

#[test]
fn nested_function_instances_use_the_processing_callers_variable_scope() {
    let mut package = Package::new("Caller scope");
    let mut source = Graph::new("Position");
    source.function = Some(getter("get_float2", "$pos"));
    package.graphs.push(source);
    let mut wrapper = Graph::new("NestedPosition");
    wrapper.function = Some(function_instance("Position", 512));
    package.graphs.push(wrapper);
    let mut graph = Graph::new("Material");
    graph.size = [4, 4];
    let pixel = graph.add_node("pixel_processor", [0.0; 2]);
    let value = graph.add_node("value_processor", [200.0, 0.0]);
    for id in [pixel, value] {
        graph.node_mut(id).unwrap().params.insert("function".into(), Value::Function(Box::new(function_instance("NestedPosition", 512))));
    }
    package.graphs.push(graph);
    let mut session = Session::new(package);
    let bytes = write_sbs(&session.pkg).unwrap();
    let mut reopened = Session::new(read_sbs(&bytes, "scope.sbs", &|_| None).unwrap().pkg);

    for session in [&mut session, &mut reopened] {
        session.graph = 2;
        let cooked = session.cook();

        assert_eq!(cooked.nodes[&pixel].error, None);
        assert!(cooked.nodes[&value].error.as_deref().unwrap().contains("undefined variable `$pos`"));
        let image = cooked.image(pixel, 0).unwrap();
        assert_eq!(&image.get(0, 0, false)[..2], &[0.03125; 2]);
        assert_eq!(&image.get(15, 15, false)[..2], &[0.96875; 2]);
    }
}

#[test]
fn environment_errors_are_atomic_and_graph_context_is_undoable() {
    let mut session = new_session();
    let original_environment = session.environment;
    let original_package = session.pkg.clone();
    for parameters in [json!({"timeSeconds": -1}), json!({"timeSeconds": 1e100}), json!({"timeSeconds": 2, "uvTile": [1]}), json!({"normalFormat": "invalid"})]
    {
        assert!(execute(&mut session, "session.environment", &parameters).is_err());
        assert_eq!(session.environment, original_environment);
        assert_eq!(session.pkg, original_package);
    }
    for parameters in [json!({"physicalSize": [1, 2]}), json!({"physicalSize": [-1, 2, 3]}), json!({"pixelRatio": 3}), json!({"tiling": [true]})] {
        assert!(execute(&mut session, "graph.context", &parameters).is_err());
        assert_eq!(session.pkg, original_package);
    }

    execute(&mut session, "graph.context", &json!({"physicalSize": [2, 3, 4], "pixelRatio": 0})).unwrap();
    assert_eq!(session.graph().unwrap().physical_size, [2.0, 3.0, 4.0]);
    assert_eq!(session.graph().unwrap().pixel_ratio_mode, ParameterInheritance::Absolute);
    session.undo().unwrap();
    assert_eq!(session.pkg, original_package);
    session.redo().unwrap();
    assert_eq!(session.graph().unwrap().physical_size, [2.0, 3.0, 4.0]);
}

#[test]
fn reusable_functions_cannot_author_context_that_belongs_to_their_caller() {
    let mut session = new_session();
    execute(&mut session, "file.new", &json!({"kind": "function", "name": "Caller context"})).unwrap();
    let original = session.pkg.clone();

    let error = execute(&mut session, "graph.context", &json!({"physicalSize": [1, 2, 3]})).unwrap_err();

    assert!(error.to_string().contains("caller's evaluation context"));
    assert_eq!(session.pkg, original);
}
