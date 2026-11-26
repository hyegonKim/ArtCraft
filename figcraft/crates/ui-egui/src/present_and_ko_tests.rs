//! Tests for Prototyping Variable Mode live switcher, Prototype Video Fill playback controls,
//! and Korean (`ko`) i18n catalog translations (`Adobe XD 가져오기`, `비디오 채우기`, `변수 모드 전환`, etc.).

use std::sync::Arc;

use base64::Engine;
use figcraft_doc::NodeKind;
use figcraft_engine::Session;
use figcraft_engine::params::id_of;
use serde_json::json;

use crate::i18n::{self, Forms, Text};
use crate::present::Presenter;

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
    tkhd_body[76..80].copy_from_slice(&(u32::from(width) << 16).to_be_bytes());
    tkhd_body[80..84].copy_from_slice(&(u32::from(height) << 16).to_be_bytes());
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

#[test]
fn prototype_variable_mode_switcher_re_evaluates_all_variable_types_and_resets() {
    let mut s = Session::new();
    let mut make = |cmd: &str, p: serde_json::Value| id_of(&s.execute(cmd, &p).unwrap()["id"]).unwrap();

    let screen = make("frame.create", json!({"x": 0, "y": 0, "width": 390, "height": 844, "name": "Screen"}));
    let card = make("shape.rectangle", json!({"x": 20, "y": 20, "width": 200, "height": 120, "parent": screen, "name": "Card"}));
    let label = make("text.create", json!({"x": 20, "y": 160, "text": "Initial", "parent": screen, "name": "Title"}));
    let badge = make("shape.ellipse", json!({"x": 20, "y": 220, "width": 24, "height": 24, "parent": screen, "name": "Badge"}));

    s.execute("variable.createCollection", &json!({"name": "Theme", "modes": ["Light", "Dark"]})).unwrap();
    s.execute("variable.create", &json!({"collection": "Theme", "name": "bg", "type": "color", "value": "#FFFFFF"})).unwrap();
    s.execute("variable.set", &json!({"variable": "bg", "mode": "Dark", "value": "#111827"})).unwrap();

    s.execute("variable.create", &json!({"collection": "Theme", "name": "radius", "type": "number", "value": 8})).unwrap();
    s.execute("variable.set", &json!({"variable": "radius", "mode": "Dark", "value": 24})).unwrap();

    s.execute("variable.create", &json!({"collection": "Theme", "name": "headline", "type": "string", "value": "Light Mode"})).unwrap();
    s.execute("variable.set", &json!({"variable": "headline", "mode": "Dark", "value": "Dark Mode"})).unwrap();

    s.execute("variable.create", &json!({"collection": "Theme", "name": "showBadge", "type": "boolean", "value": true})).unwrap();
    s.execute("variable.set", &json!({"variable": "showBadge", "mode": "Dark", "value": false})).unwrap();

    s.execute("variable.bind", &json!({"ids": [screen], "field": "fill", "variable": "bg"})).unwrap();
    s.execute("variable.bind", &json!({"ids": [card], "field": "cornerRadius", "variable": "radius"})).unwrap();
    s.execute("variable.bind", &json!({"ids": [label], "field": "characters", "variable": "headline"})).unwrap();
    s.execute("variable.bind", &json!({"ids": [badge], "field": "visible", "variable": "showBadge"})).unwrap();

    let doc = Arc::new(s.doc().unwrap().clone());
    let mut p = Presenter::new(screen);
    p.sync(&doc, 0.0);

    let colls = p.variable_collections();
    assert_eq!(colls.len(), 1);
    let (cid, cname, active_light, modes) = &colls[0];
    assert_eq!(cname, "Theme");
    assert_eq!(modes.len(), 2);
    let light_id = modes.iter().find(|(_, n)| n == "Light").unwrap().0.clone();
    let dark_id = modes.iter().find(|(_, n)| n == "Dark").unwrap().0.clone();
    assert_eq!(active_light, &light_id);
    assert!(!p.has_variable_mode_overrides());

    // Switch live to Dark mode in the presenter.
    assert!(p.switch_variable_mode(cid, &dark_id));
    assert!(p.has_variable_mode_overrides());

    let shown = p.display().unwrap();
    assert_eq!(shown.node(screen).unwrap().fills[0].solid_color().unwrap().to_hex(), "111827");
    assert_eq!(shown.node(card).unwrap().corners.radii, [24.0; 4]);
    match &shown.node(label).unwrap().kind {
        NodeKind::Text(td) => assert_eq!(td.characters, "Dark Mode"),
        other => panic!("expected Text, got {other:?}"),
    }
    assert!(!shown.node(badge).unwrap().visible);

    // Reset back to authored default mode (Light).
    assert!(p.reset_variable_modes());
    assert!(!p.has_variable_mode_overrides());

    let restored = p.display().unwrap();
    assert_eq!(restored.node(screen).unwrap().fills[0].solid_color().unwrap().to_hex(), "FFFFFF");
    assert_eq!(restored.node(card).unwrap().corners.radii, [8.0; 4]);
    match &restored.node(label).unwrap().kind {
        NodeKind::Text(td) => assert_eq!(td.characters, "Light Mode"),
        other => panic!("expected Text, got {other:?}"),
    }
    assert!(restored.node(badge).unwrap().visible);
}

#[test]
fn prototype_video_playback_autoplay_loop_pause_mute_and_seek() {
    let mut s = Session::new();
    let screen = id_of(&s.execute("frame.create", &json!({"x": 0, "y": 0, "width": 400, "height": 400, "name": "PlayerScreen"})).unwrap()["id"]).unwrap();

    let clip_loop = base64::engine::general_purpose::STANDARD.encode(minimal_mp4(640, 360, 4));
    let clip_once = base64::engine::general_purpose::STANDARD.encode(minimal_mp4(640, 360, 2));

    let v1 = id_of(
        &s.execute(
            "video.place",
            &json!({"parent": screen, "x": 10, "y": 10, "width": 160, "height": 90, "data": clip_loop, "autoplay": true, "loopPlayback": true, "muted": true}),
        )
        .unwrap()["id"],
    )
    .unwrap();
    let v2 = id_of(
        &s.execute(
            "video.place",
            &json!({"parent": screen, "x": 10, "y": 120, "width": 160, "height": 90, "data": clip_once, "autoplay": false, "loopPlayback": false, "muted": false}),
        )
        .unwrap()["id"],
    )
    .unwrap();

    let doc = Arc::new(s.doc().unwrap().clone());
    let mut p = Presenter::new(screen);
    p.sync(&doc, 0.0);

    assert_eq!(p.video_playback.len(), 2);
    assert!(p.video_playback[&(v1, 0)].playing);
    assert!(p.video_playback[&(v1, 0)].muted);
    assert!(!p.video_playback[&(v2, 0)].playing);
    assert!(!p.video_playback[&(v2, 0)].muted);

    // Step looping video across its 4s boundary (0 -> 0.8 -> 1.6 -> ... -> wraps around 4.0).
    assert!(p.step_videos(1.0));
    for t in [1.9, 2.8, 3.7, 4.6, 5.2] {
        p.step_videos(t);
    }
    let pos1 = p.video_playback[&(v1, 0)].position;
    assert!(pos1 > 0.1 && pos1 < 0.5, "looping video wrapped cleanly past 4.0s, got {pos1}");
    assert!(p.video_playback[&(v1, 0)].playing);

    // Start non-looping 2s video, advance to end, verify it pauses at 2.0s and rewinds on replay.
    assert_eq!(p.toggle_video_playback(v2, 0, 10.0), Some(true));
    p.step_videos(10.9);
    p.step_videos(11.8);
    p.step_videos(12.5);
    assert!(!p.video_playback[&(v2, 0)].playing, "non-looping video stops at end");
    assert!((p.video_playback[&(v2, 0)].position - 2.0).abs() < 1e-6);

    assert_eq!(p.toggle_video_playback(v2, 0, 13.0), Some(true));
    assert!(p.video_playback[&(v2, 0)].position < 1e-6, "replaying finished video rewinds to 0.0");

    // Toggle mute and seek.
    assert_eq!(p.toggle_video_mute(v1, 0), Some(false));
    let sought = p.seek_video(v1, 0, 0.75).unwrap();
    assert!((sought - 3.0).abs() < 1e-6);

    let inspected = p.inspect();
    assert_eq!(inspected["videos"].as_array().map(Vec::len), Some(2));
}

#[test]
fn korean_catalog_translates_xd_import_video_fill_and_variable_mode_switcher() {
    let ko = i18n::languages().iter().find(|l| l.code == "ko").expect("Korean (ko) catalog must exist");
    let plain_of = |table: &'static [(&'static str, Text)], key: &str| -> Option<&'static str> {
        table
            .binary_search_by(|(k, _)| (*k).cmp(key))
            .ok()
            .and_then(|i| table.get(i))
            .map(|(_, t)| match t {
                Text::Plain(s) | Text::Plural(Forms { other: s, .. }) => *s,
            })
    };

    assert_eq!(plain_of(ko.ui, "Import Adobe XD"), Some("Adobe XD 가져오기"));
    assert_eq!(plain_of(ko.commands, "file.importXd"), Some("Adobe XD 가져오기"));
    assert_eq!(plain_of(ko.menu, "file.importXd"), Some("Adobe XD (.xd) 가져오기…"));

    assert_eq!(plain_of(ko.ui, "Video fill"), Some("비디오 채우기"));
    assert_eq!(plain_of(ko.ui, "Video"), Some("비디오"));
    assert_eq!(plain_of(ko.ui, "Autoplay"), Some("자동 재생"));
    assert_eq!(plain_of(ko.ui, "Loop"), Some("반복"));
    assert_eq!(plain_of(ko.ui, "Muted|video"), Some("음소거"));
    assert_eq!(plain_of(ko.ui, "Muted"), Some("차분하게"));
    assert_eq!(plain_of(ko.ui, "Upload video"), Some("비디오 업로드"));
    assert_eq!(plain_of(ko.ui, "Choose video"), Some("비디오 선택"));
    assert_eq!(plain_of(ko.ui, "Play video"), Some("비디오 재생"));

    assert_eq!(plain_of(ko.ui, "Switch variable mode"), Some("변수 모드 전환"));
    assert_eq!(plain_of(ko.ui, "Reset variable modes"), Some("변수 모드 초기화"));
}
