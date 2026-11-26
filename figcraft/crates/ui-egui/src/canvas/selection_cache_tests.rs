//! Memoized selection cache invalidation tests.

use std::sync::Arc;

use figcraft_doc::{Document, NodeId};

#[test]
fn selection_cache_recomputes_when_selection_or_doc_changes() {
    let ctx = egui::Context::default();
    let mut d1 = Document::new("A");
    let page = d1.first_page().unwrap();
    let a = d1.create(page, figcraft_doc::NodeKind::Rectangle, "R1", 0.0, 0.0, 10.0, 10.0).unwrap();
    let arc1 = Arc::new(d1);

    let mut calls = 0usize;
    let v1 = crate::selection_cache::cached(&ctx, "test_slot", &arc1, &[a], &[0], || {
        calls += 1;
        42usize
    });
    let v2 = crate::selection_cache::cached(&ctx, "test_slot", &arc1, &[a], &[0], || {
        calls += 1;
        99usize
    });
    assert_eq!((v1, v2, calls), (42, 42, 1));

    let v3 = crate::selection_cache::cached(&ctx, "test_slot", &arc1, &[NodeId(999)], &[0], || {
        calls += 1;
        77usize
    });
    assert_eq!((v3, calls), (77, 2));
}
