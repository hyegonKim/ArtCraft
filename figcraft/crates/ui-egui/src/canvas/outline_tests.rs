//! Canvas outline projection tests.

use egui::{Color32, Stroke};
use figcraft_doc::NodeId;
use figcraft_engine::Session;
use serde_json::json;

use crate::layers_drag_tests::Ui;

#[test]
fn outline_shapes_returns_closed_shape_for_rectangle() {
    let mut s = Session::new();
    let r = NodeId(
        s.execute("shape.rectangle", &json!({"x": 10, "y": 20, "width": 80, "height": 40})).unwrap()["id"]
            .as_u64()
            .unwrap(),
    );
    let u = Ui::new(s);
    let doc = u.app.session.doc().unwrap();
    let shapes = super::outline::outline_shapes(&u.app, doc, r, Stroke::new(1.0, Color32::WHITE));
    assert!(!shapes.is_empty());
}
