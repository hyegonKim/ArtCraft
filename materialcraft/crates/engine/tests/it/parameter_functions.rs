use materialcraft_engine::{
    Session,
    core::{FunctionGraph, NodeId, Value},
    execute, formats, new_session,
};
use serde_json::{Value as Json, json};

fn command(session: &mut Session, name: &str, parameters: Json) -> Json {
    execute(session, name, &parameters).unwrap()
}

fn node(session: &mut Session, definition: &str) -> NodeId {
    NodeId(command(session, "graph.node.add", json!({"def": definition}))["id"].as_u64().unwrap())
}

#[test]
fn output_size_function_controls_resolution_and_survives_sbs() {
    let mut session = new_session();
    session.preview_size = None;
    command(&mut session, "graph.size", json!({"width": 64, "height": 64}));
    let owner = node(&mut session, "uniform");
    let function = FunctionGraph::constant(Value::Int2([1, -1]));

    command(&mut session, "graph.node.function", json!({"id": owner, "base": true, "param": "outputsize", "function": function}));

    let cooked = session.cook();
    let image = cooked.image(owner, 0).unwrap();
    assert_eq!((image.w, image.h), (128, 32));
    assert!(cooked.nodes[&owner].error.is_none());

    let bytes = formats::sbs::write_sbs(&session.pkg).unwrap();
    let loaded = formats::sbs::read_sbs(&bytes, "parameter-functions.sbs", &|_| None).unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    let mut reopened = Session::new(loaded.pkg);
    reopened.preview_size = None;
    let owner = reopened.graph().unwrap().nodes.iter().find(|node| node.def == "uniform").unwrap().id;
    let cooked = reopened.cook();
    let image = cooked.image(owner, 0).unwrap();
    assert_eq!((image.w, image.h), (128, 32));
}

#[test]
fn empty_creation_has_no_output_and_constant_creation_uses_the_current_value() {
    let mut session = new_session();
    let owner = node(&mut session, "blur");
    command(&mut session, "graph.node.param", json!({"id": owner, "param": "intensity", "value": 7.0}));

    command(&mut session, "graph.node.function", json!({"id": owner, "param": "intensity", "mode": "empty"}));

    let function = &session.graph().unwrap().node(owner).unwrap().functions["intensity"];
    assert!(function.nodes.is_empty());
    assert_eq!(function.output, None);

    command(&mut session, "edit.undo", json!({}));
    command(&mut session, "graph.node.function", json!({"id": owner, "param": "intensity", "mode": "constant"}));

    let function = &session.graph().unwrap().node(owner).unwrap().functions["intensity"];
    assert_eq!(function.nodes.len(), 1);
    assert_eq!(function.nodes[0].value, Some(Value::Float(7.0)));
    assert_eq!(function.output, Some(function.nodes[0].id));
}

#[test]
fn exposed_parameters_are_editable_get_functions() {
    let mut session = new_session();
    let owner = node(&mut session, "blend");

    command(&mut session, "graph.exposed.add", json!({"id": "amount", "bind": {"node": owner, "param": "opacity"}}));

    let function = &session.graph().unwrap().node(owner).unwrap().functions["opacity"];
    let output = function.node(function.output.unwrap()).unwrap();
    assert_eq!(output.op, "get_float1");
    assert_eq!(output.value, Some(Value::Text("amount".into())));
}

#[test]
fn native_dynamic_output_size_is_not_discarded_on_load() {
    let xml = br#"<package><identifier v="Original"/><content><graph><identifier v="Material"/><compNodes>
      <compNode><uid v="1"/><compImplementation><compFilter><filter v="uniform"/><parameters>
        <parameter><name v="outputsize"/><relativeTo v="0"/><paramValue><dynamicValue><rootnode v="2"/><paramNodes>
          <paramNode><uid v="2"/><function v="const_int2"/><type v="32"/><funcDatas><funcData><name v="const_int2"/><constantValue><constantValueInt2 v="6 5"/></constantValue></funcData></funcDatas></paramNode>
        </paramNodes></dynamicValue></paramValue></parameter>
      </parameters></compFilter></compImplementation></compNode>
    </compNodes></graph></content></package>"#;
    let loaded = formats::sbs::read_sbs(xml, "original.sbs", &|_| None).unwrap();
    let mut session = Session::new(loaded.pkg);
    session.preview_size = None;
    let owner = session.graph().unwrap().nodes[0].id;

    let cooked = session.cook();

    let image = cooked.image(owner, 0).unwrap();
    assert_eq!((image.w, image.h), (64, 32));
}

#[test]
fn inherited_size_functions_use_nominal_input_size_before_preview_capping() {
    use materialcraft_engine::core::{BaseParameter, ValueType};

    let mut session = new_session();
    session.preview_size = Some(16);
    command(&mut session, "graph.size", json!({"width": 64, "height": 64}));
    let source = node(&mut session, "uniform");
    command(
        &mut session,
        "graph.node.function",
        json!({"id": source, "base": true, "param": "outputsize", "function": FunctionGraph::constant(Value::Int2([1, 0]))}),
    );
    let owner = NodeId(command(&mut session, "graph.node.add", json!({"def": "blur", "connectFrom": {"node": source}}))["id"].as_u64().unwrap());
    let unrelated = node(&mut session, "uniform");
    let mut function = FunctionGraph::input("$sizelog2", ValueType::Float2);
    let size = function.output.unwrap();
    let parent = function.add_constant(Value::Float2([6.0, 6.0]));
    let difference = function.add("sub", &[size, parent], None);
    function.output = Some(function.add("toint2", &[difference], None));
    command(&mut session, "graph.node.function", json!({"id": owner, "base": true, "param": "outputsize", "function": function}));

    let cooked = session.cook();

    assert_eq!(cooked.nodes[&owner].error, None);
    assert_eq!(cooked.nodes[&owner].base.size, [256, 64]);
    assert_eq!(cooked.nodes[&owner].base_values[&BaseParameter::OutputSize], Value::Int2([1, 0]));
    assert_eq!((cooked.image(owner, 0).unwrap().w, cooked.image(owner, 0).unwrap().h), (16, 4));

    command(
        &mut session,
        "graph.node.function",
        json!({"id": source, "base": true, "param": "outputsize", "function": FunctionGraph::constant(Value::Int2([0, 0]))}),
    );
    let cooked = session.cook();

    assert_eq!(cooked.nodes[&owner].base.size, [64, 64]);
    assert!(!cooked.nodes[&owner].cached);
    assert!(cooked.nodes[&unrelated].cached);
}

#[test]
fn every_base_function_preserves_its_type_inheritance_and_value_in_sbs() {
    use materialcraft_engine::core::{BaseParameter, ParameterInheritance};

    let mut session = new_session();
    session.preview_size = Some(16);
    let owner = node(&mut session, "uniform");
    let cases = [
        (BaseParameter::OutputSize, Value::Int2([6, 5])),
        (BaseParameter::Format, Value::Int(3)),
        (BaseParameter::PixelSize, Value::Float2([0.25, 0.5])),
        (BaseParameter::PixelRatio, Value::Int(1)),
        (BaseParameter::Tiling, Value::Int(2)),
        (BaseParameter::RandomSeed, Value::Int(23)),
    ];
    for (parameter, value) in &cases {
        command(&mut session, "graph.node.base", json!({"id": owner, "param": parameter.identifier(), "inheritance": "absolute"}));
        command(
            &mut session,
            "graph.node.function",
            json!({"id": owner, "base": true, "param": parameter.identifier(), "function": FunctionGraph::constant(value.clone())}),
        );
    }

    let saved = formats::sbs::write_sbs(&session.pkg).unwrap();
    let loaded = formats::sbs::read_sbs(&saved, "base-functions.sbs", &|_| None).unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    let mut reopened = Session::new(loaded.pkg);
    reopened.preview_size = Some(16);
    let owner = reopened.graph().unwrap().nodes[0].id;
    for (parameter, _) in &cases {
        assert_eq!(parameter.inheritance(&reopened.graph().unwrap().node(owner).unwrap().base), ParameterInheritance::Absolute);
    }
    let result = &reopened.cook().nodes[&owner];

    assert_eq!(result.error, None);
    for (parameter, value) in cases {
        assert_eq!(result.base_values[&parameter], value);
    }
    assert_eq!(result.base.size, [64, 32]);
    assert_eq!(result.base.seed, 23);
    assert_eq!(result.base.pixel_size, [0.25, 0.5]);
    assert_eq!(result.base.tiling, [false, true]);
    assert_eq!(result.base.pixel_ratio, 1);
    assert_eq!(result.base.format, materialcraft_engine::core::OutputFormat::F32);
}

#[test]
fn constant_replacement_freezes_the_driven_value_and_reset_is_undoable() {
    let mut session = new_session();
    session.preview_size = Some(16);
    let owner = node(&mut session, "blur");
    command(&mut session, "graph.exposed.add", json!({"id": "amount", "bind": {"node": owner, "param": "intensity"}}));
    command(&mut session, "graph.exposed.set", json!({"id": "amount", "value": 3.0}));

    command(&mut session, "graph.node.function", json!({"id": owner, "param": "intensity", "mode": "constant"}));
    command(&mut session, "graph.exposed.set", json!({"id": "amount", "value": 8.0}));

    assert_eq!(session.cook().nodes[&owner].parameters["intensity"], Value::Float(3.0));
    let before = session.pkg.clone();
    command(&mut session, "graph.node.reset", json!({"id": owner, "param": "intensity"}));
    assert!(!session.graph().unwrap().node(owner).unwrap().functions.contains_key("intensity"));
    command(&mut session, "edit.undo", json!({}));
    assert_eq!(session.pkg, before);
    command(&mut session, "graph.node.param", json!({"id": owner, "param": "intensity", "value": 2.0}));
    assert!(!session.graph().unwrap().node(owner).unwrap().functions.contains_key("intensity"));
    assert_eq!(session.cook().nodes[&owner].parameters["intensity"], Value::Float(2.0));
}

#[test]
fn empty_and_wrong_type_functions_remain_editable_but_report_cook_errors() {
    let mut session = new_session();
    session.preview_size = Some(16);
    let owner = node(&mut session, "uniform");
    for function in [FunctionGraph::default(), FunctionGraph::constant(Value::Float(2.0))] {
        command(&mut session, "graph.node.function", json!({"id": owner, "base": true, "param": "outputsize", "function": function}));

        assert!(session.cook().nodes[&owner].error.is_some());
        let info = command(&mut session, "graph.node.parameterInfo", json!({"id": owner, "base": true, "param": "outputsize"}));
        assert_eq!(info["expectedType"], "Integer2");
        assert!(!info["warnings"].as_array().unwrap().is_empty());
        command(&mut session, "graph.node.function.edit", json!({"id": owner, "base": true, "param": "outputsize"}));
        let bytes = formats::sbs::write_sbs(&session.pkg).unwrap();
        let loaded = formats::sbs::read_sbs(&bytes, "incomplete.sbs", &|_| None).unwrap();
        assert_eq!(loaded.pkg.graphs[0].nodes[0].base.functions.values().next().unwrap().nodes.len(), function.nodes.len());
    }
}

#[test]
fn instance_parameter_functions_work_without_stored_literals_and_use_instance_size() {
    use materialcraft_engine::core::{Graph, Package, ValueType};

    let mut source = Graph::new("Source");
    let color = source.add_node("uniform", [0.0; 2]);
    source.node_mut(color).unwrap().params.insert("color_mode".into(), Value::Bool(false));
    source.node_mut(color).unwrap().functions.insert("color".into(), FunctionGraph::input("amount", ValueType::Float4));
    source.exposed.push(materialcraft_engine::core::ExposedParameter {
        description: String::new(),
        editor: None,
        clamp: false,
        step: None,
        id: "amount".into(),
        label: "Amount".into(),
        group: String::new(),
        default: Value::Float4([0.2; 4]),
        min: 0.0,
        max: 1.0,
        options: Vec::new(),
    });
    let output = source.add_node("output", [200.0, 0.0]);
    source.connect(color, "output", output, "input").unwrap();
    let dimensions = source.add_node("value_processor", [0.0; 2]);
    source.node_mut(dimensions).unwrap().params.insert("function".into(), Value::Function(Box::new(FunctionGraph::input("$size", ValueType::Float2))));
    let size_output = source.add_node("output", [200.0, 200.0]);
    source.node_mut(size_output).unwrap().params.insert("identifier".into(), Value::Text("dimensions".into()));
    source.connect(dimensions, "output", size_output, "input").unwrap();
    let mut parent = Graph::new("Parent");
    parent.size = [6, 5];
    let owner = parent.add_node("graph_instance", [0.0; 2]);
    parent.node_mut(owner).unwrap().params.insert("graph".into(), Value::Text("Source".into()));
    let mut package = Package::new("Instances");
    package.graphs = vec![parent, source];
    let mut session = Session::new(package);
    session.preview_size = None;
    command(&mut session, "graph.node.function", json!({"id": owner, "param": "amount", "function": FunctionGraph::constant(Value::Float4([0.75; 4]))}));

    let result = session.cook();

    assert_eq!(result.nodes[&owner].error, None);
    assert!(result.nodes[&owner].outputs.iter().any(|output| output.as_value() == Some(&Value::Float2([64.0, 32.0]))));
    assert!((result.image(owner, 0).unwrap().get_gray(0, 0, false) - 0.75).abs() < 1e-6);
    assert_eq!((result.image(owner, 0).unwrap().w, result.image(owner, 0).unwrap().h), (64, 32));
    assert!(!session.graph().unwrap().node(owner).unwrap().params.contains_key("amount"));

    let bytes = formats::sbs::write_sbs(&session.pkg).unwrap();
    let loaded = formats::sbs::read_sbs(&bytes, "instance.sbs", &|_| None).unwrap();
    let mut reopened = Session::new(loaded.pkg);
    let owner = reopened.graph().unwrap().nodes[0].id;
    assert!((reopened.cook().image(owner, 0).unwrap().get_gray(0, 0, false) - 0.75).abs() < 1e-6);
}

#[test]
fn color_boolean_enum_and_text_parameters_use_typed_function_outputs() {
    let mut session = new_session();
    session.preview_size = Some(16);
    for (definition, parameter, value) in [
        ("uniform", "color", Value::Float4([0.2, 0.4, 0.6, 1.0])),
        ("uniform", "color_mode", Value::Bool(false)),
        ("blend", "mode", Value::Int(2)),
        ("text", "text", Value::Text("Original".into())),
    ] {
        let owner = node(&mut session, definition);
        command(&mut session, "graph.node.function", json!({"id": owner, "param": parameter, "function": FunctionGraph::constant(value.clone())}));

        let result = &session.cook().nodes[&owner];
        assert_eq!(result.error, None, "{parameter}");
        let expected = materialcraft_engine::nodes::find(definition).unwrap().param(parameter).unwrap().default.coerce_json(&json!(value)).unwrap();
        assert_eq!(result.parameters[parameter], expected);
    }

    let owner = node(&mut session, "uniform");
    command(&mut session, "graph.node.function", json!({"id": owner, "param": "color", "function": FunctionGraph::constant(Value::Float(0.5))}));
    assert!(session.cook().nodes[&owner].error.as_ref().unwrap().contains("Float4"));
    let bitmap = node(&mut session, "bitmap");
    assert!(execute(&mut session, "graph.node.function", &json!({"id": bitmap, "param": "resource"})).is_err());
}

#[test]
fn base_functions_inherit_from_the_primary_input_and_bypasses_keep_input_dimensions() {
    let mut session = new_session();
    session.preview_size = Some(16);
    let foreground = node(&mut session, "uniform");
    command(&mut session, "graph.node.base", json!({"id": foreground, "sizeMode": "absolute", "size": [7, 6]}));
    let background = node(&mut session, "uniform");
    command(&mut session, "graph.node.base", json!({"id": background, "sizeMode": "absolute", "size": [5, 5]}));
    let blend = node(&mut session, "blend");
    session.graph_mut().unwrap().connect(foreground, "output", blend, "foreground").unwrap();
    session.graph_mut().unwrap().connect(background, "output", blend, "background").unwrap();
    command(
        &mut session,
        "graph.node.function",
        json!({"id": blend, "param": "outputsize", "base": true, "function": FunctionGraph::constant(Value::Int2([1, 0]))}),
    );

    assert_eq!(session.cook().nodes[&blend].base.size, [64, 32]);

    let blur = NodeId(command(&mut session, "graph.node.add", json!({"def": "blur", "connectFrom": {"node": blend}}))["id"].as_u64().unwrap());
    command(
        &mut session,
        "graph.node.function",
        json!({"id": blur, "param": "outputsize", "base": true, "function": FunctionGraph::constant(Value::Int2([-1, 0]))}),
    );
    command(&mut session, "graph.node.disable", json!({"ids": [blur], "disabled": true}));
    let output = NodeId(command(&mut session, "graph.node.add", json!({"def": "output", "connectFrom": {"node": blur}}))["id"].as_u64().unwrap());

    assert_eq!(session.cook().nodes[&output].base.size, [64, 32]);

    command(&mut session, "graph.node.disable", json!({"ids": [blend], "disabled": true}));

    let cooked = session.cook();
    let image = cooked.image(blend, 0).unwrap();
    assert_eq!((image.w, image.h), (16, 16));
    assert_eq!(cooked.nodes[&blend].base.size, [32, 32]);
    assert_eq!(cooked.nodes[&output].base.size, [32, 32]);
}

#[test]
fn setting_a_base_literal_keeps_inheritance_and_removes_only_its_driver() {
    use materialcraft_engine::core::{BaseParameter, ParameterInheritance};

    let mut session = new_session();
    let owner = node(&mut session, "uniform");
    command(&mut session, "graph.node.function", json!({"id": owner, "base": true, "param": "format", "mode": "constant"}));
    command(&mut session, "graph.node.function", json!({"id": owner, "base": true, "param": "outputsize", "mode": "constant"}));

    session.selection.clear();
    command(&mut session, "graph.node.base", json!({"id": owner, "param": "format", "value": 3}));

    let base = &session.graph().unwrap().node(owner).unwrap().base;
    assert_eq!(BaseParameter::Format.inheritance(base), ParameterInheritance::RelativeToInput);
    assert!(!base.functions.contains_key(&BaseParameter::Format));
    assert!(base.functions.contains_key(&BaseParameter::OutputSize));
}

#[test]
fn malformed_literals_do_not_change_a_native_parameter_functions_expected_type() {
    let mut session = new_session();
    session.preview_size = Some(16);
    let owner = node(&mut session, "blur");
    session.graph_mut().unwrap().node_mut(owner).unwrap().params.insert("intensity".into(), Value::Text("invalid".into()));

    command(&mut session, "graph.node.function", json!({"id": owner, "param": "intensity", "function": FunctionGraph::constant(Value::Float(2.0))}));

    let result = &session.cook().nodes[&owner];
    assert_eq!(result.error, None);
    assert_eq!(result.parameters["intensity"], Value::Float(2.0));
}

#[test]
fn input_value_default_functions_follow_the_selected_value_type() {
    let mut session = new_session();
    session.preview_size = Some(16);
    let owner = node(&mut session, "input_value");
    session.selection.clear();
    command(&mut session, "graph.node.param", json!({"id": owner, "param": "default", "value": Value::Int2([3, 5])}));

    command(&mut session, "graph.node.function", json!({"id": owner, "param": "default", "mode": "constant"}));

    let info = command(&mut session, "graph.node.parameterInfo", json!({"id": owner, "param": "default"}));
    assert_eq!(info["expectedType"], "Integer2");
    assert!(info["warnings"].as_array().unwrap().is_empty());
    let result = &session.cook().nodes[&owner];
    assert_eq!(result.error, None);
    assert_eq!(result.outputs[0].as_value(), Some(&Value::Int2([3, 5])));
}
