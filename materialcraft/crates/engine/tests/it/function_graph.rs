use materialcraft_engine::{
    core::{FunctionGraph, GraphKind, Value},
    execute,
    formats::sbs::{read_sbs, write_sbs},
    new_session,
    nodes::function::{EvaluationContext, EvaluationState, compile},
};
use serde_json::json;

#[test]
fn incomplete_function_round_trip_preserves_nodes_without_choosing_an_output() {
    let mut session = new_session();
    execute(&mut session, "file.new", &json!({"kind": "function", "name": "Unfinished"})).unwrap();
    for operation in ["const_float1", "mul"] {
        execute(&mut session, "graph.function.edit", &json!({"edit": {"action": "add", "operation": operation, "at": [0, 0]}})).unwrap();
    }
    assert_eq!(session.graph().unwrap().function.as_ref().unwrap().output, None);

    let bytes = write_sbs(&session.pkg).unwrap();
    let read = read_sbs(&bytes, "unfinished.sbs", &|_| None).unwrap();
    assert!(read.warnings.is_empty(), "{:?}", read.warnings);
    let (_, graph) = read.pkg.graph_by_name("Unfinished").unwrap();
    let function = graph.function.as_ref().unwrap();
    assert_eq!(function.output, None);
    assert_eq!(function.nodes.iter().map(|node| node.op.as_str()).collect::<Vec<_>>(), vec!["const_float1", "mul"]);
    assert_eq!(function.nodes[1].inputs, vec![None, None]);
}

#[test]
fn standalone_function_creation_wiring_frame_movement_and_native_round_trip() {
    let mut session = new_session();
    execute(&mut session, "file.new", &json!({"kind": "function", "name": "Multiply"})).unwrap();
    assert_eq!(session.graph().unwrap().kind(), GraphKind::Function);
    assert!(session.graph().unwrap().nodes.is_empty());
    assert!(execute(&mut session, "graph.node.add", &json!({"def": "uniform"})).is_err());

    let mut function = FunctionGraph::default();
    let first = function.add_constant(Value::Float(2.0));
    let second = function.add_constant(Value::Float(3.0));
    function.output = Some(function.add("mul", &[first, second], None));
    execute(&mut session, "graph.function.set", &json!({"function": function})).unwrap();
    let original = session.pkg.clone();
    let mut cycle = function.clone();
    cycle.nodes[2].inputs[0] = cycle.output;
    assert!(execute(&mut session, "graph.function.set", &json!({"function": cycle})).is_err());
    assert_eq!(session.pkg, original);

    let frame = execute(&mut session, "graph.frame.add", &json!({"rect": [-20, -20, 500, 240]})).unwrap()["id"].as_u64().unwrap();
    execute(&mut session, "graph.comment.add", &json!({"at": [0, 120], "text": "Two times three"})).unwrap();
    execute(&mut session, "graph.frame.move", &json!({"id": frame, "by": [32, 64]})).unwrap();
    assert_eq!(session.graph().unwrap().function.as_ref().unwrap().nodes[0].pos, [32.0, 64.0]);

    let bytes = write_sbs(&session.pkg).unwrap();
    let xml = String::from_utf8(bytes.clone()).unwrap();
    assert!(xml.contains("<function><identifier v=\"Multiply\"/>"));
    let read = read_sbs(&bytes, "functions.sbs", &|_| None).unwrap();
    assert!(read.warnings.is_empty(), "{:?}", read.warnings);
    let graph = read.pkg.graphs.iter().find(|graph| graph.name == "Multiply").unwrap();
    assert_eq!(graph.kind(), GraphKind::Function);
    assert_eq!(graph.frames.len(), 1);
    assert_eq!(graph.comments[0].text, "Two times three");
    let function = graph.function.as_ref().unwrap();
    assert_eq!(function.nodes[0].pos, [32.0, 64.0]);
    let variables = Default::default();
    let context = EvaluationContext { scope: Default::default(), position: [0.0; 2], size: [1.0; 2], images: &[], tile: true, variables: &variables };
    assert_eq!(compile(function, 0).unwrap().evaluate(&context, &mut EvaluationState::default()).unwrap(), Value::Float(6.0));
}
