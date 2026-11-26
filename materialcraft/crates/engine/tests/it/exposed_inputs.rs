use materialcraft_engine::{core::NodeId, execute, new_session};
use serde_json::json;

#[test]
fn exposing_the_same_parameter_on_two_nodes_gives_each_its_own_input() {
    let mut s = new_session();
    let nodes: Vec<NodeId> = (0..3).map(|_| NodeId(execute(&mut s, "graph.node.add", &json!({"def": "blend"})).unwrap()["id"].as_u64().unwrap())).collect();

    // The properties panel's "Expose as graph input" sends no `id`
    let ids: Vec<String> = nodes
        .iter()
        .map(|node| {
            let reply = execute(&mut s, "graph.exposed.add", &json!({"label": "Opacity", "bind": {"node": node.0, "param": "opacity"}})).unwrap();
            reply["id"].as_str().unwrap().to_string()
        })
        .collect();
    assert_eq!(ids, ["opacity", "opacity_1", "opacity_2"]);

    let graph = s.graph().unwrap();
    assert_eq!(graph.exposed.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ids);
    for (node, id) in nodes.iter().zip(&ids) {
        assert_eq!(graph.node(*node).unwrap().functions["opacity"].input_identifier(), Some(id.as_str()));
    }

    // An explicit id still has to be free, and an unbound input still needs one
    assert!(execute(&mut s, "graph.exposed.add", &json!({"id": "opacity_1", "bind": {"node": nodes[0].0, "param": "opacity"}})).is_err());
    assert!(execute(&mut s, "graph.exposed.add", &json!({"default": 0.5})).is_err());
}

#[test]
fn base_parameters_cannot_be_exposed_as_graph_inputs() {
    let mut session = new_session();
    let owner = execute(&mut session, "graph.node.add", &json!({"def": "blur"})).unwrap()["id"].clone();
    let original = session.graph().unwrap().clone();

    let result = execute(&mut session, "graph.exposed.add", &json!({"bind": {"node": owner, "param": "randomseed", "base": true}}));

    assert!(result.is_err(), "exposure belongs to Specific Parameters");
    assert_eq!(*session.graph().unwrap(), original);
}

#[test]
fn exposed_input_settings_survive_sbs_and_undo() {
    let mut session = new_session();
    let owner = execute(&mut session, "graph.node.add", &json!({"def": "blur"})).unwrap()["id"].clone();
    let original = session.graph().unwrap().clone();
    execute(
        &mut session,
        "graph.exposed.add",
        &json!({
            "id": "radius", "label": "Radius", "group": "Surface/Blur", "description": "Controls the radius",
            "editor": "input", "default": 10.0, "min": 0.0, "max": 32.0, "clamp": true, "step": 0.25,
            "bind": {"node": owner, "param": "intensity"}
        }),
    )
    .unwrap();

    let input = serde_json::to_value(&session.graph().unwrap().exposed[0]).unwrap();
    assert_eq!(input["description"], "Controls the radius");
    assert_eq!(input["editor"], "input");
    assert_eq!(input["clamp"], true);
    assert_eq!(input["step"], 0.25);
    let bytes = materialcraft_engine::formats::sbs::write_sbs(&session.pkg).unwrap();
    let reopened = materialcraft_engine::formats::sbs::read_sbs(&bytes, "exposed.sbs", &|_| None).unwrap();
    assert_eq!(reopened.pkg.graphs[0].exposed, session.graph().unwrap().exposed);

    execute(&mut session, "edit.undo", &json!({})).unwrap();
    assert_eq!(*session.graph().unwrap(), original);
}

#[test]
fn preparing_an_input_is_read_only_and_invalid_settings_are_atomic() {
    let mut session = new_session();
    let owner = execute(&mut session, "graph.node.add", &json!({"def": "blur"})).unwrap()["id"].clone();
    execute(&mut session, "graph.node.param", &json!({"id": owner, "param": "intensity", "value": 10.0})).unwrap();
    let original = session.graph().unwrap().clone();
    let binding = json!({"node": owner, "param": "intensity"});

    let draft = execute(&mut session, "graph.exposed.prepare", &json!({"bind": binding})).unwrap();

    assert_eq!(draft["id"], "intensity");
    assert_eq!(draft["label"], "Intensity");
    assert_eq!(draft["default"], json!({"t": "float", "v": 10.0}));
    assert_eq!(*session.graph().unwrap(), original);
    for changes in [
        json!({"step": 0.0}),
        json!({"step": -0.01}),
        json!({"min": 3.0, "max": 2.0}),
        json!({"id": "bad id"}),
        json!({"editor": "toggle"}),
        json!({"default": {"t": "float", "v": 1e39}}),
    ] {
        let mut parameters = changes;
        parameters["bind"] = binding.clone();
        assert!(execute(&mut session, "graph.exposed.add", &parameters).is_err(), "{parameters}");
        assert_eq!(*session.graph().unwrap(), original);
    }
}

#[test]
fn color_and_enum_exposure_preserve_their_editors_and_defaults() {
    let mut session = new_session();
    let uniform = execute(&mut session, "graph.node.add", &json!({"def": "uniform"})).unwrap()["id"].clone();
    let blend = execute(&mut session, "graph.node.add", &json!({"def": "blend"})).unwrap()["id"].clone();
    execute(&mut session, "graph.exposed.add", &json!({"editor": "input", "bind": {"node": uniform, "param": "color"}})).unwrap();
    execute(&mut session, "graph.exposed.add", &json!({"bind": {"node": blend, "param": "mode"}})).unwrap();

    let bytes = materialcraft_engine::formats::sbs::write_sbs(&session.pkg).unwrap();
    let reopened = materialcraft_engine::formats::sbs::read_sbs(&bytes, "editors.sbs", &|_| None).unwrap();

    let mut inputs = reopened.pkg.graphs[0].exposed.clone();
    let expected = &session.graph().unwrap().exposed;
    let color = inputs[0].default.as_vec4().unwrap();
    let original = expected[0].default.as_vec4().unwrap();
    assert!(color.iter().zip(original).all(|(left, right)| (left - right).abs() < 1e-6));
    inputs[0].default = expected[0].default.clone();
    assert_eq!(&inputs, expected);
    assert_eq!(reopened.pkg.graphs[0].exposed[0].editor(), materialcraft_engine::core::ParameterEditor::Input);
    assert_eq!(reopened.pkg.graphs[0].exposed[1].editor(), materialcraft_engine::core::ParameterEditor::Dropdown);
}

#[test]
fn deleting_an_exposed_input_detaches_its_parameter_functions_and_undo_restores_them() {
    let mut session = new_session();
    let owner = NodeId(execute(&mut session, "graph.node.add", &json!({"def": "blur"})).unwrap()["id"].as_u64().unwrap());
    execute(&mut session, "graph.node.param", &json!({"id": owner, "param": "intensity", "value": 2.0})).unwrap();
    execute(&mut session, "graph.exposed.add", &json!({"id": "radius", "default": 3.0, "bind": {"node": owner, "param": "intensity"}})).unwrap();
    let original = session.graph().unwrap().clone();

    execute(&mut session, "graph.exposed.delete", &json!({"id": "radius"})).unwrap();
    session.cook();

    assert_eq!(session.cooked.nodes[&owner].error, None);
    assert!(session.graph().unwrap().node(owner).unwrap().functions.is_empty());
    assert_eq!(session.cooked.nodes[&owner].parameters.get("intensity"), Some(&materialcraft_engine::core::Value::Float(2.0)));
    execute(&mut session, "edit.undo", &json!({})).unwrap();
    assert_eq!(*session.graph().unwrap(), original);
}
