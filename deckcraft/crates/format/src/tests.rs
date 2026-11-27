use super::*;

#[test]
fn chart_sidecars_are_optional_for_older_native_files() {
    use deckcraft_model::chart::{Chart, ChartType};
    let chart = Chart::sample(ChartType::Pie);
    let mut json = serde_json::to_value(&chart).expect("chart JSON");
    assert!(json.get("kept").is_none(), "empty sidecars should not enlarge ordinary files");
    assert_eq!(serde_json::from_value::<Chart>(json.clone()).expect("old chart without kept"), chart);
    json["kept"] = serde_json::json!({});
    assert_eq!(serde_json::from_value::<Chart>(json).expect("empty sidecar"), chart);
}

#[test]
fn alternate_run_language_survives_two_native_saves_and_absence() {
    use deckcraft_model::text::{Paragraph, Run, RunProps, TextBody};
    let mut p = defaults::new_presentation(None);
    Arc::make_mut(&mut p.slides[0]).shapes[0].text = Some(TextBody {
        paragraphs: vec![Paragraph {
            runs: vec![
                Run::with("bonjour", RunProps { lang: Some("en-US".into()), alt_lang: Some("fr-FR".into()), ..Default::default() }),
                Run::with(" plain", RunProps { lang: Some("en-US".into()), ..Default::default() }),
            ],
            ..Default::default()
        }],
        ..Default::default()
    });
    let mut q = load(&save(&p).expect("save")).expect("load");
    for _ in 0..2 {
        let runs = &q.slides[0].shapes[0].text.as_ref().expect("text").paragraphs[0].runs;
        assert_eq!(runs[0].props.lang.as_deref(), Some("en-US"));
        assert_eq!(runs[0].props.alt_lang.as_deref(), Some("fr-FR"));
        assert_eq!(runs[1].props.lang.as_deref(), Some("en-US"));
        assert_eq!(runs[1].props.alt_lang, None);
        q = load(&save(&q).expect("save again")).expect("load again");
    }
}

#[test]
fn native_save_keeps_chart_relationships_and_payloads() {
    use deckcraft_model::chart::{Chart, ChartKept, ChartPart, ChartRel, ChartType};
    use deckcraft_model::{Shape, ShapeId, ShapeKind};
    let kept = ChartKept {
        rels: vec![ChartRel {
            id: "rWorkbook".into(),
            rel_type: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/package".into(),
            target: "../embeddings/data.xlsx".into(),
            part: "ppt/embeddings/data.xlsx".into(),
            external: false,
        }],
        parts: vec![ChartPart {
            name: "ppt/embeddings/data.xlsx".into(),
            content_type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".into(),
            bytes: vec![0, 255, 128, 42],
            native_entry: None,
        }],
    };
    let mut p = defaults::new_presentation(None);
    let chart = Chart { kept: kept.clone(), ..Chart::sample(ChartType::Pie) };
    let s = Shape { id: ShapeId(p.alloc_id().expect("id")), kind: ShapeKind::Chart(Box::new(chart)), ..Default::default() };
    // Serialization must reach charts inside groups as well as top-level charts.
    Arc::make_mut(&mut p.slides[0]).shapes = vec![Shape {
        id: ShapeId(p.alloc_id().expect("group id")),
        kind: ShapeKind::Group { children: vec![s], child: Default::default() },
        ..Default::default()
    }];
    let mut q = load(&save(&p).expect("save")).expect("load");
    for _ in 0..2 {
        let ShapeKind::Group { children, .. } = &q.slides[0].shapes[0].kind else { panic!("group") };
        let ShapeKind::Chart(chart) = &children[0].kind else { panic!("chart") };
        assert_eq!(chart.kept, kept);
        q = load(&save(&q).expect("save again")).expect("load again");
    }
}

#[test]
fn native_table_direction_and_column_text_survive_two_saves() {
    use deckcraft_model::table::{Cell, Table};
    use deckcraft_model::{Shape, ShapeId, ShapeKind};
    use std::sync::Arc;

    for rtl in [Some(true), Some(false), None] {
        let mut p = defaults::new_presentation(None);
        let mut table = Table::new(1, 2, 200.0, 30.0);
        table.rtl = rtl;
        table.rows[0].cells[0] = Cell::new("Left column");
        table.rows[0].cells[1] = Cell::new("Right column");
        let shape = Shape { id: ShapeId(p.alloc_id().expect("id")), kind: ShapeKind::Table(table), ..Default::default() };
        Arc::make_mut(&mut p.slides[0]).shapes.push(shape);
        let mut q = load(&save(&p).expect("save")).expect("load");
        for _ in 0..2 {
            let table = q.slides[0]
                .shapes
                .iter()
                .find_map(|s| match &s.kind {
                    ShapeKind::Table(table) => Some(table),
                    _ => None,
                })
                .expect("table shape");
            assert_eq!(table.rtl, rtl);
            assert_eq!(table.rows[0].cells[0].text.paragraphs[0].runs[0].text, "Left column");
            assert_eq!(table.rows[0].cells[1].text.paragraphs[0].runs[0].text, "Right column");
            q = load(&save(&q).expect("save again")).expect("load again");
        }
    }
}

/// The same package without the entries `drop` names.
fn without(bytes: &[u8], drop: &str) -> Vec<u8> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..z.len() {
        let f = z.by_index(i).unwrap();
        if f.name() != drop {
            out.raw_copy_file(f).unwrap();
        }
    }
    out.finish().unwrap().into_inner()
}

#[test]
fn missing_or_corrupt_media_is_reported() {
    let mut p = Presentation::default();
    let id = p.add_media("a.png", "image/png", b"PICTURE-BYTES-0123456789".to_vec()).expect("media");
    let good = save(&p).unwrap();
    assert!(load_checked(&good).unwrap().1.is_empty());
    // Missing member.
    let (q, damaged) = load_checked(&without(&good, &format!("media/{}", id.0))).unwrap();
    assert_eq!(damaged.len(), 1, "{damaged:?}");
    assert!(q.media(id).is_some_and(|m| m.data.is_empty()));
    // Bad CRC: PNGs are stored, so their bytes sit in the file as they are.
    let mut bad = good.clone();
    let at = bad.windows(13).position(|w| w == b"PICTURE-BYTES").unwrap();
    bad[at] ^= 0xff;
    let (_, damaged) = load_checked(&bad).unwrap();
    assert_eq!(damaged.len(), 1, "{damaged:?}");
    // Linked media saved without its bytes isn't damage.
    let mut l = Presentation::default();
    let lid = l.add_media("v.mp4", "video/mp4", vec![]).expect("media");
    if let Some(m) = l.media.iter_mut().find(|m| m.id == lid) {
        m.link = Some("file:///movies/v.mp4".into());
    }
    assert!(load_checked(&save(&l).unwrap()).unwrap().1.is_empty());
}

#[test]
fn media_budget_is_cumulative() {
    let mut p = Presentation::default();
    p.add_media("a.bin", "application/octet-stream", vec![1; 600]);
    p.add_media("b.bin", "application/octet-stream", vec![2; 600]);
    let bytes = save(&p).unwrap();
    // Exactly at the limit opens; one byte under is refused, though each item fits.
    assert!(load_within(&bytes, Budget::media(1200, 2)).is_ok());
    assert!(load_within(&bytes, Budget::media(1199, 2)).is_err());
    assert!(load_within(&bytes, Budget::media(1200, 1)).is_err());
}

#[test]
fn corrupt_media_is_charged_for_what_it_inflated() {
    let mut p = Presentation::default();
    p.add_media("a.png", "image/png", b"PICTURE-AAAAAAAAAAAA".to_vec());
    p.add_media("b.png", "image/png", b"PICTURE-BBBBBBBBBBBB".to_vec());
    let mut bytes = save(&p).unwrap();
    // Corrupt both stored members, so each fails its CRC only after inflating all its bytes.
    for tag in [b"PICTURE-AAAA", b"PICTURE-BBBB"] {
        let at = bytes.windows(tag.len()).position(|w| w == tag).unwrap();
        bytes[at] ^= 0xff;
    }
    // Each member fits the budget on its own; the second must not reuse what the first spent.
    let result = load_within(&bytes, Budget::media(20, 2));
    assert!(result.is_err(), "two corrupt 20-byte members must not share a 20-byte budget");
    // With room for both, the file opens and reports both.
    let (_, damaged) = load_within(&bytes, Budget::media(40, 2)).unwrap();
    assert_eq!(damaged.len(), 2, "{damaged:?}");
}

#[test]
fn oversized_manifest_is_refused_before_inflating() {
    let bytes = save(&Presentation::default()).unwrap();
    let mut z = zip::ZipArchive::new(Cursor::new(bytes.as_slice())).unwrap();
    let size = z.by_name("presentation.json").unwrap().size();
    assert!(load_within(&bytes, Budget { total: 0, items: 0, manifest: size }).is_ok());
    let err = load_within(&bytes, Budget { total: 0, items: 0, manifest: size - 1 }).unwrap_err();
    assert!(err.to_string().contains("too large"), "{err}");
}

#[test]
fn many_slides_on_missing_layouts_are_repaired() {
    let mut p = Presentation::default();
    let template = p.slides[0].clone();
    let good = template.layout;
    p.slides = (0..2000u32)
        .map(|i| {
            let mut s = (*template).clone();
            s.id = deckcraft_model::SlideId(10_000 + i);
            if i % 2 == 1 {
                s.layout = deckcraft_model::LayoutId(900_000 + i);
            }
            Arc::new(s)
        })
        .collect();
    p.next_id = 20_000;
    repair(&mut p);
    assert!(p.validate().is_empty(), "{:?}", p.validate().first());
    assert!(p.slides.iter().all(|s| p.layout(s.layout).is_some()));
    assert_eq!(p.slides[0].layout, good);
}

#[test]
fn repeated_media_ids_are_read_once() {
    use std::io::Write;
    let mut p = Presentation::default();
    p.add_media("a.bin", "application/octet-stream", vec![7; 1000]);
    let good = save(&p).unwrap();
    // The same file with its media item listed 51 times.
    let mut z = zip::ZipArchive::new(Cursor::new(good.as_slice())).unwrap();
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..z.len() {
        let mut f = z.by_index(i).unwrap();
        if f.name() == "presentation.json" {
            let mut json = vec![];
            f.read_to_end(&mut json).unwrap();
            let mut v: serde_json::Value = serde_json::from_slice(&json).unwrap();
            let media = &mut v["presentation"]["media"];
            let one = media[0].clone();
            *media = serde_json::Value::Array(vec![one; 51]);
            out.start_file("presentation.json", zip::write::SimpleFileOptions::default()).unwrap();
            out.write_all(&serde_json::to_vec(&v).unwrap()).unwrap();
        } else {
            out.raw_copy_file(f).unwrap();
        }
    }
    let bytes = out.finish().unwrap().into_inner();
    // 51 references to one 1000-byte member fit a 1000-byte budget: it's inflated once.
    let q = load_within(&bytes, Budget::media(1000, 51)).unwrap().0;
    assert_eq!(q.media.len(), 1);
    assert_eq!(q.media[0].data.len(), 1000);
    // And the deck saves again.
    assert!(save(&q).is_ok());
}

#[test]
fn roundtrip_with_media() {
    let mut p = Presentation::default();
    let id = p.add_media("a.png", "image/png", vec![1, 2, 3, 4]).expect("media");
    let bytes = save(&p).unwrap();
    assert!(sniff(&bytes));
    let q = load(&bytes).unwrap();
    assert_eq!(q.slides.len(), p.slides.len());
    assert_eq!(q.media(id).map(|m| m.data.as_slice()), Some(&[1u8, 2, 3, 4][..]));
    assert_eq!(serde_json::to_value(&q.slides).unwrap(), serde_json::to_value(&p.slides).unwrap());
}

#[test]
fn garbage_is_an_error_not_a_panic() {
    assert!(load(b"").is_err());
    assert!(load(b"PK\x03\x04garbage").is_err());
    assert!(load(&[0u8; 1000]).is_err());
    let mut b = save(&Presentation::default()).unwrap();
    let n = b.len();
    b.truncate(n / 2);
    assert!(load(&b).is_err());
}

/// A video's crop is brought into the same range as a picture's (#972).
#[test]
fn a_video_crop_is_repaired_like_a_picture_crop() {
    use deckcraft_model::{MediaClip, Shape};
    let mut p = Presentation::default();
    let clip = MediaClip { video: true, crop: [f64::NAN, 0.25, -1e9, 0.0], ..Default::default() };
    Arc::make_mut(&mut p.slides[0]).shapes.push(Shape { kind: ShapeKind::Media(clip), ..Default::default() });
    let back = load(&save(&p).unwrap()).unwrap_or_else(|e| panic!("{e}"));
    let ShapeKind::Media(m) = &back.slides[0].shapes.last().unwrap().kind else { panic!("media") };
    assert_eq!(m.crop, [0.0, 0.25, -10.0, 0.0]);
}

/// Non-finite numbers are written as `null` by serde_json; the file still opens, with each
/// such number repaired (it failed as "invalid type: null, expected f64").
#[test]
fn a_file_saved_with_non_finite_numbers_reopens() {
    use deckcraft_model::{Shape, ShapeKind, Xfrm};
    let mut p = Presentation::default();
    let nan = f64::NAN;
    let inf = f64::INFINITY;
    let mut pic = Shape { xfrm: Some(Xfrm { x: nan, y: -inf, w: 1e300, h: -4.0, rot: 725.0, ..Default::default() }), ..Default::default() };
    pic.kind = ShapeKind::Picture { fill: deckcraft_model::style::PictureFill { crop: [nan, inf, 0.25, -1e9], ..Default::default() } };
    pic.line = Some(deckcraft_model::Line { width: Some(1e9), ..Default::default() });
    let mut text = Shape { xfrm: Some(Xfrm { rot: inf, w: 100.0, h: 50.0, ..Default::default() }), ..Default::default() };
    let mut body = deckcraft_model::text::TextBody::default();
    let mut para = deckcraft_model::text::Paragraph::default();
    para.runs.push(deckcraft_model::text::Run::with("x", deckcraft_model::text::RunProps { size: Some(-3.0), ..Default::default() }));
    para.end_props.size = Some(1e9);
    body.paragraphs.push(para);
    text.text = Some(body);
    let group = Shape {
        xfrm: Some(Xfrm { w: 10.0, h: 10.0, ..Default::default() }),
        kind: ShapeKind::Group { children: vec![pic, text], child: Xfrm { x: nan, w: inf, h: 10.0, ..Default::default() } },
        ..Default::default()
    };
    Arc::make_mut(&mut p.slides[0]).shapes.push(group);
    let bytes = save(&p).unwrap();
    let back = load(&bytes).unwrap_or_else(|e| panic!("{e}"));
    let g = back.slides[0].shapes.last().unwrap();
    let ShapeKind::Group { children, child } = &g.kind else { panic!("group") };
    assert_eq!((child.x, child.w), (0.0, 0.0));
    let x = children[0].xfrm.unwrap();
    assert_eq!((x.x, x.y, x.w, x.h, x.rot), (0.0, 0.0, MAX_COORD, 0.0, 5.0));
    let ShapeKind::Picture { fill } = &children[0].kind else { panic!("picture") };
    assert_eq!(fill.crop, [0.0, 0.0, 0.25, -10.0]);
    assert_eq!(children[0].line.as_ref().unwrap().width, Some(1584.0));
    assert_eq!(children[1].xfrm.unwrap().rot, 0.0);
    let para = &children[1].text.as_ref().unwrap().paragraphs[0];
    assert_eq!(para.runs[0].props.size, None);
    assert_eq!(para.end_props.size, Some(4000.0));
    // Saved again, it is clean.
    assert_eq!(load(&save(&back).unwrap()).unwrap(), back);
}

#[test]
fn reflection_transform_survives_two_native_saves_and_absent_fields_stay_absent() {
    use deckcraft_model::style::Reflection;
    use deckcraft_model::{Effects, Shape};
    let mut p = Presentation::default();
    let reflection = Reflection {
        blur: 1.0,
        start_alpha: 0.5,
        end_alpha: 0.0,
        end_pos: 0.75,
        dist: 2.0,
        st_pos: Some(20_000),
        fade_dir: Some(0),
        sx: Some(75_000),
        sy: Some(-50_000),
        kx: Some(600_000),
        align: Some("bl".into()),
        rotate_with_shape: Some(false),
        ..Default::default()
    };
    let shape = Shape { effects: Some(Effects { reflection: Some(reflection.clone()), ..Default::default() }), ..Default::default() };
    Arc::make_mut(&mut p.slides[0]).shapes.push(shape);
    let once = load(&save(&p).expect("save")).expect("load");
    let twice = load(&save(&once).expect("save again")).expect("load again");
    assert_eq!(twice.slides[0].shapes.last().and_then(|s| s.effects.as_ref()).and_then(|e| e.reflection.as_ref()), Some(&reflection));

    let absent = Reflection {
        blur: 1.0,
        start_alpha: 1.0,
        end_alpha: 0.0,
        end_pos: 1.0,
        dist: 0.0,
        st_pos: None,
        dir: None,
        fade_dir: None,
        sx: None,
        sy: None,
        kx: None,
        ky: None,
        align: None,
        rotate_with_shape: None,
    };
    let shape = Shape { effects: Some(Effects { reflection: Some(absent.clone()), ..Default::default() }), ..Default::default() };
    Arc::make_mut(&mut p.slides[0]).shapes = vec![shape];
    let reopened = load(&save(&p).expect("save absent values")).expect("load absent values");
    assert_eq!(reopened.slides[0].shapes.last().and_then(|s| s.effects.as_ref()).and_then(|e| e.reflection.as_ref()), Some(&absent));
}

/// Files written before [`finite::Finite`] (plain serde_json: NaN as `null`) open too,
/// including a `null` inside a tagged enum, where serde reports only the enclosing object.
#[test]
fn older_files_with_null_numbers_open() {
    use deckcraft_model::{Shape, ShapeKind, Xfrm};
    let nan = f64::NAN;
    for case in 0..3 {
        let mut p = Presentation::default();
        let sh = match case {
            0 => Shape { xfrm: Some(Xfrm { x: nan, rot: nan, w: 5.0, h: 5.0, ..Default::default() }), ..Default::default() },
            1 => Shape {
                kind: ShapeKind::Group { children: vec![], child: Xfrm { x: nan, w: 1.0, h: 1.0, ..Default::default() } },
                ..Default::default()
            },
            _ => Shape {
                kind: ShapeKind::Picture { fill: deckcraft_model::style::PictureFill { crop: [0.1, nan, 0.0, 0.0], ..Default::default() } },
                ..Default::default()
            },
        };
        Arc::make_mut(&mut p.slides[0]).shapes.push(sh);
        let m = Manifest { format: "deckcraft".into(), version: VERSION, generator: "old".into(), presentation: p };
        let json = serde_json::to_vec(&m).unwrap();
        assert!(json.windows(4).any(|w| w == b"null"));
        let back = parse_manifest(&json).unwrap_or_else(|e| panic!("case {case}: {e}"));
        assert_eq!(back.presentation.slides[0].shapes.len(), m.presentation.slides[0].shapes.len(), "case {case}");
    }
}

/// A body whose file had no `fontScale` (`<a:normAutofit/>`) keeps that state through the
/// native format: it is drawn at 100 % and only fitted when its text changes (#724).
#[test]
fn a_bare_normautofit_body_round_trips() {
    use deckcraft_model::text::{AutoFit, TextBody};
    let mut p = Presentation::default();
    let mut body = TextBody::from_text("Overflowing");
    body.body.autofit = Some(AutoFit::Shrink { font_scale: None, line_reduction: 0.0 });
    let sh = deckcraft_model::Shape { text: Some(body), ..Default::default() };
    Arc::make_mut(&mut p.slides[0]).shapes.push(sh);
    let back = load(&save(&p).unwrap()).unwrap_or_else(|e| panic!("{e}"));
    let body = back.slides[0].shapes.last().unwrap().text.clone().unwrap();
    assert_eq!(body.body.autofit, Some(AutoFit::Shrink { font_scale: None, line_reduction: 0.0 }));
    // A scale the file saved round-trips as itself.
    let mut body = TextBody::from_text("Overflowing");
    body.body.autofit = Some(AutoFit::Shrink { font_scale: Some(0.62), line_reduction: 0.2 });
    let sh = deckcraft_model::Shape { text: Some(body), ..Default::default() };
    Arc::make_mut(&mut p.slides[0]).shapes.push(sh);
    let back = load(&save(&p).unwrap()).unwrap_or_else(|e| panic!("{e}"));
    let body = back.slides[0].shapes.last().unwrap().text.clone().unwrap();
    assert_eq!(body.body.autofit, Some(AutoFit::Shrink { font_scale: Some(0.62), line_reduction: 0.2 }));
}

/// Only `null`s where numbers belong are patched: other damage still fails.
#[test]
fn other_damage_is_still_reported() {
    let ok = serde_json::to_vec(&serde_json::json!({"format": "deckcraft", "version": 1, "generator": "t", "presentation": {}})).unwrap();
    assert!(parse_manifest(&ok).is_ok());
    let bad = br#"{"format": null, "version": 1, "generator": "t", "presentation": {}}"#;
    assert!(parse_manifest(bad).is_err());
    let truncated = &ok[..ok.len() - 3];
    assert!(parse_manifest(truncated).is_err());
    // A null number on a later line (pretty-printed by another tool) is found too.
    let pretty = b"{\n  \"format\": \"deckcraft\",\n  \"version\": 1,\n  \"generator\": \"t\",\n  \"presentation\": {\"slides\": [{\"shapes\": [{\"xfrm\": {\"x\": null, \"y\": 1, \"w\": 2, \"h\": 3}}]}]}\n}";
    let m = parse_manifest(pretty).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(m.presentation.slides[0].shapes[0].xfrm.unwrap().x, 0.0);
}

#[test]
fn degenerate_slide_sizes_are_repaired() {
    use deckcraft_model::Size;
    let cases = [
        ((0.0, 0.0), (960.0, 540.0)),
        ((-5.0, 540.0), (960.0, 540.0)),
        ((f64::NAN, 540.0), (960.0, 540.0)),
        ((f64::INFINITY, 1.0), (960.0, 540.0)),
        ((1.0, 1.0), (72.0, 72.0)),
        ((0.5, 4000.0), (72.0, 4000.0)),
        ((1e9, 20.0), (4032.0, 72.0)),
        ((720.0, 540.0), (720.0, 540.0)),
    ];
    for ((w, h), want) in cases {
        let mut p = Presentation { slide_size: Size::new(w, h), notes_size: Size::new(0.0, f64::NAN), ..Default::default() };
        repair(&mut p);
        assert_eq!((p.slide_size.width, p.slide_size.height), want, "{w}x{h}");
        assert_eq!(p.notes_size, Presentation::default().notes_size);
    }
}

#[test]
fn repair_bounds_the_notes_page_size() {
    let mut p = Presentation { notes_size: deckcraft_model::Size::new(1e12, 0.01), ..Default::default() };
    repair(&mut p);
    assert_eq!((p.notes_size.width, p.notes_size.height), (4032.0, 72.0));
}

/// A file whose slide size is null, a string or missing a side (written by hand or by
/// another tool; NaN is also saved as null) opens with the default size rather than failing
/// as damaged.
#[test]
fn files_with_unreadable_slide_sizes_still_open() {
    let saved = save(&Presentation::default()).unwrap();
    let mut z = zip::ZipArchive::new(Cursor::new(&saved[..])).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&read_entry(&mut z, "presentation.json").unwrap()).unwrap();
    for size in [
        serde_json::json!({"width": null, "height": 540}),
        serde_json::json!({"width": "big", "height": 540}),
        serde_json::json!({"height": 0}),
        serde_json::json!(null),
        serde_json::json!(7),
    ] {
        let mut m = json.clone();
        m["presentation"]["slideSize"] = size.clone();
        let mut buf = Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let o = zip::write::SimpleFileOptions::default();
            w.start_file("mimetype", o).unwrap();
            w.write_all(MIMETYPE.as_bytes()).unwrap();
            w.start_file("presentation.json", o).unwrap();
            w.write_all(&serde_json::to_vec(&m).unwrap()).unwrap();
            w.finish().unwrap();
        }
        let p = load(buf.get_ref()).unwrap_or_else(|e| panic!("{size}: {e}"));
        assert_eq!((p.slide_size.width, p.slide_size.height), (960.0, 540.0), "{size}");
    }
}

#[test]
fn repair_fixes_missing_layout_and_size() {
    let mut p = Presentation::default();
    Arc::make_mut(&mut p.slides[0]).layout = deckcraft_model::LayoutId(99999);
    p.slide_size.width = f64::NAN;
    repair(&mut p);
    assert!(p.validate().is_empty());
    assert_eq!(p.slide_size.width, 960.0);
}

/// A duplicate slide in a deck whose ids are used up cannot be re-id'd: it stays as it is
/// (the engine refuses edits to such a deck) instead of a duplicated fresh id (#407).
#[test]
fn repair_keeps_a_duplicate_it_cannot_reid() {
    let mut p = Presentation { next_id: u32::MAX, ..Default::default() };
    p.slides.push(p.slides[0].clone());
    repair(&mut p);
    assert_eq!(p.slides.len(), 2, "the duplicate stays visible");
    assert_eq!(p.slides[0].id, p.slides[1].id, "no fresh id was invented");
}

#[test]
fn outline_roundtrip() {
    let mut p = defaults::blank_presentation(defaults::WIDE, Default::default(), false);
    let n = outline_to_slides(&mut p, "Intro\n\tPoint one\n\t\tSub point\nSecond\n\tMore");
    assert_eq!(n, 2);
    let o = slides_to_outline(&p);
    assert!(o.contains("Intro\n\tPoint one\n\t\tSub point\nSecond"), "{o}");
}

#[test]
fn outline_keeps_leading_dash_text() {
    let mut p = defaults::blank_presentation(defaults::WIDE, Default::default(), false);
    let n = outline_to_slides(&mut p, "-5 °C tonight\n\t--verbose flag\n\t*args and **kwargs\n- Bullets\n\t- Point\n\t• Other");
    assert_eq!(n, 2);
    let o = slides_to_outline(&p);
    assert!(o.contains("-5 °C tonight\n\t--verbose flag\n\t*args and **kwargs\nBullets\n\tPoint\n\tOther"), "{o}");
}

#[test]
fn outline_export_keeps_multi_paragraph_title_on_one_line() {
    let mut p = defaults::blank_presentation(defaults::WIDE, Default::default(), false);
    assert_eq!(outline_to_slides(&mut p, "Intro\n\tPoint one\nSecond\n\tMore"), 2);
    let title = Arc::make_mut(&mut p.slides[0]).shapes.iter_mut().find(|s| s.ph_type().is_some_and(|k| k.is_title())).expect("title placeholder");
    title.text = Some(TextBody::from_text("Line one\nLine two"));
    let o = slides_to_outline(&p);
    assert_eq!(o, "Line one Line two\n\tPoint one\nSecond\n\tMore\n");
    let mut q = defaults::blank_presentation(defaults::WIDE, Default::default(), false);
    assert_eq!(outline_to_slides(&mut q, &o), 2);
}

#[test]
fn markdown_headings_and_list_items() {
    assert_eq!(md_heading("# Intro"), Some("Intro"));
    assert_eq!(md_heading("   ### Deep ###  "), Some("Deep"));
    assert_eq!(md_heading("## C# and F#"), Some("C# and F#"));
    assert_eq!(md_heading("#"), Some(""));
    assert_eq!(md_heading("## Ünïcödé"), Some("Ünïcödé"));
    for not in ["#5 priority", "####### seven", "    # code", "text", "", "é#"] {
        assert_eq!(md_heading(not), None, "{not:?}");
    }
    assert_eq!(md_list_item("- a"), Some((0, "a")));
    assert_eq!(md_list_item("    * b"), Some((2, "b")));
    assert_eq!(md_list_item("\t+ c"), Some((1, "c")));
    assert_eq!(md_list_item("12) d"), Some((0, "d")));
    for not in ["-x", "1.x", "1", "1234567890. long", "• bullet", "é. x", ""] {
        assert_eq!(md_list_item(not), None, "{not:?}");
    }
}

/// #814: slides_to_outline includes table cell text, which it used to skip.
#[test]
fn outline_includes_table_text() {
    use deckcraft_model::table::{Cell, Row, Table};
    use deckcraft_model::{Shape, ShapeKind, Xfrm};
    let mut p = defaults::new_presentation(None);
    let layout = defaults::layout_of_kind(&p, LayoutType::TitleAndContent).unwrap();
    let mut s = defaults::new_slide(&mut p, layout).expect("slide");
    s.shapes.retain(|sh| !sh.ph_type().is_some_and(|k| k.is_title()));
    let mut body = Shape { xfrm: Some(Xfrm::new(0.0, 0.0, 600.0, 100.0)), ..Default::default() };
    body.text = Some(TextBody::from_text("Text box body"));
    s.shapes.push(body);

    let mut table = Table { cols: vec![200.0, 200.0], ..Default::default() };
    for texts in [["Cell A1", "Cell B1"], ["Cell A2", "Cell B2"]] {
        let cells: Vec<Cell> = texts.iter().map(|t| Cell { text: TextBody::from_text(t), ..Default::default() }).collect();
        table.rows.push(Row { height: 30.0, cells });
    }
    let mut tbl = Shape { xfrm: Some(Xfrm::new(0.0, 100.0, 400.0, 60.0)), ..Default::default() };
    tbl.kind = ShapeKind::Table(table);
    s.shapes.push(tbl);
    p.slides = vec![std::sync::Arc::new(s)];

    let out = slides_to_outline(&p);
    assert!(out.contains("Text box body"), "{out}");
    assert!(out.contains("Cell A1\tCell B1"), "{out}");
    assert!(out.contains("Cell A2\tCell B2"), "{out}");
}

/// Exercise the public file path from an explicitly ordinary-size native caller,
/// independent of the test runner's stack setting. The old parser aborted here at opt0.
#[cfg(not(target_family = "wasm"))]
#[test]
fn thirty_two_nested_groups_reopen_from_a_two_mib_caller() {
    use deckcraft_model::{Shape, ShapeId, table::Table};
    let mut table = Table::new(2, 2, 200.0, 30.0);
    table.rows[1].cells[1].text = TextBody::from_text("deepest cell");
    let mut shape = Shape { id: ShapeId(1), kind: ShapeKind::Table(table), ..Default::default() };
    for level in 0..32 {
        shape = Shape { id: ShapeId(level + 2), kind: ShapeKind::Group { children: vec![shape], child: Default::default() }, ..Default::default() };
    }
    let mut p = Presentation::default();
    Arc::make_mut(&mut p.slides[0]).shapes = vec![shape];
    let bytes = save(&p).expect("save 32 groups with a table");
    let back = std::thread::Builder::new()
        .name("two-mib-file-caller".into())
        .stack_size(2 * 1024 * 1024)
        .spawn(move || load(&bytes))
        .expect("start 2 MiB caller")
        .join()
        .expect("caller must not panic")
        .expect("32 groups must reopen");
    let mut shape = &back.slides[0].shapes[0];
    for _ in 0..32 {
        let ShapeKind::Group { children, .. } = &shape.kind else { panic!("expected all 32 groups") };
        assert_eq!(children.len(), 1);
        shape = &children[0];
    }
    let ShapeKind::Table(table) = &shape.kind else { panic!("expected the deepest table") };
    assert_eq!((table.n_rows(), table.n_cols()), (2, 2));
    assert_eq!(table.rows[1].cells[1].text.text(), "deepest cell");
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn parser_worker_preserves_bad_json_and_recursion_errors() {
    for bad in [b"{".as_slice(), b"null", br#"{"format":"deckcraft","version":1,"generator":"t","presentation":{}} trailing"#] {
        assert!(matches!(parse_manifest(bad), Err(FormatError::Damaged(_))));
    }
    // Construct hostile JSON bytes directly; building/serializing a model this deep
    // would exercise unrelated recursive code before reaching the parser under test.
    let nested_json = |depth| {
        let mut json = br#"{"format":"deckcraft","version":1,"generator":"t","presentation":{"slides":[{"shapes":["#.to_vec();
        for _ in 0..depth {
            json.extend_from_slice(br#"{"kind":{"type":"group","child":{"x":0,"y":0,"w":1,"h":1,"rot":0,"flipH":false,"flipV":false},"children":["#);
        }
        json.extend_from_slice(br#"{"kind":{"type":"shape"}}"#);
        for _ in 0..depth {
            json.extend_from_slice(b"]}}");
        }
        json.extend_from_slice(b"]}]}}");
        json
    };
    // Prove the same schema reaches the recursive model, rather than silently putting
    // `type`/`children` into unknown Shape fields that serde intentionally ignores.
    let shallow = parse_manifest(&nested_json(1)).unwrap_or_else(|e| panic!("valid shallow group: {e}"));
    let ShapeKind::Group { children, .. } = &shallow.presentation.slides[0].shapes[0].kind else { panic!("fixture must decode a group") };
    assert_eq!(children.len(), 1);
    assert!(matches!(children[0].kind, ShapeKind::Shape));
    let error = parse_manifest(&nested_json(160)).err().expect("excessive nesting must fail");
    assert!(matches!(error, FormatError::Damaged(_)));
    assert!(error.to_string().contains("recursion limit"), "{error}");
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn parser_worker_start_and_join_failures_are_results() {
    let error =
        parser_worker_result(Err(std::io::Error::other("injected thread creation failure"))).err().expect("thread creation failure must be returned");
    assert!(matches!(error, FormatError::Damaged(_)));
    assert!(error.to_string().contains("couldn't start parser: injected thread creation failure"), "{error}");
    let error = parser_worker_result(Ok(Err(Box::new("injected worker panic")))).err().expect("worker panic must be returned");
    assert!(matches!(error, FormatError::Damaged(_)));
    assert!(error.to_string().contains("couldn't finish parser: worker panicked"), "{error}");
}

#[test]
fn outline_table_rows_are_indented_and_do_not_become_slides() {
    use deckcraft_model::table::{Cell, Row, Table};
    use deckcraft_model::{Shape, ShapeKind, Xfrm};
    let mut p = defaults::new_presentation(None);
    let layout = defaults::layout_of_kind(&p, LayoutType::TitleAndContent).unwrap();
    let mut s = defaults::new_slide(&mut p, layout).expect("slide");
    for sh in s.shapes.iter_mut().filter(|sh| sh.ph_type().is_some_and(|k| k.is_title())) {
        sh.text = Some(TextBody::from_text("Slide title"));
    }
    let mut table = Table { cols: vec![200.0, 200.0], ..Default::default() };
    for texts in [["A1", "B1"], ["A2", "B2"]] {
        let cells: Vec<Cell> = texts.iter().map(|t| Cell { text: TextBody::from_text(t), ..Default::default() }).collect();
        table.rows.push(Row { height: 30.0, cells });
    }
    let mut tbl = Shape { xfrm: Some(Xfrm::new(0.0, 100.0, 400.0, 60.0)), ..Default::default() };
    tbl.kind = ShapeKind::Table(table);
    s.shapes.push(tbl);
    p.slides = vec![std::sync::Arc::new(s)];

    let out = slides_to_outline(&p);
    let mut q = defaults::new_presentation(None);
    q.slides.clear();
    assert_eq!(outline_to_slides(&mut q, &out), 1, "table rows became slides: {out:?}");
    assert_eq!(q.slides.len(), 1, "{out:?}");
    let body = q.slides[0].shapes.iter().filter_map(|sh| sh.text.as_ref()).map(|t| t.text()).collect::<Vec<_>>().join("\n");
    assert!(body.contains("A1\tB1"), "{body:?}");
    assert!(body.contains("A2\tB2"), "{body:?}");
}

#[test]
fn odp_roundtrip_preserves_mimetype_slides_shapes_tables_pictures_transitions_and_notes() {
    use deckcraft_model::anim::Transition;
    use deckcraft_model::style::{Gradient, GradientShape, GradientStop, PictureFill};
    use deckcraft_model::table::{Cell, Row, Table};
    use deckcraft_model::text::{Action, Bullet, Hyperlink, ParaProps, Paragraph, Run, RunProps};
    use deckcraft_model::{Background, ColorRef, Fill, Geom, Rgba, Shape, ShapeId, ShapeKind, Xfrm};

    let mut p = defaults::new_presentation(None);
    p.props.title = "ODP Interop Deck".into();
    p.props.author = "DeckCraft".into();
    let layout = p.slides[0].layout;
    let slide2 = defaults::new_slide(&mut p, layout).expect("slide2");
    p.slides.push(std::sync::Arc::new(slide2));

    // Minimal valid 1×1 RGBA PNG media item.
    let png_bytes: Vec<u8> = vec![
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, b'I', b'H', b'D', b'R', 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
        0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, b'I', b'D', b'A', b'T', 0x78, 0x9C, 0x63, 0x00, 0x01,
        0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82,
    ];
    let media_id = p.add_media("dot.png", "image/png", png_bytes).expect("add_media");

    {
        let s0 = std::sync::Arc::make_mut(&mut p.slides[0]);
        s0.name = "Intro Slide".into();
        s0.background = Some(Background::Fill { fill: Fill::Solid { color: ColorRef::rgb(Rgba::rgb(245, 248, 255)) } });
        s0.transition = Some(Transition {
            kind: "fade".into(),
            option: String::new(),
            duration_ms: 700,
            advance_on_click: true,
            advance_after_ms: Some(3000),
            ..Default::default()
        });
        s0.notes = TextBody::from_text("Remember to greet the audience in Korean: 안녕하세요.");
        s0.shapes = vec![
            Shape {
                id: ShapeId(10),
                name: "Banner".into(),
                xfrm: Some(Xfrm::new(48.0, 36.0, 520.0, 110.0)),
                geom: Geom::preset("roundRect"),
                kind: ShapeKind::Shape,
                fill: Some(Fill::Gradient(Gradient {
                    shape: GradientShape::Linear { angle: 30.0, scaled: true },
                    stops: vec![
                        GradientStop { pos: 0.0, color: ColorRef::rgb(Rgba::rgb(30, 80, 200)) },
                        GradientStop { pos: 1.0, color: ColorRef::rgb(Rgba::rgb(80, 180, 240)) },
                    ],
                    flip: None,
                    rotate_with_shape: true,
                    tile: [0.0; 4],
                })),
                text: Some(TextBody {
                    paragraphs: vec![
                        Paragraph {
                            runs: vec![Run::with(
                                "Quarterly ODP Report",
                                RunProps {
                                    bold: Some(true),
                                    size: Some(28.0),
                                    link: Some(Hyperlink {
                                        action: Action::Url { url: "https://example.com/report".into() },
                                        tooltip: String::new(),
                                        highlight_click: false,
                                        sound: None,
                                        stop_sound: false,
                                        zoom_return: None,
                                    }),
                                    ..Default::default()
                                },
                            )],
                            ..Default::default()
                        },
                        Paragraph {
                            level: 0,
                            props: ParaProps { bullet: Some(Bullet::Char { char: "•".into() }), ..Default::default() },
                            runs: vec![Run::new("Bullet item one")],
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }),
                ..Default::default()
            },
            Shape {
                id: ShapeId(11),
                name: "LogoPic".into(),
                xfrm: Some(Xfrm::new(620.0, 40.0, 120.0, 120.0)),
                kind: ShapeKind::Picture { fill: PictureFill { media: media_id, ..Default::default() } },
                ..Default::default()
            },
            Shape {
                id: ShapeId(12),
                name: "MetricsTable".into(),
                xfrm: Some(Xfrm::new(48.0, 200.0, 400.0, 120.0)),
                kind: ShapeKind::Table(Table {
                    cols: vec![200.0, 200.0],
                    rows: vec![
                        Row { height: 48.0, cells: vec![Cell::new("Metric"), Cell::new("Q4")] },
                        Row { height: 48.0, cells: vec![Cell::new("Revenue"), Cell::new("$4.2M")] },
                    ],
                    ..Default::default()
                }),
                ..Default::default()
            },
        ];
    }
    {
        let s1 = std::sync::Arc::make_mut(&mut p.slides[1]);
        s1.hidden = true;
        s1.transition = Some(Transition {
            kind: "push".into(),
            option: "l".into(),
            duration_ms: 500,
            advance_on_click: true,
            advance_after_ms: None,
            ..Default::default()
        });
    }

    let odp_bytes = crate::odp::save_odp(&p).expect("save_odp");
    assert!(crate::odp::sniff_odp(&odp_bytes), "sniff_odp must recognise saved .odp archive");

    // OASIS ODF 1.2 §3.3: `mimetype` must be the first ZIP entry and stored uncompressed.
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&odp_bytes)).unwrap();
    {
        let first = zip.by_index(0).unwrap();
        assert_eq!(first.name(), "mimetype");
        assert_eq!(first.compression(), zip::CompressionMethod::Stored);
    }

    let reopened = crate::odp::load_odp(&odp_bytes).expect("load_odp");
    assert!(reopened.validate().is_empty(), "{:?}", reopened.validate());
    assert_eq!(reopened.slides.len(), 2);
    assert_eq!(reopened.props.title, "ODP Interop Deck");
    assert!((reopened.slide_size.width - p.slide_size.width).abs() < 1.0);
    assert!((reopened.slide_size.height - p.slide_size.height).abs() < 1.0);
    assert!(reopened.slides[1].hidden, "hidden slide flag must survive ODP round-trip");
    assert_eq!(reopened.slides[0].transition.as_ref().map(|t| t.kind.as_str()), Some("fade"));
    assert_eq!(reopened.slides[1].transition.as_ref().map(|t| (t.kind.as_str(), t.option.as_str())), Some(("push", "l")));
    assert!(reopened.slides[0].notes_text().contains("안녕하세요"));
    assert!(!reopened.media.is_empty(), "embedded PNG must survive ODP round-trip");

    let all_text: String = reopened.slides[0]
        .shapes
        .iter()
        .map(|sh| match &sh.kind {
            ShapeKind::Table(tb) => tb.rows.iter().flat_map(|r| r.cells.iter().map(|c| c.text.text())).collect::<Vec<_>>().join(" "),
            _ => sh.text.as_ref().map(|t| t.text()).unwrap_or_default(),
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(all_text.contains("Quarterly ODP Report"), "{all_text}");
    assert!(all_text.contains("Bullet item one"), "{all_text}");
    assert!(all_text.contains("Revenue") && all_text.contains("$4.2M"), "{all_text}");
}
