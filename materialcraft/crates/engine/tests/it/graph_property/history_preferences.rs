use materialcraft_engine::{
    GraphTarget, Session,
    core::{Graph, Package},
    execute,
};
use serde_json::json;

fn session() -> Session {
    let mut package = Package::new("Source");
    package.graphs.push(Graph::new("Graph"));
    Session::new(package)
}

fn seed(session: &mut Session, document: u64, value: i32) {
    session.finish_edit();
    execute(session, "graph.seed", &json!({"document": document, "graph": 0, "seed": value})).unwrap();
}

#[test]
fn configured_history_can_exceed_the_old_limit() {
    let mut session = session();
    execute(&mut session, "session.history", &json!({"undoLimit": 201})).unwrap();
    assert!(!session.modified);

    // Cross the former cap and overflow the configured limit by one step
    for value in 1..=202 {
        seed(&mut session, 0, value);
    }
    assert_eq!(session.history().len(), 201);
    session.undo().unwrap();

    assert_eq!(session.graph().unwrap().seed, 201);
}

#[test]
fn reducing_the_limit_trims_every_package_and_bounds_redo() {
    let mut session = session();
    execute(&mut session, "file.newPackage", &json!({"name": "Canvas"})).unwrap();
    execute(&mut session, "file.new", &json!({"template": "empty"})).unwrap();
    session.reset_history();
    for document in [0, 1] {
        for value in 1..=8 {
            seed(&mut session, document, value);
        }
        for _ in 0..4 {
            execute(&mut session, "edit.undo", &json!({"document": document})).unwrap();
        }
    }

    execute(&mut session, "session.history", &json!({"undoLimit": 2})).unwrap();

    for document in [0, 1] {
        assert_eq!(session.document_history_labels(document).len(), 2);
        for _ in 0..2 {
            execute(&mut session, "edit.redo", &json!({"document": document})).unwrap();
        }
        assert!(!session.can_redo_document(document));
        assert_eq!(session.document_history_labels(document).len(), 2);
        for _ in 0..2 {
            execute(&mut session, "edit.undo", &json!({"document": document})).unwrap();
        }
        assert!(!session.can_undo_document(document));
        assert_eq!(GraphTarget { document, graph: 0 }.get(&session).unwrap().seed, 4);
    }

    execute(&mut session, "file.newPackage", &json!({"name": "Later"})).unwrap();
    execute(&mut session, "file.new", &json!({"template": "empty"})).unwrap();
    let document = session.document_id;
    for value in 1..=4 {
        seed(&mut session, document, value);
    }
    assert_eq!(session.history().len(), 2);
    assert_eq!(execute(&mut session, "session.history", &json!({})).unwrap()["undoLimit"], 2);
}

#[test]
fn trimming_history_keeps_only_the_closest_redo_states() {
    let mut session = session();
    for value in 1..=8 {
        seed(&mut session, 0, value);
    }
    session.undo().unwrap();
    session.undo().unwrap();
    execute(&mut session, "session.history", &json!({"undoLimit": 2})).unwrap();

    session.undo().unwrap();
    session.undo().unwrap();
    session.redo().unwrap();
    session.redo().unwrap();

    assert_eq!(session.graph().unwrap().seed, 6);
    assert!(!session.can_redo());
    assert_eq!(session.history().len(), 2);
}

#[test]
fn invalid_history_limits_do_not_change_preferences_or_history() {
    let mut session = session();
    seed(&mut session, 0, 1);
    let history = session.history();
    let preferences = session.history_prefs;

    for limit in [json!(0), json!(-1), json!(1.5), json!("2"), json!(10_001), json!(u64::MAX)] {
        assert!(execute(&mut session, "session.history", &json!({"undoLimit": limit})).is_err());
        assert_eq!(session.history_prefs, preferences);
        assert_eq!(session.history(), history);
    }
}
