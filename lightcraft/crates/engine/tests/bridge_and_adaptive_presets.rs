//! Integration tests for Adobe Bridge DAM workflow commands (`bridge.batchRename`,
//! `bridge.contactSheet`, `bridge.applyMetadataTemplate`) and Lightroom CC Adaptive Presets
//! (`develop.applyAdaptivePreset`).

#![allow(clippy::unwrap_used)]

use lightcraft_develop::MaskShape;
use lightcraft_engine::Session;
use serde_json::json;

#[test]
fn bridge_batch_rename_supports_tokens_preview_and_collision_safe_execution() {
    let dir = std::env::temp_dir().join(format!("lc-bridge-rename-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let make_png = |name: &str, seed: u8| {
        let data: Vec<[u8; 4]> = (0..32 * 24).map(|i| [(i % 32 * 7) as u8, (i / 32 * 9) as u8, seed, 255]).collect();
        let img = lightcraft_raster::Rgba8 { width: 32, height: 24, data };
        let bytes = lightcraft_codecs::encode_png(&lightcraft_codecs::EncodeImage::rgba8(&img), &lightcraft_codecs::EncodeMeta::default()).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        p.to_string_lossy().into_owned()
    };

    let p1 = make_png("DSC001.png", 10);
    let p2 = make_png("DSC002.png", 20);
    // Pre-create a colliding file on disk to verify collision-safe `-1` suffixing!
    std::fs::write(dir.join("20260715_CanonR5_400_5_DSC001_0001.png"), b"existing").unwrap();

    let mut s = Session::new().with_fs();
    let imp = s.execute("library.import", &json!({"paths": [p1, p2]})).unwrap();
    let id1 = imp["imported"][0].as_u64().unwrap();
    let id2 = imp["imported"][1].as_u64().unwrap();

    // Populate EXIF/capture metadata for `{date}`, `{camera}`, `{iso}`, `{rating}` tokens.
    s.execute("photo.setCaptureTime", &json!({"ids": [id1, id2], "time": "2026-07-15T09:30:00", "each": true})).unwrap();
    s.execute("photo.rate", &json!({"ids": [id1], "rating": 5})).unwrap();
    s.execute("photo.rate", &json!({"ids": [id2], "rating": 4})).unwrap();
    for (id, iso) in [(lightcraft_catalog::PhotoId(id1), 400_u32), (lightcraft_catalog::PhotoId(id2), 800_u32)] {
        let mut meta = s.catalog.photo(id).unwrap().meta.clone();
        meta.camera = "CanonR5".into();
        meta.iso = Some(iso);
        s.catalog.apply(lightcraft_catalog::Op::SetMeta { id, meta: Box::new(meta) }).unwrap();
    }

    let tpl = "{date}_{camera}_{iso}_{rating}_{orig}_{seq:04}";
    let prev = s.execute("bridge.batchRename", &json!({"ids": [id1, id2], "template": tpl, "start": 1, "preview": true})).unwrap();
    assert_eq!(prev["preview"], true);
    assert_eq!(prev["renamed"], 0);
    assert_eq!(prev["plans"][0]["to"], "20260715_CanonR5_400_5_DSC001_0001-1.png", "collision gets -1 suffix");
    assert_eq!(prev["plans"][1]["to"], "20260715_CanonR5_800_4_DSC002_0002.png");
    // Original file is untouched after preview:
    assert!(dir.join("DSC001.png").is_file());

    let exec = s.execute("bridge.batchRename", &json!({"ids": [id1, id2], "template": tpl, "start": 1, "preview": false})).unwrap();
    assert_eq!(exec["renamed"], 2);
    assert!(dir.join("20260715_CanonR5_400_5_DSC001_0001-1.png").is_file());
    assert!(dir.join("20260715_CanonR5_800_4_DSC002_0002.png").is_file());

    // Undo moves files back cleanly.
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(dir.join("DSC001.png").is_file());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bridge_metadata_template_and_contact_sheet_export_pdf_and_png() {
    let dir = std::env::temp_dir().join(format!("lc-bridge-sheet-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mut s = Session::with_demo().with_fs();
    let ids: Vec<u64> = s.catalog.photos().take(3).map(|p| p.id.0).collect();

    let stamped = s
        .execute(
            "bridge.applyMetadataTemplate",
            &json!({
                "ids": ids,
                "creator": "Studio Kim",
                "copyright": "© 2026 Studio Kim",
                "copyrightStatus": "copyrighted",
                "credit": "Editorial Desk",
                "usageTerms": "Editorial use only",
                "keywords": ["Architecture", "Seoul"],
                "rating": 4
            }),
        )
        .unwrap();
    assert_eq!(stamped["changed"], 3);

    for &raw_id in &ids {
        let p = s.catalog.photo(lightcraft_catalog::PhotoId(raw_id)).unwrap();
        assert_eq!(p.meta.creator, "Studio Kim");
        assert_eq!(p.meta.copyright, "© 2026 Studio Kim");
        assert_eq!(p.meta.credit, "Editorial Desk");
        assert_eq!(p.meta.usage_terms, "Editorial use only");
        assert!(p.meta.keywords.iter().any(|k| k == "Seoul"));
        assert_eq!(p.rating, 4);
    }

    let pdf_path = dir.join("sheet.pdf");
    let pdf_out = s
        .execute(
            "bridge.contactSheet",
            &json!({
                "ids": ids,
                "rows": 2,
                "cols": 2,
                "margins": {"top": 40, "right": 40, "bottom": 40, "left": 40},
                "captions": true,
                "showExif": true,
                "path": pdf_path.to_string_lossy()
            }),
        )
        .unwrap();
    assert_eq!(pdf_out["format"], "pdf");
    assert_eq!(pdf_out["pages"], 1);
    assert_eq!(pdf_out["photos"], 3);
    assert!(pdf_path.is_file());
    assert!(std::fs::metadata(&pdf_path).unwrap().len() > 500);

    let png_path = dir.join("sheet.png");
    let png_out = s
        .execute(
            "bridge.contactSheet",
            &json!({
                "ids": ids,
                "rows": 2,
                "cols": 2,
                "format": "png",
                "path": png_path.to_string_lossy()
            }),
        )
        .unwrap();
    assert_eq!(png_out["format"], "png");
    assert!(png_path.is_file());

    // Single undo reverts all 3 metadata/rating stamps at once!
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.catalog.photo(lightcraft_catalog::PhotoId(ids[0])).unwrap().meta.credit, "");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn adaptive_presets_generate_semantic_masks_and_calibrated_local_adjustments() {
    let mut s = Session::with_demo();
    let id = s.active().unwrap();

    for (name, expected_kind) in [
        ("Adaptive: Subject Pop", "subject"),
        ("Adaptive: Dramatic Sky", "sky"),
        ("Adaptive: Warm Portrait", "subject"),
        ("Adaptive: Crisp Architecture", "background"),
    ] {
        let res = s.execute("develop.applyAdaptivePreset", &json!({"preset": name, "amount": 120.0, "replace": true})).unwrap();
        assert_eq!(res["changed"], 1);
        assert_eq!(res["maskKind"], expected_kind);

        let dev = s.develop_of(id).unwrap();
        assert_eq!(dev.masks.len(), 1);
        let mask = &dev.masks[0];
        assert_eq!(mask.components.len(), 1);
        match (expected_kind, &mask.components[0].shape) {
            ("subject", MaskShape::Subject) | ("sky", MaskShape::Sky) | ("background", MaskShape::Background) => {}
            other => panic!("unexpected mask shape: {other:?}"),
        }
        assert!(mask.adjust.exposure != 0.0 || mask.adjust.contrast != 0.0);
    }

    // Undo removes the last adaptive preset step cleanly.
    s.execute("edit.undo", &json!({})).unwrap();
    let prev = s.develop_of(id).unwrap();
    assert_eq!(prev.masks[0].name, "Warm Portrait");
}
