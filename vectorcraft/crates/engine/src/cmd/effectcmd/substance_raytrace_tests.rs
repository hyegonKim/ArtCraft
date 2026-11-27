#![allow(clippy::unwrap_used, clippy::panic)]

use serde_json::json;
use vectorcraft_doc::{NodeId, NodeKind, SubstanceMaterialPreset, ThreeDMaterialSettings};

use crate::Session;

#[test]
fn substance_presets_and_raytracing_execute_undo_redo_and_round_trip_vectorcraft() {
    for effect in ["threeD.extrude", "threeD.revolve", "threeD.inflate", "threeD.rotate"] {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 300, "height": 300})).unwrap();
        let id = NodeId(s.execute("shape.rectangle", &json!({"x": 80, "y": 80, "width": 90, "height": 90})).unwrap()["id"].as_u64().unwrap());
        s.execute("paint.setFill", &json!({"color": "#d66833"})).unwrap();

        // Legacy params without new keys deserialize cleanly to defaults.
        s.execute("effect.apply", &json!({"effect": effect, "item": null, "params": {}})).unwrap();
        let default_fx = &s.doc().unwrap().doc.node(id).unwrap().appearance.effects[0];
        assert_eq!(default_fx.three_d_material(), ThreeDMaterialSettings::default(), "{effect}");

        s.doc_mut().unwrap().mark_saved();
        let before = s.doc().unwrap().doc.clone();
        let history = s.doc().unwrap().history.undo.len();

        let update = json!({
            "index": 0,
            "item": null,
            "params": {
                "substancePreset": "polishedGold",
                "roughness": 18.0,
                "metallic": 95.0,
                "materialScale": 140.0,
                "materialWear": 30.0,
                "normalStrength": 120.0,
                "materialSeed": 7,
                "raytrace": true,
                "ambientOcclusion": 65.0,
                "shadow": true
            }
        });
        s.check_params("effect.setParams", &update).unwrap();
        s.execute("effect.setParams", &update).unwrap();
        assert_eq!(s.doc().unwrap().history.undo.len(), history + 1, "{effect}");

        let after = s.doc().unwrap().doc.clone();
        let mat = after.node(id).unwrap().appearance.effects[0].three_d_material();
        assert_eq!(mat.substance_preset, SubstanceMaterialPreset::PolishedGold);
        assert!(mat.raytrace);
        assert_eq!(mat.material_scale, 140.0);
        assert_eq!(mat.material_wear, 30.0);
        assert_eq!(mat.normal_strength, 120.0);
        assert_eq!(mat.material_seed, 7);
        assert_eq!(mat.ambient_occlusion, 65.0);

        // Native .vectorcraft round-trip preserves all Substance + raytrace fields.
        let saved = vectorcraft_format::save(&after, false);
        let reopened = vectorcraft_format::load(&saved).unwrap();
        assert_eq!(reopened.node(id).unwrap().appearance.effects[0].three_d_material(), mat);

        // Undo & Redo restore exact states.
        s.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(s.doc().unwrap().doc, before);
        s.execute("edit.redo", &json!({})).unwrap();
        assert_eq!(s.doc().unwrap().doc, after);

        // Expand Appearance on a raytraced 3D effect adopts embedded PNG pixels into doc.images.
        s.execute("effect.expandAppearance", &json!({})).unwrap();
        let expanded_doc = &s.doc().unwrap().doc;
        assert!(!expanded_doc.images.is_empty(), "{effect} raytraced expansion must store image blob");
        let mut saw_image = false;
        expanded_doc.walk(|n| {
            if let NodeKind::Image(im) = &n.kind {
                saw_image = true;
                assert!(im.pixels.is_none(), "adopted image must store pixels on Document, not Node");
                assert!(expanded_doc.images.contains_key(&im.key));
            }
        });
        assert!(saw_image, "{effect} expanded tree must contain raytraced Image node");
    }
}

#[test]
fn invalid_substance_and_raytrace_parameters_are_rejected_atomically() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 300, "height": 300})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 60, "y": 60, "width": 80, "height": 80})).unwrap();
    s.execute("effect.apply", &json!({"effect": "threeD.extrude", "item": null})).unwrap();

    for bad_params in [
        json!({"substancePreset": "unobtainium"}),
        json!({"substancePreset": 42}),
        json!({"raytrace": "yes"}),
        json!({"materialScale": 2.0}),
        json!({"materialScale": 900.0}),
        json!({"materialWear": -5.0}),
        json!({"normalStrength": 250.0}),
        json!({"ambientOcclusion": 150.0}),
    ] {
        s.doc_mut().unwrap().mark_saved();
        let before = s.doc().unwrap().doc.clone();
        let history = s.doc().unwrap().history.undo.len();
        let res = s.execute("effect.setParams", &json!({"index": 0, "item": null, "params": bad_params}));
        assert!(res.is_err(), "expected error for {bad_params}");
        assert_eq!(s.doc().unwrap().doc, before);
        assert_eq!(s.doc().unwrap().history.undo.len(), history);
        assert!(!s.doc().unwrap().is_dirty());
    }
}
