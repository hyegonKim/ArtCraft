//! Shape builder merges styled from vector-region paints.

use figcraft_color::{Color, Paint, PaintKind};
use figcraft_doc::{NodeKind, Region, Segment, VectorNetwork, Vertex, WindingRule};
use serde_json::json;

use crate::Session;
use crate::params::id_of;

#[test]
fn merge_picks_region_paint_over_node_default_fill() {
    let mut s = Session::new();
    let page = s.page().unwrap();

    // Build a vector network with two adjacent square regions [0..50] and [50..100], each painted
    // with its own region fill, while the node itself has a blue fallback fill.
    let net = VectorNetwork {
        vertices: vec![
            Vertex::new(0.0, 0.0),
            Vertex::new(50.0, 0.0),
            Vertex::new(50.0, 50.0),
            Vertex::new(0.0, 50.0),
            Vertex::new(100.0, 0.0),
            Vertex::new(100.0, 50.0),
        ],
        segments: vec![
            Segment::line(0, 1),
            Segment::line(1, 2),
            Segment::line(2, 3),
            Segment::line(3, 0),
            Segment::line(1, 4),
            Segment::line(4, 5),
            Segment::line(5, 2),
        ],
        regions: vec![
            Region {
                winding: WindingRule::NonZero,
                loops: vec![vec![0, 1, 2, 3]],
                fills: Some(vec![Paint::solid(Color::from_rgb8_const(0xFF, 0x00, 0x00))]),
                paint_space: None,
            },
            Region {
                winding: WindingRule::NonZero,
                loops: vec![vec![4, 5, 6, 1]],
                fills: Some(vec![Paint::solid(Color::from_rgb8_const(0x00, 0xAA, 0x00))]),
                paint_space: None,
            },
        ],
    };
    let vid = s
        .doc_mut()
        .unwrap()
        .create(page, NodeKind::Vector { network: net }, "Split", 0.0, 0.0, 100.0, 50.0)
        .unwrap();
    s.doc_mut().unwrap().get_mut(vid).unwrap().fills = vec![Paint::solid(Color::from_rgb8_const(0x00, 0x00, 0xFF))];

    let rect = id_of(
        &s.execute("shape.rectangle", &json!({"x": 60, "y": 10, "width": 60, "height": 30})).unwrap()["id"],
    )
    .unwrap();
    s.execute("fill.set", &json!({"ids": [rect], "color": "#CCCCCC"})).unwrap();
    // Put the multi-region vector on top so the right region's green fill styles the merged shape.
    s.execute("arrange.front", &json!({"ids": [vid]})).unwrap();

    let out = s
        .execute(
            "shapeBuilder.merge",
            &json!({"ids": [rect, vid], "points": [[75.0, 25.0], [110.0, 25.0]]}),
        )
        .unwrap();
    let merged = id_of(&out["id"]).unwrap();
    let fills = &s.doc().unwrap().node(merged).unwrap().fills;
    assert_eq!(fills.len(), 1);
    match &fills[0].kind {
        PaintKind::Solid { color: c } => assert_eq!(c.to_hex(), "00AA00"),
        other => panic!("expected solid green region fill, got {other:?}"),
    }
}
