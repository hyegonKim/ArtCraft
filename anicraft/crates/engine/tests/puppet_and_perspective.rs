//! End-to-end verification of Puppet Rigging & Procedural Behaviors (`puppet.*`),
//! 2.5D Perspective (`element.set_perspective` + XFL `matrix3D` round-trip), and
//! Layer Parenting (`layer.set_parent`) + Z-Depth Camera Parallax.

use anicraft_doc::{ElementId, ElementKind, LayerId, LoopMode, evaluate, evaluate_scene};
use anicraft_engine::Session;
use anicraft_geom::Vec2;
use serde_json::json;

#[test]
fn puppet_head_turn_swap_sets_and_procedural_behaviors_work_with_undo() {
    let mut s = Session::default();
    let nose_layer = s.layer;
    s.execute("layer.rename", &json!({"name": "Nose"})).unwrap();
    s.execute("draw.ellipse", &json!({"x": 620.0, "y": 340.0, "w": 40.0, "h": 40.0, "object": true})).unwrap();

    let ear_res = s.execute("layer.new", &json!({"name": "LeftEar"})).unwrap();
    let ear_layer = ear_res["layer"].as_u64().unwrap();
    s.execute("draw.ellipse", &json!({"x": 560.0, "y": 340.0, "w": 30.0, "h": 50.0, "object": true})).unwrap();

    // Configure 5-pose head-turn parallax rig.
    let cfg = s
        .execute(
            "puppet.configure_head_turn",
            &json!({
                "name": "HeroHead",
                "pivot": [640.0, 360.0],
                "right_quarter": {"dx": 32.0, "dy": 0.0, "sx": 0.95, "sy": 1.0, "rotation": 5.0},
                "features": [
                    {"name": "Nose", "layer": nose_layer.0, "feature": "nose"},
                    {"name": "Ear", "layer": ear_layer, "feature": "left_ear"}
                ]
            }),
        )
        .unwrap();
    let rig_id = cfg["rig"].as_u64().unwrap();

    // Pose head to RightQuarter at frame 6: nose moves right (+X), ear counter-moves left (-X).
    let posed = s.execute("puppet.pose_head", &json!({"rig": rig_id, "pose": "right_quarter", "frame": 6})).unwrap();
    assert_eq!(posed["layers_updated"], 2);
    let dl0 = evaluate_scene(&s.doc, 0, 0);
    let dl6 = evaluate_scene(&s.doc, 0, 6);
    let nose_x0 = dl0.items.iter().find(|i| i.layer == nose_layer).unwrap().transform.translation().x;
    let nose_x6 = dl6.items.iter().find(|i| i.layer == nose_layer).unwrap().transform.translation().x;
    let ear_x0 = dl0.items.iter().find(|i| i.layer == LayerId(ear_layer)).unwrap().transform.translation().x;
    let ear_x6 = dl6.items.iter().find(|i| i.layer == LayerId(ear_layer)).unwrap().transform.translation().x;
    assert!(nose_x6 > nose_x0 + 20.0, "nose shifts +X on right turn");
    assert!(ear_x6 < ear_x0 - 10.0, "ear counter-shifts -X on right turn");

    // Define and trigger a Hold & Latch Swap Set on a multi-frame Graphic symbol.
    let hand_layer_res = s.execute("layer.new", &json!({"name": "RightHand"})).unwrap();
    let hand_layer = hand_layer_res["layer"].as_u64().unwrap();
    s.execute("draw.rect", &json!({"x": 700.0, "y": 420.0, "w": 50.0, "h": 50.0, "object": true})).unwrap();
    let conv = s.execute("modify.convert_to_symbol", &json!({"name": "HandPoses", "kind": "graphic"})).unwrap();
    let hand_sym = conv["symbol"].as_u64().unwrap();
    // Extend symbol to 4 frames so SingleFrame choices 0, 1, 2 are distinct.
    s.execute("symbol.edit", &json!({"symbol": hand_sym})).unwrap();
    s.execute("timeline.insert_keyframe", &json!({"frame": 1})).unwrap();
    s.execute("timeline.insert_keyframe", &json!({"frame": 2})).unwrap();
    s.execute("symbol.exit", &json!({"all": true})).unwrap();

    s.execute(
        "puppet.define_swap_set",
        &json!({
            "rig": rig_id,
            "name": "Hands",
            "layer": hand_layer,
            "mode": "latch",
            "default_choice": 0,
            "choices": [
                {"name": "Open", "key": "1", "frame": 0},
                {"name": "Point", "key": "2", "frame": 1},
                {"name": "Peace", "key": "3", "frame": 2}
            ]
        }),
    )
    .unwrap();
    let trig = s
        .execute(
            "puppet.trigger_swap",
            &json!({
                "rig": rig_id,
                "swap_set": "Hands",
                "frames": 24,
                "events": [
                    {"frame": 5, "trigger": "2"},
                    {"frame": 15, "trigger": "2"}
                ]
            }),
        )
        .unwrap();
    assert_eq!(trig["keyframes"], 3);
    let hl = s.doc.timeline(0).layer(LayerId(hand_layer)).unwrap();
    let first_at = |f: u32| match &hl.keyframe_at(f).unwrap().elements[0].kind {
        ElementKind::Symbol { loop_mode, first_frame, .. } => (*loop_mode, *first_frame),
        other => panic!("expected symbol, got {other:?}"),
    };
    assert_eq!(first_at(2), (LoopMode::SingleFrame, 0));
    assert_eq!(first_at(10), (LoopMode::SingleFrame, 1));
    assert_eq!(first_at(18), (LoopMode::SingleFrame, 0));

    // Procedural AutoBlink, Breathe, and WalkCycle + undo verification.
    let before_walk = s.doc.clone();
    let walk = s.execute("puppet.walk_cycle", &json!({"cycle_frames": 24, "stride_length": 50.0, "from": 0, "to": 24})).unwrap();
    assert!(walk["keyframes"].as_u64().unwrap() >= 8);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc, before_walk);

    let blink = s.execute("puppet.auto_blink", &json!({"layer": nose_layer.0, "interval": 20, "duration": 2, "from": 0, "to": 60})).unwrap();
    assert!(blink["keyframes"].as_u64().unwrap() >= 3);

    let breathe = s.execute("puppet.breathe", &json!({"layer": ear_layer, "period": 32, "scale_y": 1.08, "from": 0, "to": 32})).unwrap();
    assert!(breathe["keyframes"].as_u64().unwrap() >= 5);
}

#[test]
fn perspective_2_5d_and_layer_parenting_round_trip_through_xfl() {
    let mut s = Session::default();
    let torso_layer = s.layer;
    s.execute("layer.rename", &json!({"name": "Torso"})).unwrap();
    let torso_draw = s.execute("draw.rect", &json!({"x": 400.0, "y": 300.0, "w": 120.0, "h": 160.0, "object": true})).unwrap();
    let torso_el = ElementId(torso_draw["element"].as_u64().unwrap());

    // Apply 2.5D perspective tilt + Z offset on Torso.
    s.execute("element.select", &json!({"ids": [torso_el.0]})).unwrap();
    s.execute("element.set_perspective", &json!({"rot_x": 20.0, "rot_y": -30.0, "z": 180.0, "fov": 60.0})).unwrap();

    // Create Head layer parented to Torso, and set Z-depth for camera parallax.
    let head_res = s.execute("layer.new", &json!({"name": "Head"})).unwrap();
    let head_layer = LayerId(head_res["layer"].as_u64().unwrap());
    s.execute("draw.ellipse", &json!({"x": 420.0, "y": 200.0, "w": 80.0, "h": 80.0, "object": true})).unwrap();
    s.execute("layer.set_parent", &json!({"layer": head_layer.0, "parent": torso_layer.0})).unwrap();
    // Cycle attempt must be rejected cleanly.
    assert!(s.execute("layer.set_parent", &json!({"layer": torso_layer.0, "parent": head_layer.0}),).is_err());

    // Move Torso at frame 10: Head must follow Torso's delta automatically!
    s.execute("layer.select", &json!({"layer": head_layer.0})).unwrap();
    s.execute("timeline.insert_frame", &json!({"frame": 10})).unwrap();
    s.execute("layer.select", &json!({"layer": torso_layer.0})).unwrap();
    s.execute("timeline.insert_keyframe", &json!({"frame": 10})).unwrap();
    s.execute("timeline.goto", &json!({"frame": 10})).unwrap();
    s.execute("element.select", &json!({"ids": [torso_el.0]})).unwrap();
    s.execute("element.translate", &json!({"dx": 90.0, "dy": -25.0})).unwrap();

    let dl0 = evaluate_scene(&s.doc, 0, 0);
    let dl10 = evaluate_scene(&s.doc, 0, 10);
    let head_center = |dl: &anicraft_doc::DisplayList| {
        let it = dl.items.iter().find(|i| i.layer == head_layer).unwrap();
        it.path.bounds().transformed(&it.transform).center()
    };
    let c0 = head_center(&dl0);
    let c10 = head_center(&dl10);
    assert!((c10 - (c0 + Vec2::new(90.0, -25.0))).length() < 0.5, "child Head layer followed parent Torso delta: c0={c0:?}, c10={c10:?}");

    // Camera Z-depth parallax verification.
    s.execute("layer.set_depth", &json!({"layer": torso_layer.0, "z_depth": 1000.0})).unwrap();
    s.execute("camera.add", &json!({})).unwrap();
    s.execute("camera.set", &json!({"dx": 100.0, "frame": 0})).unwrap();
    let cam_dl = evaluate(&s.doc, 0);
    assert!(!cam_dl.items.is_empty());

    // Round-trip XFL package in memory (`matrix3D`, `parentLayerIndex`, `zDepth`).
    // Remove camera layer before XFL round-trip check on normal layers.
    s.execute("camera.remove", &json!({})).unwrap();
    let (files, _) = anicraft_xfl::export_package(&s.doc, "roundtrip");
    let map: std::collections::HashMap<String, Vec<u8>> = files.into_iter().collect();
    let dom_xml = String::from_utf8(map["DOMDocument.xml"].clone()).unwrap();
    assert!(dom_xml.contains("matrix3D=\""), "XFL export writes matrix3D");
    assert!(dom_xml.contains("parentLayerIndex=\""), "XFL export writes parentLayerIndex");
    assert!(dom_xml.contains("zDepth=\"1000.00\""), "XFL export writes zDepth");

    let (reimported, _) = anicraft_xfl::import_with(&|rel| Ok(map.get(rel).cloned())).unwrap();
    let re_tl = reimported.timeline(0);
    let re_head = re_tl.layers.iter().find(|l| l.name == "Head").unwrap();
    let re_torso = re_tl.layers.iter().find(|l| l.name == "Torso").unwrap();
    assert_eq!(re_head.parent_layer, Some(re_torso.id));
    assert!((re_torso.z_depth() - 1000.0).abs() < 0.1);
    let re_persp = re_torso.keyframes[0].perspectives.values().next().copied().unwrap();
    assert!((re_persp.rot_x_deg - 20.0).abs() < 0.2);
    assert!((re_persp.rot_y_deg - (-30.0)).abs() < 0.2);
    assert!((re_persp.z_offset - 180.0).abs() < 0.5);
}
