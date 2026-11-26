use materialcraft_engine::{
    FunctionGraphTarget, Session,
    core::{FunctionGraph, NodeId, PatternGraph, PatternNodeKind, Value},
    execute, new_session,
};
use serde_json::json;

fn target_session(kind: &str) -> (Session, FunctionGraphTarget) {
    let mut session = new_session();
    let target = match kind {
        "dynamic" => {
            let node = session.graph_mut().unwrap().add_node("blur", [0.0; 2]);
            session.graph_mut().unwrap().node_mut(node).unwrap().functions.insert("intensity".into(), FunctionGraph::default());
            FunctionGraphTarget::Dynamic { node, parameter: "intensity".into() }
        }
        "pattern" => {
            let owner = session.graph_mut().unwrap().add_node("fx_map", [0.0; 2]);
            let mut pattern = PatternGraph::with_root(PatternNodeKind::Quadrant);
            pattern.nodes[0].parameters.insert("patternrotation".into(), Value::Function(Box::default()));
            session.graph_mut().unwrap().node_mut(owner).unwrap().params.insert("pattern".into(), Value::PatternGraph(Box::new(pattern)));
            FunctionGraphTarget::Pattern { owner, node: 1, parameter: "patternrotation".into() }
        }
        _ => {
            let parameters = if kind == "pixel_processor" { json!({"color_mode": false}) } else { json!({}) };
            let node = NodeId(execute(&mut session, "graph.node.add", &json!({"def": kind, "params": parameters})).unwrap()["id"].as_u64().unwrap());
            FunctionGraphTarget::Parameter { node, parameter: "function".into() }
        }
    };
    (session, target)
}

#[test]
fn selecting_an_output_checks_the_target_parameter_type() {
    for kind in ["dynamic", "pattern", "pixel_processor"] {
        let (mut session, target) = target_session(kind);
        let mut function = FunctionGraph::default();
        let scalar = function.add_constant(Value::Float(0.5));
        let vector = function.add_constant(Value::Float3([0.5; 3]));
        execute(&mut session, "graph.function.set", &json!({"target": target, "function": function})).unwrap();
        let before = session.pkg.clone();
        let history = session.history();
        assert!(execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "output", "id": vector}})).is_err(), "{kind}");
        assert_eq!(session.pkg, before);
        assert_eq!(session.history(), history);
        execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "output", "id": scalar}})).unwrap();
        assert!(materialcraft_engine::function_inspection::warnings(session.graph().unwrap()).is_empty());
    }
}

#[test]
fn mismatched_existing_outputs_warn_and_remain_editable() {
    let (mut session, target) = target_session("dynamic");
    let function = FunctionGraph::constant(Value::Float3([0.5; 3]));
    execute(&mut session, "graph.function.set", &json!({"target": target, "function": function})).unwrap();
    let warnings = materialcraft_engine::function_inspection::warnings(session.graph().unwrap());
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].1[0].contains("Float3"));
    assert!(warnings[0].1[0].contains("expected Float"));
    execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "move", "ids": [1], "by": [10, 0]}})).unwrap();
}

#[test]
fn value_processor_outputs_have_no_parameter_type_constraint() {
    let (mut session, target) = target_session("value_processor");
    for value in [Value::Float3([0.5; 3]), Value::Int(2), Value::Bool(true)] {
        let mut function = FunctionGraph::default();
        let output = function.add_constant(value);
        execute(&mut session, "graph.function.set", &json!({"target": target, "function": function})).unwrap();
        execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "output", "id": output}})).unwrap();
        assert!(materialcraft_engine::function_inspection::warnings(session.graph().unwrap()).is_empty());
    }
}
