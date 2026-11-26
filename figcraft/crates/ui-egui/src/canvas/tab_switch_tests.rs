//! Canvas texture validity across document tab switches.

use figcraft_engine::Session;
use serde_json::json;

use crate::layers_drag_tests::Ui;

#[test]
fn switching_tabs_tracks_active_document_uid() {
    let mut s = Session::new();
    s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 80, "height": 80})).unwrap();
    let uid0 = s.ds().unwrap().uid;
    s.execute("file.new", &json!({"name": "Second"})).unwrap();
    let uid1 = s.ds().unwrap().uid;
    assert_ne!(uid0, uid1);

    let mut u = Ui::new(s);
    u.frame(vec![]);
    assert_eq!(u.app.session.ds().unwrap().uid, uid1);
    u.app.run("doc.switch", json!({"index": 0}));
    u.frame(vec![]);
    assert_eq!(u.app.session.ds().unwrap().uid, uid0);
}
