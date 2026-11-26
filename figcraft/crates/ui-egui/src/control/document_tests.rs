//! Control-channel active-document query tests.

use figcraft_engine::Session;
use serde_json::json;

use crate::layers_drag_tests::Ui;

#[test]
fn control_handle_reads_active_document_state() {
    let mut s = Session::new();
    s.execute("shape.rectangle", &json!({"x": 12, "y": 24, "width": 64, "height": 32, "name": "Card"})).unwrap();
    let mut u = Ui::new(s);
    let out = super::handle(&mut u.app, &u.ctx, "document.inspect", &json!({})).expect("document.inspect succeeds");
    assert_eq!(out["name"], "Untitled");
}
