//! Comprehensive unit tests for Adobe XD (`.xd`) ZIP+AGC import and Video Fill format round-trips.

use std::io::{Cursor, Write};

use figcraft_doc::color::{GradientKind, ImageScaleMode, Paint, PaintKind};
use figcraft_doc::proto::{Action, Overflow, Transition, Trigger};
use figcraft_doc::{EffectKind, Node, NodeKind, TextAutoResize, TextDecoration};
use serde_json::json;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use super::{import, is_xd, try_import};

fn tiny_png_bytes() -> Vec<u8> {
    let img = image::RgbaImage::from_fn(4, 4, |x, y| image::Rgba([(x * 60) as u8, (y * 60) as u8, 180, 255]));
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
    out
}

fn minimal_mp4_bytes(width: u16, height: u16, duration_secs: u32) -> Vec<u8> {
    let mut ftyp = Vec::new();
    ftyp.extend_from_slice(&20u32.to_be_bytes());
    ftyp.extend_from_slice(b"ftyp");
    ftyp.extend_from_slice(b"isom");
    ftyp.extend_from_slice(&0u32.to_be_bytes());
    ftyp.extend_from_slice(b"isom");

    // mvhd box (version 0): 8 hdr + 20 body = 28 bytes
    let mut mvhd = Vec::new();
    mvhd.extend_from_slice(&28u32.to_be_bytes());
    mvhd.extend_from_slice(b"mvhd");
    mvhd.extend_from_slice(&[0, 0, 0, 0]); // ver 0 + flags
    mvhd.extend_from_slice(&0u32.to_be_bytes()); // creation
    mvhd.extend_from_slice(&0u32.to_be_bytes()); // modification
    mvhd.extend_from_slice(&1000u32.to_be_bytes()); // timescale = 1000
    mvhd.extend_from_slice(&(duration_secs * 1000).to_be_bytes()); // duration

    // tkhd box (version 0): 8 hdr + 84 body = 92 bytes
    let mut tkhd_body = vec![0u8; 84];
    let w_fixed = (u32::from(width)) << 16;
    let h_fixed = (u32::from(height)) << 16;
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

fn build_synthetic_xd_archive() -> Vec<u8> {
    let png = tiny_png_bytes();
    let mp4 = minimal_mp4_bytes(1280, 720, 12);

    let manifest = json!({
        "type": "application/vnd.adobe.sparkler.project+dcxucf",
        "name": "ShopFlow XD",
        "uxdesign#homeArtboard": "ab-home",
        "children": [
            {
                "name": "artwork",
                "path": "artwork",
                "children": [
                    {
                        "id": "ab-home",
                        "name": "Home",
                        "path": "artboard-ab-home",
                        "uxdesign#isHomeArtboard": true,
                        "uxdesign#bounds": { "x": 0.0, "y": 0.0, "width": 390.0, "height": 1200.0 },
                        "uxdesign#viewport": { "height": 844.0 },
                        "components": [
                            { "path": "graphics/graphicContent.agc", "type": "application/vnd.adobe.agc+json" }
                        ]
                    },
                    {
                        "id": "ab-detail",
                        "name": "Detail",
                        "path": "artboard-ab-detail",
                        "uxdesign#bounds": { "x": 500.0, "y": 0.0, "width": 390.0, "height": 844.0 },
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
                    { "path": "hero-img-uid", "type": "image/png" },
                    { "path": "promo-vid-uid", "type": "video/mp4" }
                ]
            }
        ]
    });

    let resources_agc = json!({
        "version": "1.0",
        "resources": {
            "gradients": {
                "grad-brand": {
                    "type": "linear",
                    "stops": [
                        { "offset": 0.0, "color": { "mode": "RGB", "value": { "r": 255, "g": 64, "b": 129 }, "alpha": 1.0 } },
                        { "offset": 1.0, "color": { "mode": "RGB", "value": { "r": 124, "g": 77, "b": 255 }, "alpha": 1.0 } }
                    ]
                }
            },
            "meta": {
                "ux": {
                    "symbols": [
                        {
                            "id": "sym-cta",
                            "type": "group",
                            "name": "Primary CTA",
                            "group": {
                                "children": [
                                    {
                                        "id": "sym-cta-bg",
                                        "type": "shape",
                                        "name": "Button BG",
                                        "shape": { "type": "rect", "x": 0.0, "y": 0.0, "width": 180.0, "height": 48.0, "r": [12.0, 12.0, 12.0, 12.0] },
                                        "style": {
                                            "fill": {
                                                "type": "gradient",
                                                "gradientRef": "grad-brand",
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

    let home_agc = json!({
        "version": "1.0",
        "children": [
            {
                "id": "ab-home",
                "type": "artboard",
                "name": "Home",
                "style": {
                    "fill": { "type": "solid", "color": { "mode": "RGB", "value": { "r": 248, "g": 249, "b": 252 }, "alpha": 1.0 } }
                },
                "artboard": {
                    "children": [
                        {
                            "id": "card-rect",
                            "type": "shape",
                            "name": "Hero Card",
                            "transform": { "a": 1.0, "b": 0.0, "c": 0.0, "d": 1.0, "tx": 20.0, "ty": 40.0 },
                            "shape": { "type": "rect", "x": 0.0, "y": 0.0, "width": 350.0, "height": 200.0, "r": [16.0, 16.0, 8.0, 8.0] },
                            "style": {
                                "fill": {
                                    "type": "pattern",
                                    "pattern": { "meta": { "ux": { "uid": "hero-img-uid", "scaleBehavior": "fill" } } }
                                },
                                "stroke": {
                                    "type": "solid",
                                    "width": 2.0,
                                    "align": "inside",
                                    "color": { "mode": "RGB", "value": { "r": 30, "g": 40, "b": 60 }, "alpha": 0.5 }
                                },
                                "filters": [
                                    {
                                        "type": "uxdesign#dropShadow",
                                        "visible": true,
                                        "params": {
                                            "dropShadows": [
                                                { "dx": 0.0, "dy": 8.0, "r": 16.0, "color": { "mode": "RGB", "value": { "r": 0, "g": 0, "b": 0 }, "alpha": 0.2 } }
                                            ]
                                        }
                                    }
                                ]
                            }
                        },
                        {
                            "id": "promo-video-rect",
                            "type": "shape",
                            "name": "Promo Video",
                            "transform": { "a": 1.0, "b": 0.0, "c": 0.0, "d": 1.0, "tx": 20.0, "ty": 260.0 },
                            "shape": { "type": "rect", "x": 0.0, "y": 0.0, "width": 350.0, "height": 196.0, "r": 12.0 },
                            "style": {
                                "fill": {
                                    "type": "pattern",
                                    "pattern": { "meta": { "ux": { "uid": "promo-vid-uid", "scaleBehavior": "fill", "autoplay": true, "loop": true, "muted": true } } }
                                }
                            }
                        },
                        {
                            "id": "check-icon",
                            "type": "shape",
                            "name": "Check Vector",
                            "transform": { "a": 1.0, "b": 0.0, "c": 0.0, "d": 1.0, "tx": 24.0, "ty": 480.0 },
                            "shape": { "type": "path", "path": "M 0 12 L 8 20 L 24 4", "winding": "nonzero" },
                            "style": {
                                "stroke": { "type": "solid", "width": 3.0, "cap": "round", "join": "round", "color": "#10B981" }
                            }
                        },
                        {
                            "id": "headline-text",
                            "type": "text",
                            "name": "Headline",
                            "transform": { "a": 1.0, "b": 0.0, "c": 0.0, "d": 1.0, "tx": 20.0, "ty": 530.0 },
                            "text": {
                                "frame": { "type": "area", "width": 350.0, "height": 64.0 },
                                "rawText": "Hello ShopFlow",
                                "styleRanges": [
                                    { "length": 6, "style": { "font": { "family": "Inter", "style": "Regular", "size": 24.0 } } },
                                    { "length": 8, "style": { "font": { "family": "Inter", "style": "Bold", "size": 24.0 }, "textAttributes": { "underline": true } } }
                                ]
                            },
                            "style": {
                                "font": { "family": "Inter", "style": "Regular", "size": 24.0 },
                                "fill": { "type": "solid", "color": "#111827" }
                            }
                        },
                        {
                            "id": "cta-instance",
                            "type": "syncRef",
                            "name": "Buy Now Button",
                            "syncRef": "sym-cta",
                            "transform": { "a": 1.0, "b": 0.0, "c": 0.0, "d": 1.0, "tx": 20.0, "ty": 620.0 },
                            "meta": {
                                "ux": {
                                    "interactions": [
                                        {
                                            "trigger": "tap",
                                            "action": "transition",
                                            "properties": {
                                                "destination": "ab-detail",
                                                "transition": "push",
                                                "slideDirection": "left",
                                                "easing": "ease-out",
                                                "duration": 0.35
                                            }
                                        },
                                        {
                                            "trigger": "voice",
                                            "action": "speech"
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

    let detail_agc = json!({
        "version": "1.0",
        "children": [
            {
                "id": "ab-detail",
                "type": "artboard",
                "name": "Detail",
                "artboard": {
                    "children": [
                        {
                            "id": "back-btn",
                            "type": "shape",
                            "name": "Back Circle",
                            "shape": { "type": "circle", "cx": 40.0, "cy": 40.0, "r": 20.0 },
                            "style": {
                                "fill": { "type": "solid", "color": "#E5E7EB" },
                                "filters": [
                                    { "type": "uxdesign#backgroundBlur", "visible": true, "params": { "blurAmount": 12.0 } }
                                ]
                            },
                            "meta": {
                                "ux": {
                                    "interactions": [
                                        { "trigger": "tap", "action": "previousArtboard" }
                                    ]
                                }
                            }
                        }
                    ]
                }
            }
        ]
    });

    let mut buf = Cursor::new(Vec::new());
    {
        let mut z = ZipWriter::new(&mut buf);
        let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let deflated = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        z.start_file("mimetype", stored).unwrap();
        z.write_all(b"application/vnd.adobe.sparkler.project+dcxucf").unwrap();

        z.start_file("manifest", deflated).unwrap();
        z.write_all(&serde_json::to_vec(&manifest).unwrap()).unwrap();

        z.start_file("resources/graphics/graphicContent.agc", deflated).unwrap();
        z.write_all(&serde_json::to_vec(&resources_agc).unwrap()).unwrap();

        z.start_file("resources/hero-img-uid", stored).unwrap();
        z.write_all(&png).unwrap();

        z.start_file("resources/promo-vid-uid", stored).unwrap();
        z.write_all(&mp4).unwrap();

        z.start_file("artwork/artboard-ab-home/graphics/graphicContent.agc", deflated).unwrap();
        z.write_all(&serde_json::to_vec(&home_agc).unwrap()).unwrap();

        z.start_file("artwork/artboard-ab-detail/graphics/graphicContent.agc", deflated).unwrap();
        z.write_all(&serde_json::to_vec(&detail_agc).unwrap()).unwrap();

        z.finish().unwrap();
    }
    buf.into_inner()
}

#[test]
fn synthetic_xd_archive_imports_artboards_shapes_text_assets_symbols_and_interactions() {
    let bytes = build_synthetic_xd_archive();
    assert!(is_xd(&bytes));

    let (doc, warnings) = try_import(&bytes, "fallback.xd").expect("detected as .xd").expect("imports cleanly");
    assert_eq!(doc.name, "ShopFlow XD");
    assert_eq!(doc.pages.len(), 1);
    assert!(warnings.iter().any(|w| w.contains("voice")), "expected voice trigger warning, got {warnings:?}");

    let page_id = doc.pages[0];
    let page = doc.node(page_id).unwrap();

    // Master symbol + 2 artboards on the page
    let home = page
        .children
        .iter()
        .filter_map(|&id| doc.node(id))
        .find(|n| n.name == "Home" && matches!(n.kind, NodeKind::Frame))
        .expect("Home artboard frame");
    assert_eq!((home.width, home.height), (390.0, 1200.0));
    assert_eq!(home.overflow, Overflow::Vertical, "1200px artboard with 844px viewport scrolls vertically");

    let detail = page
        .children
        .iter()
        .filter_map(|&id| doc.node(id))
        .find(|n| n.name == "Detail" && matches!(n.kind, NodeKind::Frame))
        .expect("Detail artboard frame");
    assert_eq!(detail.transform.translation().x, 500.0);

    // Flow starting point points at Home artboard
    if let NodeKind::Page { flows, .. } = &page.kind {
        assert_eq!(flows.len(), 1);
        assert_eq!(flows[0].start, home.id);
    } else {
        panic!("expected Page kind");
    }

    // Hero Card: asymmetric corner radii + PNG image fill + drop shadow
    let card = home
        .children
        .iter()
        .filter_map(|&id| doc.node(id))
        .find(|n| n.name == "Hero Card")
        .expect("Hero Card");
    assert_eq!(card.corners.radii, [16.0, 16.0, 8.0, 8.0]);
    assert!(matches!(card.fills.first().map(|p| &p.kind), Some(PaintKind::Image(_))));
    assert!(matches!(card.effects.first().map(|e| &e.kind), Some(EffectKind::DropShadow(_))));

    // Promo Video: MP4 video fill with probed 1280×720, 12s duration, muted = true, poster generated
    let promo = home
        .children
        .iter()
        .filter_map(|&id| doc.node(id))
        .find(|n| n.name == "Promo Video")
        .expect("Promo Video");
    let Some(PaintKind::Video(vp)) = promo.fills.first().map(|p| &p.kind) else {
        panic!("expected Video fill on Promo Video, got {:?}", promo.fills);
    };
    assert!(vp.autoplay && vp.loop_playback && vp.muted);
    let vd = doc.videos.get(&vp.video).expect("video data registered in doc.videos");
    assert_eq!((vd.width, vd.height), (1280, 720));
    assert!((vd.duration_secs - 12.0).abs() < 1e-6);
    let poster_key = vp.poster.as_ref().expect("poster key");
    assert!(doc.images.contains_key(poster_key));

    // Check Vector: SVG path converted to editable VectorNetwork
    let vec_node = home
        .children
        .iter()
        .filter_map(|&id| doc.node(id))
        .find(|n| n.name == "Check Vector")
        .expect("Check Vector");
    let NodeKind::Vector { network } = &vec_node.kind else {
        panic!("expected Vector node");
    };
    assert_eq!(network.segments.len(), 2);

    // Headline: area text with Bold + Underline second style run
    let text_node = home
        .children
        .iter()
        .filter_map(|&id| doc.node(id))
        .find(|n| n.name == "Headline")
        .expect("Headline");
    let NodeKind::Text(td) = &text_node.kind else {
        panic!("expected Text node");
    };
    assert_eq!(td.characters, "Hello ShopFlow");
    assert_eq!(td.auto_resize, TextAutoResize::None);
    assert_eq!(td.runs.len(), 1);
    assert_eq!((td.runs[0].start, td.runs[0].end), (6, 14));
    assert_eq!(td.runs[0].style.font_style, "Bold");
    assert_eq!(td.runs[0].style.decoration, TextDecoration::Underline);

    // Symbol syncRef -> Instance linked to Primary CTA Component (which has linear gradient fill)
    let cta_inst = home
        .children
        .iter()
        .filter_map(|&id| doc.node(id))
        .find(|n| n.name == "Buy Now Button")
        .expect("Buy Now Button instance");
    let NodeKind::Instance { component, .. } = cta_inst.kind else {
        panic!("expected Instance kind");
    };
    let comp = doc.node(component).expect("master component");
    assert_eq!(comp.name, "Primary CTA");
    let comp_bg = doc.node(comp.children[0]).unwrap();
    assert!(matches!(comp_bg.fills.first().map(|p| &p.kind), Some(PaintKind::Gradient(g)) if g.kind == GradientKind::Linear));

    // Tap interaction on Buy Now Button -> Navigate to Detail artboard with Push Left 350ms
    assert_eq!(cta_inst.interactions.len(), 1);
    let inter = &cta_inst.interactions[0];
    assert_eq!(inter.trigger, Trigger::OnClick);
    assert_eq!(inter.duration_ms, 350);
    assert!(matches!(inter.transition, Transition::Push { .. }));
    assert_eq!(inter.actions, vec![Action::Navigate { dest: detail.id, preserve_scroll: false }]);

    // Back interaction on Detail's Back Circle
    let back_circle = doc.node(detail.children[0]).unwrap();
    assert_eq!(back_circle.interactions.len(), 1);
    assert_eq!(back_circle.interactions[0].actions, vec![Action::Back]);
}

#[test]
fn video_fill_round_trips_through_figcraft_and_fig_archives() {
    let mut doc = figcraft_doc::Document::new("Video Deck");
    let page = doc.pages[0];
    let mp4 = minimal_mp4_bytes(1920, 1080, 8);
    let (vhash, phash, w, h, dur) = crate::add_video(&mut doc, mp4.clone(), None).unwrap();
    assert_eq!((w, h), (1920, 1080));
    assert!((dur - 8.0).abs() < 1e-6);

    let nid = doc.alloc_id();
    let mut rect = Node::new(nid, NodeKind::Rectangle, "Hero Video", 640.0, 360.0);
    let mut paint = Paint::video(vhash.clone(), Some(phash.clone()), ImageScaleMode::Fit);
    if let PaintKind::Video(vp) = &mut paint.kind {
        vp.autoplay = false;
        vp.loop_playback = true;
        vp.muted = true;
    }
    rect.fills = vec![paint];
    doc.insert(rect, page, None).unwrap();

    // 1. Native `.figcraft` archive round-trip
    let saved = crate::save_bytes(&doc, None).unwrap();
    let (loaded, issues) = crate::load_bytes(&saved).unwrap();
    assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    let vd = loaded.videos.get(&vhash).expect("video bytes restored from videos/<hash>");
    assert_eq!(vd.bytes.as_slice(), mp4.as_slice());
    assert_eq!((vd.width, vd.height), (1920, 1080));
    let loaded_rect = loaded.node(nid).unwrap();
    let PaintKind::Video(vp2) = &loaded_rect.fills[0].kind else {
        panic!("expected Video paint after .figcraft load");
    };
    assert!(!vp2.autoplay && vp2.loop_playback && vp2.muted);
    assert_eq!(vp2.scale_mode, ImageScaleMode::Fit);

    // 2. `.fig` export + import round-trip
    let (fig_bytes, fig_warns) = crate::fig::export(&doc, None).unwrap();
    assert!(fig_warns.is_empty(), "unexpected .fig export warnings: {fig_warns:?}");
    let (fig_doc, import_warns) = crate::fig::import(&fig_bytes, "Video Deck").unwrap();
    assert!(import_warns.is_empty(), "unexpected .fig import warnings: {import_warns:?}");
    let fig_page = fig_doc.node(fig_doc.pages[0]).unwrap();
    let fig_rect = fig_doc.node(fig_page.children[0]).unwrap();
    let PaintKind::Video(fvp) = &fig_rect.fills[0].kind else {
        panic!("expected Video paint after .fig round-trip");
    };
    assert!(!fvp.autoplay && fvp.loop_playback && fvp.muted);
    assert_eq!(fvp.scale_mode, ImageScaleMode::Fit);
    assert!(fig_doc.videos.contains_key(&fvp.video));
}

#[test]
fn non_xd_inputs_are_cleanly_rejected() {
    assert!(!is_xd(b"not a zip"));
    assert!(try_import(b"not a zip", "bad.xd").is_none());
    assert!(import(b"not a zip", "bad.xd").is_err());
}
