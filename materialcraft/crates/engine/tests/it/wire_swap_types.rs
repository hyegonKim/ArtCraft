use materialcraft_engine::{core::ValueType, execute, new_session};
use serde_json::json;

#[test]
fn swaps_cannot_bypass_connection_type_checks() {
    let mut session = new_session();
    let graph = session.graph_mut().unwrap();
    let image = graph.add_node("uniform", [0.0; 2]);
    let value = graph.add_node("input_value", [0.0; 2]);
    let blur = graph.add_node("blur", [0.0; 2]);
    let processor = graph.add_node("value_processor", [0.0; 2]);
    graph.node_mut(processor).unwrap().input_values.insert("scalar".into(), ValueType::Float);
    for (from, to, port) in [(image, blur, "input"), (value, processor, "scalar")] {
        execute(&mut session, "graph.connect", &json!({"from": from.0, "to": to.0, "toPort": port})).unwrap();
    }
    let parameters = json!({"wires": [{"to": blur.0, "toPort": "input"}, {"to": processor.0, "toPort": "scalar"}]});
    let before = session.pkg.clone();
    let history = session.history();
    assert!(!materialcraft_engine::wire_swap::available(&session, &parameters));
    assert!(execute(&mut session, "graph.wire.swap", &parameters).is_err());
    assert_eq!(session.pkg, before);
    assert_eq!(session.history(), history);
}
