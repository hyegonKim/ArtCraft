//! 3D Solid & Clay Modeler window (`ui.dialog.solid3d`): interactive primitive creation,
//! smooth-clay CSG boolean combination (`smooth_k`), STL/OBJ mesh export, and `FLATSHOT` / `VIEWBASE`
//! orthographic 2D CAD projection.

use egui::vec2;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{CadApp, Pick};

const PRIMITIVE_KINDS: [(&str, &str); 6] =
    [("box", "Box"), ("sphere", "Sphere"), ("cylinder", "Cylinder"), ("cone", "Cone"), ("torus", "Torus"), ("capsule", "Capsule")];

const BOOLEAN_OPS: [(&str, &str); 3] = [("union", "Union"), ("subtract", "Subtract"), ("intersect", "Intersect")];

const PROJECTION_VIEWS: [(&str, &str); 4] = [("top", "Top"), ("front", "Front"), ("right", "Right"), ("isometric", "Isometric")];

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ModelerForm {
    prim_kind: String,
    size_x: f64,
    size_y: f64,
    size_z: f64,
    radius: f64,
    minor_radius: f64,
    top_radius: f64,
    pos_x: f64,
    pos_y: f64,
    pos_z: f64,
    bool_op: String,
    smooth_k: f64,
    left_id: u64,
    right_id: u64,
    keep_inputs: bool,
    resolution: u64,
    stl_binary: bool,
    proj_view: String,
    proj_x: f64,
    proj_y: f64,
    proj_scale: f64,
    show_hidden: bool,
}

impl Default for ModelerForm {
    fn default() -> Self {
        Self {
            prim_kind: "box".into(),
            size_x: 40.0,
            size_y: 30.0,
            size_z: 20.0,
            radius: 12.0,
            minor_radius: 4.0,
            top_radius: 0.0,
            pos_x: 0.0,
            pos_y: 0.0,
            pos_z: 0.0,
            bool_op: "union".into(),
            smooth_k: 2.0,
            left_id: 0,
            right_id: 0,
            keep_inputs: false,
            resolution: 36,
            stl_binary: true,
            proj_view: "top".into(),
            proj_x: 0.0,
            proj_y: 0.0,
            proj_scale: 1.0,
            show_hidden: false,
        }
    }
}

pub fn dialog(app: &mut CadApp, ctx: &egui::Context, open: &mut bool) {
    let state_id = egui::Id::unique("solid3d_modeler_form");
    let mut form = ctx.data_mut(|d| d.get_temp::<ModelerForm>(state_id)).unwrap_or_default();
    let solids: Vec<(u64, String)> =
        app.session.doc().map(|d| d.solids3d.iter().map(|s| (s.id, format!("#{} {}", s.id, s.name))).collect()).unwrap_or_default();
    sync_boolean_ids(&mut form, &solids);

    let mut action = DialogAction::None;
    egui::Window::new(crate::tl!("3D Solid Modeler")).id(egui::Id::unique("3D Solid Modeler")).open(open).default_width(380.0).resizable(true).show(
        ctx,
        |ui| {
            primitive_section(ui, &mut form, &mut action);
            ui.separator();
            solids_list_section(ui, &solids, &mut action);
            ui.separator();
            boolean_section(ui, &mut form, &solids, &mut action);
            ui.separator();
            projection_and_export_section(ui, &mut form, &solids, &mut action);
        },
    );

    apply_dialog_action(app, &form, action);
    ctx.data_mut(|d| d.insert_temp(state_id, form));
}

enum DialogAction {
    None,
    AddPrimitive,
    ApplyBoolean,
    DeleteSolid(u64),
    ClearSolids,
    ProjectFlatshot,
    ProjectViewbase,
    ExportStl,
    ExportObj,
}

fn sync_boolean_ids(form: &mut ModelerForm, solids: &[(u64, String)]) {
    if solids.is_empty() {
        form.left_id = 0;
        form.right_id = 0;
        return;
    }
    if !solids.iter().any(|(id, _)| *id == form.left_id) {
        form.left_id = solids[0].0;
    }
    if !solids.iter().any(|(id, _)| *id == form.right_id) || (solids.len() >= 2 && form.right_id == form.left_id) {
        form.right_id = solids.iter().rev().find(|(id, _)| *id != form.left_id).map_or(solids[0].0, |(id, _)| *id);
    }
}

fn combo_pairs(ui: &mut egui::Ui, salt: &str, current: &mut String, options: &[(&str, &str)]) {
    let selected = options.iter().find(|(k, _)| *k == current.as_str()).map_or(current.as_str(), |(_, l)| *l);
    craft_ui::ComboBox::from_id_salt(salt).selected_text(crate::i18n::t(selected)).width(150.0).show_ui(ui, |ui| {
        for &(key, label) in options {
            if ui.selectable_label(current == key, crate::i18n::t(label)).clicked() {
                *current = key.to_string();
            }
        }
    });
}

fn primitive_section(ui: &mut egui::Ui, form: &mut ModelerForm, action: &mut DialogAction) {
    ui.strong(crate::tl!("Add 3D Primitive"));
    egui::Grid::new("solid3d_prim_grid").num_columns(2).spacing(vec2(10.0, 5.0)).show(ui, |ui| {
        ui.label(crate::tl!("Shape"));
        combo_pairs(ui, "solid3d_shape", &mut form.prim_kind, &PRIMITIVE_KINDS);
        ui.end_row();

        match form.prim_kind.as_str() {
            "box" => {
                ui.label(crate::tl!("Size (W, D, H)"));
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut form.size_x).speed(0.5).range(0.1..=10000.0));
                    ui.add(egui::DragValue::new(&mut form.size_y).speed(0.5).range(0.1..=10000.0));
                    ui.add(egui::DragValue::new(&mut form.size_z).speed(0.5).range(0.1..=10000.0));
                });
                ui.end_row();
            }
            "sphere" => {
                ui.label(crate::tl!("Radius"));
                ui.add(egui::DragValue::new(&mut form.radius).speed(0.5).range(0.1..=10000.0));
                ui.end_row();
            }
            "torus" => {
                ui.label(crate::tl!("Major / Minor radius"));
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut form.radius).speed(0.5).range(0.2..=10000.0));
                    ui.add(egui::DragValue::new(&mut form.minor_radius).speed(0.2).range(0.1..=5000.0));
                });
                ui.end_row();
            }
            "cone" => {
                ui.label(crate::tl!("Base / Top / Height"));
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut form.radius).speed(0.5).range(0.1..=10000.0));
                    ui.add(egui::DragValue::new(&mut form.top_radius).speed(0.5).range(0.0..=10000.0));
                    ui.add(egui::DragValue::new(&mut form.size_z).speed(0.5).range(0.1..=10000.0));
                });
                ui.end_row();
            }
            _ => {
                ui.label(crate::tl!("Radius / Height"));
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut form.radius).speed(0.5).range(0.1..=10000.0));
                    ui.add(egui::DragValue::new(&mut form.size_z).speed(0.5).range(0.1..=10000.0));
                });
                ui.end_row();
            }
        }

        ui.label(crate::tl!("Position (X, Y, Z)"));
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut form.pos_x).speed(0.5).range(-10000.0..=10000.0));
            ui.add(egui::DragValue::new(&mut form.pos_y).speed(0.5).range(-10000.0..=10000.0));
            ui.add(egui::DragValue::new(&mut form.pos_z).speed(0.5).range(-10000.0..=10000.0));
        });
        ui.end_row();
    });
    if ui.button(crate::tl!("Create 3D Primitive")).clicked() {
        *action = DialogAction::AddPrimitive;
    }
}

fn solids_list_section(ui: &mut egui::Ui, solids: &[(u64, String)], action: &mut DialogAction) {
    ui.horizontal(|ui| {
        ui.strong(crate::tl!("3D Solids in Drawing"));
        if !solids.is_empty() && ui.small_button(crate::tl!("Clear All")).clicked() {
            *action = DialogAction::ClearSolids;
        }
    });
    if solids.is_empty() {
        ui.label(crate::tl!("No 3D solids yet."));
        return;
    }
    egui::ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
        for (id, label) in solids {
            ui.horizontal(|ui| {
                ui.label(label);
                if ui.small_button(crate::tl!("Delete")).clicked() {
                    *action = DialogAction::DeleteSolid(*id);
                }
            });
        }
    });
}

fn boolean_section(ui: &mut egui::Ui, form: &mut ModelerForm, solids: &[(u64, String)], action: &mut DialogAction) {
    ui.strong(crate::tl!("Clay & CSG Boolean"));
    ui.add_enabled_ui(solids.len() >= 2, |ui| {
        egui::Grid::new("solid3d_bool_grid").num_columns(2).spacing(vec2(10.0, 5.0)).show(ui, |ui| {
            ui.label(crate::tl!("Operation"));
            combo_pairs(ui, "solid3d_bool_op", &mut form.bool_op, &BOOLEAN_OPS);
            ui.end_row();

            ui.label(crate::tl!("Target solid"));
            solid_id_combo(ui, "solid3d_left", &mut form.left_id, solids);
            ui.end_row();

            ui.label(crate::tl!("Tool solid"));
            solid_id_combo(ui, "solid3d_right", &mut form.right_id, solids);
            ui.end_row();

            ui.label(crate::tl!("Smooth blend (k)"));
            ui.add(egui::Slider::new(&mut form.smooth_k, 0.0..=20.0));
            ui.end_row();
        });
        ui.checkbox(&mut form.keep_inputs, crate::tl!("Keep input solids"));
        if ui.button(crate::tl!("Apply Boolean")).clicked() {
            *action = DialogAction::ApplyBoolean;
        }
    });
}

fn solid_id_combo(ui: &mut egui::Ui, salt: &str, value: &mut u64, solids: &[(u64, String)]) {
    let text = solids.iter().find(|(id, _)| *id == *value).map_or("", |(_, l)| l.as_str());
    craft_ui::ComboBox::from_id_salt(salt).selected_text(text).width(150.0).show_ui(ui, |ui| {
        for (id, label) in solids {
            ui.selectable_value(value, *id, label);
        }
    });
}

fn projection_and_export_section(ui: &mut egui::Ui, form: &mut ModelerForm, solids: &[(u64, String)], action: &mut DialogAction) {
    ui.strong(crate::tl!("2D CAD Projection & Mesh Export"));
    ui.add_enabled_ui(!solids.is_empty(), |ui| {
        egui::Grid::new("solid3d_proj_grid").num_columns(2).spacing(vec2(10.0, 5.0)).show(ui, |ui| {
            ui.label(crate::tl!("Mesh resolution"));
            ui.add(egui::Slider::new(&mut form.resolution, 12..=72));
            ui.end_row();

            ui.label(crate::tl!("Projection view"));
            combo_pairs(ui, "solid3d_proj_view", &mut form.proj_view, &PROJECTION_VIEWS);
            ui.end_row();

            ui.label(crate::tl!("Origin (X, Y) & Scale"));
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut form.proj_x).speed(1.0));
                ui.add(egui::DragValue::new(&mut form.proj_y).speed(1.0));
                ui.add(egui::DragValue::new(&mut form.proj_scale).speed(0.1).range(0.01..=100.0));
            });
            ui.end_row();
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut form.show_hidden, crate::tl!("Show hidden lines"));
            ui.checkbox(&mut form.stl_binary, crate::tl!("Binary STL"));
        });
        ui.horizontal(|ui| {
            if ui.button(crate::tl!("Project View (FLATSHOT)")).clicked() {
                *action = DialogAction::ProjectFlatshot;
            }
            if ui.button(crate::tl!("4-View Layout (VIEWBASE)")).clicked() {
                *action = DialogAction::ProjectViewbase;
            }
        });
        ui.horizontal(|ui| {
            if ui.button(crate::tl!("Export STL...")).clicked() {
                *action = DialogAction::ExportStl;
            }
            if ui.button(crate::tl!("Export OBJ...")).clicked() {
                *action = DialogAction::ExportObj;
            }
        });
    });
}

fn apply_dialog_action(app: &mut CadApp, form: &ModelerForm, action: DialogAction) {
    match action {
        DialogAction::None => {}
        DialogAction::AddPrimitive => {
            let params = json!({
                "kind": form.prim_kind,
                "size": [form.size_x, form.size_y, form.size_z],
                "radius": form.radius,
                "baseRadius": form.radius,
                "topRadius": form.top_radius,
                "majorRadius": form.radius,
                "minorRadius": form.minor_radius,
                "height": form.size_z,
                "position": [form.pos_x, form.pos_y, form.pos_z],
            });
            let _ = app.run("solid3d.primitive", params);
        }
        DialogAction::ApplyBoolean => {
            let params = json!({
                "op": form.bool_op,
                "leftId": form.left_id,
                "rightId": form.right_id,
                "smoothK": form.smooth_k,
                "keepInputs": form.keep_inputs,
            });
            let _ = app.run("solid3d.boolean", params);
        }
        DialogAction::DeleteSolid(id) => {
            let _ = app.run("solid3d.delete", json!({ "id": id }));
        }
        DialogAction::ClearSolids => {
            let _ = app.run("solid3d.clear", json!({}));
        }
        DialogAction::ProjectFlatshot => {
            let params = json!({
                "view": form.proj_view,
                "at": [form.proj_x, form.proj_y],
                "scale": form.proj_scale,
                "resolution": form.resolution,
                "showHidden": form.show_hidden,
            });
            if app.run("flatshot", params).is_ok() {
                app.canvas.zoom_pending = true;
            }
        }
        DialogAction::ProjectViewbase => {
            let params = json!({
                "at": [form.proj_x, form.proj_y],
                "scale": form.proj_scale,
                "resolution": form.resolution,
                "showHidden": form.show_hidden,
            });
            if app.run("viewbase", params).is_ok() {
                app.canvas.zoom_pending = true;
            }
        }
        DialogAction::ExportStl => {
            let res = form.resolution;
            let binary = form.stl_binary;
            let _ = app.pick_then(Pick::Save("model.stl".into()), move |app, path| {
                app.session.execute("solid3d.exportstl", &json!({ "path": path, "resolution": res, "binary": binary })).map_err(|e| e.to_string())
            });
        }
        DialogAction::ExportObj => {
            let res = form.resolution;
            let _ = app.pick_then(Pick::Save("model.obj".into()), move |app, path| {
                app.session.execute("solid3d.exportobj", &json!({ "path": path, "resolution": res })).map_err(|e| e.to_string())
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use cadcraft_engine::Session;
    use serde_json::json;

    use crate::{CadApp, Services};

    #[test]
    fn solid3d_dialog_renders_in_english_and_korean_and_drives_engine() {
        assert_eq!(crate::i18n::tr("ko", "3D Solid Modeler"), "3D 솔리드 모델러");
        assert_eq!(crate::i18n::tr("ko", "Project View (FLATSHOT)"), "뷰 투영 (FLATSHOT)");

        let mut app = CadApp::new(Session::new(), Services::default());
        app.run("ui.dialog.solid3d", json!({})).unwrap();
        assert_eq!(app.ui.dialog.as_deref(), Some("solid3d"));

        app.run("solid3d.primitive", json!({ "kind": "box", "size": [40.0, 30.0, 20.0] })).unwrap();
        app.run("solid3d.primitive", json!({ "kind": "sphere", "radius": 14.0, "position": [0.0, 0.0, 12.0] })).unwrap();
        app.run("solid3d.boolean", json!({ "op": "union", "smoothK": 3.0 })).unwrap();
        app.run("flatshot", json!({ "view": "isometric", "resolution": 20 })).unwrap();

        let ctx = egui::Context::default();
        let input =
            egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))), ..Default::default() };
        app.logic(&ctx);
        let mut out = ctx.run_ui(input, |ui| app.ui(ui));
        out.textures_delta.clear();
        assert_eq!(app.ui.dialog.as_deref(), Some("solid3d"));
    }
}
