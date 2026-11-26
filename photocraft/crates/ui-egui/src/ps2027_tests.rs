//! End-to-end verification of Adobe Photoshop 2027 (`28.0.0.20`) and Camera Raw `19.0`
//! (`19.0.0.2794`) features across `photocraft-ui-egui`, `photocraft-engine`, `photocraft-io`,
//! `photocraft-compose`, `photocraft-paint`, `photocraft-text`, and official Korean (`ko`) i18n.

use photocraft_doc::LayerContent;
use serde_json::json;

use crate::PhotocraftApp;
use crate::state::Tool;

fn test_app() -> PhotocraftApp {
    let mut app = PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default());
    app.run("file.new", json!({"width": 96, "height": 64, "background": "white"})).unwrap();
    // Paint non-uniform color gradient/detail so filters, relight, ACR, ambient removal, and
    // raster-to-text have real contrast.
    if let Some(st) = app.session.active_mut() {
        let doc = std::sync::Arc::make_mut(&mut st.doc);
        if let Some(layer) = doc.layers.first_mut()
            && let Some(surf) = layer.surface_mut()
        {
            for y in 0..64i32 {
                for x in 0..96i32 {
                    let is_glyph_bar = (16..28).contains(&y)
                        && ((12..18).contains(&x) || (24..30).contains(&x) || (36..44).contains(&x));
                    let r = if is_glyph_bar { 0.05 } else { (x as f32 / 95.0) * 0.8 + 0.1 };
                    let g = if is_glyph_bar { 0.05 } else { (y as f32 / 63.0) * 0.7 + 0.15 };
                    let b = if is_glyph_bar { 0.08 } else { 0.55 };
                    surf.write_pixel(x, y, &[r, g, b, 1.0]);
                }
            }
        }
    }
    app
}

#[test]
fn ko_ps2027_catalog_covers_all_ps2027_and_acr19_strings_and_preserves_ellipsis() {
    for &(en, ko) in crate::ko_ps2027::PLAIN_KO {
        assert!(!ko.is_empty(), "empty Korean translation for {en:?}");
        assert_eq!(
            en.ends_with('…'),
            ko.ends_with('…'),
            "ellipsis mismatch for plain row {en:?} -> {ko:?}",
        );
    }

    let ko = crate::i18n::ko_ps2027_lang();
    assert_eq!(ko.code(), "ko");

    for (en, expected) in [
        ("Timelapse", "타임랩스"),
        ("Export Timelapse…", "타임랩스 내보내기…"),
        ("High", "높음"),
        ("Medium", "중간"),
        ("Low", "낮음"),
        ("Fireflies", "반딧불이"),
        ("Quantity", "수량"),
        ("Depth Variation", "깊이 베리에이션"),
        ("Blurriness", "흐림"),
        ("Color", "컬러"),
        ("Progress", "진행률"),
        ("Water Ripple", "물결"),
        ("Horizon", "수평선"),
        ("Amplitude", "진폭"),
        ("Frequency", "주파수"),
        ("Oil Brush Tool", "유화 브러시 도구"),
        ("Blocky", "블록"),
        ("Glaze", "윤기"),
        ("Round", "원형"),
        ("Paint Mix", "페인트 혼합"),
        ("Shadows", "그림자"),
        ("Reflections", "반사"),
        ("Lens flares", "렌즈 플레어"),
        ("Atmospheric", "분위기 있는"),
        ("Remove Related Elements", "관련 요소 제거"),
        ("Enhance", "향상"),
    ] {
        assert_eq!(crate::i18n::tr(ko, en), expected, "{en}");
    }

    for (id, en, expected) in [
        ("window.toggle.timelapse", "Timelapse", "타임랩스"),
        ("file.export.timelapse", "Export Timelapse…", "타임랩스 내보내기…"),
        ("filter.nge.fireflies", "Fireflies…", "반딧불이…"),
        ("filter.nge.waterRipple", "Water Ripple…", "물결…"),
        ("layer.newRelightLayer", "New Relight Layer…", "새 재조명 레이어…"),
        ("filter.render.relight3d", "Relight…", "재조명…"),
        ("edit.removeAmbientEffects", "Remove Ambient Effects…", "주변 효과 제거…"),
    ] {
        assert_eq!(crate::i18n::tr_id(ko, id, en), expected, "{id}");
    }
}

#[test]
fn ko_ps2027_catalog_covers_type_recompose_mockup_skills_and_acr19() {
    let ko = crate::i18n::ko_ps2027_lang();

    for (id, en, expected) in [
        ("type.rasterToText", "Convert to Text Layers", "텍스트 레이어로 변환"),
        ("type.releaseDynamicFit", "Release Dynamic Fit", "다이나믹 맞춤 풀기"),
        ("type.pasteAttributes", "Paste Text Attributes", "텍스트 속성 붙여넣기"),
        ("edit.generativeRecompose", "Generative Recompose…", "생성형 재구성…"),
        ("filter.stylize.restyle", "Generative Restyle…", "생성형 스타일 변경…"),
        ("layer.newRestyleLayer", "New Restyle Layer…", "새 스타일 변경 레이어…"),
        ("layer.restyle.captureAsLayer", "Capture as Layer", "레이어로 캡처"),
        ("layer.mockup.release", "Release Mockup", "목업 풀기"),
        ("agent.skill.cinematicGrade", "Cinematic Grade", "시네마틱 그레이드"),
        ("agent.skill.enhanceAndResize", "Enhance and Resize", "향상 및 크기 조정"),
        ("agent.skill.perfectPortrait", "Perfect Portrait", "완벽한 인물 사진"),
        ("agent.skill.productStudioShot", "Product Studio Shot", "제품 스튜디오 샷"),
        ("agent.skill.removeBackground", "Remove Background", "배경 제거"),
        ("agent.skill.retouchPortrait", "Retouch Portrait", "인물 사진 리터치"),
        ("agent.skill.socialMediaHero", "Social Media Hero", "소셜 미디어 히어로"),
        ("agent.skill.subjectSpotlight", "Subject Spotlight", "피사체 강조하기"),
        ("view.quickCompare", "Quick Compare", "빠른 비교"),
    ] {
        assert_eq!(crate::i18n::tr_id(ko, id, en), expected, "{id}");
    }

    for (en, expected) in [
        ("Mockup", "목업"),
        ("Environmental Lighting", "환경 조명"),
        ("Agent Skills", "에이전트 스킬"),
        ("Channel Gradient Background", "채널의 그레이디언트 배경"),
        ("Grid Size", "그리드 크기"),
        ("Multithreaded PSD Saving", "멀티스레드 PSD 저장"),
        ("Eyeglass Reflections", "안경 반사 제거"),
        ("Window Reflections", "창문 반사 제거"),
        ("Non-Raw Denoise", "비 Raw 노이즈 감소"),
        ("Adaptive Color", "적응형 컬러"),
        ("HDR Range", "HDR 범위"),
    ] {
        assert_eq!(crate::i18n::tr(ko, en), expected, "{en}");
    }

    crate::i18n::with_language(ko, || {
        assert_eq!(crate::i18n::t("Timelapse"), "타임랩스");
        assert_eq!(crate::i18n::t("Oil Brush Tool"), "유화 브러시 도구");
    });
}

#[test]
fn ps2027_menus_and_oil_brush_tool_are_wired_end_to_end() {
    let mut app = test_app();
    let ids: std::collections::HashSet<String> =
        crate::menus::menu_items(&app).into_iter().map(|i| i.id).collect();

    for required in [
        "file.export.timelapse",
        "edit.removeAmbientEffects",
        "edit.generativeRecompose",
        "layer.newAdjustmentLayer.fireflies",
        "layer.newAdjustmentLayer.waterRipple",
        "layer.newRelightLayer",
        "layer.newRestyleLayer",
        "layer.restyle.captureAsLayer",
        "layer.mockup.create",
        "layer.mockup.release",
        "type.rasterToText",
        "type.setDynamicFit",
        "type.releaseDynamicFit",
        "edit.copyTextAttributes",
        "edit.pasteTextAttributes",
        "filter.nge.fireflies",
        "filter.nge.waterRipple",
        "filter.render.relight3d",
        "filter.stylize.restyle",
        "view.quickCompare",
        "window.toggle.timelapse",
        "agent.skill.cinematicGrade",
        "agent.skill.enhanceAndResize",
        "agent.skill.perfectPortrait",
        "agent.skill.productStudioShot",
        "agent.skill.removeBackground",
        "agent.skill.retouchPortrait",
        "agent.skill.socialMediaHero",
        "agent.skill.subjectSpotlight",
    ] {
        assert!(ids.contains(required), "missing menu item id: {required}");
    }

    // Oil Brush Tool (70th tool)
    let ctx = egui::Context::default();
    assert_eq!(Tool::ALL.len(), 70);
    assert!(Tool::ALL.contains(&Tool::OilBrush));
    crate::menus::invoke(&mut app, &ctx, "tools.select.oilBrush", json!({})).unwrap();
    assert_eq!(app.ui.tool, Tool::OilBrush);

    for preset in ["blocky", "glaze", "round"] {
        app.run(
            "tools.setOilBrush",
            json!({"preset": preset, "size": 18.0, "opacity": 0.9, "paintMix": 0.65}),
        )
        .unwrap();
        app.run(
            "paint.oilBrush",
            json!({
                "preset": preset,
                "size": 14.0,
                "opacity": 0.85,
                "paintMix": 0.6,
                "color": [0.9, 0.2, 0.1, 1.0],
                "points": [[10.0, 12.0], [28.0, 22.0], [50.0, 30.0]]
            }),
        )
        .unwrap();
    }
}

#[test]
fn timelapse_panel_and_export_write_valid_mp4_and_gif() {
    let mut app = test_app();
    let ctx = egui::Context::default();
    assert!(!app.ui.panels.timelapse);
    crate::menus::invoke(&mut app, &ctx, "window.toggle.timelapse", json!({})).unwrap();
    assert!(app.ui.panels.timelapse);

    app.run(
        "timelapse.configure",
        json!({"resolution": "1920", "quality": "high"}),
    )
    .unwrap();
    app.run("timelapse.start", json!({})).unwrap();
    app.run(
        "paint.oilBrush",
        json!({"preset": "round", "size": 10.0, "points": [[8.0, 8.0], [40.0, 32.0]]}),
    )
    .unwrap();
    app.run("filter.nge.fireflies", json!({"quantity": 0.6, "blurriness": 0.4})).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let mp4_path = dir.path().join("timelapse_out.mp4");
    let gif_path = dir.path().join("timelapse_out.gif");

    app.run(
        "file.export.timelapse",
        json!({
            "path": mp4_path.to_string_lossy(),
            "resolution": "1920",
            "quality": "high",
            "fps": 24
        }),
    )
    .unwrap();
    let mp4_bytes = std::fs::read(&mp4_path).unwrap();
    assert!(mp4_bytes.len() > 64);
    assert_eq!(&mp4_bytes[4..8], b"ftyp");

    app.run(
        "file.export.timelapse",
        json!({
            "path": gif_path.to_string_lossy(),
            "resolution": "1920",
            "quality": "medium",
            "fps": 12
        }),
    )
    .unwrap();
    let gif_bytes = std::fs::read(&gif_path).unwrap();
    assert!(gif_bytes.starts_with(b"GIF89a"));
}

#[test]
fn ps2027_live_layers_ambient_recompose_type_mockup_skills_psd_and_acr19_work() {
    let mut app = test_app();

    // 1. Live NextGen Fireflies, Water Ripple, Relight 3D & Restyle + Capture as Layer
    app.run("layer.newAdjustmentLayer.fireflies", json!({"quantity": 0.7, "progress": 0.4})).unwrap();
    app.run("layer.newAdjustmentLayer.waterRipple", json!({"horizon": 0.55, "amplitude": 0.5, "frequency": 0.6})).unwrap();
    app.run("layer.newRelightLayer", json!({"ambient": 0.3, "normalStrength": 1.4})).unwrap();
    app.run("layer.newRestyleLayer", json!({"preset": "warmCinema", "intensity": 0.8})).unwrap();
    let layers_before_capture = app.session.active().unwrap().doc.layers.len();
    app.run("layer.restyle.captureAsLayer", json!({})).unwrap();
    assert_eq!(app.session.active().unwrap().doc.layers.len(), layers_before_capture + 1);

    // 2. Mockup Group + Environmental Lighting + Release Mockup
    // Select two raster layers and group into a Mockup
    let first_id = app.session.active().unwrap().doc.layers[0].id.0;
    let last_id = app.session.active().unwrap().doc.layers.last().unwrap().id.0;
    app.run("layer.select", json!({"layer": first_id})).unwrap();
    app.run("layer.select", json!({"layer": last_id, "add": true})).unwrap();
    app.run("layer.mockup.create", json!({"lighting": 0.75})).unwrap();
    app.run("layer.mockup.setLighting", json!({"environmentalLighting": 0.9})).unwrap();
    app.run("layer.mockup.release", json!({})).unwrap();

    // 3. Remove Ambient Effects across all 4 modes
    app.run("layer.select", json!({"layer": first_id})).unwrap();
    for mode in ["shadows", "reflections", "lensFlares", "atmospheric"] {
        app.run(
            "edit.removeAmbientEffects",
            json!({"mode": mode, "strength": 65.0, "removeRelatedElements": true, "enhance": true}),
        )
        .unwrap();
    }

    // 4. Type 2027: Convert Raster to Text, Dynamic Text Fit, Release Dynamic Fit, Copy/Paste Attributes
    app.run("layer.select", json!({"layer": first_id})).unwrap();
    app.run("type.rasterToText", json!({"keepOriginal": true})).unwrap();
    let text_id = app
        .session
        .active()
        .unwrap()
        .doc
        .layers
        .iter()
        .rev()
        .find(|l| matches!(l.content, LayerContent::Text(_)))
        .map(|l| l.id.0)
        .expect("converted text layer");
    app.run("layer.select", json!({"layer": text_id})).unwrap();
    app.run("type.setDynamicFit", json!({"enabled": true})).unwrap();
    app.run("edit.copyTextAttributes", json!({})).unwrap();
    app.run("type.releaseDynamicFit", json!({})).unwrap();
    app.run("edit.pasteTextAttributes", json!({})).unwrap();

    // 5. Generative Recompose
    app.run("edit.generativeRecompose", json!({"aspectRatio": "16:9", "width": 112, "height": 63})).unwrap();
    assert_eq!(app.session.active().unwrap().doc.size.width, 112);
    assert_eq!(app.session.active().unwrap().doc.size.height, 63);

    // 6. Camera Raw 19.0 filter params
    let raster_id = app
        .session
        .active()
        .unwrap()
        .doc
        .layers
        .iter()
        .find(|l| matches!(l.content, LayerContent::Raster(_)))
        .map(|l| l.id.0)
        .expect("raster layer");
    app.run("layer.select", json!({"layer": raster_id})).unwrap();
    app.run(
        "filter.cameraRaw",
        json!({
            "adaptiveProfile": true,
            "hdrMode": true,
            "hdrRangeEv": 3,
            "nonRawDenoise": 35.0,
            "eyeglassReflections": 50.0,
            "windowReflections": 45.0
        }),
    )
    .unwrap();

    // 7. Quick Compare toggle
    let ctx = egui::Context::default();
    assert!(!app.ui.view.quick_compare);
    crate::menus::invoke(&mut app, &ctx, "view.quickCompare", json!({})).unwrap();
    assert!(app.ui.view.quick_compare);
    crate::menus::invoke(&mut app, &ctx, "view.quickCompare", json!({})).unwrap();
    assert!(!app.ui.view.quick_compare);

    // 8. Multithreaded PSD save
    let dir = tempfile::tempdir().unwrap();
    let psd_path = dir.path().join("ps2027_mt.psd");
    app.run(
        "prefs.set",
        json!({"path": "performance.multithreadedPsdSaving", "value": true}),
    )
    .unwrap();
    assert!(app.session.prefs().performance.multithreaded_psd_saving);
    app.run("file.saveACopy", json!({"path": psd_path.to_string_lossy()})).unwrap();
    let psd_bytes = std::fs::read(&psd_path).unwrap();
    assert!(psd_bytes.starts_with(b"8BPS"));

    // 9. All 8 Agent Skills one-click workflows on fresh documents
    for skill in [
        "agent.skill.cinematicGrade",
        "agent.skill.enhanceAndResize",
        "agent.skill.perfectPortrait",
        "agent.skill.productStudioShot",
        "agent.skill.removeBackground",
        "agent.skill.retouchPortrait",
        "agent.skill.socialMediaHero",
        "agent.skill.subjectSpotlight",
    ] {
        let mut skill_app = test_app();
        skill_app.run(skill, json!({})).unwrap_or_else(|e| panic!("{skill} failed: {e}"));
    }
}
