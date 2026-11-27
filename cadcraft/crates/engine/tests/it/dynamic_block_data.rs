//! `blocks.dynamic` and `blocks.list` report a dynamic block's parameters and actions as read
//! from another program's file (kept foreign data, built by hand here).

use std::sync::Arc;

use cadcraft_engine::Session;
use cadcraft_engine::doc::foreign::{Foreign, ForeignExtra, ForeignObject};
use cadcraft_engine::doc::{Handle, RawTag};
use serde_json::json;

fn obj(handle: u64, kind: &str, tags: &[(i32, &str)]) -> ForeignObject {
    ForeignObject { handle: Handle(handle), kind: kind.into(), tags: tags.iter().map(|(c, v)| RawTag { code: *c, value: (*v).into() }).collect() }
}

/// A session whose block "Door" has a visibility parameter with two states.
fn session() -> Session {
    let mut s = Session::new();
    let l = s.execute("line", &json!({ "points": [[0, 0], [1, 0]] })).unwrap()["handles"][0].clone();
    s.execute("block", &json!({ "name": "Door", "base": [0, 0], "handles": [l] })).unwrap();
    let line = s.doc().unwrap().block("Door").unwrap().entities.handles()[0].hex();
    let mut f = Foreign::default();
    f.records.insert(("BLOCK_RECORD".into(), "DOOR".into()), ForeignExtra { xdict: Some(Handle(0xF00)), ..Default::default() });
    f.objects.push(obj(0xF00, "DICTIONARY", &[(100, "AcDbDictionary"), (3, "ACAD_ENHANCEDBLOCK"), (360, "F01")]));
    f.objects.push(obj(0xF01, "ACAD_EVALUATION_GRAPH", &[(100, "AcDbEvalGraph"), (91, "0"), (95, "1"), (360, "F02")]));
    f.objects.push(obj(
        0xF02,
        "BLOCKVISIBILITYPARAMETER",
        &[
            (100, "AcDbEvalExpr"),
            (90, "1"),
            (100, "AcDbBlockElement"),
            (300, "Visibility State"),
            (100, "AcDbBlockParameter"),
            (280, "1"),
            (100, "AcDbBlock1PtParameter"),
            (1010, "2"),
            (1020, "3"),
            (100, "AcDbBlockVisibilityParameter"),
            (301, "Visibility1"),
            (302, ""),
            (93, "1"),
            (331, &line),
            (92, "2"),
            (303, "Open"),
            (94, "1"),
            (332, &line),
            (95, "0"),
            (303, "Closed"),
            (94, "0"),
            (95, "0"),
        ],
    ));
    let mut d = s.doc().unwrap().clone();
    d.foreign = Arc::new(f);
    s.open_drawing(d, "dynamic", None);
    s
}

#[test]
fn dynamic_block_data_is_reported() {
    let mut s = session();
    let list = s.execute("blocks.list", &json!({})).unwrap();
    let flags: Vec<_> = list["blocks"].as_array().unwrap().iter().map(|b| (b["name"].as_str().unwrap().to_string(), b["dynamic"].clone())).collect();
    assert!(flags.contains(&("Door".into(), json!(true))), "{flags:?}");

    let m = s.execute("blocks.dynamic", &json!({ "name": "DOOR" })).unwrap();
    let p = &m["parameters"][0];
    assert_eq!((p["node"].clone(), p["element"].clone(), p["showProperties"].clone()), (json!(1), json!("Visibility State"), json!(true)));
    assert_eq!(p["kind"]["type"], "visibility");
    assert_eq!(p["kind"]["label"], "Visibility1");
    assert_eq!(p["kind"]["at"], json!({ "x": 2.0, "y": 3.0, "z": 0.0 }));
    let states: Vec<_> =
        p["kind"]["states"].as_array().unwrap().iter().map(|st| (st["name"].clone(), st["visible"].as_array().unwrap().len())).collect();
    assert_eq!(states, [(json!("Open"), 1), (json!("Closed"), 0)]);
    assert!(p["kind"]["controlled"][0].is_string(), "handles are hex strings");
    assert_eq!(m["actions"], json!([]));
}

#[test]
fn blocks_without_dynamic_data_and_bad_names() {
    let mut s = session();
    let l = s.execute("line", &json!({ "points": [[0, 0], [1, 1]] })).unwrap()["handles"][0].clone();
    s.execute("block", &json!({ "name": "Window", "base": [0, 0], "handles": [l] })).unwrap();
    assert_eq!(s.execute("blocks.dynamic", &json!({ "name": "Window" })).unwrap(), json!(null));
    assert_eq!(s.execute("blocks.dynamic", &json!({ "name": "*Model_Space" })).unwrap(), json!(null));
    let list = s.execute("blocks.list", &json!({})).unwrap();
    let flags: Vec<_> = list["blocks"].as_array().unwrap().iter().map(|b| (b["name"].as_str().unwrap().to_string(), b["dynamic"].clone())).collect();
    assert!(flags.contains(&("Window".into(), json!(false))), "{flags:?}");
    assert!(flags.contains(&("Door".into(), json!(true))), "{flags:?}");
    for p in [json!({}), json!({ "name": 3 }), json!({ "name": "Nope" }), json!(null)] {
        assert!(s.execute("blocks.dynamic", &p).is_err(), "{p}");
    }
}

#[test]
fn a_reference_to_a_dynamic_block_reports_its_custom_properties() {
    let mut s = session();
    // BLOCK made the line a reference to Door, which has Door's own values.
    let insert = s.doc().unwrap().model.handles()[0].hex();
    let r = s.execute("blocks.reference", &json!({ "handle": insert })).unwrap();
    assert_eq!(r["definition"], "Door");
    assert_eq!(r["values"], json!({}));
    assert_eq!(r["properties"], json!([{ "name": "Visibility1", "value": "Open", "choices": ["Open", "Closed"], "editable": true }]));

    // A plain block's reference, objects that aren't references and bad parameters.
    let l = s.execute("line", &json!({ "points": [[0, 0], [1, 1]] })).unwrap()["handles"][0].clone();
    let plain = s.execute("block", &json!({ "name": "Window", "base": [0, 0], "handles": [l.clone()] })).unwrap()["insert"].clone();
    assert_eq!(s.execute("blocks.reference", &json!({ "handle": plain })).unwrap(), json!(null));
    let l = s.execute("line", &json!({ "points": [[0, 0], [2, 2]] })).unwrap()["handles"][0].clone();
    assert!(s.execute("blocks.reference", &json!({ "handle": l })).is_err());
    for p in [json!({}), json!({ "handle": "zz" }), json!({ "handle": "FFFFFF" }), json!(null)] {
        assert!(s.execute("blocks.reference", &p).is_err(), "{p}");
    }
}

#[test]
fn a_visibility_state_is_changed_in_one_undo_step() {
    let mut s = session();
    let insert = s.doc().unwrap().model.handles()[0].hex();
    let r = s.execute("blocks.reference.set", &json!({ "handle": insert, "name": "Visibility1", "value": "Closed" })).unwrap();
    assert_eq!(r["properties"][0]["value"], "Closed");
    let block = |s: &Session| match s.doc().unwrap().model.iter().next().map(|e| e.kind.clone()) {
        Some(cadcraft_engine::doc::EntityKind::Insert(i)) => i.block,
        other => panic!("{other:?}"),
    };
    assert!(block(&s).starts_with("*U"));
    // Still a reference to Door, in its new state.
    let r = s.execute("blocks.reference", &json!({ "handle": insert })).unwrap();
    assert_eq!((r["definition"].clone(), r["properties"][0]["value"].clone()), (json!("Door"), json!("Closed")));

    s.execute("undo", &json!({})).unwrap();
    assert_eq!(block(&s), "Door");
    let r = s.execute("blocks.reference", &json!({ "handle": insert })).unwrap();
    assert_eq!(r["properties"][0]["value"], "Open");

    for p in [
        json!({ "handle": insert, "name": "Visibility1", "value": "Ajar" }),
        json!({ "handle": insert, "name": "Visibility1" }),
        json!({ "name": "Visibility1", "value": "Open" }),
        json!({ "handle": insert, "value": "Open" }),
    ] {
        assert!(s.execute("blocks.reference.set", &p).is_err(), "{p}");
    }
}

#[test]
fn bedit_authoring_bparameter_baction_bvstate_blookup_updates_references_and_roundtrips_dxf() {
    let mut s = Session::new();
    // Create a block "Bracket" with a non-zero base point [5, 5] to verify coordinate shifting.
    let top = s.execute("line", &json!({ "points": [[5, 5], [15, 5]] })).unwrap()["handles"][0].clone();
    let badge = s.execute("line", &json!({ "points": [[5, 8], [10, 8]] })).unwrap()["handles"][0].clone();
    let blk_res = s.execute("block", &json!({ "name": "Bracket", "base": [5, 5], "handles": [top, badge] })).unwrap();
    let ins_handle = blk_res["insert"].as_str().unwrap().to_string();

    // Authoring commands must be refused outside BEDIT.
    assert!(s.execute("bparameter", &json!({ "type": "linear", "base": [0, 0], "end": [10, 0] })).is_err());

    // Enter BEDIT on "Bracket": in editor coordinates, `top` runs from (0,0) to (10,0), `badge` from (0,3) to (5,3).
    s.execute("bedit", &json!({ "name": "Bracket" })).unwrap();
    let ed_handles: Vec<String> = s.doc().unwrap().model.handles().into_iter().map(|h| h.hex()).collect();
    assert_eq!(ed_handles.len(), 2);
    let (ed_top, ed_badge) = (ed_handles[0].clone(), ed_handles[1].clone());

    // 1. Linear parameter "Width" + Stretch action on `ed_top`'s right endpoint (10, 0).
    let p_lin = s.execute("bparameter", &json!({ "type": "linear", "label": "Width", "base": [0, 0], "end": [10, 0] })).unwrap();
    let lin_node = p_lin["node"].as_i64().unwrap();
    s.execute(
        "baction",
        &json!({
            "type": "stretch",
            "parameter": lin_node,
            "frame": [[8, -2], [12, 2]],
            "handles": [ed_top]
        }),
    )
    .unwrap();

    // 2. Rotation parameter "Turn" + Rotate action on `ed_badge`.
    s.execute("bparameter", &json!({ "type": "rotation", "label": "Turn", "base": [0, 0], "end": [5, 0] })).unwrap();
    s.execute("baction", &json!({ "type": "rotate", "parameter": "Turn", "handles": [ed_badge] })).unwrap();

    // 3. Flip parameter "Mirror" + Flip action on `ed_badge`.
    s.execute("bparameter", &json!({ "type": "flip", "label": "Mirror", "base": [0, -5], "end": [0, 10] })).unwrap();
    s.execute("baction", &json!({ "type": "flip", "parameter": "Mirror", "handles": [ed_badge] })).unwrap();

    // 4. Visibility parameter "View" with states ["Full", "Minimal"], hiding `ed_badge` in "Minimal".
    s.execute("bparameter", &json!({ "type": "visibility", "label": "View", "at": [0, 6], "states": ["Full", "Minimal"] })).unwrap();
    s.execute("bvstate", &json!({ "op": "hide", "state": "Minimal", "handles": [ed_badge] })).unwrap();
    s.execute("bvstate", &json!({ "op": "new", "name": "Extra" })).unwrap();

    // 5. Lookup parameter "SizePreset" + BLOOKUP table mapping Width 10 -> "Std", 25 -> "Long".
    s.execute("bparameter", &json!({ "type": "lookup", "label": "SizePreset", "at": [0, -4] })).unwrap();
    s.execute(
        "blookup",
        &json!({
            "parameter": "SizePreset",
            "columns": [
                { "parameter": "Width", "property": "UpdatedDistance", "lookup": false },
                { "parameter": "SizePreset", "property": "lookupString", "lookup": true }
            ],
            "rows": [
                ["10", "Std"],
                ["25", "Long"]
            ]
        }),
    )
    .unwrap();

    // Save & close BEDIT.
    s.execute("bsave", &json!({})).unwrap();
    s.execute("bclose", &json!({})).unwrap();

    // Verify dynamic block definition on "Bracket".
    let dyn_def = s.execute("blocks.dynamic", &json!({ "name": "Bracket" })).unwrap();
    assert_eq!(dyn_def["parameters"].as_array().unwrap().len(), 5);
    assert_eq!(dyn_def["actions"].as_array().unwrap().len(), 4);

    // Inspect custom properties on the existing reference.
    let ref_info = s.execute("blocks.reference", &json!({ "handle": ins_handle })).unwrap();
    assert_eq!(ref_info["definition"], "Bracket");
    let grips = s.execute("blocks.reference.grips", &json!({ "handle": ins_handle })).unwrap();
    assert!(!grips.as_array().unwrap().is_empty());

    // Stretch "Width" from 10 -> 25 via `blocks.reference.set` and verify anonymous block geometry!
    s.execute("blocks.reference.set", &json!({ "handle": ins_handle, "name": "Width", "value": 25.0 })).unwrap();
    let check_top_len = |session: &Session, h_hex: &str| -> (usize, f64) {
        let doc = session.doc().unwrap();
        let h = Handle::parse_hex(h_hex).unwrap();
        let cadcraft_engine::doc::EntityKind::Insert(ins) = &doc.entity(h).unwrap().kind else { panic!("expected insert") };
        let b = doc.block(&ins.block).unwrap();
        let cadcraft_engine::doc::EntityKind::Line(ln) = &b.entities.iter().next().unwrap().kind else { panic!("expected line") };
        let visible_count = b.entities.iter().filter(|e| e.common.visible).count();
        (visible_count, (ln.b.x - ln.a.x).abs())
    };
    let (count_full, stretched_len) = check_top_len(&s, &ins_handle);
    assert_eq!(count_full, 2);
    assert!((stretched_len - 25.0).abs() < 1e-6, "expected 25.0, got {stretched_len}");

    // Switch visibility to "Minimal" -> `ed_badge` is hidden so anonymous block has 1 entity!
    s.execute("blocks.reference.set", &json!({ "handle": ins_handle, "name": "View", "value": "Minimal" })).unwrap();
    let (count_min, _) = check_top_len(&s, &ins_handle);
    assert_eq!(count_min, 1);

    // Switch back to "Full" and flip "Mirror" -> `ed_badge` mirrors across x = 5 (base x) in block coords!
    s.execute("blocks.reference.set", &json!({ "handle": ins_handle, "name": "View", "value": "Full" })).unwrap();
    s.execute("blocks.reference.set", &json!({ "handle": ins_handle, "name": "Mirror", "value": "Flipped" })).unwrap();
    {
        let doc = s.doc().unwrap();
        let h = Handle::parse_hex(&ins_handle).unwrap();
        let cadcraft_engine::doc::EntityKind::Insert(ins) = &doc.entity(h).unwrap().kind else { panic!("expected insert") };
        let b = doc.block(&ins.block).unwrap();
        let cadcraft_engine::doc::EntityKind::Line(badge_ln) = &b.entities.iter().nth(1).unwrap().kind else { panic!("expected badge line") };
        // Originally [5, 8] -> [10, 8], mirrored across x = 5 becomes [5, 8] -> [0, 8]!
        assert!((badge_ln.b.x - 0.0).abs() < 1e-6, "expected mirrored x = 0.0, got {}", badge_ln.b.x);
    }

    // Round-trip the entire drawing through DXF and verify both definition & reference editing survive!
    let dxf_text = cadcraft_io::write_dxf(s.doc().unwrap());
    let reopened_doc = cadcraft_io::read_dxf(dxf_text.as_bytes()).unwrap();
    let mut s2 = Session::new();
    s2.open_drawing(reopened_doc, "reopened.dxf", None);

    let dyn_after = s2.execute("blocks.dynamic", &json!({ "name": "Bracket" })).unwrap();
    assert_eq!(dyn_after["parameters"].as_array().unwrap().len(), 5);
    assert_eq!(dyn_after["actions"].as_array().unwrap().len(), 4);

    // Insert a brand-new reference to "Bracket" in the reopened DXF session and stretch it to 30!
    let new_ins = s2.execute("insert", &json!({ "name": "Bracket", "at": [0, 0] })).unwrap()["handle"].as_str().unwrap().to_string();
    s2.execute("blocks.reference.set", &json!({ "handle": new_ins, "name": "Width", "value": 30.0 })).unwrap();
    let (_, reopened_len) = check_top_len(&s2, &new_ins);
    assert!((reopened_len - 30.0).abs() < 1e-6, "expected 30.0 after DXF round-trip, got {reopened_len}");
}
