//! Vector edit vertex position tests.

use serde_json::json;

use crate::Session;
use figcraft_doc::{NodeId, NodeKind};

#[test]
fn moving_a_vector_vertex_refits_its_layer_box() {
    let mut s = Session::new();
    let id = NodeId(s.execute("shape.rectangle", &json!({"x": 20, "y": 30, "width": 80, "height": 40})).unwrap()["id"].as_u64().unwrap());
    s.execute("select.set", &json!({"ids": [id]})).unwrap();
    s.execute("vector.edit.begin", &json!({})).unwrap();
    assert!(matches!(s.doc().unwrap().node(id).unwrap().kind, NodeKind::Vector { .. }));
    s.execute("vector.edit.moveVertices", &json!({"vertices": [0], "dx": -10, "dy": -5})).unwrap();
    let b = s.doc().unwrap().world_bounds(id).unwrap();
    assert!((b.x0 - 10.0).abs() < 1e-6 && (b.y0 - 25.0).abs() < 1e-6, "{b:?}");
}
