//! Block Authoring Palette / Dynamic Block Parameters dialog (`ui.dialog.bauthor`) inside the
//! Block Editor (`BEDIT`). Lets users add, inspect, and delete Dynamic Block Parameters, Actions,
//! and Visibility States interactively.

use cadcraft_engine::doc::dynblock::{ActionKind, DynamicBlock, Parameter, ParameterKind, dynamic_block};
use egui::{RichText, Stroke, vec2};
use serde_json::{Value, json};

use crate::CadApp;
use crate::theme::Tokens;

const MODEL_GRAPH: &str = "*Model_Space";

const PARAM_TYPES: [(&str, &str); 8] = [
    ("linear", "Linear"),
    ("point", "Point"),
    ("polar", "Polar"),
    ("xy", "XY"),
    ("rotation", "Rotation"),
    ("flip", "Flip"),
    ("visibility", "Visibility"),
    ("lookup", "Lookup"),
];

const ACTION_TYPES: [(&str, &str); 6] =
    [("stretch", "Stretch"), ("move", "Move"), ("scale", "Scale"), ("rotate", "Rotate"), ("flip", "Flip"), ("lookup", "Lookup")];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AuthorTab {
    #[default]
    Parameters,
    Actions,
    Visibility,
}

/// Persistent UI state of the Dynamic Block Parameters authoring window.
pub struct State {
    pub tab: AuthorTab,
    pub param_type: String,
    pub param_label: String,
    pub base_x: f64,
    pub base_y: f64,
    pub end_x: f64,
    pub end_y: f64,
    pub from_midpoint: bool,
    pub action_type: String,
    pub action_name: String,
    pub target_param: i32,
    pub frame_x1: f64,
    pub frame_y1: f64,
    pub frame_x2: f64,
    pub frame_y2: f64,
    pub new_vis_state: String,
    pub error: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        State {
            tab: AuthorTab::Parameters,
            param_type: "linear".into(),
            param_label: String::new(),
            base_x: 0.0,
            base_y: 0.0,
            end_x: 10.0,
            end_y: 0.0,
            from_midpoint: false,
            action_type: "stretch".into(),
            action_name: String::new(),
            target_param: 0,
            frame_x1: -2.0,
            frame_y1: -2.0,
            frame_x2: 12.0,
            frame_y2: 12.0,
            new_vis_state: String::new(),
            error: None,
        }
    }
}

/// Route `bauthorpalette` / `bauthorpaletteclose` UI commands.
pub fn route(app: &mut CadApp, id: &str, params: &Value) -> Option<Result<Value, String>> {
    if !params.is_null() {
        return None;
    }
    match id {
        "bauthorpalette" => {
            app.ui.dialog = Some("bauthor".into());
            Some(Ok(Value::Null))
        }
        "bauthorpaletteclose" => {
            if app.ui.dialog.as_deref() == Some("bauthor") {
                app.ui.dialog = None;
            }
            Some(Ok(Value::Null))
        }
        _ => None,
    }
}

/// Show the Dynamic Block Parameters authoring window.
pub fn dialog(app: &mut CadApp, ctx: &egui::Context, open: &mut bool) {
    let in_bedit = app.session.state().ok().and_then(|st| st.block_editor().map(str::to_string));
    let model = app.session.state().ok().and_then(|st| dynamic_block(&st.doc, MODEL_GRAPH));
    let sel_count = app.session.selection().len();
    let t = Tokens::get();

    egui::Window::new(crate::tl!("Dynamic Block Parameters"))
        .id(egui::Id::unique("Dynamic Block Parameters"))
        .open(open)
        .resizable(true)
        .default_width(500.0)
        .show(ctx, |ui| {
            let Some(block_name) = in_bedit else {
                ui.label(crate::tl!("Open a block in the Block Editor (BEDIT) to author dynamic parameters and actions."));
                ui.add_space(6.0);
                if ui.button(crate::tl!("Block Editor")).clicked() {
                    let _ = crate::menus::run_ui_command(app, "bedit", &Value::Null);
                }
                return;
            };

            ui.horizontal(|ui| {
                ui.label(RichText::new(crate::tl!("Block:")).color(t.text_dim));
                ui.label(RichText::new(&block_name).strong());
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.selectable_value(&mut app.bauthor.tab, AuthorTab::Parameters, crate::tl!("Parameters"));
                ui.selectable_value(&mut app.bauthor.tab, AuthorTab::Actions, crate::tl!("Actions"));
                ui.selectable_value(&mut app.bauthor.tab, AuthorTab::Visibility, crate::tl!("Visibility States"));
            });
            ui.separator();

            match app.bauthor.tab {
                AuthorTab::Parameters => draw_parameters_tab(app, ui, model.as_ref()),
                AuthorTab::Actions => draw_actions_tab(app, ui, model.as_ref(), sel_count),
                AuthorTab::Visibility => draw_visibility_tab(app, ui, model.as_ref(), sel_count),
            }

            if let Some(err) = &app.bauthor.error {
                ui.add_space(4.0);
                ui.colored_label(t.warn, err);
            }
        });
}

fn param_kind_name(p: &Parameter) -> &'static str {
    match &p.kind {
        ParameterKind::Linear { .. } => "Linear",
        ParameterKind::Point { .. } => "Point",
        ParameterKind::Polar { .. } => "Polar",
        ParameterKind::Xy { .. } => "XY",
        ParameterKind::Rotation { .. } => "Rotation",
        ParameterKind::Flip { .. } => "Flip",
        ParameterKind::Visibility { .. } => "Visibility",
        ParameterKind::Lookup { .. } => "Lookup",
        ParameterKind::Alignment { .. } => "Alignment",
        ParameterKind::BasePoint { .. } => "Base Point",
        ParameterKind::Other { .. } => "Other",
    }
}

fn draw_parameters_tab(app: &mut CadApp, ui: &mut egui::Ui, model: Option<&DynamicBlock>) {
    let t = Tokens::get();
    let mut delete_id: Option<i32> = None;
    let params = model.map_or(&[][..], |m| m.parameters.as_slice());

    egui::Frame::new().fill(t.field).stroke(Stroke::new(1.0, t.border)).corner_radius(4.0).inner_margin(6.0).show(ui, |ui| {
        if params.is_empty() {
            ui.label(RichText::new(crate::tl!("No dynamic parameters yet.")).color(t.text_dim));
        } else {
            egui::ScrollArea::vertical().id_salt("bauthor-params").max_height(150.0).show(ui, |ui| {
                egui::Grid::new("bauthor-params-grid").striped(true).num_columns(4).spacing([10.0, 4.0]).show(ui, |ui| {
                    for p in params {
                        ui.label(format!("#{}", p.node));
                        ui.label(crate::i18n::t(param_kind_name(p)));
                        ui.label(RichText::new(p.display_label()).strong());
                        if ui.small_button(crate::tl!("Delete")).clicked() {
                            delete_id = Some(p.node);
                        }
                        ui.end_row();
                    }
                });
            });
        }
    });

    if let Some(node) = delete_id {
        run_author_cmd(app, "bparameter", json!({ "action": "delete", "node": node }));
    }

    ui.add_space(6.0);
    ui.strong(crate::tl!("Add Parameter"));
    ui.horizontal(|ui| {
        let type_lbl = ui.label(crate::tl!("Type"));
        let selected = PARAM_TYPES
            .iter()
            .find(|(k, _)| *k == app.bauthor.param_type)
            .map(|(_, v)| crate::i18n::t(v).to_string())
            .unwrap_or_else(|| app.bauthor.param_type.clone());
        let cb = craft_ui::ComboBox::from_id_salt("bauthor-ptype").selected_text(selected).show_ui(ui, |ui| {
            for (k, label) in PARAM_TYPES {
                ui.selectable_value(&mut app.bauthor.param_type, k.to_string(), crate::i18n::t(label));
            }
        });
        cb.response.labelled_by(type_lbl.id);

        let name_lbl = ui.label(crate::tl!("Label"));
        ui.add(egui::TextEdit::singleline(&mut app.bauthor.param_label).desired_width(140.0)).labelled_by(name_lbl.id);
    });

    let one_pt = matches!(app.bauthor.param_type.as_str(), "point" | "visibility" | "lookup");
    ui.horizontal(|ui| {
        ui.label(if one_pt { crate::tl!("Location") } else { crate::tl!("Start point") });
        ui.add(egui::DragValue::new(&mut app.bauthor.base_x).speed(0.5).prefix("X: "));
        ui.add(egui::DragValue::new(&mut app.bauthor.base_y).speed(0.5).prefix("Y: "));
        if !one_pt {
            ui.label(crate::tl!("End point"));
            ui.add(egui::DragValue::new(&mut app.bauthor.end_x).speed(0.5).prefix("X: "));
            ui.add(egui::DragValue::new(&mut app.bauthor.end_y).speed(0.5).prefix("Y: "));
        }
    });
    if app.bauthor.param_type == "linear" {
        ui.checkbox(&mut app.bauthor.from_midpoint, crate::tl!("Midpoint base"));
    }

    ui.horizontal(|ui| {
        if ui.button(crate::tl!("Add Parameter")).clicked() {
            let mut p = json!({
                "type": app.bauthor.param_type,
                "fromMidpoint": app.bauthor.from_midpoint,
            });
            if !app.bauthor.param_label.trim().is_empty()
                && let Some(o) = p.as_object_mut()
            {
                o.insert("label".into(), json!(app.bauthor.param_label.trim()));
            }
            if let Some(o) = p.as_object_mut() {
                if one_pt {
                    o.insert("at".into(), json!([app.bauthor.base_x, app.bauthor.base_y]));
                } else {
                    o.insert("base".into(), json!([app.bauthor.base_x, app.bauthor.base_y]));
                    o.insert("end".into(), json!([app.bauthor.end_x, app.bauthor.end_y]));
                }
            }
            if run_author_cmd(app, "bparameter", p) {
                app.bauthor.param_label.clear();
            }
        }
        if ui.button(crate::tl!("Pick on Canvas")).clicked() {
            app.start("bparameter");
        }
    });
}

fn action_kind_name(k: &ActionKind) -> &'static str {
    match k {
        ActionKind::Stretch { .. } => "Stretch",
        ActionKind::Move { .. } => "Move",
        ActionKind::Scale => "Scale",
        ActionKind::Rotate => "Rotate",
        ActionKind::Flip => "Flip",
        ActionKind::Lookup { .. } => "Lookup",
        ActionKind::Array => "Array",
        ActionKind::PolarStretch => "Polar Stretch",
        ActionKind::Other { .. } => "Other",
    }
}

fn draw_actions_tab(app: &mut CadApp, ui: &mut egui::Ui, model: Option<&DynamicBlock>, sel_count: usize) {
    let t = Tokens::get();
    let mut delete_id: Option<i32> = None;
    let params = model.map_or(&[][..], |m| m.parameters.as_slice());
    let actions = model.map_or(&[][..], |m| m.actions.as_slice());

    if app.bauthor.target_param == 0
        && let Some(first) = params.first()
    {
        app.bauthor.target_param = first.node;
    }

    egui::Frame::new().fill(t.field).stroke(Stroke::new(1.0, t.border)).corner_radius(4.0).inner_margin(6.0).show(ui, |ui| {
        if actions.is_empty() {
            ui.label(RichText::new(crate::tl!("No dynamic actions yet.")).color(t.text_dim));
        } else {
            egui::ScrollArea::vertical().id_salt("bauthor-actions").max_height(150.0).show(ui, |ui| {
                egui::Grid::new("bauthor-actions-grid").striped(true).num_columns(5).spacing([10.0, 4.0]).show(ui, |ui| {
                    for a in actions {
                        ui.label(format!("#{}", a.node));
                        ui.label(crate::i18n::t(action_kind_name(&a.kind)));
                        ui.label(RichText::new(&a.name).strong());
                        ui.label(format!("{} {}", a.selection.len(), crate::i18n::t("objects")));
                        if ui.small_button(crate::tl!("Delete")).clicked() {
                            delete_id = Some(a.node);
                        }
                        ui.end_row();
                    }
                });
            });
        }
    });

    if let Some(node) = delete_id {
        run_author_cmd(app, "baction", json!({ "action": "delete", "node": node }));
    }

    ui.add_space(6.0);
    ui.strong(crate::tl!("Add Action"));
    if params.is_empty() {
        ui.label(RichText::new(crate::tl!("Add a parameter first before creating an action.")).color(t.text_dim));
        return;
    }

    ui.horizontal(|ui| {
        let p_lbl = ui.label(crate::tl!("Parameter"));
        let current_param = params
            .iter()
            .find(|p| p.node == app.bauthor.target_param)
            .map(|p| format!("#{} {}", p.node, p.display_label()))
            .unwrap_or_else(|| format!("#{}", app.bauthor.target_param));
        let cb_p = craft_ui::ComboBox::from_id_salt("bauthor-target-p").selected_text(current_param).show_ui(ui, |ui| {
            for p in params {
                ui.selectable_value(&mut app.bauthor.target_param, p.node, format!("#{} {}", p.node, p.display_label()));
            }
        });
        cb_p.response.labelled_by(p_lbl.id);

        let a_lbl = ui.label(crate::tl!("Action"));
        let selected_act = ACTION_TYPES
            .iter()
            .find(|(k, _)| *k == app.bauthor.action_type)
            .map(|(_, v)| crate::i18n::t(v).to_string())
            .unwrap_or_else(|| app.bauthor.action_type.clone());
        let cb_a = craft_ui::ComboBox::from_id_salt("bauthor-atype").selected_text(selected_act).show_ui(ui, |ui| {
            for (k, label) in ACTION_TYPES {
                ui.selectable_value(&mut app.bauthor.action_type, k.to_string(), crate::i18n::t(label));
            }
        });
        cb_a.response.labelled_by(a_lbl.id);
    });

    if app.bauthor.action_type == "stretch" {
        ui.horizontal(|ui| {
            ui.label(crate::tl!("Stretch frame"));
            ui.add(egui::DragValue::new(&mut app.bauthor.frame_x1).speed(0.5).prefix("X1: "));
            ui.add(egui::DragValue::new(&mut app.bauthor.frame_y1).speed(0.5).prefix("Y1: "));
            ui.add(egui::DragValue::new(&mut app.bauthor.frame_x2).speed(0.5).prefix("X2: "));
            ui.add(egui::DragValue::new(&mut app.bauthor.frame_y2).speed(0.5).prefix("Y2: "));
        });
    }

    ui.horizontal(|ui| {
        ui.label(format!("{sel_count} {}", crate::i18n::t("selected objects")));
        if ui.button(crate::tl!("Add Action")).clicked() {
            let mut p = json!({
                "type": app.bauthor.action_type,
                "parameter": app.bauthor.target_param,
            });
            if !app.bauthor.action_name.trim().is_empty()
                && let Some(o) = p.as_object_mut()
            {
                o.insert("name".into(), json!(app.bauthor.action_name.trim()));
            }
            if app.bauthor.action_type == "stretch"
                && let Some(o) = p.as_object_mut()
            {
                o.insert("frame".into(), json!([[app.bauthor.frame_x1, app.bauthor.frame_y1], [app.bauthor.frame_x2, app.bauthor.frame_y2]]));
            }
            if run_author_cmd(app, "baction", p) {
                app.bauthor.action_name.clear();
            }
        }
        if ui.button(crate::tl!("Pick on Canvas")).clicked() {
            app.start("baction");
        }
    });
}

fn draw_visibility_tab(app: &mut CadApp, ui: &mut egui::Ui, model: Option<&DynamicBlock>, sel_count: usize) {
    let t = Tokens::get();
    let states = model
        .and_then(|m| {
            m.parameters.iter().find_map(|p| match &p.kind {
                ParameterKind::Visibility { states, .. } => Some(states.as_slice()),
                _ => None,
            })
        })
        .unwrap_or(&[]);

    let mut pending_op: Option<Value> = None;
    egui::Frame::new().fill(t.field).stroke(Stroke::new(1.0, t.border)).corner_radius(4.0).inner_margin(6.0).show(ui, |ui| {
        if states.is_empty() {
            ui.label(RichText::new(crate::tl!("No visibility states yet. Add one below.")).color(t.text_dim));
        } else {
            egui::ScrollArea::vertical().id_salt("bauthor-vstates").max_height(160.0).show(ui, |ui| {
                for (idx, st) in states.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let badge = if idx == 0 { format!("★ {}", st.name) } else { st.name.clone() };
                        ui.allocate_ui(vec2(150.0, 20.0), |ui| {
                            ui.label(RichText::new(badge).strong());
                        });
                        ui.label(format!("{} {}", st.visible.len(), crate::i18n::t("visible")));
                        if idx > 0 && ui.small_button(crate::tl!("Set Default")).clicked() {
                            pending_op = Some(json!({ "op": "default", "name": st.name }));
                        }
                        if ui.add_enabled(sel_count > 0, egui::Button::new(crate::tl!("Show Selected")).small()).clicked() {
                            pending_op = Some(json!({ "op": "show", "state": st.name }));
                        }
                        if ui.add_enabled(sel_count > 0, egui::Button::new(crate::tl!("Hide Selected")).small()).clicked() {
                            pending_op = Some(json!({ "op": "hide", "state": st.name }));
                        }
                        if states.len() > 1 && ui.small_button(crate::tl!("Delete")).clicked() {
                            pending_op = Some(json!({ "op": "delete", "name": st.name }));
                        }
                    });
                }
            });
        }
    });

    if let Some(op) = pending_op {
        run_author_cmd(app, "bvstate", op);
    }

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let lbl = ui.label(crate::tl!("New state"));
        ui.add(egui::TextEdit::singleline(&mut app.bauthor.new_vis_state).desired_width(180.0)).labelled_by(lbl.id);
        if ui.button(crate::tl!("Add State")).clicked() {
            let name = app.bauthor.new_vis_state.trim().to_string();
            if run_author_cmd(app, "bvstate", json!({ "op": "new", "name": name })) {
                app.bauthor.new_vis_state.clear();
            }
        }
    });
}

fn run_author_cmd(app: &mut CadApp, cmd: &str, params: Value) -> bool {
    match app.run(cmd, params) {
        Ok(_) => {
            app.bauthor.error = None;
            true
        }
        Err(e) => {
            app.bauthor.error = Some(e);
            false
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::Services;
    use cadcraft_engine::Session;

    #[test]
    fn bauthor_and_stylesmanager_dialogs_open_and_render_in_block_editor() {
        let mut app = CadApp::new(Session::new(), Services::default());
        app.run("line", json!({ "points": [[0, 0], [10, 0]] })).unwrap();
        let h = app.session.doc().unwrap().model.handles()[0].hex();
        app.run("block", json!({ "name": "Panel", "base": [0, 0], "handles": [h] })).unwrap();
        app.run("bedit", json!({ "name": "Panel" })).unwrap();
        app.run("bparameter", json!({ "type": "linear", "label": "Len", "base": [0, 0], "end": [10, 0] })).unwrap();
        app.run("bauthorpalette", Value::Null).unwrap();
        assert_eq!(app.ui.dialog.as_deref(), Some("bauthor"));

        let ctx = egui::Context::default();
        let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(1280.0, 800.0))), ..Default::default() };
        for tab in [AuthorTab::Parameters, AuthorTab::Actions, AuthorTab::Visibility] {
            app.bauthor.tab = tab;
            let mut out = ctx.run_ui(raw.clone(), |ui| app.ui(ui));
            out.textures_delta.clear();
        }
        app.run("bauthorpaletteclose", Value::Null).unwrap();
        assert_eq!(app.ui.dialog, None);
    }
}
