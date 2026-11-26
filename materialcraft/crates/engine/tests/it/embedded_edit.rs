use materialcraft_engine::{
    FunctionGraphTarget, Session,
    core::{FunctionGraph, NodeId, PatternGraph, PatternNodeKind, Value},
    execute, new_session,
};
use serde_json::json;

fn function_session(kind: &str) -> (Session, FunctionGraphTarget) {
    let mut session = new_session();
    let target = match kind {
        "graph" => {
            execute(&mut session, "file.new", &json!({"kind": "function"})).unwrap();
            FunctionGraphTarget::Graph
        }
        "pattern" => {
            let owner = NodeId(execute(&mut session, "graph.node.add", &json!({"def": "fx_map"})).unwrap()["id"].as_u64().unwrap());
            let mut graph = PatternGraph::with_root(PatternNodeKind::Quadrant);
            let node = graph.nodes[0].id;
            graph.nodes[0].parameters.insert("patternrotation".into(), Value::Function(Box::default()));
            execute(&mut session, "graph.pattern.set", &json!({"id": owner.0, "pattern": graph})).unwrap();
            FunctionGraphTarget::Pattern { owner, node, parameter: "patternrotation".into() }
        }
        _ => {
            let node = NodeId(
                execute(&mut session, "graph.node.add", &json!({"def": "pixel_processor", "params": {"color_mode": kind == "dynamic"}})).unwrap()["id"]
                    .as_u64()
                    .unwrap(),
            );
            if kind == "dynamic" {
                let owner = session.graph_mut().unwrap().node_mut(node).unwrap();
                owner.params.insert("function".into(), Value::Function(Box::new(FunctionGraph::constant(Value::Float4([0.5; 4])))));
                owner.functions.insert("test".into(), FunctionGraph::default());
                FunctionGraphTarget::Dynamic { node, parameter: "test".into() }
            } else {
                FunctionGraphTarget::Parameter { node, parameter: "function".into() }
            }
        }
    };

    execute(&mut session, "graph.function.set", &json!({"target": target, "function": FunctionGraph::default()})).unwrap();
    session.reset_history();
    (session, target)
}

#[test]
fn dots_cannot_be_designated_as_outputs_for_any_function_target() {
    for kind in ["graph", "parameter", "dynamic", "pattern"] {
        let (mut session, target) = function_session(kind);
        let mut graph = FunctionGraph::constant(Value::Float(0.5));
        let source = graph.output.unwrap();
        let dot = graph.add("passthrough", &[source], None);
        execute(&mut session, "graph.function.set", &json!({"target": target, "function": graph})).unwrap();
        let before = session.pkg.clone();
        let history = session.history();
        assert!(execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "output", "id": dot}})).is_err(), "{kind}");
        graph.output = Some(dot);
        assert!(execute(&mut session, "graph.function.set", &json!({"target": target, "function": graph})).is_err(), "{kind}");
        assert_eq!(session.pkg, before);
        assert_eq!(session.history(), history);
    }
}

#[test]
fn function_swaps_compare_output_pin_identity_not_values() {
    for kind in ["graph", "parameter", "dynamic", "pattern"] {
        let (mut session, target) = function_session(kind);
        let mut graph = FunctionGraph::default();
        let first = graph.add_constant(Value::Float(0.5));
        let second = graph.add_constant(Value::Float(0.5));
        let subtract = graph.add("sub", &[first, first], None);
        let interpolation = graph.add("lerp", &[first], None);
        execute(&mut session, "graph.function.set", &json!({"target": target, "function": graph})).unwrap();
        let parameters = json!({"function": target, "wires": [{"to": subtract, "toPort": "0"}, {"to": interpolation, "toPort": "0"}]});
        let before = session.pkg.clone();
        let history = session.history();
        assert!(!materialcraft_engine::wire_swap::available(&session, &parameters));
        assert!(execute(&mut session, "graph.wire.swap", &parameters).is_err(), "{kind}");
        assert_eq!(session.pkg, before);
        assert_eq!(session.history(), history);

        execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "connect", "from": second, "to": subtract, "input": 0}}))
            .unwrap();
        assert!(materialcraft_engine::wire_swap::available(&session, &parameters));
        execute(&mut session, "graph.wire.swap", &parameters).unwrap();
        let graph = target.current(&session).unwrap();
        assert_eq!(graph.node(subtract).unwrap().inputs, [Some(first), Some(first)]);
        assert_eq!(graph.node(interpolation).unwrap().inputs, [Some(second)]);
    }
}

#[test]
fn material_swaps_compare_output_pin_identity_not_node_identity() {
    let mut session = new_session();
    execute(&mut session, "file.new", &json!({"template": "empty"})).unwrap();
    let graph = session.graph_mut().unwrap();
    let source = graph.add_node(materialcraft_engine::core::MISSING_NODE, [0.0, 0.0]);
    graph.node_mut(source).unwrap().params.insert("outputs".into(), Value::Text("first:gray,second:gray".into()));
    let first = graph.add_node("levels", [300.0, 0.0]);
    let second = graph.add_node("levels", [300.0, 200.0]);
    graph.connect(source, "first", first, "input").unwrap();
    graph.connect(source, "first", second, "input").unwrap();
    let parameters = json!({"wires": [{"to": first.0, "toPort": "input"}, {"to": second.0, "toPort": "input"}]});
    let before = session.pkg.clone();
    let history = session.history();
    assert!(!materialcraft_engine::wire_swap::available(&session, &parameters));
    assert!(execute(&mut session, "graph.wire.swap", &parameters).is_err());
    assert_eq!(session.pkg, before);
    assert_eq!(session.history(), history);

    session.graph_mut().unwrap().connect(source, "second", second, "input").unwrap();
    assert!(materialcraft_engine::wire_swap::available(&session, &parameters));
    execute(&mut session, "graph.wire.swap", &parameters).unwrap();
    let graph = session.graph().unwrap();
    assert_eq!(graph.input_of(first, "input").unwrap().from_port, "second");
    assert_eq!(graph.input_of(second, "input").unwrap().from_port, "first");
}

#[test]
fn pattern_links_swap_between_parents_and_reject_invalid_pairs_atomically() {
    let mut session = new_session();
    let owner = execute(&mut session, "graph.node.add", &json!({"def": "fx_map"})).unwrap()["id"].as_u64().unwrap();
    let mut graph = PatternGraph::with_root(PatternNodeKind::Switch);
    let a = graph.add(PatternNodeKind::Quadrant).unwrap();
    let b = graph.add(PatternNodeKind::Quadrant).unwrap();
    let other = graph.add(PatternNodeKind::Iterate).unwrap();
    graph.nodes[0].children = vec![Some(a), Some(b)];
    graph.nodes[3].children = vec![Some(b), None];
    execute(&mut session, "graph.pattern.set", &json!({"id": owner, "pattern": graph})).unwrap();
    let before = session.pkg.clone();
    session.reset_history();
    let wires = json!([{"to": 1, "toPort": "output0"}, {"to": other, "toPort": "output0"}]);
    let parameters = json!({"pattern": owner, "wires": wires});
    execute(&mut session, "graph.wire.swap", &parameters).unwrap();
    let Value::PatternGraph(swapped) = &session.graph().unwrap().node(NodeId(owner)).unwrap().params["pattern"] else { panic!() };
    assert_eq!(swapped.nodes[0].children, vec![Some(b), Some(b)]);
    assert_eq!(swapped.nodes[3].children, vec![Some(a), None]);
    assert_eq!(swapped.root, graph.root);
    assert_eq!(session.history().len(), 1);
    session.undo().unwrap();
    assert_eq!(session.pkg, before);
    session.redo().unwrap();
    execute(&mut session, "graph.wire.swap", &parameters).unwrap();
    assert_eq!(session.pkg, before);

    for wires in [
        json!([]),
        json!([{"to": 1, "toPort": "output0"}]),
        json!([{"to": 1, "toPort": "output0"}, {"to": 1, "toPort": "output0"}]),
        json!([{"to": 1, "toPort": "output0"}, {"to": 1, "toPort": "output1"}, {"to": other, "toPort": "output0"}]),
        json!([{"to": 1, "toPort": "output0"}, {"to": a, "toPort": "output0"}]),
        json!([{"to": 1, "toPort": "output0"}, {"to": u32::MAX, "toPort": "output0"}]),
        json!([{"to": 1, "toPort": "output0"}, {"to": other, "toPort": "invalid"}]),
    ] {
        let history = session.history();
        assert!(execute(&mut session, "graph.wire.swap", &json!({"pattern": owner, "wires": wires})).is_err());
        assert_eq!(session.pkg, before);
        assert_eq!(session.history(), history);
    }
    execute(&mut session, "graph.wire.delete", &json!({"pattern": owner, "wires": wires})).unwrap();
    session.undo().unwrap();
    assert_eq!(session.pkg, before);

    execute(&mut session, "graph.pattern.edit", &json!({"id": owner, "edit": {"action": "connect", "parent": a, "slot": 0, "child": b}})).unwrap();
    let before = session.pkg.clone();
    let history = session.history();
    let cycle = json!({"pattern": owner, "wires": [{"to": 1, "toPort": "output0"}, {"to": a, "toPort": "output0"}]});
    assert!(execute(&mut session, "graph.wire.swap", &cycle).is_err());
    assert_eq!(session.pkg, before);
    assert_eq!(session.history(), history);
}

#[test]
fn selected_function_links_swap_sources_and_delete_in_one_undo_step() {
    for kind in ["graph", "parameter", "dynamic", "pattern"] {
        let (mut session, target) = function_session(kind);
        let mut graph = FunctionGraph::default();
        let a = graph.add_constant(Value::Float3([1.0, 2.0, 3.0]));
        let b = graph.add_constant(Value::Float3([4.0, 5.0, 6.0]));
        let subtract = graph.add("sub", &[a, b], None);
        let other = graph.add("passthrough", &[b], None);
        graph.output = Some(subtract);
        execute(&mut session, "graph.function.set", &json!({"target": target, "function": graph})).unwrap();
        session.reset_history();
        let before = session.pkg.clone();
        let parameters = json!({"function": target, "wires": [{"to": subtract, "toPort": "0"}, {"to": subtract, "toPort": "1"}]});

        execute(&mut session, "graph.wire.swap", &parameters).unwrap();
        let swapped = target.current(&session).unwrap();
        assert_eq!(swapped.node(subtract).unwrap().inputs, vec![Some(b), Some(a)]);
        assert_eq!(swapped.node(other).unwrap().inputs, vec![Some(b)]);
        assert_eq!(swapped.output, Some(subtract));
        assert_eq!(session.history().len(), 1);
        session.undo().unwrap();
        assert_eq!(session.pkg, before);
        session.redo().unwrap();
        execute(&mut session, "graph.wire.swap", &parameters).unwrap();
        assert_eq!(session.pkg, before);

        execute(&mut session, "graph.wire.delete", &parameters).unwrap();
        assert_eq!(target.current(&session).unwrap().node(subtract).unwrap().inputs, vec![None, None]);
        session.undo().unwrap();
        assert_eq!(session.pkg, before);

        let selection = session.selection.clone();
        let parameters = json!({"function": target, "wires": [{"to": subtract, "toPort": "0"}, {"to": other, "toPort": "0"}]});
        execute(&mut session, "graph.wire.swap", &parameters).unwrap();
        let swapped = target.current(&session).unwrap();
        assert_eq!(swapped.node(subtract).unwrap().inputs, vec![Some(b), Some(b)]);
        assert_eq!(swapped.node(other).unwrap().inputs, vec![Some(a)]);
        assert_eq!(session.selection, selection);
        session.undo().unwrap();
        assert_eq!(session.pkg, before);
    }
}

#[test]
fn function_link_swap_rejects_cycles_and_invalid_selections_atomically() {
    let (mut session, target) = function_session("graph");
    let mut graph = FunctionGraph::default();
    let scalar = graph.add_constant(Value::Float(1.0));
    let vector = graph.add_constant(Value::Float2([1.0, 2.0]));
    let first = graph.add("add", &[scalar, scalar], None);
    let second = graph.add("add", &[first, scalar], None);
    let vector_sum = graph.add("add", &[vector, vector], None);
    execute(&mut session, "graph.function.set", &json!({"function": graph})).unwrap();
    let package = session.pkg.clone();
    let history = session.history();
    for wires in [
        json!([]),
        json!([{"to": first, "toPort": "0"}]),
        json!([{"to": first, "toPort": "0"}, {"to": first, "toPort": "0"}]),
        json!([{"to": first, "toPort": "0"}, {"to": first, "toPort": "1"}, {"to": second, "toPort": "1"}]),
        json!([{"to": first, "toPort": "0"}, {"to": second, "toPort": "0"}]),
        json!([{"to": first, "toPort": "0"}, {"to": u32::MAX, "toPort": "0"}]),
        json!([{"to": first, "toPort": "0"}, {"to": second, "toPort": u64::MAX}]),
    ] {
        assert!(execute(&mut session, "graph.wire.swap", &json!({"function": target, "wires": wires})).is_err(), "{wires}");
        assert_eq!(session.pkg, package);
        assert_eq!(session.history(), history);
    }
    let parameters = json!({"wires": [{"to": second, "toPort": "1"}, {"to": vector_sum, "toPort": "1"}]});
    execute(&mut session, "graph.wire.delete", &parameters).unwrap();
    let package = session.pkg.clone();
    let history = session.history();
    assert!(execute(&mut session, "graph.wire.swap", &parameters).is_err());
    assert_eq!(session.pkg, package);
    assert_eq!(session.history(), history);
}

#[test]
fn creation_and_connections_use_the_same_rules_for_every_function_target() {
    for kind in ["graph", "parameter", "dynamic", "pattern"] {
        let (mut session, target) = function_session(kind);
        let mut ids = Vec::new();
        for operation in ["const_float1", "mul"] {
            let result = execute(
                &mut session,
                "graph.function.edit",
                &json!({"target": target, "edit": {"action": "add", "operation": operation, "at": [19, -19], "snap": true}}),
            )
            .unwrap();
            ids.push(result["id"].as_u64().unwrap() as u32);
        }
        let graph = target.current(&session).unwrap();
        assert_eq!(graph.node(ids[1]).unwrap().inputs, vec![None, None]);
        assert_eq!(graph.node(ids[1]).unwrap().pos, [32.0, -32.0]);
        assert_eq!(graph.output, None);
        execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "connect", "from": ids[0], "to": ids[1], "input": 0}}))
            .unwrap();
        assert_eq!(target.current(&session).unwrap().node(ids[1]).unwrap().inputs[0], Some(ids[0]));
        let before = session.pkg.clone();
        for edit in [
            json!({"action": "connect", "from": ids[1], "to": ids[1], "input": 0}),
            json!({"action": "connect", "from": ids[0], "to": ids[1], "input": 1000000}),
            json!({"action": "add", "operation": "unknown", "at": [0, 0]}),
        ] {
            assert!(execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": edit})).is_err());
            assert_eq!(session.pkg, before);
        }
    }
}

#[test]
fn output_designation_is_explicit_and_undoable_for_every_function_target() {
    for kind in ["graph", "parameter", "dynamic", "pattern"] {
        let (mut session, target) = function_session(kind);
        let mut ids = Vec::new();
        for operation in ["const_float1", "neg"] {
            let result =
                execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "add", "operation": operation, "at": [0, 0]}}))
                    .unwrap();
            ids.push(result["id"].as_u64().unwrap() as u32);
        }
        assert_eq!(target.current(&session).unwrap().output, None, "{kind}");

        for id in &ids {
            execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "output", "id": id}})).unwrap();
            assert_eq!(target.current(&session).unwrap().output, Some(*id));
        }
        session.undo().unwrap();
        assert_eq!(target.current(&session).unwrap().output, Some(ids[0]));
        session.redo().unwrap();
        assert_eq!(target.current(&session).unwrap().output, Some(ids[1]));

        let package = session.pkg.clone();
        let history = session.history();
        assert!(execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "output", "id": u32::MAX}})).is_err());
        assert_eq!(session.pkg, package);
        assert_eq!(session.history(), history);

        execute(&mut session, "edit.delete", &json!({"function": target, "ids": [ids[1]]})).unwrap();
        assert_eq!(target.current(&session).unwrap().output, None);
        execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "add", "operation": "const_float1", "at": [0, 0]}})).unwrap();
        assert_eq!(target.current(&session).unwrap().output, None, "adding a node must not replace a deleted output");

        session.undo().unwrap();
        session.undo().unwrap();
        assert_eq!(session.pkg, package);
    }
}

#[test]
fn function_node_info_uses_the_explicit_target_without_editing_the_document() {
    for kind in ["graph", "parameter", "dynamic", "pattern"] {
        let (mut session, target) = function_session(kind);
        let added =
            execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "add", "operation": "const_float1", "at": [0, 0]}}))
                .unwrap();
        let id = added["id"].as_u64().unwrap();
        execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "output", "id": id}})).unwrap();
        let package = session.pkg.clone();
        let history = session.history();
        let result = execute(&mut session, "graph.function.node.info", &json!({"target": target, "id": id})).unwrap();
        let info: serde_json::Value = serde_json::from_str(result["text"].as_str().unwrap()).unwrap();
        assert_eq!(info["node"]["id"], id);
        assert_eq!(info["node"]["op"], "const_float1");
        assert_eq!(info["label"], "Float");
        assert_eq!(info["output"], true);
        assert_eq!(info["target"], json!(target));
        for id in [u64::MAX, 999] {
            assert!(execute(&mut session, "graph.function.node.info", &json!({"target": target, "id": id})).is_err());
        }
        assert_eq!(session.pkg, package);
        assert_eq!(session.history(), history);
    }
}

#[test]
fn incomplete_function_warnings_follow_edits_for_every_target() {
    for kind in ["graph", "parameter", "dynamic", "pattern"] {
        let (mut session, target) = function_session(kind);
        let mut graph = FunctionGraph::default();
        let source = graph.add_constant(Value::Float(0.5));
        let multiply = graph.add("mul", &[], None);
        graph.add("add", &[], None);
        execute(&mut session, "graph.function.set", &json!({"target": target, "function": graph})).unwrap();
        let warning = "Output node has no connected value source";
        let expected = json!([{"target": target, "messages": ["No output node defined"]}]);
        assert_eq!(materialcraft_engine::inspect::session(&session, None)["graph"]["functionWarnings"], expected, "{kind}");

        execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "output", "id": multiply}})).unwrap();
        assert_eq!(materialcraft_engine::inspect::session(&session, None)["graph"]["functionWarnings"], json!([{"target": target, "messages": [warning]}]));
        execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "connect", "from": source, "to": multiply, "input": 1}}))
            .unwrap();
        assert_eq!(materialcraft_engine::inspect::session(&session, None)["graph"]["functionWarnings"], json!([]));

        session.undo().unwrap();
        session.undo().unwrap();
        assert_eq!(materialcraft_engine::inspect::session(&session, None)["graph"]["functionWarnings"], expected);
    }
}

#[test]
fn value_gestures_merge_but_separate_edits_undo_independently() {
    for kind in ["graph", "parameter", "dynamic", "pattern"] {
        let (mut session, target) = function_session(kind);
        let added =
            execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "add", "operation": "const_float1", "at": [0, 0]}}))
                .unwrap();
        let id = added["id"].as_u64().unwrap();
        let initial = target.current(&session).unwrap().node(id as u32).unwrap().value.clone();
        session.reset_history();
        for value in [1.0, 2.0, 3.0] {
            execute(
                &mut session,
                "graph.function.edit",
                &json!({"target": target, "merge": true, "edit": {"action": "value", "id": id, "value": Value::Float(value)}}),
            )
            .unwrap();
        }
        assert_eq!(session.history().len(), 1, "{kind}");
        execute(&mut session, "edit.finish", &json!({})).unwrap();
        for value in [4.0, 5.0] {
            execute(
                &mut session,
                "graph.function.edit",
                &json!({"target": target, "merge": true, "edit": {"action": "value", "id": id, "value": Value::Float(value)}}),
            )
            .unwrap();
        }
        assert_eq!(session.history().len(), 2, "{kind}");
        session.undo();
        assert_eq!(target.current(&session).unwrap().node(id as u32).unwrap().value, Some(Value::Float(3.0)));
        session.undo();
        assert_eq!(target.current(&session).unwrap().node(id as u32).unwrap().value, initial);
        for _ in 0..2 {
            execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "add", "operation": "mul", "at": [0, 0]}})).unwrap();
        }
        session.undo();
        assert_eq!(target.current(&session).unwrap().nodes.len(), 2);
    }
}

#[test]
fn pattern_value_gestures_and_invalid_connections_are_atomic() {
    let mut session = new_session();
    let owner = execute(&mut session, "graph.node.add", &json!({"def": "fx_map"})).unwrap()["id"].as_u64().unwrap();
    let result = execute(&mut session, "graph.pattern.edit", &json!({"id": owner, "edit": {"action": "add", "kind": "paramset", "at": [0, 0]}})).unwrap();
    let id = result["id"].as_u64().unwrap();
    session.reset_history();
    for value in [1.0, 2.0, 3.0] {
        execute(
            &mut session,
            "graph.pattern.edit",
            &json!({"id": owner, "merge": true, "edit": {"action": "value", "id": id, "parameter": "patternrotation", "value": Value::Float(value)}}),
        )
        .unwrap();
    }
    assert_eq!(session.history().len(), 1);
    execute(&mut session, "edit.finish", &json!({})).unwrap();
    execute(
        &mut session,
        "graph.pattern.edit",
        &json!({"id": owner, "merge": true, "edit": {"action": "value", "id": id, "parameter": "patternrotation", "value": Value::Float(4.0)}}),
    )
    .unwrap();
    assert_eq!(session.history().len(), 2);
    let before = session.pkg.clone();
    assert!(execute(&mut session, "graph.pattern.edit", &json!({"id": owner, "edit": {"action": "connect", "parent": id, "slot": 0, "child": id}})).is_err());
    assert_eq!(session.pkg, before);
    assert!(execute(&mut session, "graph.function.edit", &json!({"target": {"kind": "pattern", "owner": owner, "node": id, "parameter": "absent"}, "edit": {"action": "add", "operation": "mul", "at": [0, 0]}})).is_err());
    assert_eq!(session.pkg, before);
}

#[test]
fn selection_commands_work_without_a_ui_for_every_function_target() {
    for kind in ["graph", "parameter", "dynamic", "pattern"] {
        let (mut session, target) = function_session(kind);
        for operation in ["const_float1", "passthrough"] {
            execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "add", "operation": operation, "at": [0, 0]}})).unwrap();
        }
        let target_parameters = if kind == "graph" { json!({}) } else { json!({"function": target}) };
        let parent_selection = session.selection.clone();
        session.reset_history();
        let selected = execute(&mut session, "edit.selectAll", &target_parameters).unwrap();
        assert_eq!(selected["selected"], 2, "{kind}");
        let mut parameters = target_parameters.clone();
        parameters["ids"] = selected["ids"].clone();
        let copied = execute(&mut session, "edit.copy", &parameters).unwrap();
        assert_eq!(copied["copied"], 2);
        assert!(session.history().is_empty());
        let duplicated = execute(&mut session, "edit.duplicate", &parameters).unwrap();
        assert_eq!(target.current(&session).unwrap().nodes.len(), 4);
        parameters["ids"] = duplicated["ids"].clone();
        execute(&mut session, "edit.delete", &parameters).unwrap();
        assert_eq!(target.current(&session).unwrap().nodes.len(), 2);
        parameters["ids"] = json!([]);
        parameters["clip"] = copied["clip"].clone();
        execute(&mut session, "edit.paste", &parameters).unwrap();
        assert_eq!(target.current(&session).unwrap().nodes.len(), 4);
        if kind != "graph" {
            assert_eq!(session.selection, parent_selection, "embedded commands changed the parent selection");
        }
    }
}

#[test]
fn rejected_function_duplicate_preserves_document_selection_clipboard_and_history() {
    let (mut session, target) = function_session("graph");
    let mut graph = FunctionGraph::default();
    for _ in 0..2049 {
        graph.add("passthrough", &[], None);
    }
    execute(&mut session, "graph.function.set", &json!({"function": graph})).unwrap();
    execute(&mut session, "edit.selectAll", &json!({})).unwrap();
    execute(&mut session, "edit.copy", &json!({})).unwrap();
    let before = session.pkg.clone();
    let selection = session.selection.clone();
    let clipboard = serde_json::to_value(&session.embedded_clipboard).unwrap();
    let history = session.history();
    assert!(execute(&mut session, "edit.duplicate", &json!({})).is_err());
    assert_eq!(session.pkg, before);
    assert_eq!(session.selection, selection);
    assert_eq!(serde_json::to_value(&session.embedded_clipboard).unwrap(), clipboard);
    assert_eq!(session.history(), history);
    assert_eq!(target.current(&session).unwrap().nodes.len(), 2049);
}

#[test]
fn graph_validation_is_shared_by_creation_legacy_setters_and_targeted_edits() {
    let mut cyclic = FunctionGraph::default();
    let id = cyclic.add("passthrough", &[], None);
    cyclic.nodes[0].inputs = vec![Some(id)];
    cyclic.output = Some(id);
    let mut session = new_session();
    let owner = execute(&mut session, "graph.node.add", &json!({"def": "pixel_processor"})).unwrap()["id"].as_u64().unwrap();
    let before = session.pkg.clone();
    for (command, parameters) in [
        ("graph.node.add", json!({"def": "pixel_processor", "params": {"function": Value::Function(Box::new(cyclic.clone()))}})),
        ("graph.node.param", json!({"id": owner, "param": "function", "value": Value::Function(Box::new(cyclic.clone()))})),
        ("graph.node.function", json!({"id": owner, "param": "color_mode", "function": cyclic})),
        ("graph.function.set", json!({"target": {"kind": "parameter", "node": owner, "parameter": "function"}, "function": cyclic})),
    ] {
        let error = execute(&mut session, command, &parameters).unwrap_err().to_string();
        assert!(error.contains("cycle"), "{command}: {error}");
        assert_eq!(session.pkg, before);
    }
    let owner = execute(&mut session, "graph.node.add", &json!({"def": "fx_map"})).unwrap()["id"].as_u64().unwrap();
    let mut graph = PatternGraph::with_root(PatternNodeKind::Quadrant);
    graph.nodes[0].parameters.insert("patternrotation".into(), Value::Function(Box::new(cyclic.clone())));
    let before = session.pkg.clone();
    for (command, parameters) in [
        ("graph.pattern.set", json!({"id": owner, "pattern": graph})),
        ("graph.node.param", json!({"id": owner, "param": "pattern", "value": Value::PatternGraph(Box::new(graph))})),
        (
            "graph.pattern.edit",
            json!({"id": owner, "edit": {"action": "value", "id": 1, "parameter": "patternrotation", "value": Value::Function(Box::new(cyclic))}}),
        ),
    ] {
        assert!(execute(&mut session, command, &parameters).unwrap_err().to_string().contains("cycle"), "{command}");
        assert_eq!(session.pkg, before);
    }
}

#[test]
fn pattern_selection_commands_and_cross_graph_clipboards_keep_ownership() {
    let mut session = new_session();
    let owner = execute(&mut session, "graph.node.add", &json!({"def": "fx_map"})).unwrap()["id"].as_u64().unwrap();
    let selected = execute(&mut session, "edit.selectAll", &json!({"pattern": owner})).unwrap();
    let parameters = json!({"pattern": owner, "ids": selected["ids"]});
    execute(&mut session, "edit.copy", &parameters).unwrap();
    let before = session.pkg.clone();
    assert!(execute(&mut session, "edit.deleteRelink", &parameters).is_err());
    assert!(execute(&mut session, "edit.delete", &json!({"pattern": owner, "ids": [1], "graph": 99})).is_err());
    assert_eq!(session.pkg, before);
    execute(&mut session, "edit.duplicate", &parameters).unwrap();
    assert_eq!(session.selection, vec![NodeId(owner)]);

    let (mut session, _) = function_session("graph");
    let mut graph = FunctionGraph::default();
    let source = graph.add_constant(Value::Float(1.0));
    let child = graph.add("passthrough", &[source], None);
    execute(&mut session, "graph.function.set", &json!({"function": graph})).unwrap();
    let copied = execute(&mut session, "edit.copy", &json!({"ids": [child]})).unwrap();
    execute(&mut session, "graph.duplicate", &json!({})).unwrap();
    let pasted = execute(&mut session, "edit.paste", &json!({"clip": copied["clip"]})).unwrap();
    let id = pasted["ids"][0].as_u64().unwrap() as u32;
    assert_eq!(session.graph().unwrap().function.as_ref().unwrap().node(id).unwrap().inputs, vec![None]);
}

#[test]
fn alignment_targets_embedded_functions_and_undo_restores_them() {
    for kind in ["parameter", "dynamic", "pattern"] {
        let (mut session, target) = function_session(kind);
        for at in [[10, 30], [50, 90]] {
            execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "add", "operation": "const_float1", "at": at}})).unwrap();
        }
        let before = session.pkg.clone();
        execute(&mut session, "graph.align", &json!({"function": target, "ids": [1, 2], "mode": "left"})).unwrap();
        let positions: Vec<_> = target.current(&session).unwrap().nodes.iter().map(|node| node.pos).collect();
        assert_eq!(positions, vec![[10.0, 30.0], [10.0, 90.0]]);
        session.undo().unwrap();
        assert_eq!(session.pkg, before);
    }
}

#[test]
fn replacing_a_package_discards_embedded_clipboard_provenance() {
    let (mut session, _) = function_session("graph");
    execute(&mut session, "graph.function.edit", &json!({"edit": {"action": "add", "operation": "const_float1", "at": [0, 0]}})).unwrap();
    execute(&mut session, "edit.selectAll", &json!({})).unwrap();
    execute(&mut session, "edit.copy", &json!({})).unwrap();
    assert!(session.embedded_clipboard.is_some());
    let package = session.pkg.clone();
    materialcraft_engine::file_cmds::replace_package(&mut session, package, None);
    assert!(session.embedded_clipboard.is_none());
}

#[test]
fn processor_node_availability_is_shared_and_preserves_existing_sampler_graphs() {
    for definition in ["pixel_processor", "value_processor"] {
        let mut session = new_session();
        let node = NodeId(execute(&mut session, "graph.node.add", &json!({"def": definition})).unwrap()["id"].as_u64().unwrap());
        let target = FunctionGraphTarget::Parameter { node, parameter: "function".into() };
        let operations: Vec<_> = target.available_operations(&session).map(|operation| operation.id).collect();
        let full_catalog = materialcraft_engine::nodes::function::FUNCTIONS;
        assert_eq!(operations.len(), full_catalog.len());
        assert!(operations.contains(&"add"));
        for sampler in ["samplelum", "samplecol"] {
            assert!(operations.contains(&sampler));
            let original = session.pkg.clone();

            execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "add", "operation": sampler, "at": [0, 0]}})).unwrap();

            let inserted = session.pkg.clone();
            session.undo().unwrap();
            assert_eq!(session.pkg, original);
            session.redo().unwrap();
            assert_eq!(session.pkg, inserted);
        }
        let mut existing = FunctionGraph::default();
        existing.add("samplelum", &[], Some(Value::Int2([0, 1])));
        execute(&mut session, "graph.function.set", &json!({"target": target, "function": existing})).unwrap();
        assert_eq!(target.current(&session).unwrap().nodes[0].op, "samplelum");
    }
    for kind in ["graph", "pattern", "dynamic"] {
        let (session, target) = function_session(kind);
        assert_eq!(target.available_operations(&session).count(), materialcraft_engine::nodes::function::FUNCTIONS.len());
    }
}

#[test]
fn library_samplers_read_image_inputs_in_both_processors() {
    for definition in ["pixel_processor", "value_processor"] {
        for sampler in ["samplelum", "samplecol"] {
            let mut session = new_session();
            session.preview_size = Some(16);
            let color = [0.25, 0.5, 0.75, 1.0];
            let image = NodeId(
                execute(&mut session, "graph.node.add", &json!({"def": "uniform", "params": {"color_mode": true, "color": color}})).unwrap()["id"]
                    .as_u64()
                    .unwrap(),
            );
            let parameters = if definition == "pixel_processor" { json!({"color_mode": sampler == "samplecol"}) } else { json!({}) };
            let owner = NodeId(execute(&mut session, "graph.node.add", &json!({"def": definition, "params": parameters})).unwrap()["id"].as_u64().unwrap());
            session.graph_mut().unwrap().connect(image, "output", owner, "input0").unwrap();
            let target = FunctionGraphTarget::Parameter { node: owner, parameter: "function".into() };
            let mut function = FunctionGraph::default();
            let position = function.add_constant(Value::Float2([0.5; 2]));
            execute(&mut session, "graph.function.set", &json!({"target": target, "function": function})).unwrap();

            let result = execute(&mut session, "library.add", &json!({"entry": format!("function:{sampler}"), "function": target, "at": [160, 0]})).unwrap();
            let id = result["result"]["id"].as_u64().unwrap();
            execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "connect", "from": position, "to": id, "input": 0}}))
                .unwrap();
            execute(&mut session, "graph.function.edit", &json!({"target": target, "edit": {"action": "output", "id": id}})).unwrap();
            session.cook();

            if definition == "value_processor" {
                let expected = if sampler == "samplecol" { Value::Float4(color) } else { Value::Float(color[0]) };
                assert_eq!(session.cooked.value(owner, 0), Some(&expected));
            } else {
                let expected = if sampler == "samplecol" { color } else { [color[0], color[0], color[0], 1.0] };
                assert_eq!(session.cooked.image(owner, 0).unwrap().get(0, 0, false), expected);
            }
        }
    }
}
