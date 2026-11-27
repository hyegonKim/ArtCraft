use std::collections::HashSet;

use crate::{
    ASE_MAGIC, ASE_VERSION, AseColor, AseColorGroup, AseColorType, AseItem, AsePalette, AseSwatch, BLOCK_COLOR_ENTRY, BLOCK_GROUP_END,
    BLOCK_GROUP_START, CCLIBS_CONTAINER_MIME, CcElement, CcElementType, CcLibraryArchive, CcLibraryGroup, CcRepresentation, Error,
    LIBS_SCHEMA_VERSION, Limits, is_ase_bytes, is_cclibs_zip, valid_archive_asset_path,
};

#[test]
fn all_19_libs_v4_20_2_mime_element_types_round_trip() {
    let expected_mimes = [
        "application/vnd.adobe.element.color+dcx",
        "application/vnd.adobe.element.colortheme+dcx",
        "application/vnd.adobe.element.gradient+dcx",
        "application/vnd.adobe.element.characterstyle+dcx",
        "application/vnd.adobe.element.paragraphstyle+dcx",
        "application/vnd.adobe.element.brush+dcx",
        "application/vnd.adobe.element.layerstyle+dcx",
        "application/vnd.adobe.element.image+dcx",
        "application/vnd.adobe.element.3d+dcx",
        "application/vnd.adobe.element.material+dcx",
        "application/vnd.adobe.element.light+dcx",
        "application/vnd.adobe.element.audio+dcx",
        "application/vnd.adobe.element.video+dcx",
        "application/vnd.adobe.element.animation+dcx",
        "application/vnd.adobe.element.template+dcx",
        "application/vnd.adobe.element.component+dcx",
        "application/vnd.adobe.element.text+dcx",
        "application/vnd.adobe.element.agc+json",
        "application/vnd.adobe.element.agc+dcx",
    ];
    assert_eq!(CcElementType::ALL.len(), 19);
    assert_eq!(expected_mimes.len(), 19);

    let mut unique = HashSet::new();
    for (variant, expected_mime) in CcElementType::ALL.into_iter().zip(expected_mimes) {
        assert!(unique.insert(variant), "duplicate variant {variant:?}");
        assert_eq!(variant.mime_type(), expected_mime);
        assert_eq!(CcElementType::from_mime(expected_mime), Some(variant));
        assert_eq!(CcElementType::from_short_name(variant.short_name()), Some(variant));
        assert_eq!(variant.is_dcx_container(), variant != CcElementType::AgcJson);

        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!("{expected_mime:?}"));
        let decoded: CcElementType = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, variant);
    }

    assert_eq!(CcElementType::from_mime("application/vnd.adobe.agc+json"), Some(CcElementType::AgcJson));
    assert_eq!(CcElementType::from_mime("application/vnd.adobe.agc+dcx"), Some(CcElementType::AgcDcx));
    assert_eq!(CcElementType::from_mime("application/vnd.adobe.element.unknown+dcx"), None);
}

#[test]
fn cclibs_zip_archive_round_trips_all_19_element_types_and_binary_assets() {
    let mut archive = CcLibraryArchive::new("lib-brand-2027", "ArtCraft Brand System");
    assert_eq!(archive.container_type, CCLIBS_CONTAINER_MIME);
    assert_eq!(archive.libs_version, LIBS_SCHEMA_VERSION);
    archive.groups.push(CcLibraryGroup { id: "grp-core".into(), name: "Core Tokens".into() });

    for (idx, elem_type) in CcElementType::ALL.into_iter().enumerate() {
        let elem_id = format!("elem-{:02}", idx + 1);
        let mut elem = CcElement::new(&elem_id, format!("Asset {}", elem_type.short_name()), elem_type);
        elem.group_id = Some("grp-core".into());

        if elem_type == CcElementType::Color {
            elem = CcElement::from_color(&elem_id, "Brand Cobalt", AseColor::Rgb { r: 0.184, g: 0.482, b: 0.961 }, AseColorType::Spot);
            elem.group_id = Some("grp-core".into());
        } else if elem_type == CcElementType::ColorTheme {
            elem = CcElement::from_color_theme(
                &elem_id,
                "Sunset Theme",
                &[
                    (AseColor::Rgb { r: 0.95, g: 0.35, b: 0.25 }, AseColorType::Global),
                    (AseColor::Cmyk { c: 0.10, m: 0.80, y: 0.90, k: 0.05 }, AseColorType::Process),
                ],
            );
        } else if elem_type == CcElementType::AgcJson {
            elem.representations.push(CcRepresentation::inline_json(
                "agc-payload",
                "application/json",
                serde_json::json!({"schema": "agc-1.0", "nodes": [1, 2, 3]}),
            ));
        } else {
            let asset_path = format!("elements/{elem_id}/primary.bin");
            let payload = format!("binary-payload-for-{}", elem_type.short_name()).into_bytes();
            archive.set_asset(&asset_path, payload).unwrap();
            elem.representations.push(CcRepresentation::asset_file("rep-primary", "application/octet-stream", "primary", asset_path));
        }
        elem.metadata.insert("index".into(), serde_json::json!(idx));
        archive.add_element(elem);
    }

    let zip_bytes = archive.to_zip_bytes().unwrap();
    assert!(is_cclibs_zip(&zip_bytes));
    assert_eq!(craft_zip::stored_mimetype(&zip_bytes), Some(CCLIBS_CONTAINER_MIME.as_bytes()));

    let restored = CcLibraryArchive::from_zip_bytes(&zip_bytes).unwrap();
    assert_eq!(restored, archive);
    assert_eq!(restored.elements.len(), 19);
    for elem_type in CcElementType::ALL {
        assert_eq!(restored.elements_by_type(elem_type).count(), 1, "missing {elem_type}");
    }
    assert_eq!(restored.asset("elements/elem-08/primary.bin"), Some(b"binary-payload-for-image+dcx".as_slice()));
}

#[test]
fn cclibs_archive_rejects_traversal_paths_and_missing_referenced_assets() {
    for bad in ["", "/abs/path.png", "elements/../secret.txt", "elements\\win.png", "manifest.json", "mimetype"] {
        assert!(!valid_archive_asset_path(bad), "expected invalid path: {bad}");
    }
    assert!(valid_archive_asset_path("elements/elem-1/rendition.png"));

    let mut archive = CcLibraryArchive::new("lib-bad", "Broken");
    assert!(matches!(archive.set_asset("../escape.bin", vec![1, 2, 3]), Err(Error::InvalidAssetPath(_))));

    let mut orphan = CcElement::new("e1", "Missing File", CcElementType::Image);
    orphan.representations.push(CcRepresentation::asset_file("r1", "image/png", "primary", "elements/e1/missing.png"));
    archive.add_element(orphan);
    assert!(matches!(archive.to_zip_bytes(), Err(Error::InvalidArchive(_))));
}

#[test]
fn ase_binary_swatch_round_trips_all_color_models_types_and_groups() {
    let mut palette = AsePalette::new();
    palette.push_swatch(AseSwatch { name: "Pure White Gray".into(), color: AseColor::Gray { value: 1.0 }, color_type: AseColorType::Process });
    palette.push_group(AseColorGroup {
        name: "Brand Colors 한국어".into(),
        swatches: vec![
            AseSwatch {
                name: "ArtCraft Blue".into(),
                color: AseColor::Rgb { r: 0.184_313_73, g: 0.482_352_94, b: 0.960_784_3 },
                color_type: AseColorType::Global,
            },
            AseSwatch {
                name: "Print Magenta Spot".into(),
                color: AseColor::Cmyk { c: 0.02, m: 0.94, y: 0.15, k: 0.0 },
                color_type: AseColorType::Spot,
            },
            AseSwatch { name: "Perceptual Gold Lab".into(), color: AseColor::Lab { l: 0.78, a: 8.5, b: 64.25 }, color_type: AseColorType::Process },
        ],
    });
    palette.push_swatch(AseSwatch { name: "Charcoal Ink".into(), color: AseColor::Gray { value: 0.15 }, color_type: AseColorType::Spot });

    assert_eq!(palette.block_count().unwrap(), 7); // 2 top-level + (group start + 3 swatches + group end)

    let encoded = palette.to_bytes().unwrap();
    assert!(is_ase_bytes(&encoded));
    assert_eq!(&encoded[0..4], &ASE_MAGIC);
    assert_eq!(u16::from_be_bytes([encoded[4], encoded[5]]), ASE_VERSION.0);
    assert_eq!(u16::from_be_bytes([encoded[6], encoded[7]]), ASE_VERSION.1);
    assert_eq!(u32::from_be_bytes([encoded[8], encoded[9], encoded[10], encoded[11]]), 7);

    let decoded = AsePalette::from_bytes(&encoded).unwrap();
    assert_eq!(decoded, palette);
    assert!(matches!(&decoded.items[0], AseItem::Swatch(s) if s.name == "Pure White Gray"));
    assert!(matches!(&decoded.items[1], AseItem::Group(g) if g.name == "Brand Colors 한국어"));
    assert_eq!(decoded.all_swatches().count(), 5);
    assert_eq!(decoded.groups().count(), 1);

    // Verify sRGB preview conversion produces sane RGB triples across all four color models.
    for swatch in decoded.all_swatches() {
        let rgb = swatch.color.to_srgb8();
        if swatch.name == "Pure White Gray" {
            assert_eq!(rgb, [255, 255, 255]);
        }
    }
}

#[test]
fn cclibs_and_ase_interoperate_cleanly() {
    let mut ase = AsePalette::new();
    ase.push_swatch(AseSwatch { name: "Primary Red".into(), color: AseColor::Rgb { r: 0.9, g: 0.1, b: 0.15 }, color_type: AseColorType::Spot });
    ase.push_group(AseColorGroup {
        name: "Print CMYK Triad".into(),
        swatches: vec![
            AseSwatch { name: "PrintTriad 1".into(), color: AseColor::Cmyk { c: 0.85, m: 0.1, y: 0.0, k: 0.0 }, color_type: AseColorType::Global },
            AseSwatch { name: "PrintTriad 2".into(), color: AseColor::Lab { l: 0.65, a: -24.0, b: 18.0 }, color_type: AseColorType::Process },
        ],
    });

    let lib = CcLibraryArchive::from_ase_palette("lib-swatches", "Converted Swatches", &ase);
    assert_eq!(lib.elements_by_type(CcElementType::Color).count(), 1);
    assert_eq!(lib.elements_by_type(CcElementType::ColorTheme).count(), 1);

    let zip = lib.to_zip_bytes().unwrap();
    let reopened = CcLibraryArchive::from_zip_bytes(&zip).unwrap();
    let round_tripped_ase = reopened.to_ase_palette();
    assert_eq!(round_tripped_ase.all_swatches().count(), 3);
    assert_eq!(round_tripped_ase.groups().count(), 1);
}

#[test]
fn ase_reader_rejects_malformed_and_hostile_inputs_without_panicking() {
    assert!(matches!(AsePalette::from_bytes(b"NOT_ASEF_DATA"), Err(Error::InvalidAse(_))));

    // Exceeds max_ase_blocks limit before allocating.
    let mut huge_count = Vec::new();
    huge_count.extend_from_slice(&ASE_MAGIC);
    huge_count.extend_from_slice(&1u16.to_be_bytes());
    huge_count.extend_from_slice(&0u16.to_be_bytes());
    huge_count.extend_from_slice(&100u32.to_be_bytes());
    let tight = Limits { max_ase_blocks: 10, ..Limits::default() };
    assert!(matches!(AsePalette::from_bytes_with_limits(&huge_count, tight), Err(Error::LimitExceeded { what: "ASE block count", limit: 10 })));

    // Unclosed 0xC001 group block.
    let mut unclosed = Vec::new();
    unclosed.extend_from_slice(&ASE_MAGIC);
    unclosed.extend_from_slice(&1u16.to_be_bytes());
    unclosed.extend_from_slice(&0u16.to_be_bytes());
    unclosed.extend_from_slice(&1u32.to_be_bytes());
    unclosed.extend_from_slice(&BLOCK_GROUP_START.to_be_bytes());
    unclosed.extend_from_slice(&4u32.to_be_bytes());
    unclosed.extend_from_slice(&1u16.to_be_bytes()); // 1 UTF-16 unit (NUL)
    unclosed.extend_from_slice(&0u16.to_be_bytes());
    assert!(matches!(AsePalette::from_bytes(&unclosed), Err(Error::InvalidAse(_))));

    // Unmatched 0xC002 group end block.
    let mut unmatched = Vec::new();
    unmatched.extend_from_slice(&ASE_MAGIC);
    unmatched.extend_from_slice(&1u16.to_be_bytes());
    unmatched.extend_from_slice(&0u16.to_be_bytes());
    unmatched.extend_from_slice(&1u32.to_be_bytes());
    unmatched.extend_from_slice(&BLOCK_GROUP_END.to_be_bytes());
    unmatched.extend_from_slice(&0u32.to_be_bytes());
    assert!(matches!(AsePalette::from_bytes(&unmatched), Err(Error::InvalidAse(_))));

    // Non-finite float rejected on both write and read.
    let mut nan_palette = AsePalette::new();
    nan_palette.push_swatch(AseSwatch { name: "NaN".into(), color: AseColor::Gray { value: f32::NAN }, color_type: AseColorType::Process });
    assert!(matches!(nan_palette.to_bytes(), Err(Error::InvalidAse(_))));

    let mut raw_nan = Vec::new();
    raw_nan.extend_from_slice(&ASE_MAGIC);
    raw_nan.extend_from_slice(&1u16.to_be_bytes());
    raw_nan.extend_from_slice(&0u16.to_be_bytes());
    raw_nan.extend_from_slice(&1u32.to_be_bytes());
    raw_nan.extend_from_slice(&BLOCK_COLOR_ENTRY.to_be_bytes());
    raw_nan.extend_from_slice(&14u32.to_be_bytes());
    raw_nan.extend_from_slice(&1u16.to_be_bytes());
    raw_nan.extend_from_slice(&0u16.to_be_bytes());
    raw_nan.extend_from_slice(b"Gray");
    raw_nan.extend_from_slice(&f32::INFINITY.to_be_bytes());
    raw_nan.extend_from_slice(&2u16.to_be_bytes());
    assert!(matches!(AsePalette::from_bytes(&raw_nan), Err(Error::InvalidAse(_))));
}
