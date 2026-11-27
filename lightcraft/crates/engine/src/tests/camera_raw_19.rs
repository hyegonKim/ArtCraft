use std::sync::Arc;

use lightcraft_catalog::{Album, AlbumId, MediaKind, Op, Photo, PhotoId, Source};
use lightcraft_denoise::manifest::{DenoiserManifest, Domain, Gain};
use lightcraft_develop::{DevelopSettings, Hdr, Preset};
use lightcraft_pipeline::{Overlay, RenderRequest, SourceInfo};
use lightcraft_raster::{Rgb32f, Rgba8};
use serde_json::json;

use crate::Session;
use crate::denoise::{JobSpec, Loader, RunOn, make_product};
use crate::tests_xmp::temp_dir;

fn dummy_loader() -> Loader {
    Arc::new(|_, _| Err("unused for non-raw RGB denoise".into()))
}

fn dummy_manifest() -> DenoiserManifest {
    DenoiserManifest {
        id: "builtin".into(),
        name: "Built-in".into(),
        version: "1.0".into(),
        licence: Default::default(),
        source: None,
        sha256: None,
        size_bytes: None,
        provenance: "test".into(),
        domain: Domain::BayerToRgb,
        gain: Gain::None,
        tile: 64,
        overlap: 16,
    }
}

fn encode_png_rgba(rgba: &Rgba8) -> Vec<u8> {
    lightcraft_codecs::encode_png(&lightcraft_codecs::EncodeImage::rgba8(rgba), &lightcraft_codecs::EncodeMeta::default()).unwrap()
}

#[test]
fn reflection_removal_develop_crs_roundtrip_and_overlay() {
    let mut s = Session::with_demo();
    let id = s.active().unwrap();
    let base = s.render_now(id, 96, 96).unwrap().image;

    s.execute("develop.set", &json!({"control": "reflections.eyeglass", "value": 75.0})).unwrap();
    s.execute("develop.set", &json!({"control": "reflections.window", "value": 60.0})).unwrap();

    let dev = s.develop_of(id).unwrap();
    assert_eq!(dev.reflections.eyeglass, 75.0);
    assert_eq!(dev.reflections.window, 60.0);

    let preset = Preset {
        id: "user.refl".into(),
        name: "Refl".into(),
        group: "User Presets".into(),
        settings: dev.to_json(),
        favorite: false,
        builtin: false,
    };
    let xmp = crate::crs::preset_to_xmp(&preset);
    assert!(xmp.contains("crs:EyeglassReflections=\"75\""));
    assert!(xmp.contains("crs:WindowReflections=\"60\""));

    let mut round = DevelopSettings::default();
    let parsed = crate::crs::preset_from_xmp(&xmp, "Refl").unwrap();
    round = parsed.apply(&round, 100.0);
    assert_eq!(round.reflections.eyeglass, 75.0);
    assert_eq!(round.reflections.window, 60.0);

    let suppressed = s.render_now(id, 96, 96).unwrap().image;
    assert_ne!(base.data, suppressed.data, "reflection suppression alters rendered pixels");

    let mut src = Rgb32f::new(48, 48);
    for y in 0..48 {
        for x in 0..48 {
            let glare = if (16..32).contains(&x) && (16..32).contains(&y) { 0.85 } else { 0.18 };
            src.set(x, y, [glare, glare * 0.96, glare * 1.02]);
        }
    }
    let req = RenderRequest { overlay: Overlay::Reflections, ..RenderRequest::fit(48, 48) };
    let vis = lightcraft_pipeline::render(&src, &SourceInfo::default(), &dev, &req).image;
    assert_eq!((vis.width, vis.height), (48, 48));
}

#[test]
fn hdr_sdr_curves_and_headroom_ranges() {
    assert_eq!(Hdr::HEADROOM_RANGES, [1.0, 2.0, 3.0, 4.0]);

    let mut s = Session::with_demo();
    let id = s.active().unwrap();
    s.execute("develop.hdr", &json!({"enabled": true, "maxEv": 3.0, "sdrPreview": false})).unwrap();
    s.execute("develop.curve", &json!({"channel": "hdr", "points": [[0.0, 0.0], [0.5, 0.72], [1.0, 1.0]]})).unwrap();
    s.execute("develop.curve", &json!({"channel": "sdr", "points": [[0.0, 0.0], [0.5, 0.35], [1.0, 0.92]]})).unwrap();

    let dev = s.develop_of(id).unwrap();
    assert_eq!(dev.hdr.max_ev, 3.0);
    assert_eq!(dev.curve.hdr_master.len(), 3);
    assert_eq!(dev.curve.sdr_master.len(), 3);

    let sdr = dev.sdr_rendition();
    assert!(!sdr.hdr.enabled);
    assert_eq!(sdr.curve.master, dev.curve.sdr_master);
    assert!(sdr.curve.hdr_master.is_empty());

    s.execute("curve.reset", &json!({"channel": "hdr"})).unwrap();
    assert!(s.develop_of(id).unwrap().curve.hdr_master.is_empty());
    assert_eq!(s.develop_of(id).unwrap().curve.sdr_master.len(), 3);

    s.execute("curve.reset", &json!({"channel": "sdr"})).unwrap();
    assert!(s.develop_of(id).unwrap().curve.sdr_master.is_empty());
}

#[test]
fn adaptive_profiles_and_point_color_swatches() {
    let mut dark = Rgb32f::new(32, 32);
    let mut bright = Rgb32f::new(32, 32);
    for y in 0..32 {
        for x in 0..32 {
            let t = x as f32 / 31.0;
            dark.set(x, y, [0.03 + 0.08 * t, 0.025 + 0.07 * t, 0.04 + 0.09 * t]);
            bright.set(x, y, [0.55 + 0.40 * t, 0.52 + 0.38 * t, 0.48 + 0.36 * t]);
        }
    }
    let mut d = DevelopSettings::default();
    d.profile.id = "lc.adaptive".into();
    let info = SourceInfo::default();
    let eff_dark = lightcraft_pipeline::profiles::effective_for_image(&d, &dark, &info);
    let eff_bright = lightcraft_pipeline::profiles::effective_for_image(&d, &bright, &info);
    assert!(
        eff_dark.light.shadows > eff_bright.light.shadows,
        "adaptive profile lifts shadows more on dark scenes ({} vs {})",
        eff_dark.light.shadows,
        eff_bright.light.shadows
    );

    let mut s = Session::with_demo();
    let id = s.active().unwrap();
    let list = s.execute("pointColor.swatches", &json!({})).unwrap();
    let swatches = list["swatches"].as_array().unwrap();
    assert!(swatches.len() >= 6);

    s.execute("pointColor.applySwatch", &json!({"swatchId": "builtin.skin-warm"})).unwrap();
    assert_eq!(s.develop_of(id).unwrap().point_colors.len(), 1);

    let saved = s.execute("pointColor.saveSwatch", &json!({"name": "Custom Skin", "index": 0})).unwrap();
    let custom_id = saved["swatch"]["id"].as_str().unwrap().to_string();
    let list2 = s.execute("pointColor.swatches", &json!({})).unwrap();
    assert_eq!(list2["swatches"].as_array().unwrap().len(), swatches.len() + 1);

    s.execute("pointColor.deleteSwatch", &json!({"swatchId": custom_id})).unwrap();
    let list3 = s.execute("pointColor.swatches", &json!({})).unwrap();
    assert_eq!(list3["swatches"].as_array().unwrap().len(), swatches.len());
}

#[test]
fn non_raw_denoise_product_and_eligibility() {
    let dir = temp_dir("nonraw_denoise");
    let mut rgba = Rgba8::new(48, 36);
    for y in 0..36 {
        for x in 0..48 {
            let n = (((x * 37 + y * 91) % 29) as i32 - 14) as i16;
            let base = if x < 24 { 70i16 } else { 185i16 };
            let c = (base + n).clamp(0, 255) as u8;
            rgba.data[y * 48 + x] = [c, c.saturating_add(4), c.saturating_sub(3), 255];
        }
    }
    let png_bytes = encode_png_rgba(&rgba);
    let img_path = dir.join("noisy.png");
    std::fs::write(&img_path, &png_bytes).unwrap();

    let mut photo =
        Photo::new(PhotoId(77), Source::File { path: img_path.to_string_lossy().into_owned() }, "noisy.png", "PNG", 48, 36, "2026-11-14T10:00:00");
    photo.kind = MediaKind::Image;
    assert!(crate::denoise::eligible(&photo), "rendered image on disk is eligible for Non-Raw Denoise");

    let prod_path = dir.join("noisy.lcdn");
    let spec = JobSpec {
        path: img_path.to_string_lossy().into_owned(),
        read: None,
        product: prod_path.clone(),
        key: "nonraw-key".into(),
        manifest: dummy_manifest(),
        onnx: dir.join("unused.onnx"),
        model: Default::default(),
        loader: dummy_loader(),
        parallel: 2,
        run_on: RunOn::Cpu,
    };
    let wrote = make_product(&spec, None).unwrap();
    assert!(wrote);
    let denoised = lightcraft_denoise::product::read(&prod_path, "nonraw-key", 1).unwrap();
    assert_eq!((denoised.width, denoised.height), (48, 36));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hdr_panorama_merge_brackets_and_headroom() {
    let dir = temp_dir("hdr_pano_merge");
    let mut s = Session::new().with_fs();
    let mut ids = Vec::new();

    // 2 pan positions x 2 exposure brackets = 4 frames with genuine perspective overlap
    let views = lightcraft_merge::synth::pano_views(320, 240, 280.0, &[-16.0, 16.0]).unwrap();
    for (pan, base_view) in views.into_iter().enumerate() {
        for (br, gain) in [(0usize, 0.35f32), (1usize, 1.0f32)] {
            let mut v = base_view.clone();
            for px in &mut v.data {
                px[0] *= gain;
                px[1] *= gain;
                px[2] *= gain;
            }
            let rgba = v.to_srgb8();
            let p = dir.join(format!("frame_{pan}_{br}.png"));
            std::fs::write(&p, encode_png_rgba(&rgba)).unwrap();
            let rep = crate::import::import(&mut s, &[p.to_string_lossy().into_owned()], crate::import::ImportMode::Add).unwrap();
            ids.extend(rep.imported);
        }
    }
    assert_eq!(ids.len(), 4);

    let info = s.execute("merge.hdrPanorama", &json!({"ids": ids, "bracket": 2, "preview": true, "deghost": "medium", "showOverlay": true})).unwrap();
    assert_eq!(info["bracket"], 2);
    assert_eq!(info["bracketGroups"], 2);
    assert!(info["recommendedHeadroomEv"].as_f64().unwrap() >= 1.0);
    assert!(info["previewSize"][0].as_u64().unwrap() > 0);

    let merged = s.execute("merge.hdrPanorama", &json!({"ids": ids, "bracket": 2, "hdrOutput": true})).unwrap();
    let new_id = lightcraft_catalog::PhotoId(merged["id"].as_u64().unwrap());
    let dev = s.develop_of(new_id).unwrap();
    assert!(dev.hdr.enabled, "hdrOutput enables HDR mode on merged HDR Panorama");
    assert!(Hdr::HEADROOM_RANGES.contains(&dev.hdr.max_ev));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn book_module_and_pdf_export() {
    let dir = temp_dir("book_pdf");
    let mut s = Session::with_demo();
    s.execute(
        "book.set",
        &json!({
            "format": "standardLandscape",
            "title": "Portfolio 2027",
            "subtitle": "Selected Works",
            "author": "LightCraft",
            "dpi": 72
        }),
    )
    .unwrap();
    let auto = s.execute("book.autoLayout", &json!({"layout": "twoUp"})).unwrap();
    assert!(!auto["pages"].as_array().unwrap().is_empty());

    let pdf_path = dir.join("portfolio.pdf");
    let exported = s.execute("book.exportPdf", &json!({"path": pdf_path.to_string_lossy()})).unwrap();
    assert!(exported["pageCount"].as_u64().unwrap() >= 2);
    let bytes = std::fs::read(&pdf_path).unwrap();
    assert!(bytes.starts_with(b"%PDF-"), "exported book is a valid PDF");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tether_hot_folder_and_video_trim() {
    let dir = temp_dir("tether_and_video");
    let watch = dir.join("watch");
    let dest = dir.join("dest");
    std::fs::create_dir_all(&watch).unwrap();
    std::fs::create_dir_all(&dest).unwrap();

    let mut s = Session::new().with_fs();
    let album_id = AlbumId(s.catalog.alloc_album_id().0);
    s.commit("Create Tether Album", Op::AddAlbum { album: Album::new(album_id, "Tether Selects") }).unwrap();

    s.execute(
        "tether.start",
        &json!({
            "watchDir": watch.to_string_lossy(),
            "destinationDir": dest.to_string_lossy(),
            "sessionName": "Portrait Shoot",
            "namingPrefix": "SHOOT",
            "startNumber": 1,
            "albumId": album_id.0,
            "autoAdvance": true
        }),
    )
    .unwrap();

    let mut rgba = Rgba8::new(32, 24);
    rgba.data.fill([180, 180, 180, 255]);
    std::fs::write(watch.join("DSC0001.png"), encode_png_rgba(&rgba)).unwrap();

    // First poll records candidate size; second poll settles & imports!
    let p1 = s.execute("tether.poll", &json!({})).unwrap();
    assert_eq!(p1["imported"].as_array().unwrap().len(), 0);
    let p2 = s.execute("tether.poll", &json!({})).unwrap();
    assert_eq!(p2["imported"].as_array().unwrap().len(), 1);

    let status = s.execute("tether.status", &json!({})).unwrap();
    assert_eq!(status["importedCount"], 1);
    let imported_id = PhotoId(p2["imported"][0].as_u64().unwrap());
    assert_eq!(s.active(), Some(imported_id));
    assert_eq!(s.catalog.photo(imported_id).unwrap().file_name, "SHOOT_0001.png");
    assert!(s.catalog.album(album_id).unwrap().photos.contains(&imported_id));

    s.execute("tether.stop", &json!({})).unwrap();
    assert_eq!(s.execute("tether.status", &json!({})).unwrap()["active"], false);

    // Video trim metadata & undo/redo:
    let mut vid = Photo::new(PhotoId(999), Source::Demo { scene: 0 }, "clip.mp4", "MP4", 1920, 1080, "2026-11-14T11:00:00");
    vid.kind = MediaKind::Video;
    vid.duration = Some(24.5);
    s.commit("Add Video", Op::AddPhoto { photo: Box::new(vid) }).unwrap();

    let trimmed = s.execute("video.trim", &json!({"id": 999, "inSec": 2.5, "outSec": 18.0, "posterSec": 5.0})).unwrap();
    assert_eq!(trimmed["inSec"], 2.5);
    assert_eq!(trimmed["outSec"], 18.0);
    assert_eq!(trimmed["posterSec"], 5.0);
    assert_eq!(trimmed["trimmedDuration"], 15.5);

    let info = s.execute("video.trimInfo", &json!({"id": 999})).unwrap();
    assert_eq!(info["inSec"], 2.5);
    assert_eq!(info["outSec"], 18.0);

    let _ = std::fs::remove_dir_all(&dir);
}
