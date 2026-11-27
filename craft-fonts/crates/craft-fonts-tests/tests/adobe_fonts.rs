//! Integration tests for Adobe Fonts capabilities (`fvar` + `STAT` + `name` variable axis
//! inspector, curated `FontPackCategory` packs, `resolve_missing_font`, and `FontActivationRegistry`)
//! against the real bundled `.ttf` and `.otf` font files in `fonts/`.

use craft_fonts::{
    FONTS, Font, FontActivationRegistry, FontClassification, FontMatchKind, FontPackCategory,
    KnownAxisTag, LATIN_FONTS, all_font_packs, font_pack, inspect_variable_font,
    parse_variation_axes, resolve_missing_font,
};
use skrifa::{FontRef, MetadataProvider};

fn read_font_bytes(f: &Font) -> Vec<u8> {
    std::fs::read(craft_fonts_tests::repo_root().join(f.file))
        .unwrap_or_else(|e| panic!("{}: {e}", f.file))
}

#[test]
fn real_bundled_variable_fonts_match_skrifa_axes_and_extract_instances() {
    let mut variable_count = 0usize;
    for f in LATIN_FONTS {
        let data = read_font_bytes(f);
        let parsed = parse_variation_axes(&data);
        let skrifa_font = FontRef::new(&data).unwrap_or_else(|e| panic!("{}: {e}", f.file));
        let skrifa_axes = skrifa_font.axes();

        assert_eq!(
            parsed.len(),
            skrifa_axes.len(),
            "{}: axis count mismatch between pure-Rust parser and skrifa",
            f.file
        );

        if f.style.ends_with("Variable") {
            variable_count += 1;
            let info = inspect_variable_font(&data)
                .unwrap_or_else(|| panic!("{}: expected VariableFontInfo", f.file));
            assert!(
                info.family_name
                    .as_deref()
                    .is_some_and(|name| name.contains(f.family)),
                "{}: unexpected family_name {:?}",
                f.file,
                info.family_name
            );
            assert!(
                !info.instances.is_empty(),
                "{}: real variable fonts in latin-manifest declare named instances",
                f.file
            );
            assert!(
                info.has_stat_table,
                "{}: expected STAT table on bundled Google Fonts variable face",
                f.file
            );

            for (ours, theirs) in parsed.iter().zip(skrifa_axes.iter()) {
                assert_eq!(ours.tag_str(), theirs.tag().to_string(), "{}: tag", f.file);
                assert!(
                    (ours.min_value - theirs.min_value()).abs() < 0.01,
                    "{}: min {} vs {}",
                    f.file,
                    ours.min_value,
                    theirs.min_value()
                );
                assert!(
                    (ours.default_value - theirs.default_value()).abs() < 0.01,
                    "{}: default {} vs {}",
                    f.file,
                    ours.default_value,
                    theirs.default_value()
                );
                assert!(
                    (ours.max_value - theirs.max_value()).abs() < 0.01,
                    "{}: max {} vs {}",
                    f.file,
                    ours.max_value,
                    theirs.max_value()
                );
                assert_eq!(ours.is_hidden(), theirs.is_hidden(), "{}: hidden", f.file);
            }
        } else {
            assert!(
                parsed.is_empty(),
                "{}: static face should have zero variation axes",
                f.file
            );
        }
    }
    assert_eq!(variable_count, 10);
}

#[test]
fn nunito_sans_and_roboto_expose_multi_axis_opsz_wdth_wght() {
    let nunito = LATIN_FONTS
        .iter()
        .find(|f| f.family == "Nunito Sans" && f.style == "Variable")
        .unwrap();
    let axes = parse_variation_axes(&read_font_bytes(nunito));
    assert!(
        axes.iter().any(|a| a.known_tag == KnownAxisTag::Weight),
        "Nunito Sans must expose wght"
    );
    assert!(
        axes.iter()
            .any(|a| a.known_tag == KnownAxisTag::Width || a.known_tag == KnownAxisTag::OpticalSize),
        "Nunito Sans must expose additional wdth/opsz axes: {axes:?}"
    );

    // Static OTF (CFF) font in default manifest has no fvar table and parses cleanly as empty.
    let cjk_otf = FONTS
        .iter()
        .find(|f| f.family == "Noto Sans CJK SC")
        .unwrap();
    let cjk_bytes = read_font_bytes(cjk_otf);
    assert_eq!(&cjk_bytes[..4], b"OTTO");
    assert!(parse_variation_axes(&cjk_bytes).is_empty());
}

#[test]
fn curated_packs_and_activation_registry_load_real_font_files() {
    let mut registry = FontActivationRegistry::new();
    for pack in all_font_packs() {
        let count = registry.activate_pack(pack.category);
        assert!(count > 0, "{} added no families", pack.id);
    }

    let json = registry.to_json();
    let reloaded = FontActivationRegistry::from_json(&json).unwrap();
    assert_eq!(reloaded, registry);

    let active_faces = reloaded.active_catalog_fonts();
    assert!(active_faces.len() >= 40);
    for f in active_faces {
        assert!(
            craft_fonts_tests::repo_root().join(f.file).is_file(),
            "missing active font file {}",
            f.file
        );
    }

    assert_eq!(
        font_pack(FontPackCategory::EditorialPublishing).specimens[0].classification,
        FontClassification::Serif
    );
}

#[test]
fn missing_font_resolver_maps_to_real_on_disk_catalog_faces() {
    let cases = [
        (
            "HelveticaNeue-Bold",
            "Liberation Sans",
            FontClassification::Sans,
            FontMatchKind::MetricSubstitute,
        ),
        (
            "AdobeMyungjoStd-Medium",
            "NanumGothic",
            FontClassification::Cjk,
            FontMatchKind::ScriptSubstitute,
        ),
        (
            "NotoSansCJKkr-Regular",
            "NanumGothic",
            FontClassification::Cjk,
            FontMatchKind::ScriptSubstitute,
        ),
        (
            "AdobeSongStd-Light",
            "Noto Sans CJK SC",
            FontClassification::Cjk,
            FontMatchKind::ScriptSubstitute,
        ),
        (
            "NotoSansCJKsc-Bold",
            "Noto Sans CJK SC",
            FontClassification::Cjk,
            FontMatchKind::ExactFamily,
        ),
        (
            "$ID/KozGoPr6N-Bold",
            "BIZ UDPGothic",
            FontClassification::Cjk,
            FontMatchKind::ScriptSubstitute,
        ),
        (
            "PlayfairDisplay-Italic",
            "Playfair Display",
            FontClassification::Serif,
            FontMatchKind::ExactFamily,
        ),
    ];

    for (query, expected_family, expected_class, expected_kind) in cases {
        let resolved = resolve_missing_font(query);
        assert_eq!(resolved.family, expected_family, "{query}");
        assert_eq!(resolved.classification, expected_class, "{query}");
        assert_eq!(resolved.match_kind, expected_kind, "{query}");
        let face = resolved
            .catalog_font
            .unwrap_or_else(|| panic!("{query}: expected resolved catalog_font"));
        assert!(
            craft_fonts_tests::repo_root().join(face.file).is_file(),
            "{query}: resolved file {} does not exist",
            face.file
        );
    }
}
