//! Integration tests for Adobe XD (`.xd`) open/import commands and Video Fill (`video.place`,
//! `video.upload`, clipboard copy/paste, and `.figcraft` persistence).

use std::io::{Cursor, Write};

use base64::Engine;
use figcraft_color::{GradientKind, ImageScaleMode, PaintKind};
use figcraft_doc::proto::{Action, Trigger};
use figcraft_doc::{NodeKind, TextDecoration};
use serde_json::json;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::Session;
use crate::params::id_of;

fn tiny_png(rgb: [u8; 3]) -> Vec<u8> {
    let img = image::RgbaImage::from_fn(8, 6, |_, _| image::Rgba([rgb[0], rgb[1], rgb[2], 255]));
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
    out
}

fn minimal_mp4(width: u16, height: u16, duration_secs: u32) -> Vec<u8> {
    let mut ftyp = Vec::new();
    ftyp.extend_from_slice(&20u32.to_be_bytes());
    ftyp.extend_from_slice(b"ftyp");
    ftyp.extend_from_slice(b"isom");
    ftyp.extend_from_slice(&0u32.to_be_bytes());
    ftyp.extend_from_slice(b"isom");

    let mut mvhd = Vec::new();
    mvhd.extend_from_slice(&28u32.to_be_bytes());
    mvhd.extend_from_slice(b"mvhd");
    mvhd.extend_from_slice(&[0, 0, 0, 0]);
    mvhd.extend_from_slice(&0u32.to_be_bytes());
    mvhd.extend_from_slice(&0u32.to_be_bytes());
    mvhd.extend_from_slice(&1000u32.to_be_bytes());
    mvhd.extend_from_slice(&(duration_secs * 1000).to_be_bytes());

    let mut tkhd_body = vec![0u8; 84];
    let w_fixed = u32::from(width) << 16;
    let h_fixed = u32::from(height) << 16;
    tkhd_body[76..80].copy_from_slice(&w_fixed.to_be_bytes());
    tkhd_body[80..84].copy_from_slice(&h_fixed.to_be_bytes());
    let mut tkhd = Vec::new();
    tkhd.extend_from_slice(&92u32.to_be_bytes());
    tkhd.extend_from_slice(b"tkhd");
    tkhd.extend_from_slice(&tkhd_body);

    let trak_len = 8 + tkhd.len() as u32;
    let mut trak = Vec::new();
    trak.extend_from_slice(&trak_len.to_be_bytes());
    trak.extend_from_slice(b"trak");
    trak.extend_from_slice(&tkhd);

    let moov_len = 8 + mvhd.len() as u32 + trak.len() as u32;
    let mut moov = Vec::new();
    moov.extend_from_slice(&moov_len.to_be_bytes());
    moov.extend_from_slice(b"moov");
    moov.extend_from_slice(&mvhd);
    moov.extend_from_slice(&trak);

    let mut out = ftyp;
    out.extend_from_slice(&moov);
    out
}

fn synthetic_xd_bytes() -> Vec<u8> {
    let png = tiny_png([32, 160, 240]);
    let mp4 = minimal_mp4(640, 360, 6);

    let manifest = json!({
        "type": "application/vnd.adobe.sparkler.project+dcxucf",
        "name": "Engine XD Suite",
        "uxdesign#homeArtboard": "ab-start",
        "children": [
            {
                "name": "artwork",
                "path": "artwork",
                "children": [
                    {
                        "id": "ab-start",
                        "name": "Landing",
                        "path": "artboard-ab-start",
                        "uxdesign#isHomeArtboard": true,
                        "uxdesign#bounds": { "x": 0.0, "y": 0.0, "width": 400.0, "height": 900.0 },
                        "uxdesign#viewport": { "height": 800.0 },
                        "components": [
                            { "path": "graphics/graphicContent.agc", "type": "application/vnd.adobe.agc+json" }
                        ]
                    },
                    {
                        "id": "ab-next",
                        "name": "Checkout",
                        "path": "artboard-ab-next",
                        "uxdesign#bounds": { "x": 480.0, "y": 0.0, "width": 400.0, "height": 800.0 },
                        "components": [
                            { "path": "graphics/graphicContent.agc", "type": "application/vnd.adobe.agc+json" }
                        ]
                    }
                ]
            },
            {
                "name": "resources",
                "path": "resources",
                "components": [
                    { "path": "graphics/graphicContent.agc", "type": "application/vnd.adobe.agc+json" },
                    { "path": "banner-png", "type": "image/png" },
                    { "path": "hero-mp4", "type": "video/mp4" }
                ]
            }
        ]
    });

    let resources_agc = json!({
        "version": "1.0",
        "resources": {
            "gradients": {
                "grad-accent": {
                    "type": "linear",
                    "stops": [
                        { "offset": 0.0, "color": { "mode": "RGB", "value": { "r": 16, "g": 185, "b": 129 }, "alpha": 1.0 } },
                        { "offset": 1.0, "color": { "mode": "RGB", "value": { "r": 59, "g": 130, "b": 246 }, "alpha": 1.0 } }
                    ]
                }
            },
            "meta": {
                "ux": {
                    "symbols": [
                        {
                            "id": "sym-pill",
                            "type": "group",
                            "name": "Action Pill",
                            "group": {
                                "children": [
                                    {
                                        "id": "sym-pill-bg",
                                        "type": "shape",
                                        "name": "Pill Shape",
                                        "shape": { "type": "rect", "x": 0.0, "y": 0.0, "width": 160.0, "height": 44.0, "r": [22.0, 22.0, 22.0, 22.0] },
                                        "style": {
                                            "fill": {
                                                "type": "gradient",
                                                "gradientRef": "grad-accent",
                                                "gradient": { "x1": 0.0, "y1": 0.5, "x2": 1.0, "y2": 0.5 }
                                            }
                                        }
                                    }
                                ]
                            }
                        }
                    ]
                }
            }
        }
    });

    let start_agc = json!({
        "version": "1.0",
        "children": [
            {
                "type": "artboard",
                "style": {
                    "fill": { "type": "solid", "color": { "mode": "RGB", "value": { "r": 250, "g": 250, "b": 252 }, "alpha": 1.0 } }
                },
                "artboard": {
                    "children": [
                        {
                            "id": "card-rect",
                            "type": "shape",
                            "name": "Feature Card",
                            "transform": { "a": 1.0, "b": 0.0, "c": 0.0, "d": 1.0, "tx": 20.0, "ty": 24.0 },
                            "shape": { "type": "rect", "x": 0.0, "y": 0.0, "width": 360.0, "height": 180.0, "r": [16.0, 4.0, 16.0, 4.0] },
                            "style": {
                                "fill": {
                                    "type": "pattern",
                                    "pattern": { "width": 8.0, "height": 6.0, "meta": { "ux": { "uid": "banner-png", "scaleBehavior": "cover" } } }
                                }
                            }
                        },
                        {
                            "id": "video-slot",
                            "type": "shape",
                            "name": "Promo Video",
                            "transform": { "a": 1.0, "b": 0.0, "c": 0.0, "d": 1.0, "tx": 20.0, "ty": 220.0 },
                            "shape": { "type": "rect", "x": 0.0, "y": 0.0, "width": 360.0, "height": 202.0 },
                            "style": {
                                "fill": {
                                    "type": "pattern",
                                    "pattern": { "width": 640.0, "height": 360.0, "meta": { "ux": { "uid": "hero-mp4", "scaleBehavior": "cover", "loop": true, "autoplay": true } } }
                                }
                            }
                        },
                        {
                            "id": "badge-path",
                            "type": "shape",
                            "name": "Check Icon",
                            "transform": { "a": 1.0, "b": 0.0, "c": 0.0, "d": 1.0, "tx": 24.0, "ty": 440.0 },
                            "shape": { "type": "path", "path": "M 0 12 L 8 20 L 24 4 Z" },
                            "style": {
                                "fill": { "type": "solid", "color": { "mode": "RGB", "value": { "r": 34, "g": 197, "b": 94 }, "alpha": 1.0 } }
                            }
                        },
                        {
                            "id": "title-text",
                            "type": "text",
                            "name": "Heading",
                            "transform": { "a": 1.0, "b": 0.0, "c": 0.0, "d": 1.0, "tx": 20.0, "ty": 480.0 },
                            "text": {
                                "rawText": "Hello XD",
                                "frame": { "type": "area", "width": 300.0, "height": 40.0 },
                                "paragraphs": [
                                    {
                                        "lines": [[{ "from": 0, "to": 8 }]],
                                        "styleRanges": [
                                            { "length": 5, "fontFamily": "Inter", "fontStyle": "Bold", "fontSize": 24.0, "underline": true },
                                            { "length": 3, "fontFamily": "Inter", "fontStyle": "Regular", "fontSize": 24.0 }
                                        ]
                                    }
                                ]
                            },
                            "style": {
                                "font": { "family": "Inter", "style": "Regular", "size": 24.0 },
                                "fill": { "type": "solid", "color": { "mode": "RGB", "value": { "r": 17, "g": 24, "b": 39 }, "alpha": 1.0 } }
                            }
                        },
                        {
                            "id": "cta-inst",
                            "type": "syncRef",
                            "name": "Buy Button",
                            "syncSourceGuid": "sym-pill",
                            "transform": { "a": 1.0, "b": 0.0, "c": 0.0, "d": 1.0, "tx": 20.0, "ty": 540.0 },
                            "meta": {
                                "ux": {
                                    "interactions": [
                                        {
                                            "trigger": { "type": "tap" },
                                            "action": {
                                                "type": "goToArtboard",
                                                "destination": "ab-next",
                                                "transition": { "type": "slide", "direction": "left", "duration": 0.3, "easing": "ease-out" }
                                            }
                                        }
                                    ]
                                }
                            }
                        }
                    ]
                }
            }
        ]
    });

    let next_agc = json!({
        "version": "1.0",
        "children": [
            {
                "type": "artboard",
                "style": {
                    "fill": { "type": "solid", "color": { "mode": "RGB", "value": { "r": 255, "g": 255, "b": 255 }, "alpha": 1.0 } }
                },
                "artboard": { "children": [] }
            }
        ]
    });

    let mut buf = Cursor::new(Vec::new());
    {
        let mut zw = ZipWriter::new(&mut buf);
        let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let deflated = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        zw.start_file("mimetype", stored).unwrap();
        zw.write_all(b"application/vnd.adobe.sparkler.project+dcxucf").unwrap();

        zw.start_file("manifest", deflated).unwrap();
        zw.write_all(manifest.to_string().as_bytes()).unwrap();

        zw.start_file("resources/graphics/graphicContent.agc", deflated).unwrap();
        zw.write_all(resources_agc.to_string().as_bytes()).unwrap();

        zw.start_file("resources/banner-png", stored).unwrap();
        zw.write_all(&png).unwrap();

        zw.start_file("resources/hero-mp4", stored).unwrap();
        zw.write_all(&mp4).unwrap();

        zw.start_file("artwork/artboard-ab-start/graphics/graphicContent.agc", deflated).unwrap();
        zw.write_all(start_agc.to_string().as_bytes()).unwrap();

        zw.start_file("artwork/artboard-ab-next/graphics/graphicContent.agc", deflated).unwrap();
        zw.write_all(next_agc.to_string().as_bytes()).unwrap();

        zw.finish().unwrap();
    }
    buf.into_inner()
}

#[test]
fn xd_open_import_and_import_xd_commands_preserve_nodes_interactions_and_keep_journal_compact() {
    let xd = synthetic_xd_bytes();
    let b64 = base64::engine::general_purpose::STANDARD.encode(&xd);

    // 1. Open `.xd` via `file.open` (base64 data).
    let mut s = Session::new();
    let open_res = s.execute("file.open", &json!({"data": b64, "name": "shop.xd"})).unwrap();
    assert!(open_res["summary"]["pages"].as_u64().unwrap_or(0) >= 1);

    let doc = s.doc().unwrap();
    let landing = doc.nodes().find(|n| n.name == "Landing").expect("Landing artboard frame");
    let checkout = doc.nodes().find(|n| n.name == "Checkout").expect("Checkout artboard frame");
    assert!(matches!(landing.kind, NodeKind::Frame));
    assert_eq!((landing.width, landing.height), (400.0, 900.0), "full artboard height preserved on frame");
    assert_eq!(landing.overflow, figcraft_doc::proto::Overflow::Vertical, "smaller viewport height enables vertical scrolling");

    let card = doc.nodes().find(|n| n.name == "Feature Card").expect("Feature Card");
    assert_eq!(card.corners.radii, [16.0, 4.0, 16.0, 4.0]);
    assert!(matches!(card.fills.first().map(|f| &f.kind), Some(PaintKind::Image(_))));

    let vid_layer = doc.nodes().find(|n| n.name == "Promo Video").expect("Promo Video");
    assert!(matches!(vid_layer.fills.first().map(|f| &f.kind), Some(PaintKind::Video(_))));
    assert_eq!(doc.videos.len(), 1);

    let vec_node = doc.nodes().find(|n| n.name == "Check Icon").expect("Check Icon");
    assert!(matches!(&vec_node.kind, NodeKind::Vector { network: net } if net.vertices.len() >= 3));

    let txt = doc.nodes().find(|n| n.name == "Heading").expect("Heading");
    if let NodeKind::Text(td) = &txt.kind {
        assert_eq!(td.characters, "Hello XD");
        assert!(td.runs.iter().any(|r| r.style.decoration == TextDecoration::Underline));
    } else {
        panic!("expected Text node");
    }

    let comp = doc.nodes().find(|n| matches!(n.kind, NodeKind::Component { .. }) && n.name == "Action Pill");
    assert!(comp.is_some(), "symbol converted to reusable Component");
    let pill_shape = doc.nodes().find(|n| n.name == "Pill Shape").expect("Pill Shape inside component");
    assert!(matches!(pill_shape.fills.first().map(|f| &f.kind), Some(PaintKind::Gradient(g)) if g.kind == GradientKind::Linear));

    let inst = doc.nodes().find(|n| n.name == "Buy Button").expect("Buy Button syncRef instance");
    assert!(matches!(inst.kind, NodeKind::Instance { .. }));
    let checkout_id = checkout.id;
    assert!(inst.interactions.iter().any(|it| {
        matches!(it.trigger, Trigger::OnClick)
            && it.actions.iter().any(|a| matches!(a, Action::Navigate { dest, .. } if *dest == checkout_id))
    }));

    // 2. Import `.xd` via `file.import` (page append) and `file.importXd` (coordinate placement) + undo/redo.
    let mut s2 = Session::new();
    let pages_before = s2.doc().unwrap().pages.len();
    let imp_res = s2.execute("file.import", &json!({"data": b64, "name": "shop.xd"})).unwrap();
    assert!(imp_res["summary"]["layers"].as_u64().unwrap_or(0) >= 5);
    assert!(s2.doc().unwrap().pages.len() > pages_before);

    let placed_res = s2.execute("file.importXd", &json!({"data": b64, "name": "shop.xd", "x": 120.0, "y": 80.0})).unwrap();
    let placed_ids: Vec<_> = placed_res["ids"].as_array().unwrap().iter().filter_map(id_of).collect();
    assert!(!placed_ids.is_empty());
    let first_placed = s2.doc().unwrap().node(placed_ids[0]).unwrap();
    let t = first_placed.transform.translation();
    assert!((t.x - 120.0).abs() < 1e-6 && (t.y - 80.0).abs() < 1e-6);

    s2.execute("edit.undo", &json!({})).unwrap();
    assert!(s2.doc().unwrap().node(placed_ids[0]).is_none(), "undo removes placed XD artboards");
    s2.execute("edit.redo", &json!({})).unwrap();
    assert!(s2.doc().unwrap().node(placed_ids[0]).is_some(), "redo restores placed XD artboards");

    assert!(
        !s2.journal.iter().any(|(_, params)| params.get("data").is_some()),
        "journal must strip raw base64 archive bytes"
    );
}

#[test]
fn video_place_upload_clipboard_and_figcraft_roundtrip() {
    let mp4 = minimal_mp4(1920, 1080, 9);
    let mp4_b64 = base64::engine::general_purpose::STANDARD.encode(&mp4);
    let custom_poster = tiny_png([240, 48, 80]);
    let poster_b64 = base64::engine::general_purpose::STANDARD.encode(&custom_poster);

    let mut s = Session::new();
    let placed = s
        .execute(
            "video.place",
            &json!({
                "data": mp4_b64,
                "name": "Intro Clip",
                "x": 40.0,
                "y": 60.0,
                "autoplay": true,
                "loopPlayback": true,
                "muted": true
            }),
        )
        .unwrap();
    let vid_node = id_of(&placed["id"]).unwrap();
    assert!((placed["durationSecs"].as_f64().unwrap() - 9.0).abs() < 1e-6);

    // Upload a video with custom poster onto a second shape.
    let rect_res = s.execute("shape.rectangle", &json!({"x": 300, "y": 60, "width": 240, "height": 135})).unwrap();
    let rect_id = id_of(&rect_res["id"]).unwrap();
    let up = s
        .execute(
            "video.upload",
            &json!({
                "ids": [rect_id],
                "data": mp4_b64,
                "posterData": poster_b64,
                "mode": "fit",
                "autoplay": false,
                "loopPlayback": false,
                "muted": false
            }),
        )
        .unwrap();
    assert_eq!(up["changed"], 1);
    let uploaded_fill = &s.doc().unwrap().node(rect_id).unwrap().fills[0];
    if let PaintKind::Video(vp) = &uploaded_fill.kind {
        assert_eq!(vp.scale_mode, ImageScaleMode::Fit);
        assert!(!vp.autoplay && !vp.loop_playback && !vp.muted);
        assert!(vp.poster.as_ref().is_some_and(|h| s.doc().unwrap().images.contains_key(h)));
    } else {
        panic!("expected PaintKind::Video after video.upload");
    }

    // Copy/paste into a brand-new tab preserves both `videos` and poster `images`.
    s.execute("select.set", &json!({"ids": [vid_node, rect_id]})).unwrap();
    s.execute("edit.copy", &json!({})).unwrap();
    s.execute("file.new", &json!({"name": "Dest"})).unwrap();
    let pasted = s.execute("edit.paste", &json!({})).unwrap();
    let pasted_ids: Vec<_> = pasted["ids"].as_array().unwrap().iter().filter_map(id_of).collect();
    assert_eq!(pasted_ids.len(), 2);
    let dest_doc = s.doc().unwrap();
    assert_eq!(dest_doc.videos.len(), 1);
    for pid in pasted_ids {
        let fill = &dest_doc.node(pid).unwrap().fills[0];
        if let PaintKind::Video(vp) = &fill.kind {
            assert!(dest_doc.videos.contains_key(&vp.video));
            assert!(vp.poster.as_ref().is_some_and(|ph| dest_doc.images.contains_key(ph)));
        } else {
            panic!("pasted node lost PaintKind::Video");
        }
    }

    // Round-trip through `.figcraft` archive bytes.
    let saved = figcraft_format::save_bytes(dest_doc, None).unwrap();
    let (reloaded, _) = figcraft_format::load_bytes(&saved).unwrap();
    assert_eq!(reloaded.videos.len(), 1);
    let v_entry = reloaded.videos.values().next().unwrap();
    assert_eq!((v_entry.width, v_entry.height), (1920, 1080));
    assert!((v_entry.duration_secs - 9.0).abs() < 1e-6);

    assert!(
        !s.journal.iter().any(|(_, params)| params.get("data").is_some() || params.get("posterData").is_some()),
        "journal must strip raw video and poster bytes"
    );
}
