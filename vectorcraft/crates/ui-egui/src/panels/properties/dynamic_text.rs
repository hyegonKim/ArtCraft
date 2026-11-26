//! Dynamic Text in the Properties panel's Paragraph section: the Dynamic Text checkbox for the
//! selected area type (Block, `type.dynamicText`) and, while it is on, its Word Gap and Line Gap
//! rows ([Dynamic Text overview](https://helpx.adobe.com/photoshop/desktop/text-typography/work-with-dynamic-text/dynamic-text-overview.html)
//! describes Block and its spacing between words and between lines).

use egui::Ui;
use serde_json::{Value, json};
use vectorcraft_doc::{AreaOptions, NodeKind, PathEffect, TextKind};

use crate::VectorcraftApp;
use crate::widgets;

/// What the selected area type shows: whether Block is on in every frame (`None` when they
/// differ), the gaps every frame shares (`None` when they differ), and whether every frame can
/// take it (none is threaded).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Shown {
    pub(crate) on: Option<bool>,
    pub(crate) word_gap: Option<f64>,
    pub(crate) line_gap: Option<f64>,
    pub(crate) enabled: bool,
}

/// Dynamic Text of the selected area type (and area type in selected groups); `None` when the
/// selection holds no area type, or holds other type too (the command refuses it).
pub(crate) fn shown(app: &VectorcraftApp) -> Option<Shown> {
    let st = app.session.active()?;
    let threaded = |id| st.doc.text_threads.iter().any(|t| t.len() > 1 && t.contains(&id));
    let mut frames = vec![];
    let mut other = false;
    for n in st.selection.objects.iter().filter_map(|id| st.doc.node(*id)) {
        n.walk(&mut |c| {
            let NodeKind::Text(t) = &c.kind else { return };
            if matches!(t.kind, TextKind::Area { .. }) {
                let (word, line) = t.area.block_gaps();
                frames.push((t.dynamic_text, word, line, threaded(c.id)));
            } else {
                other = true;
            }
        });
    }
    let &(on, word, line, _) = frames.first().filter(|_| !other)?;
    Some(Shown {
        on: frames.iter().all(|f| f.0 == on).then_some(on),
        word_gap: frames.iter().all(|f| f.1 == word).then_some(word),
        line_gap: frames.iter().all(|f| f.2 == line).then_some(line),
        enabled: frames.iter().all(|f| !f.3),
    })
}

fn run(app: &mut VectorcraftApp, p: Value) {
    if let Err(e) = app.run("type.dynamicText", p) {
        app.status(e);
    }
}

const PATH_ALIGNMENTS: [(&str, PathEffect); 5] = [
    ("Rainbow", PathEffect::Rainbow),
    ("Skew", PathEffect::Skew),
    ("3D Ribbon", PathEffect::Ribbon3d),
    ("Stair Step", PathEffect::StairStep),
    ("Gravity", PathEffect::Gravity),
];

#[derive(Clone, Copy, Debug, PartialEq)]
struct PathShown {
    on: Option<bool>,
    spacing: Option<f64>,
    effect: Option<PathEffect>,
    enabled: bool,
}

fn path_shown(app: &VectorcraftApp) -> Option<PathShown> {
    let st = app.session.active()?;
    let threaded = |id| st.doc.text_threads.iter().any(|t| t.len() > 1 && t.contains(&id));
    let mut paths = vec![];
    let mut other = false;
    for n in st.selection.objects.iter().filter_map(|id| st.doc.node(*id)) {
        n.walk(&mut |c| {
            let NodeKind::Text(t) = &c.kind else { return };
            if matches!(t.kind, TextKind::OnPath { .. }) {
                paths.push((t.dynamic_text, t.path_spacing, t.path_effect, threaded(c.id)));
            } else {
                other = true;
            }
        });
    }
    let &(on, spacing, effect, _) = paths.first().filter(|_| !other)?;
    Some(PathShown {
        on: paths.iter().all(|p| p.0 == on).then_some(on),
        spacing: paths.iter().all(|p| p.1 == spacing).then_some(spacing),
        effect: paths.iter().all(|p| p.2 == effect).then_some(effect),
        enabled: paths.iter().all(|p| !p.3),
    })
}

fn path_rows(app: &mut VectorcraftApp, ui: &mut Ui) -> bool {
    let Some(ps) = path_shown(app) else { return false };
    if widgets::check(ui, tl!("Dynamic Text"), ps.on == Some(true), ps.enabled) {
        run(app, json!({ "on": ps.on != Some(true) }));
    }
    let fw = super::super::field_width(ui);
    let unit = app.session.general_unit();
    ui.horizontal(|ui| {
        ui.label(tl!("Path Alignment"));
        let labels = ["Rainbow", "Skew", "3D Ribbon", "Stair Step", "Gravity"];
        let cur = ps
            .effect
            .and_then(|eff| PATH_ALIGNMENTS.iter().find(|(_, e)| *e == eff).map(|(l, _)| *l))
            .unwrap_or("—");
        if let Some(idx) = widgets::dropdown(ui, "props-dt-path-align", cur, &labels, fw * 1.4)
            && let Some((_, eff)) = PATH_ALIGNMENTS.get(idx)
            && let Err(e) = app.run("type.pathOptions", json!({ "effect": eff.id() }))
        {
            app.status(e);
        }
    });
    ui.horizontal(|ui| {
        ui.label(tl!("Spacing"));
        if let Some(v) = widgets::spin_field_named(ui, "props-dt-path-spacing", Some(tl!("Spacing")), ps.spacing, unit, fw, 1.0, -1000.0, &[])
            && let Err(e) = app.run("type.pathOptions", json!({ "spacing": v.clamp(-1000.0, 1000.0) }))
        {
            app.status(e);
        }
    });
    ui.horizontal(|ui| {
        if widgets::secondary_button(ui, tl!("Path Options…")).clicked() {
            crate::menus::invoke(app, "type.pathOptions", json!({}));
        }
        if widgets::secondary_button(ui, tl!("Reverse Direction")).clicked()
            && let Err(e) = app.run("type.reversePathDirection", json!({}))
        {
            app.status(e);
        }
    });
    true
}

/// The checkbox, and the gap rows while every selected frame has Block on (or path controls when
/// type on a path is selected). Returns whether anything was drawn.
pub(crate) fn rows(app: &mut VectorcraftApp, ui: &mut Ui) -> bool {
    let Some(s) = shown(app) else {
        return path_rows(app, ui);
    };
    if widgets::check(ui, tl!("Dynamic Text"), s.on == Some(true), s.enabled) {
        run(app, json!({ "on": s.on != Some(true) }));
    }
    if s.on != Some(true) {
        return true;
    }
    let fw = super::super::field_width(ui);
    let (lo, hi) = AreaOptions::WORD_GAP;
    ui.horizontal(|ui| {
        ui.label(tl!("Word Gap"));
        if let Some(v) = widgets::spin_plain_named(ui, "props-dt-word", Some(tl!("Word Gap")), s.word_gap, "%", 0, fw, 10.0, lo, &[]) {
            run(app, json!({ "wordGap": v.clamp(lo, hi) }));
        }
    });
    let (lo, hi) = AreaOptions::LINE_GAP;
    let unit = app.session.general_unit();
    ui.horizontal(|ui| {
        ui.label(tl!("Line Gap"));
        if let Some(v) = widgets::spin_field_named(ui, "props-dt-line", Some(tl!("Line Gap")), s.line_gap, unit, fw, 1.0, lo, &[]) {
            run(app, json!({ "lineGap": v.clamp(lo, hi) }));
        }
    });
    true
}
