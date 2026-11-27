use super::{Dialog, SlideApp, Ui, Value, check, field, json, num, vec2};

/// The export dialog's Animated GIF options (size, seconds per slide, loop, slide range, hidden
/// slides) as `file.export` params; a slide range that doesn't fit the deck is an error message.
pub(super) fn gif_options(app: &SlideApp, ui: &mut Ui, d: &mut Dialog) -> Result<Value, String> {
    use deckcraft_engine::gif_export::{MAX_SECONDS, MAX_WIDTH, MIN_SECONDS, MIN_WIDTH, SIZE_PRESETS};
    let n = app.session.active().map_or(0, |s| s.doc.slides.len());
    let size = d.get("gifSize", "medium");
    let size_label = |k: &str| match k {
        "small" => "Small (640 px wide)",
        "large" => "Large (1280 px wide)",
        "extraLarge" => "Extra Large (1920 px wide)",
        "custom" => "Custom width",
        _ => "Medium (960 px wide)",
    };
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let field = ui.add_sized(vec2(120.0, 20.0), egui::Label::new("Size:"));
        let combo = craft_ui::ComboBox::from_id_salt("gif_size").selected_text(size_label(&size)).show_ui(ui, |ui| {
            for k in SIZE_PRESETS.iter().map(|(k, _)| *k).chain(["custom"]) {
                if ui.selectable_label(size == k, size_label(k)).clicked() {
                    d.fields.insert("gifSize".into(), k.into());
                }
            }
        });
        let _ = combo.response.labelled_by(field.id);
    });
    if size == "custom" {
        field(ui, d, "Width (pixels):", "gifWidth", "960");
    }
    field(ui, d, "Seconds per slide:", "gifSeconds", "5");
    let n_text = n.max(1).to_string();
    field(ui, d, "From slide:", "gifFrom", "1");
    field(ui, d, "To slide:", "gifTo", &n_text);
    let forever = check(ui, d, "Loop forever", "gifLoop", true);
    let hidden = check(ui, d, "Include hidden slides", "gifHidden", false);

    let width = if size == "custom" {
        let w = num(d, "gifWidth", 960.0).round();
        if !(f64::from(MIN_WIDTH)..=f64::from(MAX_WIDTH)).contains(&w) {
            return Err(format!("Width: enter {MIN_WIDTH} to {MAX_WIDTH} pixels"));
        }
        json!(w as u32)
    } else {
        json!(size)
    };
    let seconds = num(d, "gifSeconds", f64::NAN);
    if deckcraft_engine::gif_export::check_seconds(seconds).is_err() {
        return Err(format!("Seconds per slide: enter {MIN_SECONDS} to {MAX_SECONDS}"));
    }
    let mut p = json!({"width": width, "secondsPerSlide": seconds, "loop": forever, "includeHidden": hidden});
    let (from, to) = (num(d, "gifFrom", 1.0).round(), num(d, "gifTo", n as f64).round());
    if !(from >= 1.0 && from <= to && to <= n as f64) {
        return Err(format!("Slides: choose a range from 1 to {n}"));
    }
    if from > 1.0 || to < n as f64 {
        p["slides"] = json!(((from as usize - 1)..(to as usize)).collect::<Vec<_>>());
    }
    Ok(p)
}

/// Animated Video / GIF export dialog controls (`"exportVideo"`, `애니메이션 비디오 내보내기`).
/// Returns `(format_id, extra_params)` when valid, or an inline validation message.
pub(super) fn video_export_options(app: &SlideApp, ui: &mut Ui, d: &mut Dialog) -> Result<(String, Value), String> {
    use deckcraft_engine::video_export::{MAX_DIM, MAX_FPS, MAX_HOLD_SECONDS, MIN_DIM, MIN_FPS, MIN_HOLD_SECONDS};

    let loc = app.ui.locale;
    let n = app.session.active().map_or(0, |s| s.doc.slides.len());
    let fmt = d.get("vFmt", "mp4");
    let fmt_label = |k: &str| match k {
        "webm" => "WebM Video (.webm)",
        "gif" => "Animated GIF (.gif)",
        _ => "MP4 Video (.mp4)",
    };
    ui.horizontal(|ui| {
        let lbl = ui.add_sized(vec2(140.0, 20.0), egui::Label::new(crate::i18n::tr_ui(ui.ctx(), loc, "Format:")));
        let combo = craft_ui::ComboBox::from_id_salt("video_export_fmt").width(200.0).selected_text(fmt_label(&fmt)).show_ui(ui, |ui| {
            for k in ["mp4", "webm", "gif"] {
                if ui.selectable_label(fmt == k, fmt_label(k)).clicked() {
                    d.fields.insert("vFmt".into(), k.into());
                }
            }
        });
        let _ = combo.response.labelled_by(lbl.id);
    });

    let res = d.get("vRes", if fmt == "gif" { "small" } else { "large" });
    let res_label = |k: &str| match k {
        "small" => "640 × 360 (SD)",
        "medium" => "960 × 540 (qHD)",
        "extraLarge" => "1920 × 1080 (Full HD)",
        "custom" => "Custom",
        _ => "1280 × 720 (HD)",
    };
    ui.horizontal(|ui| {
        let lbl = ui.add_sized(vec2(140.0, 20.0), egui::Label::new(crate::i18n::tr_ui(ui.ctx(), loc, "Resolution:")));
        let combo = craft_ui::ComboBox::from_id_salt("video_export_res").width(200.0).selected_text(res_label(&res)).show_ui(ui, |ui| {
            for k in ["small", "medium", "large", "extraLarge", "custom"] {
                if ui.selectable_label(res == k, res_label(k)).clicked() {
                    d.fields.insert("vRes".into(), k.into());
                }
            }
        });
        let _ = combo.response.labelled_by(lbl.id);
    });
    if res == "custom" {
        field(ui, d, crate::i18n::tr_ui(ui.ctx(), loc, "Width (pixels):"), "vWidth", "1280");
        field(ui, d, crate::i18n::tr_ui(ui.ctx(), loc, "Height (pixels):"), "vHeight", "720");
    }

    let fps_str = d.get("vFps", if fmt == "gif" { "15" } else { "24" });
    ui.horizontal(|ui| {
        let lbl = ui.add_sized(vec2(140.0, 20.0), egui::Label::new(crate::i18n::tr_ui(ui.ctx(), loc, "Frame rate (FPS):")));
        let combo = craft_ui::ComboBox::from_id_salt("video_export_fps").width(200.0).selected_text(format!("{fps_str} fps")).show_ui(ui, |ui| {
            for f in ["15", "24", "30"] {
                if ui.selectable_label(fps_str == f, format!("{f} fps")).clicked() {
                    d.fields.insert("vFps".into(), f.into());
                }
            }
        });
        let _ = combo.response.labelled_by(lbl.id);
    });

    field(ui, d, crate::i18n::tr_ui(ui.ctx(), loc, "Slide hold (seconds):"), "vHold", "2");
    let n_text = n.max(1).to_string();
    field(ui, d, crate::i18n::tr_ui(ui.ctx(), loc, "From slide:"), "vFrom", "1");
    field(ui, d, crate::i18n::tr_ui(ui.ctx(), loc, "To slide:"), "vTo", &n_text);

    let transitions = check(ui, d, crate::i18n::tr_ui(ui.ctx(), loc, "Include slide transitions"), "vTrans", true);
    let builds = check(ui, d, crate::i18n::tr_ui(ui.ctx(), loc, "Include build animations"), "vBuilds", true);
    let hidden = check(ui, d, crate::i18n::tr_ui(ui.ctx(), loc, "Include hidden slides"), "vHidden", false);
    let loop_forever = if fmt == "gif" { check(ui, d, crate::i18n::tr_ui(ui.ctx(), loc, "Loop forever"), "vLoop", true) } else { true };

    let fps =
        fps_str.trim().parse::<u32>().ok().filter(|v| (MIN_FPS..=MAX_FPS).contains(v)).ok_or_else(|| format!("FPS: enter {MIN_FPS} to {MAX_FPS}"))?;
    let hold = num(d, "vHold", f64::NAN);
    if !(MIN_HOLD_SECONDS..=MAX_HOLD_SECONDS).contains(&hold) {
        return Err(format!("Slide hold: enter {MIN_HOLD_SECONDS} to {MAX_HOLD_SECONDS} seconds"));
    }

    let mut p = json!({
        "format": fmt,
        "fps": fps,
        "holdSeconds": hold,
        "includeHidden": hidden,
        "includeBuilds": builds,
        "includeTransitions": transitions,
        "loop": loop_forever,
        "animate": true,
    });
    if res == "custom" {
        let w = num(d, "vWidth", 1280.0).round();
        let h = num(d, "vHeight", 720.0).round();
        if !(f64::from(MIN_DIM)..=f64::from(MAX_DIM)).contains(&w) {
            return Err(format!("Width: enter {MIN_DIM} to {MAX_DIM} pixels"));
        }
        if !(f64::from(MIN_DIM)..=f64::from(MAX_DIM)).contains(&h) {
            return Err(format!("Height: enter {MIN_DIM} to {MAX_DIM} pixels"));
        }
        p["width"] = json!(w as u32);
        p["height"] = json!(h as u32);
    } else {
        p["width"] = json!(res);
    }

    if n > 0 {
        let (from, to) = (num(d, "vFrom", 1.0).round(), num(d, "vTo", n as f64).round());
        if !(from >= 1.0 && from <= to && to <= n as f64) {
            return Err(format!("Slides: choose a range from 1 to {n}"));
        }
        if from > 1.0 || to < n as f64 {
            p["slides"] = json!(((from as usize - 1)..(to as usize)).collect::<Vec<_>>());
        }
    }
    Ok((fmt, p))
}
