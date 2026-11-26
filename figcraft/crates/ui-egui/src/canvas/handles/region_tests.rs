//! Editing selected vector-region fills via `edited_list`.

use figcraft_color::{Color, Paint};
use figcraft_doc::{NodeKind, Region, Segment, VectorNetwork, Vertex, WindingRule};
use figcraft_engine::Session;

use crate::layers_drag_tests::Ui;

#[test]
fn edited_list_prefers_selected_vector_region_fills() {
    let mut s = Session::new();
    let page = s.page().unwrap();
    let net = VectorNetwork {
        vertices: vec![
            Vertex::new(0.0, 0.0),
            Vertex::new(40.0, 0.0),
            Vertex::new(40.0, 40.0),
            Vertex::new(0.0, 40.0),
        ],
        segments: vec![
            Segment::line(0, 1),
            Segment::line(1, 2),
            Segment::line(2, 3),
            Segment::line(3, 0),
        ],
        regions: vec![Region {
            winding: WindingRule::NonZero,
            loops: vec![vec![0, 1, 2, 3]],
            fills: Some(vec![Paint::solid(Color::from_rgb8_const(0xAA, 0xBB, 0xCC))]),
            paint_space: None,
        }],
    };
    let vid = s
        .doc_mut()
        .unwrap()
        .create(page, NodeKind::Vector { network: net }, "Vec", 0.0, 0.0, 40.0, 40.0)
        .unwrap();
    figcraft_engine::vectoredit::begin(&mut s, vid).unwrap();
    s.ds_mut().unwrap().vector_edit.as_mut().unwrap().regions = vec![0];

    let u = Ui::new(s);
    let node = u.app.session.doc().unwrap().node(vid).unwrap();
    let list = super::edited_list(&u.app, node, true).expect("selected region fills");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].solid_color().unwrap().to_hex(), "AABBCC");
}
