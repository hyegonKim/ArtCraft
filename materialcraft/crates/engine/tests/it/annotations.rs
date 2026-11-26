use materialcraft_engine::{
    AnnotationSelection,
    core::{NodeId, Pin},
    execute,
    formats::sbs::{read_sbs, write_sbs},
    new_session,
};
use serde_json::json;

#[test]
fn moving_a_frame_carries_every_contained_item_once_without_node_selection() {
    let mut session = new_session();
    let node = NodeId(execute(&mut session, "graph.node.add", &json!({"def": "uniform", "at": [35, 45]})).unwrap()["id"].as_u64().unwrap());
    let outside = NodeId(execute(&mut session, "graph.node.add", &json!({"def": "uniform", "at": [800, 800]})).unwrap()["id"].as_u64().unwrap());
    let nested = execute(&mut session, "graph.frame.add", &json!({"rect": [20, 20, 200, 160]})).unwrap()["id"].as_u64().unwrap();
    execute(&mut session, "graph.comment.add", &json!({"at": [50, 100], "text": "Carried comment"})).unwrap();
    session.graph_mut().unwrap().pins.push(Pin { id: 99, name: "Bookmark".into(), pos: [60.0, 60.0] });
    let frame = execute(&mut session, "graph.frame.add", &json!({"rect": [0, 0, 400, 300]})).unwrap()["id"].as_u64().unwrap();
    assert!(session.selection.is_empty());
    assert_eq!(session.annotation_selection, Some(AnnotationSelection::Frame(frame)));
    let original = session.pkg.clone();

    execute(&mut session, "graph.frame.move", &json!({"id": frame, "by": [50, 20], "snapToGrid": true})).unwrap();
    let graph = session.graph().unwrap();
    assert_eq!(graph.node(node).unwrap().pos, [99.0, 77.0]);
    assert_eq!(graph.node(outside).unwrap().pos, [800.0, 800.0]);
    assert_eq!(graph.comments[0].pos, [114.0, 132.0]);
    assert_eq!(graph.pins[0].pos, [124.0, 92.0]);
    assert_eq!(graph.frames.iter().find(|candidate| candidate.id == nested).unwrap().rect, [84.0, 52.0, 200.0, 160.0]);
    assert_eq!(graph.frames.iter().find(|candidate| candidate.id == frame).unwrap().rect, [64.0, 32.0, 400.0, 300.0]);

    session.undo().unwrap();
    assert_eq!(session.pkg, original);
    session.redo().unwrap();
    assert_eq!(session.graph().unwrap().node(node).unwrap().pos, [99.0, 77.0]);
}

#[test]
fn annotation_properties_and_selection_survive_edits_undo_and_native_save() {
    let mut session = new_session();
    let frame = execute(&mut session, "graph.frame.add", &json!({})).unwrap()["id"].as_u64().unwrap();
    execute(
        &mut session,
        "graph.frame.edit",
        &json!({"id": frame, "title": "Hidden title", "titleVisible": false, "description": "Description", "color": [0.1, 0.2, 0.3, 0.4]}),
    )
    .unwrap();
    let comment = execute(&mut session, "graph.comment.add", &json!({"text": "Plain text"})).unwrap()["id"].as_u64().unwrap();
    let loaded = read_sbs(&write_sbs(&session.pkg).unwrap(), "annotations.sbs", &|_| None).unwrap();
    assert!(loaded.warnings.is_empty());
    let graph = &loaded.pkg.graphs[0];
    assert_eq!(graph.frames[0].title, "Hidden title");
    assert!(!graph.frames[0].title_visible);
    assert_eq!(graph.frames[0].description, "Description");
    assert_eq!(graph.comments[0].text, "Plain text");
    for (actual, expected) in graph.frames[0].color.iter().zip([0.1, 0.2, 0.3, 0.4]) {
        assert!((actual - expected).abs() < 0.00001);
    }

    execute(&mut session, "edit.delete", &json!({})).unwrap();
    assert!(session.graph().unwrap().comments.is_empty());
    session.undo().unwrap();
    assert_eq!(session.annotation_selection, Some(AnnotationSelection::Comment(comment)));
    execute(&mut session, "graph.annotation.select", &json!({"kind": "frame", "id": frame})).unwrap();
    execute(&mut session, "edit.delete", &json!({})).unwrap();
    assert!(session.graph().unwrap().frames.is_empty());
    assert_eq!(session.graph().unwrap().comments.len(), 1);
}

/// Pins share annotation selection, properties, deletion and undo.
#[test]
fn navigation_pins_select_edit_delete_and_save() {
    for kind in ["material", "function"] {
        let mut session = new_session();
        if kind == "function" {
            execute(&mut session, "file.new", &json!({"kind": kind})).unwrap();
        }
        let id = execute(&mut session, "graph.pin.add", &json!({"at": [-64, 128]})).unwrap()["id"].as_u64().unwrap();
        execute(&mut session, "graph.annotation.select", &json!({"kind": "pin", "id": id})).unwrap();
        execute(&mut session, "graph.pin.edit", &json!({"id": id, "description": "Start & <finish>", "at": [48, -96]})).unwrap();
        let loaded = read_sbs(&write_sbs(&session.pkg).unwrap(), "pins.sbs", &|_| None).unwrap();
        assert!(loaded.warnings.is_empty());
        let graph = loaded.pkg.graph_by_name(&session.graph().unwrap().sbs_path()).unwrap().1;
        assert_eq!(graph.pins[0].name, "Start & <finish>");
        assert_eq!(graph.pins[0].pos, [48.0, -96.0]);

        execute(&mut session, "edit.delete", &json!({})).unwrap();
        assert!(session.graph().unwrap().pins.is_empty());
        assert!(session.annotation_selection.is_none());
        session.undo().unwrap();
        assert_eq!(session.graph().unwrap().pins[0].name, "Start & <finish>");
        assert_eq!(serde_json::to_value(session.annotation_selection).unwrap(), json!({"kind": "pin", "id": id}));
        let original = session.pkg.clone();
        assert!(execute(&mut session, "graph.pin.edit", &json!({"id": 999999, "description": "Missing"})).is_err());
        assert_eq!(session.pkg, original);
    }
}

/// Every graph item edits its destination canvas and survives native SBS save/reload.
#[test]
fn library_graph_items_work_in_material_function_and_embedded_canvases() {
    use materialcraft_engine::{
        AnnotationTarget, FunctionGraphTarget, Session,
        core::{FunctionGraph, NodeId, Value},
    };
    for kind in ["material", "function", "pixel_processor", "value_processor", "dynamic", "fx_map"] {
        let mut session = new_session();
        let target = match kind {
            "material" => AnnotationTarget::Graph,
            "function" => {
                execute(&mut session, "file.new", &json!({"kind": "function"})).unwrap();
                AnnotationTarget::Graph
            }
            _ => {
                let definition = if kind == "dynamic" { "uniform" } else { kind };
                let node = NodeId(execute(&mut session, "graph.node.add", &json!({"def": definition})).unwrap()["id"].as_u64().unwrap());
                match kind {
                    "fx_map" => AnnotationTarget::Pattern { owner: node },
                    "dynamic" => {
                        session.graph_mut().unwrap().node_mut(node).unwrap().functions.insert("color".into(), FunctionGraph::default());
                        AnnotationTarget::Function { target: FunctionGraphTarget::Dynamic { node, parameter: "color".into() } }
                    }
                    _ => AnnotationTarget::Function { target: FunctionGraphTarget::Parameter { node, parameter: "function".into() } },
                }
            }
        };
        let mut parameters = target.parameters();
        parameters["at"] = json!([64, 128]);
        for entry in ["item:comment", "item:frame", "item:pin", "node:dot"] {
            parameters["entry"] = json!(entry);
            let before = session.pkg.clone();
            execute(&mut session, "library.add", &parameters).unwrap();
            let after = session.pkg.clone();
            session.undo().unwrap();
            assert_eq!(session.pkg, before, "{kind}: {entry} undo");
            session.redo().unwrap();
            assert_eq!(session.pkg, after, "{kind}: {entry} redo");
        }
        let items = target.items(&session).unwrap();
        assert_eq!((items.comments.len(), items.frames.len(), items.pins.len()), (1, 1, 1), "{kind}");
        let mut pin_parameters = target.parameters();
        pin_parameters["id"] = json!(items.pins[0].id);
        pin_parameters["kind"] = json!("pin");
        execute(&mut session, "graph.annotation.select", &pin_parameters).unwrap();
        pin_parameters["description"] = json!("Checkpoint & <finish>");
        execute(&mut session, "graph.pin.edit", &pin_parameters).unwrap();

        let loaded = read_sbs(&write_sbs(&session.pkg).unwrap(), "graph-items.sbs", &|_| None).unwrap();
        let mut reopened = Session::new(loaded.pkg);
        reopened.graph = reopened.pkg.graph_by_name(&session.graph().unwrap().sbs_path()).unwrap().0;
        let graph = reopened.graph().unwrap();
        let saved = match kind {
            "material" | "function" => AnnotationTarget::Graph.items(&reopened).unwrap(),
            "fx_map" => match &graph.nodes.iter().find(|node| node.def == kind).unwrap().params["pattern"] {
                Value::PatternGraph(graph) => graph.annotations.clone(),
                other => panic!("unexpected {other:?}"),
            },
            "dynamic" => graph.nodes.iter().find(|node| node.def == "uniform").unwrap().functions["color"].annotations.clone(),
            _ => match &graph.nodes.iter().find(|node| node.def == kind).unwrap().params["function"] {
                Value::Function(graph) => graph.annotations.clone(),
                other => panic!("unexpected {other:?}"),
            },
        };
        assert_eq!((saved.comments.len(), saved.frames.len(), saved.pins.len()), (1, 1, 1), "{kind}");
        assert_eq!(saved.pins[0].name, "Checkpoint & <finish>", "{kind}");
        assert_eq!(saved.pins[0].pos, [64.0, 128.0], "{kind}");
        if target != AnnotationTarget::Graph {
            assert!(graph.frames.is_empty() && graph.comments.is_empty() && graph.pins.is_empty(), "{kind}: parent must stay untouched");
        }
        execute(&mut session, "edit.delete", &json!({})).unwrap();
        assert!(target.items(&session).unwrap().pins.is_empty(), "{kind}");
        session.undo().unwrap();
        assert_eq!(target.items(&session).unwrap().pins[0].name, "Checkpoint & <finish>");
    }
}
