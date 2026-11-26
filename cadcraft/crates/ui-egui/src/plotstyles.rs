//! Plot Style Manager (STYLESMANAGER): the plot style tables the drawing can use, binary/text
//! `.ctb` and `.stb` file import and export, a table's styles in an editable grid, and the current
//! layout's table and "Display plot styles".
//!
//! The dialog reads the drawing and changes it only through STYLESMANAGER and PAGESETUP
//! (`app.run`), so every edit is undoable and scriptable.

use cadcraft_doc::{EndStyle, FillStyle, JoinStyle, PlotStyle, PlotStyleTable};
use egui::{RichText, vec2};
use serde_json::{Value, json};

use crate::theme::Tokens;
use crate::{CadApp, Pick};

/// Open the dialog for a typed or menu-invoked STYLESMANAGER (no parameters).
pub fn route(app: &mut CadApp, id: &str, params: &Value) -> Option<Result<Value, String>> {
    if !params.is_null() || id != "stylesmanager" {
        return None;
    }
    app.ui.dialog = Some("stylesmanager".into());
    Some(Ok(Value::Null))
}

pub(crate) fn select_table(ctx: &egui::Context, name: &str) {
    if !cadcraft_doc::no_plot_style_table(name) {
        set_temp(ctx, "pstyle_sel", name.to_string());
    }
}

fn temp<T: Clone + Send + Sync + 'static>(ctx: &egui::Context, id: &str) -> Option<T> {
    ctx.data_mut(|d| d.get_temp::<T>(egui::Id::unique(id)))
}

fn set_temp<T: Clone + Send + Sync + 'static>(ctx: &egui::Context, id: &str, v: T) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(id), v));
}

/// Lineweights offered for a style, in millimetres.
const LINEWEIGHTS: [f64; 12] = [0.0, 0.09, 0.13, 0.18, 0.25, 0.3, 0.35, 0.5, 0.7, 1.0, 1.4, 2.11];

const END_STYLES: [(EndStyle, &str, &str); 5] = [
    (EndStyle::Object, "object", "Use object end style"),
    (EndStyle::Butt, "butt", "Butt"),
    (EndStyle::Square, "square", "Square"),
    (EndStyle::Round, "round", "Round"),
    (EndStyle::Diamond, "diamond", "Diamond"),
];

const JOIN_STYLES: [(JoinStyle, &str, &str); 5] = [
    (JoinStyle::Object, "object", "Use object join style"),
    (JoinStyle::Miter, "miter", "Miter"),
    (JoinStyle::Bevel, "bevel", "Bevel"),
    (JoinStyle::Round, "round", "Round"),
    (JoinStyle::Diamond, "diamond", "Diamond"),
];

const FILL_STYLES: [(FillStyle, &str, &str); 10] = [
    (FillStyle::Object, "object", "Use object fill style"),
    (FillStyle::Solid, "solid", "Solid"),
    (FillStyle::Checkerboard, "checkerboard", "Checkerboard"),
    (FillStyle::Crosshatch, "crosshatch", "Crosshatch"),
    (FillStyle::Diamonds, "diamonds", "Diamonds"),
    (FillStyle::HorizontalBars, "horizontalBars", "Horizontal bars"),
    (FillStyle::SlantLeft, "slantLeft", "Slant left"),
    (FillStyle::SlantRight, "slantRight", "Slant right"),
    (FillStyle::SquareDots, "squareDots", "Square dots"),
    (FillStyle::VerticalBars, "verticalBars", "Vertical bars"),
];

/// A command to run after the dialog is drawn.
type Action = Option<(&'static str, Value)>;

#[derive(Clone, Default)]
struct FileBar {
    mode: Option<String>,
    path: String,
}

#[derive(Default)]
struct WindowResult {
    action: Action,
    pick_load: bool,
    pick_save: Option<String>,
}

pub fn dialog(app: &mut CadApp, ctx: &egui::Context, open: &mut bool) {
    let out = window(app, ctx, open);
    if let Some((cmd, p)) = out.action {
        let _ = app.run(cmd, p);
    }
    if out.pick_load {
        let ctx_clone = ctx.clone();
        let _ = app.pick_then(Pick::Open, move |app, path| {
            let res = app.run("stylesmanager", json!({ "action": "load", "path": path }))?;
            if let Some(loaded) = res.get("name").and_then(Value::as_str) {
                set_temp(&ctx_clone, "pstyle_sel", loaded.to_string());
            }
            Ok(res)
        });
    }
    if let Some(table_name) = out.pick_save {
        let suggested = if table_name.to_ascii_lowercase().ends_with(".ctb") || table_name.to_ascii_lowercase().ends_with(".stb") {
            table_name.clone()
        } else {
            format!("{table_name}.ctb")
        };
        let _ = app.pick_then(Pick::Save(suggested), move |app, path| {
            app.run("stylesmanager", json!({ "action": "save", "name": table_name, "path": path }))
        });
    }
}

/// Plot Style Manager window; returns the command (`stylesmanager` or `pagesetup`) and file picks.
fn window(app: &mut CadApp, ctx: &egui::Context, open: &mut bool) -> WindowResult {
    let t = Tokens::get();
    let title = crate::tl!("Plot Style Manager");
    let has_open_picker = app.services.pick_open.is_some();
    let has_save_picker = app.services.pick_save.is_some();
    let Ok(d) = app.session.doc() else {
        egui::Window::new(title).open(open).collapsible(false).resizable(false).show(ctx, |ui| {
            ui.label(crate::tl!("Open a drawing first."));
        });
        return WindowResult::default();
    };
    let names = cadcraft_doc::plot_style_table_names(d);
    let space = app.session.layout_space();
    let layout = space.layout().and_then(|n| d.layout(n).map(|l| (n.to_string(), l.page.plot_style_table.clone(), l.page.show_plot_styles)));
    let mut sel = temp::<String>(ctx, "pstyle_sel")
        .filter(|s| names.iter().any(|n| n.eq_ignore_ascii_case(s)))
        .or_else(|| layout.as_ref().map(|l| l.1.clone()).filter(|n| names.iter().any(|x| x.eq_ignore_ascii_case(n))))
        .or_else(|| names.first().cloned())
        .unwrap_or_default();
    let table = cadcraft_doc::plot_style_table(d, &sel);
    let in_drawing = d.plot_style_tables.iter().any(|x| x.name.eq_ignore_ascii_case(&sel));
    let linetypes: Vec<String> =
        d.linetypes.iter().map(|l| l.name.clone()).filter(|n| !n.eq_ignore_ascii_case("ByLayer") && !n.eq_ignore_ascii_case("ByBlock")).collect();
    let mut new_style = temp::<String>(ctx, "pstyle_new").unwrap_or_default();
    let mut file_bar = temp::<FileBar>(ctx, "pstyle_file_bar").unwrap_or_default();
    let mut res = WindowResult::default();

    egui::Window::new(title).id(egui::Id::unique("Plot Style Manager")).open(open).default_size(vec2(860.0, 480.0)).resizable(true).show(ctx, |ui| {
        draw_toolbar(ui, &names, &mut sel, in_drawing, has_open_picker, has_save_picker, &mut file_bar, &mut res);
        draw_file_bar(ui, &sel, &mut file_bar, &mut res);
        if let Some((name, current, show)) = &layout {
            draw_layout_bar(ui, &t, name, current, *show, &sel, &mut res.action);
        }
        ui.separator();
        let Some(table) = &table else {
            ui.label(crate::tl!("No plot style tables."));
            return;
        };
        let named = !table.is_color_dependent();
        ui.label(RichText::new(&table.description).color(t.text_dim));
        if !in_drawing {
            ui.label(RichText::new(crate::tl!("A built-in table: editing it keeps an edited copy in the drawing.")).color(t.text_faint));
        }
        draw_styles_grid(ui, table, &linetypes, named, &mut res.action);
        if named {
            ui.separator();
            ui.horizontal(|ui| {
                let label = ui.label(crate::tl!("New style"));
                ui.add(egui::TextEdit::singleline(&mut new_style).desired_width(160.0)).labelled_by(label.id);
                if ui.add_enabled(!new_style.trim().is_empty(), egui::Button::new(crate::tl!("Add"))).clicked() {
                    res.action = Some(("stylesmanager", json!({ "action": "set", "name": table.name, "style": { "name": new_style.trim() } })));
                    new_style.clear();
                }
            });
        }
    });
    set_temp(ctx, "pstyle_sel", sel);
    set_temp(ctx, "pstyle_new", new_style);
    set_temp(ctx, "pstyle_file_bar", file_bar);
    res
}

#[allow(clippy::too_many_arguments)]
fn draw_toolbar(
    ui: &mut egui::Ui,
    names: &[String],
    sel: &mut String,
    in_drawing: bool,
    has_open_picker: bool,
    has_save_picker: bool,
    file_bar: &mut FileBar,
    res: &mut WindowResult,
) {
    ui.horizontal_wrapped(|ui| {
        ui.label(crate::tl!("Plot style table"));
        craft_ui::ComboBox::from_id_salt("pstyle_table").selected_text(sel.as_str()).width(190.0).show_ui(ui, |ui| {
            for n in names {
                if ui.selectable_label(n.eq_ignore_ascii_case(sel), n).clicked() {
                    *sel = n.clone();
                }
            }
        });
        if ui.button(crate::tl!("Load...")).on_hover_text(crate::tl!("Import an AutoCAD .ctb or .stb plot style table file")).clicked() {
            if has_open_picker {
                res.pick_load = true;
            } else {
                file_bar.mode = Some("load".into());
            }
        }
        if ui
            .add_enabled(!sel.is_empty(), egui::Button::new(crate::tl!("Save...")))
            .on_hover_text(crate::tl!("Export this plot style table as a binary .ctb or .stb file"))
            .clicked()
        {
            if has_save_picker {
                res.pick_save = Some(sel.clone());
            } else {
                file_bar.mode = Some("save".into());
                if file_bar.path.trim().is_empty() {
                    file_bar.path = sel.clone();
                }
            }
        }
        if ui.button(crate::tl!("Copy")).on_hover_text(crate::tl!("Keep an editable copy of this table in the drawing")).clicked() {
            let stem = sel.rsplit_once('.').map(|(s, _)| s).unwrap_or(sel);
            let copy = format!("{stem}-copy");
            res.action = Some(("stylesmanager", json!({ "action": "new", "name": copy, "from": sel })));
        }
        if ui.button(crate::tl!("New CTB")).clicked() {
            res.action = Some(("stylesmanager", json!({ "action": "new", "name": "new.ctb" })));
        }
        if ui.button(crate::tl!("New STB")).clicked() {
            res.action = Some(("stylesmanager", json!({ "action": "new", "name": "new.stb" })));
        }
        if ui.add_enabled(in_drawing, egui::Button::new(crate::tl!("Delete"))).clicked() {
            res.action = Some(("stylesmanager", json!({ "action": "delete", "name": sel })));
        }
    });
}

fn draw_file_bar(ui: &mut egui::Ui, sel: &str, file_bar: &mut FileBar, res: &mut WindowResult) {
    let Some(mode) = file_bar.mode.clone() else { return };
    ui.horizontal(|ui| {
        let is_load = mode == "load";
        let label = ui.label(if is_load { crate::tl!("Import file (.ctb/.stb)") } else { crate::tl!("Export file (.ctb/.stb)") });
        ui.add(egui::TextEdit::singleline(&mut file_bar.path).desired_width(280.0)).labelled_by(label.id);
        let can_run = !file_bar.path.trim().is_empty();
        if ui.add_enabled(can_run, egui::Button::new(if is_load { crate::tl!("Import") } else { crate::tl!("Export") })).clicked() {
            let trimmed = file_bar.path.trim().to_string();
            res.action = Some(if is_load {
                ("stylesmanager", json!({ "action": "load", "path": trimmed }))
            } else {
                ("stylesmanager", json!({ "action": "save", "name": sel, "path": trimmed }))
            });
            file_bar.mode = None;
        }
        if ui.button(crate::tl!("Cancel")).clicked() {
            file_bar.mode = None;
        }
    });
}

fn draw_layout_bar(ui: &mut egui::Ui, t: &Tokens, name: &str, current: &str, show: bool, sel: &str, action: &mut Action) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(crate::tf!(
                "Layout {name}: {table}",
                name = name,
                table = if current.is_empty() { crate::i18n::t("None") } else { current }
            ))
            .color(t.text_dim),
        );
        if ui.add_enabled(!current.eq_ignore_ascii_case(sel), egui::Button::new(crate::tl!("Use for this layout"))).clicked() {
            *action = Some(("pagesetup", json!({ "layout": name, "plotStyleTable": sel })));
        }
        let mut s = show;
        if ui.checkbox(&mut s, crate::tl!("Display plot styles")).changed() {
            *action = Some(("pagesetup", json!({ "layout": name, "displayPlotStyles": s })));
        }
    });
}

fn draw_styles_grid(ui: &mut egui::Ui, table: &PlotStyleTable, linetypes: &[String], named: bool, action: &mut Action) {
    egui::ScrollArea::both().id_salt("pstyle_rows").max_height(300.0).auto_shrink([false, true]).show(ui, |ui| {
        egui::Grid::new("pstyle_grid").striped(true).num_columns(if named { 10 } else { 9 }).spacing(vec2(10.0, 4.0)).show(ui, |ui| {
            for h in [
                crate::tl!("Plot style"),
                crate::tl!("Color"),
                crate::tl!("Grayscale"),
                crate::tl!("Screening"),
                crate::tl!("Linetype"),
                crate::tl!("Lineweight"),
                crate::tl!("End style"),
                crate::tl!("Join style"),
                crate::tl!("Fill style"),
            ] {
                ui.label(RichText::new(h).strong());
            }
            if named {
                ui.label("");
            }
            ui.end_row();
            for st in &table.styles {
                draw_style_row(ui, &table.name, st, linetypes, named, action);
            }
        });
    });
}

fn draw_style_row(ui: &mut egui::Ui, table_name: &str, st: &PlotStyle, linetypes: &[String], named: bool, action: &mut Action) {
    let set = |k: &str, v: Value| Some(("stylesmanager", json!({ "action": "set", "name": table_name, "style": { "name": st.name, k: v } })));
    ui.label(&st.name);
    ui.horizontal(|ui| {
        let mut object = st.color.is_none();
        if ui.checkbox(&mut object, crate::tl!("Object")).changed() {
            *action = set("color", if object { Value::Null } else { json!("#000000") });
        }
        if let Some(c) = st.color {
            let mut rgb = [c.0, c.1, c.2];
            if ui.color_edit_button_srgb(&mut rgb).changed() {
                *action = set("color", json!(cadcraft_color::Rgb(rgb[0], rgb[1], rgb[2]).hex()));
            }
        }
    });
    let mut gray = st.grayscale;
    if ui.checkbox(&mut gray, "").changed() {
        *action = set("grayscale", json!(gray));
    }
    let mut screen = st.screening.min(100);
    if ui.add(egui::DragValue::new(&mut screen).range(0..=100).suffix(" %")).changed() {
        *action = set("screening", json!(screen));
    }
    draw_linetype_combo(ui, table_name, st, linetypes, action);
    draw_lineweight_combo(ui, table_name, st, action);
    draw_pen_combos(ui, table_name, st, action);
    if named {
        let normal = st.name.eq_ignore_ascii_case(cadcraft_doc::NORMAL_STYLE);
        if ui.add_enabled(!normal, egui::Button::new(crate::tl!("Remove"))).clicked() {
            *action = Some(("stylesmanager", json!({ "action": "set", "name": table_name, "removeStyle": st.name })));
        }
    }
    ui.end_row();
}

fn draw_linetype_combo(ui: &mut egui::Ui, table_name: &str, st: &PlotStyle, linetypes: &[String], action: &mut Action) {
    let lt = st.linetype.clone().unwrap_or_else(|| crate::tl!("Use object linetype").to_string());
    craft_ui::ComboBox::from_id_salt(("pstyle_lt", &st.name)).selected_text(lt).width(125.0).show_ui(ui, |ui| {
        if ui.selectable_label(st.linetype.is_none(), crate::tl!("Use object linetype")).clicked() {
            *action = Some(("stylesmanager", json!({ "action": "set", "name": table_name, "style": { "name": st.name, "linetype": Value::Null } })));
        }
        for n in linetypes {
            if ui.selectable_label(st.linetype.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(n)), n).clicked() {
                *action = Some(("stylesmanager", json!({ "action": "set", "name": table_name, "style": { "name": st.name, "linetype": n } })));
            }
        }
    });
}

fn draw_lineweight_combo(ui: &mut egui::Ui, table_name: &str, st: &PlotStyle, action: &mut Action) {
    let lw = st.lineweight.map(|w| format!("{:.2} mm", f64::from(w) / 100.0)).unwrap_or_else(|| crate::tl!("Use object lineweight").to_string());
    craft_ui::ComboBox::from_id_salt(("pstyle_lw", &st.name)).selected_text(lw).width(120.0).show_ui(ui, |ui| {
        if ui.selectable_label(st.lineweight.is_none(), crate::tl!("Use object lineweight")).clicked() {
            *action = Some(("stylesmanager", json!({ "action": "set", "name": table_name, "style": { "name": st.name, "lineweight": Value::Null } })));
        }
        for mm in LINEWEIGHTS {
            let hundredths = (mm * 100.0).round() as u16;
            if ui.selectable_label(st.lineweight == Some(hundredths), format!("{mm:.2} mm")).clicked() {
                *action = Some(("stylesmanager", json!({ "action": "set", "name": table_name, "style": { "name": st.name, "lineweight": mm } })));
            }
        }
    });
}

fn draw_pen_combos(ui: &mut egui::Ui, table_name: &str, st: &PlotStyle, action: &mut Action) {
    let cur_end = END_STYLES.iter().find(|(v, _, _)| *v == st.end).map_or("Object", |(_, _, l)| crate::i18n::t(l));
    craft_ui::ComboBox::from_id_salt(("pstyle_end", &st.name)).selected_text(cur_end).width(105.0).show_ui(ui, |ui| {
        for (val, wire, label) in END_STYLES {
            if ui.selectable_label(st.end == val, crate::i18n::t(label)).clicked() && st.end != val {
                *action = Some(("stylesmanager", json!({ "action": "set", "name": table_name, "style": { "name": st.name, "end": wire } })));
            }
        }
    });
    let cur_join = JOIN_STYLES.iter().find(|(v, _, _)| *v == st.join).map_or("Object", |(_, _, l)| crate::i18n::t(l));
    craft_ui::ComboBox::from_id_salt(("pstyle_join", &st.name)).selected_text(cur_join).width(105.0).show_ui(ui, |ui| {
        for (val, wire, label) in JOIN_STYLES {
            if ui.selectable_label(st.join == val, crate::i18n::t(label)).clicked() && st.join != val {
                *action = Some(("stylesmanager", json!({ "action": "set", "name": table_name, "style": { "name": st.name, "join": wire } })));
            }
        }
    });
    let cur_fill = FILL_STYLES.iter().find(|(v, _, _)| *v == st.fill).map_or("Object", |(_, _, l)| crate::i18n::t(l));
    craft_ui::ComboBox::from_id_salt(("pstyle_fill", &st.name)).selected_text(cur_fill).width(115.0).show_ui(ui, |ui| {
        for (val, wire, label) in FILL_STYLES {
            if ui.selectable_label(st.fill == val, crate::i18n::t(label)).clicked() && st.fill != val {
                *action = Some(("stylesmanager", json!({ "action": "set", "name": table_name, "style": { "name": st.name, "fill": wire } })));
            }
        }
    });
}
