//! Window → Icons (`iconspanel` / `IconSheet.aip` — Illustrator 2027 `31.0`): searchable built-in
//! procedural vector icon library with live vector tile previews, one-click canvas insertion,
//! Generate Icon Sheet dialog launch, and multi-size batch SVG/PNG export (`16–128 px`).

use egui::{Pos2, Rect, Sense, Stroke, Ui, vec2};
use serde_json::json;
use vectorcraft_engine::cmd::icons::{CATALOG, ICON_CATEGORIES, icon_path, matching_icons};
use vectorcraft_geom::PathEl;

use crate::state::Dialog;
use crate::theme::Tokens;
use crate::{VectorcraftApp, picks, widgets};

pub const ID: &str = "icons";

const SEARCH_KEY: &str = "icons-panel-query";
const CAT_KEY: &str = "icons-panel-category";
const SIZE_KEY: &str = "icons-panel-size";

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let has_doc = app.session.active().is_some();

    let query_id = ui.make_persistent_id(SEARCH_KEY);
    let cat_id = ui.make_persistent_id(CAT_KEY);
    let size_id = ui.make_persistent_id(SIZE_KEY);

    let mut query = ui.data(|d| d.get_temp::<String>(query_id)).unwrap_or_default();
    let mut category = ui.data(|d| d.get_temp::<String>(cat_id)).unwrap_or_default();
    let mut insert_size = ui.data(|d| d.get_temp::<f64>(size_id)).unwrap_or(24.0);

    ui.horizontal(|ui| {
        widgets::dim_label(ui, tl!("Search:"));
        let r = ui.add(
            egui::TextEdit::singleline(&mut query)
                .hint_text(tl!("Search icons…"))
                .desired_width((ui.available_width() - 8.0).max(80.0)),
        );
        if r.changed() {
            ui.data_mut(|d| d.insert_temp(query_id, query.clone()));
        }
    });

    ui.add_space(4.0);
    let cat_labels = [
        tl!("All"),
        tl!("UI"),
        tl!("Arrows"),
        tl!("Media"),
        tl!("Files"),
        tl!("Commerce"),
        tl!("Badges"),
    ];
    let cat_idx = ICON_CATEGORIES
        .iter()
        .position(|c| c.eq_ignore_ascii_case(&category))
        .map_or(0, |i| i + 1);

    ui.horizontal(|ui| {
        widgets::dim_label(ui, tl!("Category:"));
        if let Some(picked) = widgets::dropdown(ui, "icons-cat", cat_labels[cat_idx], &cat_labels, 108.0) {
            category = if picked == 0 {
                String::new()
            } else {
                ICON_CATEGORIES.get(picked - 1).copied().unwrap_or_default().to_string()
            };
            ui.data_mut(|d| d.insert_temp(cat_id, category.clone()));
        }
        if let Some(sz) = widgets::range_field(ui, "icons-size", insert_size, 8.0..=256.0, " pt", 0, 56.0) {
            insert_size = sz;
            ui.data_mut(|d| d.insert_temp(size_id, insert_size));
        }
    });

    let filtered = matching_icons(Some(&query), (!category.is_empty()).then_some(category.as_str()));
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(crate::i18n::fmt(
            tl!("{shown} of {total} icons"),
            &[("shown", &filtered.len().to_string()), ("total", &CATALOG.len().to_string())],
        ))
        .size(11.0)
        .color(t.text_dim),
    );
    ui.add_space(4.0);

    let mut clicked_icon: Option<&'static str> = None;
    egui::ScrollArea::vertical().max_height(220.0).auto_shrink([false, false]).show(ui, |ui| {
        let avail = ui.available_width().max(48.0);
        let tile = 42.0_f32;
        let gap = 6.0_f32;
        let cols = (((avail + gap) / (tile + gap)).floor() as usize).max(1);
        egui::Grid::new("icons-tile-grid").spacing([gap, gap]).show(ui, |ui| {
            for (idx, ic) in filtered.iter().enumerate() {
                if draw_icon_tile(ui, ic.name, ic.label, has_doc, tile, &t) {
                    clicked_icon = Some(ic.name);
                }
                if (idx + 1) % cols == 0 {
                    ui.end_row();
                }
            }
        });
    });

    if let Some(name) = clicked_icon
        && let Err(e) = app.run("icon.insert", json!({"name": name, "size": insert_size}))
    {
        app.status(e);
    }

    ui.separator();
    ui.horizontal_wrapped(|ui| {
        if ui.add_enabled(has_doc, egui::Button::new(tl!("Generate Icon Sheet…"))).clicked() {
            let mut f = crate::dialogs::icon_sheet::fields();
            if !category.is_empty()
                && let Some(o) = f.as_object_mut()
            {
                o.insert("category".into(), json!(category.clone()));
            }
            app.ui.dialog = Some(Dialog::new(crate::dialogs::icon_sheet::KIND, f));
        }
        let can_folder = picks::can(app, &picks::PickRequest::Folder);
        if ui.add_enabled(can_folder, egui::Button::new(tl!("Batch Export…"))).clicked() {
            let cat_filter = category.clone();
            let q_filter = query.clone();
            picks::button(app, move |app| {
                if let Some(dir) = picks::folder(app) {
                    let mut p = json!({
                        "dir": dir,
                        "sizes": [16, 24, 32, 48, 64, 128],
                        "formats": ["svg", "png"],
                    });
                    if !cat_filter.is_empty() {
                        p["category"] = json!(cat_filter);
                    }
                    if !q_filter.trim().is_empty() {
                        p["query"] = json!(q_filter.trim());
                    }
                    if let Err(e) = app.run("icon.exportBatch", p) {
                        app.status(e);
                    }
                }
            });
        }
    });
}

pub fn menu(app: &mut VectorcraftApp, ui: &mut Ui) {
    let has_doc = app.session.active().is_some();
    if widgets::menu_item(ui, tl!("Generate Icon Sheet…"), has_doc, false) {
        app.ui.dialog = Some(Dialog::new(crate::dialogs::icon_sheet::KIND, crate::dialogs::icon_sheet::fields()));
        ui.close();
    }
    let can_folder = picks::can(app, &picks::PickRequest::Folder);
    if widgets::menu_item(ui, tl!("Batch Export…"), can_folder, false) {
        picks::button(app, |app| {
            if let Some(dir) = picks::folder(app)
                && let Err(e) = app.run(
                    "icon.exportBatch",
                    json!({"dir": dir, "sizes": [16, 24, 32, 48, 64, 128], "formats": ["svg", "png"]}),
                )
            {
                app.status(e);
            }
        });
        ui.close();
    }
}

fn draw_icon_tile(ui: &mut Ui, name: &str, label: &str, enabled: bool, side: f32, t: &Tokens) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(side, side), if enabled { Sense::click() } else { Sense::hover() });
    let hovered = enabled && resp.hovered();
    let bg = if hovered { t.hover } else { t.input };
    let border = if hovered { t.accent } else { t.input_border };
    ui.painter().rect_filled(rect, 4.0, bg);
    ui.painter().rect_stroke(rect, 4.0, Stroke::new(1.0, border), egui::StrokeKind::Inside);

    if let Some(path) = icon_path(name) {
        let inner = Rect::from_center_size(rect.center(), vec2(22.0, 22.0));
        let stroke_col = if enabled { t.text } else { t.text_disabled };
        paint_icon_bez(ui, inner, &path.to_bezpath(), Stroke::new(1.5, stroke_col));
    }
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.on_hover_text(format!("{label} ({name})")).clicked() && enabled
}

fn paint_icon_bez(ui: &Ui, target: Rect, bez: &vectorcraft_geom::BezPath, stroke: Stroke) {
    let map_pt = |p: vectorcraft_geom::Point| -> Pos2 {
        Pos2::new(
            target.left() + (p.x as f32 / 24.0) * target.width(),
            target.top() + (p.y as f32 / 24.0) * target.height(),
        )
    };
    let mut start = Pos2::ZERO;
    let mut cur = Pos2::ZERO;
    for el in bez.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                cur = map_pt(p);
                start = cur;
            }
            PathEl::LineTo(p) => {
                let next = map_pt(p);
                ui.painter().line_segment([cur, next], stroke);
                cur = next;
            }
            PathEl::QuadTo(c, p) => {
                let (cp, end) = (map_pt(c), map_pt(p));
                let mut prev = cur;
                for step in 1..=6 {
                    let u = step as f32 / 6.0;
                    let inv = 1.0 - u;
                    let nx = inv * inv * cur.x + 2.0 * inv * u * cp.x + u * u * end.x;
                    let ny = inv * inv * cur.y + 2.0 * inv * u * cp.y + u * u * end.y;
                    let pt = Pos2::new(nx, ny);
                    ui.painter().line_segment([prev, pt], stroke);
                    prev = pt;
                }
                cur = end;
            }
            PathEl::CurveTo(c1, c2, p) => {
                let (p1, p2, end) = (map_pt(c1), map_pt(c2), map_pt(p));
                let mut prev = cur;
                for step in 1..=8 {
                    let u = step as f32 / 8.0;
                    let inv = 1.0 - u;
                    let b0 = inv * inv * inv;
                    let b1 = 3.0 * inv * inv * u;
                    let b2 = 3.0 * inv * u * u;
                    let b3 = u * u * u;
                    let pt = Pos2::new(
                        b0 * cur.x + b1 * p1.x + b2 * p2.x + b3 * end.x,
                        b0 * cur.y + b1 * p1.y + b2 * p2.y + b3 * end.y,
                    );
                    ui.painter().line_segment([prev, pt], stroke);
                    prev = pt;
                }
                cur = end;
            }
            PathEl::ClosePath => {
                if (cur - start).length_sq() > 1e-4 {
                    ui.painter().line_segment([cur, start], stroke);
                }
                cur = start;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::ID;
    use crate::VectorcraftApp;
    use crate::i18n::{Lang, tr, trn};

    #[test]
    fn icons_panel_and_illustrator_2027_menus_and_dialogs_wire_end_to_end() {
        let row = crate::state::ICON_PANELS.iter().find(|(id, ..)| *id == ID).expect("Icons in ICON_PANELS");
        assert_eq!(row.1, "Icons");
        assert!(crate::icons::exists(row.2));

        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.run("file.new", json!({"width": 600, "height": 400})).unwrap();
        app.run("shape.rectangle", json!({"x": 40, "y": 40, "width": 120, "height": 80})).unwrap();

        let entries = crate::menus::menu_entries(&app);
        assert!(entries.iter().any(|e| e.label == "Icons" && e.params.get("panel").and_then(|v| v.as_str()) == Some(ID)));
        for cmd in [
            "document.preflight",
            "object.turntable",
            "icon.generateSheet",
            "object.generativeShapeFill",
            "type.reversePathDirection",
        ] {
            assert!(entries.iter().any(|e| e.command.as_deref() == Some(cmd)), "missing menu entry for {cmd}");
        }

        crate::menus::invoke(&mut app, "object.turntable", json!({}));
        assert_eq!(app.ui.dialog.as_ref().map(|d| d.kind.as_str()), Some(crate::dialogs::turntable::KIND));

        crate::menus::invoke(&mut app, "icon.generateSheet", json!({}));
        assert_eq!(app.ui.dialog.as_ref().map(|d| d.kind.as_str()), Some(crate::dialogs::icon_sheet::KIND));

        crate::menus::invoke(&mut app, "document.preflight", json!({}));
        assert_eq!(app.ui.dialog.as_ref().map(|d| d.kind.as_str()), Some(crate::dialogs::preflight::KIND));

        crate::menus::invoke(&mut app, "object.generativeShapeFill", json!({}));
        assert_eq!(app.ui.dialog.as_ref().map(|d| d.kind.as_str()), Some(crate::dialogs::generative_shape_fill::KIND));

        let ko = Lang::from_code("ko").unwrap();
        assert_eq!(tr(ko, "Generate Icon Sheet"), "아이콘 시트 생성");
        assert_eq!(tr(ko, "Turntable"), "턴테이블");
        assert_eq!(tr(ko, "Review Before Export"), "내보내기 전 프리플라이트");
        assert_eq!(tr(ko, "Generative Shape Fill"), "생성형 모양 채우기");
        assert_eq!(trn(ko, 3, "{n} artboard selected", "{n} artboards selected"), "3개 대지 선택됨");
    }
}
