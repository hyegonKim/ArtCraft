//! Layers panel node visibility and selection synchronization tests.

use figcraft_doc::NodeId;
use figcraft_engine::Session;
use serde_json::json;

use crate::layers_drag_tests::Ui;

#[test]
fn selecting_a_child_keeps_canvas_and_session_in_sync() {
    let mut s = Session::new();
    let f = NodeId(
        s.execute("frame.create", &json!({"x": 0, "y": 0, "width": 200, "height": 200})).unwrap()["id"]
            .as_u64()
            .unwrap(),
    );
    let c = NodeId(
        s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 50, "height": 50, "parent": f})).unwrap()["id"]
            .as_u64()
            .unwrap(),
    );
    s.execute("select.set", &json!({"ids": [c]})).unwrap();
    let mut u = Ui::new(s);
    u.frame(vec![]);
    assert_eq!(u.app.session.selection(), vec![c]);
}
