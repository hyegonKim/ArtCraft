//! Editing character-shared fills on text layers via `edited_list`.

use figcraft_engine::Session;
use serde_json::json;

use crate::layers_drag_tests::Ui;

#[test]
fn edited_list_reads_shared_character_fills_or_layer_fills() {
    let mut s = Session::new();
    let tid = figcraft_doc::NodeId(s.execute("text.create", &json!({"x": 0, "y": 0, "text": "Hello"})).unwrap()["id"].as_u64().unwrap());
    s.execute("fill.set", &json!({"ids": [tid], "color": "#112233"})).unwrap();
    let u = Ui::new(s);
    let node = u.app.session.doc().unwrap().node(tid).unwrap();
    let list = super::edited_list(&u.app, node, true).expect("fills list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].solid_color().unwrap().to_hex(), "112233");
}
